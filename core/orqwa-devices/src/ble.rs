//! Bluetooth LE discovery and drivers for FTMS trainers, power meters, cadence sensors,
//! heart-rate sensors, Shimano Di2 shifters and the Zwift Click, original and v2.

use std::time::{Duration, Instant};

use std::collections::BTreeSet;

use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{
    Central, CentralEvent, CharPropFlags, Characteristic, Manager as _, Peripheral as _,
    PeripheralProperties, RetrievePeripheralsOptions, ScanFilter, ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use orqwa_domain::battery::Battery;
pub use orqwa_domain::shifting::ControllerModel;
use orqwa_domain::telemetry::{Telemetry, TrainerControl};
use tokio::time::{sleep, timeout};
use tracing::{debug, info, trace, warn};

use crate::battery;
use crate::cycling_power;
use crate::di2;
use crate::ftms::{self, ControlResult, ResistanceRange};
use crate::handle::{DeviceError, DeviceEvent, DeviceHandle, DriverChannels};
use crate::heart_rate;
use crate::speed_cadence;
use crate::zwift_click;
use crate::zwift_click_v2::{self, Answer, Side};

const RECONNECT_DELAY: Duration = Duration::from_secs(2);
/// How long a reconnect scans for a device the system no longer knows.
const RECONNECT_SCAN: Duration = Duration::from_secs(5);
/// How long connecting and setting up may take before the attempt starts over.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
/// A battery that does not answer in this time stays unknown rather than holding up the
/// connection (R97).
const BATTERY_TIMEOUT: Duration = Duration::from_secs(3);

/// What a discovered device can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// A controllable trainer (FTMS).
    Trainer,
    /// A heart-rate sensor.
    HeartRateSensor,
    /// A power meter (Cycling Power Service), e.g. power pedals.
    PowerMeter,
    /// A cadence sensor (Cycling Speed and Cadence Service).
    CadenceSensor,
    /// A shifter whose buttons can shift the virtual gears (R7): a [`ControllerModel`].
    Controller,
}

/// A device found by [`Bluetooth::scan`].
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    /// Advertised name.
    pub name: String,
    /// Device capability.
    pub kind: DeviceKind,
    /// Which shifter it is, for a controller.
    pub model: Option<ControllerModel>,
    /// Signal strength in dBm at discovery, if known.
    pub rssi: Option<i16>,
    peripheral: Peripheral,
    /// Which of a Click v2's pods it is.
    side: Option<Side>,
    /// A Click v2's other pod, found in the same scan: driven over the same handle.
    partner: Option<Pod>,
}

/// A Click v2's other pod.
#[derive(Debug, Clone)]
struct Pod {
    side: Side,
    peripheral: Peripheral,
}

impl DiscoveredDevice {
    /// The system's identifier for the device, stable on this computer (address or UUID).
    #[must_use]
    pub fn id(&self) -> String {
        format!("{:?}", self.peripheral.id())
    }
}

/// Access to the system's Bluetooth adapter.
#[derive(Clone)]
pub struct Bluetooth {
    adapter: Adapter,
}

impl Bluetooth {
    /// Opens the first Bluetooth adapter.
    ///
    /// # Errors
    /// [`DeviceError::NoAdapter`] if there is none, or the Bluetooth stack's error.
    pub async fn new() -> Result<Self, DeviceError> {
        let manager = Manager::new().await?;
        let adapter = manager
            .adapters()
            .await?
            .into_iter()
            .next()
            .ok_or(DeviceError::NoAdapter)?;
        Ok(Self { adapter })
    }

    /// Scans for trainers, power meters, cadence and heart-rate sensors and controllers for
    /// `duration`.
    ///
    /// # Errors
    /// Returns the Bluetooth stack's error.
    pub async fn scan(&self, duration: Duration) -> Result<Vec<DiscoveredDevice>, DeviceError> {
        // Unfiltered: Di2 units do not always advertise their service (see `kind_of`).
        self.adapter.start_scan(ScanFilter::default()).await?;
        sleep(duration).await;
        self.adapter.stop_scan().await?;

        let mut devices = Vec::new();
        for peripheral in self.adapter.peripherals().await? {
            let Some(properties) = peripheral.properties().await? else {
                continue;
            };
            let name = properties
                .local_name
                .clone()
                .or(properties.advertisement_name.clone())
                .unwrap_or_else(|| "Unnamed device".to_owned());
            // Trainers often offer the Cycling Power Service too: they stay trainers.
            let (kind, model, side) = if properties.services.contains(&uuid_from_u16(ftms::SERVICE))
            {
                (DeviceKind::Trainer, None, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(cycling_power::SERVICE))
            {
                (DeviceKind::PowerMeter, None, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(heart_rate::SERVICE))
            {
                (DeviceKind::HeartRateSensor, None, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(speed_cadence::SERVICE))
            {
                (DeviceKind::CadenceSensor, None, None)
            } else if let Some((model, side)) = zwift_model(&properties, &name) {
                (DeviceKind::Controller, Some(model), side)
            } else if properties.services.iter().any(|s| s.to_string() == di2::SERVICE)
                || properties.manufacturer_data.contains_key(&di2::MANUFACTURER)
                // Rear derailleurs advertise as their model, e.g. "RDR9250".
                || name.starts_with("RDR")
            {
                (DeviceKind::Controller, Some(ControllerModel::Di2), None)
            } else {
                continue;
            };
            devices.push(DiscoveredDevice {
                name,
                kind,
                model,
                rssi: properties.rssi,
                peripheral,
                side,
                partner: None,
            });
        }
        devices.sort_by_key(|d| std::cmp::Reverse(d.rssi));
        Ok(pair_click_v2_pods(devices))
    }

    /// Connects to a discovered device and keeps it connected until the handle is dropped.
    ///
    /// Must be called within a Tokio runtime.
    #[must_use]
    pub fn connect(&self, device: DiscoveredDevice) -> DeviceHandle {
        let controllable = device.kind == DeviceKind::Trainer;
        let adapter = self.adapter.clone();
        DeviceHandle::spawn(device.name.clone(), controllable, move |channels| {
            Driver {
                adapter,
                device,
                channels,
                last_control: None,
                resistance_range: ResistanceRange::default(),
                buttons: di2::Buttons::default(),
                click: zwift_click::Buttons::default(),
                click_v2: zwift_click_v2::Buttons::default(),
                keepalive: Vec::new(),
                answered: Vec::new(),
                battery: Battery::default(),
                cadence: cycling_power::Cadence::default(),
            }
            .run()
        })
    }
}

/// Which Zwift shifter an advertisement is, and which of a Click v2's pods. Zwift's advertising
/// data names the model; without it (some stacks pass it on, some not) the unit's service or
/// its name tells, which the Play, the Ride and the Click v2's pods share, so those are only
/// kept out, or told apart, where the data is there.
fn zwift_model(
    properties: &PeripheralProperties,
    name: &str,
) -> Option<(ControllerModel, Option<Side>)> {
    match properties
        .manufacturer_data
        .get(&zwift_click::MANUFACTURER)
        .and_then(|data| data.first())
    {
        Some(&zwift_click::DEVICE_TYPE) => Some((ControllerModel::ZwiftClick, None)),
        Some(&device_type) => Side::from_device_type(device_type)
            .map(|side| (ControllerModel::ZwiftClickV2, Some(side))),
        None => (properties
            .services
            .iter()
            .any(|s| is_zwift_service(&s.to_string()))
            || name == zwift_click::NAME)
            .then_some((ControllerModel::ZwiftClick, None)),
    }
}

/// Joins a Click v2's two pods, found as devices of their own, into one device driven over
/// one handle (R77): the right pod first, as it is never locked, the left as its partner. A
/// pod alone is the device by itself; a second pair in reach is left out.
fn pair_click_v2_pods(devices: Vec<DiscoveredDevice>) -> Vec<DiscoveredDevice> {
    let mut right = None;
    let mut left = None;
    let mut others = Vec::with_capacity(devices.len());
    for device in devices {
        match (device.model, device.side) {
            (Some(ControllerModel::ZwiftClickV2), Some(Side::Right)) if right.is_none() => {
                right = Some(device);
            }
            (Some(ControllerModel::ZwiftClickV2), Some(Side::Left)) if left.is_none() => {
                left = Some(device);
            }
            (Some(ControllerModel::ZwiftClickV2), _) => {}
            _ => others.push(device),
        }
    }
    let pair = match (right, left) {
        (Some(mut right), Some(left)) => {
            right.rssi = right.rssi.max(left.rssi);
            right.partner = Some(Pod {
                side: Side::Left,
                peripheral: left.peripheral,
            });
            Some(right)
        }
        (Some(pod), None) | (None, Some(pod)) => Some(pod),
        (None, None) => None,
    };
    if let Some(mut pair) = pair {
        zwift_click_v2::NAME.clone_into(&mut pair.name);
        others.push(pair);
        others.sort_by_key(|d| std::cmp::Reverse(d.rssi));
    }
    others
}

/// A device's notifications, each with the Click v2 pod it came from.
type Notifications =
    std::pin::Pin<Box<dyn futures::Stream<Item = (Option<Side>, ValueNotification)> + Send>>;

enum SessionEnd {
    HandleDropped,
    Disconnected,
}

struct Driver {
    adapter: Adapter,
    device: DiscoveredDevice,
    channels: DriverChannels,
    last_control: Option<TrainerControl>,
    resistance_range: ResistanceRange,
    /// Decodes a Di2 shifter's button indications.
    buttons: di2::Buttons,
    /// Decodes a Zwift Click's button states.
    click: zwift_click::Buttons,
    /// Decodes a Zwift Click v2's pods' button states.
    click_v2: zwift_click_v2::Buttons,
    /// Where a Click v2's handshake is written again every [`zwift_click_v2::KEEPALIVE`].
    keepalive: Vec<(Peripheral, Characteristic)>,
    /// The Click v2's pods that answered the handshake since connecting: later answers are to
    /// the keepalive, every few seconds, so they go to the trace log only.
    answered: Vec<Side>,
    /// What the device told of its batteries since connecting (R97).
    battery: Battery,
    /// A power meter's or cadence sensor's cadence, from its crank counter.
    cadence: cycling_power::Cadence,
}

impl Driver {
    async fn run(mut self) {
        let closed = self.channels.events.clone();
        let mut first = true;
        loop {
            let attempt = async {
                // The device found by the scan is fresh; after a drop it must be found again.
                if !std::mem::take(&mut first) {
                    self.find_again().await?;
                }
                self.session().await
            };
            let end = tokio::select! {
                end = attempt => end,
                () = closed.closed() => break,
            };
            let failure = match end {
                Ok(SessionEnd::HandleDropped) => break,
                Ok(SessionEnd::Disconnected) => {
                    info!(device = %self.device.name, "disconnected");
                    None
                }
                Err(error) => {
                    warn!(device = %self.device.name, %error, "connection failed");
                    Some(error.to_string())
                }
            };
            self.disconnect_partner().await;
            if self
                .channels
                .events
                .send(DeviceEvent::Disconnected)
                .await
                .is_err()
            {
                break;
            }
            // The rider sees why, beside the device, while the driver tries again (R80).
            if let Some(reason) = failure
                && self
                    .channels
                    .events
                    .send(DeviceEvent::Failed(reason))
                    .await
                    .is_err()
            {
                break;
            }
            tokio::select! {
                () = sleep(RECONNECT_DELAY) => {}
                () = self.channels.events.closed() => break,
            }
        }
        if let Err(error) = self.device.peripheral.disconnect().await {
            debug!(device = %self.device.name, %error, "disconnect failed");
        }
        self.disconnect_partner().await;
    }

    /// Drops a Click v2's other pod with the first: the two are connected again together.
    async fn disconnect_partner(&self) {
        if let Some(partner) = &self.device.partner
            && let Err(error) = partner.peripheral.disconnect().await
        {
            let pod = partner.side.name();
            debug!(device = %self.device.name, pod, %error, "disconnect failed");
        }
    }

    /// Looks the device up again after it dropped, and a Click v2's other pod with it. A pod
    /// not found (asleep, out of reach) leaves the other to go on alone; it is looked for again
    /// the next time.
    async fn find_again(&mut self) -> Result<(), DeviceError> {
        let name = self.device.name.clone();
        self.device.peripheral =
            find_peripheral(&self.adapter, self.device.peripheral.id(), &name).await?;
        if let Some(partner) = &mut self.device.partner {
            match find_peripheral(&self.adapter, partner.peripheral.id(), &name).await {
                Ok(peripheral) => partner.peripheral = peripheral,
                Err(error) => {
                    warn!(device = %name, pod = partner.side.name(), %error, "pod not found");
                }
            }
        }
        Ok(())
    }

    /// One connection, from connecting until it drops or the handle goes away; a Click v2's
    /// two pods are one connection here, dropped and made again together.
    async fn session(&mut self) -> Result<SessionEnd, DeviceError> {
        let peripheral = self.device.peripheral.clone();
        let partner = self.device.partner.clone();
        let mut central_events = self.adapter.events().await?;
        // A separate sender, so waiting for closure does not borrow `channels` during `next_control`.
        let events = self.channels.events.clone();

        info!(device = %self.device.name, "connecting");
        self.keepalive.clear();
        self.answered.clear();
        self.battery = Battery::default();
        let set_up = timeout(CONNECT_TIMEOUT, self.set_up(&peripheral)).await;
        let (mut notifications, control_point) = match set_up {
            Ok(Ok(set_up)) => set_up,
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                // Otherwise the pending connection may complete later, with no one listening.
                // Bounded: not every stack confirms cancelling a connection never made.
                let _ = timeout(RECONNECT_DELAY, peripheral.disconnect()).await;
                return Err(DeviceError::Timeout);
            }
        };
        if let Some(partner) = &partner {
            notifications = self.set_up_partner(partner, notifications).await;
        }
        info!(device = %self.device.name, "connected");
        if events.send(DeviceEvent::Connected).await.is_err()
            || (self.battery.is_known()
                && events
                    .send(DeviceEvent::Battery(self.battery))
                    .await
                    .is_err())
        {
            return Ok(SessionEnd::HandleDropped);
        }

        let mut keepalive = tokio::time::interval(zwift_click_v2::KEEPALIVE);
        // The first tick is at once, and the handshake was just written.
        keepalive.tick().await;
        loop {
            tokio::select! {
                notification = notifications.next() => {
                    let Some((side, notification)) = notification else {
                        return Ok(SessionEnd::Disconnected);
                    };
                    if let Some(event) = self.decode(side, &notification)
                        && events.send(event).await.is_err()
                    {
                        return Ok(SessionEnd::HandleDropped);
                    }
                }
                event = central_events.next() => match event {
                    Some(CentralEvent::DeviceDisconnected(id))
                        if id == peripheral.id()
                            || partner.as_ref().is_some_and(|p| p.peripheral.id() == id) =>
                    {
                        return Ok(SessionEnd::Disconnected);
                    }
                    Some(_) => {}
                    None => return Ok(SessionEnd::Disconnected),
                },
                control = self.channels.next_control() => {
                    let Some(control) = control else {
                        return Ok(SessionEnd::HandleDropped);
                    };
                    self.last_control = Some(control);
                    if let Some(control_point) = &control_point {
                        let command = ftms::encode_control(&control, self.resistance_range);
                        peripheral.write(control_point, &command, WriteType::WithResponse).await?;
                    }
                }
                _ = keepalive.tick(), if !self.keepalive.is_empty() => {
                    for (pod, control) in &self.keepalive {
                        pod.write(control, zwift_click::HANDSHAKE, WriteType::WithoutResponse).await?;
                    }
                }
                () = events.closed() => return Ok(SessionEnd::HandleDropped),
            }
        }
    }

    /// Connects a Click v2's other pod and joins its notifications to the first's. Without it
    /// (asleep, out of reach, or not answering in time) the first pod goes on alone: the pods
    /// are tried again together when either drops.
    async fn set_up_partner(
        &mut self,
        partner: &Pod,
        notifications: Notifications,
    ) -> Notifications {
        let name = self.device.name.clone();
        let pod = partner.side.name();
        let set_up = timeout(CONNECT_TIMEOUT, async {
            partner.peripheral.connect().await?;
            partner.peripheral.discover_services().await?;
            let notifications = partner.peripheral.notifications().await?;
            self.set_up_zwift_click_v2(&partner.peripheral, partner.side)
                .await?;
            self.set_up_battery(&partner.peripheral, Some(partner.side))
                .await;
            Ok::<_, DeviceError>(notifications)
        })
        .await;
        match set_up {
            Ok(Ok(more)) => {
                let side = Some(partner.side);
                Box::pin(futures::stream::select(
                    notifications,
                    more.map(move |notification| (side, notification)),
                ))
            }
            Ok(Err(error)) => {
                warn!(device = %name, pod, %error, "pod not connected");
                notifications
            }
            Err(_) => {
                warn!(device = %name, pod, "connecting the pod timed out");
                let _ = timeout(RECONNECT_DELAY, partner.peripheral.disconnect()).await;
                notifications
            }
        }
    }

    /// Connects, finds the services and subscribes to what the device reports; returns its
    /// notifications and, for a trainer, its control point.
    async fn set_up(
        &mut self,
        peripheral: &Peripheral,
    ) -> Result<(Notifications, Option<Characteristic>), DeviceError> {
        peripheral.connect().await?;
        peripheral.discover_services().await?;
        let side = self.device.side;
        let notifications = peripheral
            .notifications()
            .await?
            .map(move |notification| (side, notification));

        let control_point = match self.device.kind {
            DeviceKind::Trainer => Some(self.set_up_trainer(peripheral).await?),
            DeviceKind::HeartRateSensor => {
                let measurement = characteristic(peripheral, heart_rate::MEASUREMENT)?;
                peripheral.subscribe(&measurement).await?;
                None
            }
            DeviceKind::PowerMeter | DeviceKind::CadenceSensor => {
                // The counter from before a drop may be minutes old.
                self.cadence = cycling_power::Cadence::default();
                let uuid = if self.device.kind == DeviceKind::PowerMeter {
                    cycling_power::MEASUREMENT
                } else {
                    speed_cadence::MEASUREMENT
                };
                peripheral
                    .subscribe(&characteristic(peripheral, uuid)?)
                    .await?;
                None
            }
            DeviceKind::Controller if self.device.model == Some(ControllerModel::ZwiftClick) => {
                // A button down when the link dropped is let go of as far as ORQWA knows.
                self.click = zwift_click::Buttons::default();
                self.start_zwift_click(peripheral).await?;
                None
            }
            DeviceKind::Controller if self.device.model == Some(ControllerModel::ZwiftClickV2) => {
                self.click_v2 = zwift_click_v2::Buttons::default();
                // Told from the original Click by the advertising data, which names the pod.
                let side = self.device.side.unwrap_or(Side::Right);
                self.set_up_zwift_click_v2(peripheral, side).await?;
                None
            }
            DeviceKind::Controller => {
                // Indications; the system asks to pair the unit first if it is not yet.
                let buttons = peripheral
                    .characteristics()
                    .into_iter()
                    .find(|c| {
                        c.service_uuid.to_string() == di2::SERVICE
                            && c.uuid.to_string().starts_with(di2::BUTTONS_PREFIX)
                    })
                    .ok_or(DeviceError::MissingService(di2::SERVICE))?;
                self.buttons = di2::Buttons::default();
                // Otherwise the first press after connecting only shows where the channels stand.
                if buttons.properties.contains(CharPropFlags::READ) {
                    let name = &self.device.name;
                    match peripheral.read(&buttons).await {
                        Ok(value) => {
                            if let Err(error) = self.buttons.start_from(&value) {
                                debug!(device = %name, %error, "unreadable D-Fly buttons");
                            }
                        }
                        Err(error) => debug!(device = %name, %error, "cannot read D-Fly buttons"),
                    }
                }
                peripheral.subscribe(&buttons).await?;
                None
            }
        };
        self.set_up_battery(peripheral, side).await;
        Ok((Box::pin(notifications), control_point))
    }

    /// Reads how full the battery is where the device has the Battery Service, and follows
    /// its changes where the device notifies them (R97). Many devices have none, and none of
    /// this keeps a device from connecting.
    async fn set_up_battery(&mut self, peripheral: &Peripheral, side: Option<Side>) {
        let Ok(level) = characteristic(peripheral, battery::LEVEL) else {
            return;
        };
        let name = &self.device.name;
        let read = timeout(BATTERY_TIMEOUT, async {
            if level.properties.contains(CharPropFlags::NOTIFY)
                && let Err(error) = peripheral.subscribe(&level).await
            {
                debug!(device = %name, %error, "battery level not followed");
            }
            peripheral.read(&level).await
        })
        .await;
        match read {
            Ok(Ok(value)) => {
                if let Some(level) = battery::parse_level(&value) {
                    battery::note(&mut self.battery, side, level);
                }
            }
            Ok(Err(error)) => debug!(device = %name, %error, "battery level not read"),
            Err(_) => debug!(device = %name, "reading the battery level timed out"),
        }
    }

    /// The batteries' event, if a `level` told by the device, or by its `side` of a pair,
    /// changed them (R97).
    fn battery_told(&mut self, side: Option<Side>, level: u8) -> Option<DeviceEvent> {
        let battery = battery::note(&mut self.battery, side, level)?;
        debug!(device = %self.device.name, pod = ?side, level, "battery level");
        Some(DeviceEvent::Battery(battery))
    }

    /// Subscribes to a Zwift Click's (or a Click v2 pod's) buttons and starts its session
    /// (R75): it only tells of its buttons after the handshake, which it answers on the
    /// response characteristic. Returns the control characteristic the handshake went to.
    async fn start_zwift_click(
        &mut self,
        peripheral: &Peripheral,
    ) -> Result<Characteristic, DeviceError> {
        // The Click v2's pods keep the characteristics under the Ride's service (#271).
        let expected = if self.device.model == Some(ControllerModel::ZwiftClickV2) {
            zwift_click::RIDE_SERVICE
        } else {
            zwift_click::SERVICE
        };
        let characteristics = peripheral.characteristics();
        let characteristic = |uuid: &'static str| {
            zwift_characteristic(&characteristics, uuid)
                .ok_or(DeviceError::MissingService(expected))
        };
        let buttons = characteristic(zwift_click::BUTTONS)?;
        let response = characteristic(zwift_click::RESPONSE)?;
        let control = characteristic(zwift_click::CONTROL)?;
        peripheral.subscribe(&response).await?;
        peripheral.subscribe(&buttons).await?;
        peripheral
            .write(&control, zwift_click::HANDSHAKE, WriteType::WithoutResponse)
            .await?;
        Ok(control)
    }

    /// Starts a Click v2 pod's session (R77) as a Click's, then keeps it: a left pod gets the
    /// acknowledgement that keeps it unlocked, and every pod the handshake again every
    /// [`zwift_click_v2::KEEPALIVE`], so it does not fall asleep.
    async fn set_up_zwift_click_v2(
        &mut self,
        peripheral: &Peripheral,
        side: Side,
    ) -> Result<(), DeviceError> {
        let control = self.start_zwift_click(peripheral).await?;
        if side == Side::Left {
            peripheral
                .write(
                    &control,
                    zwift_click_v2::UNLOCK_ACK,
                    WriteType::WithoutResponse,
                )
                .await?;
        }
        self.keepalive.push((peripheral.clone(), control));
        Ok(())
    }

    /// Subscribes to trainer data, takes control and restores the last control after a reconnect.
    async fn set_up_trainer(
        &mut self,
        peripheral: &Peripheral,
    ) -> Result<Characteristic, DeviceError> {
        let data = characteristic(peripheral, ftms::INDOOR_BIKE_DATA)?;
        let control_point = characteristic(peripheral, ftms::CONTROL_POINT)?;

        if let Ok(range) = characteristic(peripheral, ftms::SUPPORTED_RESISTANCE_LEVEL_RANGE) {
            match ftms::ResistanceRange::parse(&peripheral.read(&range).await?) {
                Ok(parsed) => self.resistance_range = parsed,
                Err(error) => warn!(device = %self.device.name, %error, "invalid resistance range"),
            }
        }
        debug!(device = %self.device.name, range = ?self.resistance_range, "resistance range");

        peripheral.subscribe(&data).await?;
        peripheral.subscribe(&control_point).await?;
        peripheral
            .write(
                &control_point,
                &ftms::request_control(),
                WriteType::WithResponse,
            )
            .await?;
        peripheral
            .write(
                &control_point,
                &ftms::start_or_resume(),
                WriteType::WithResponse,
            )
            .await?;
        if let Some(control) = &self.last_control {
            let command = ftms::encode_control(control, self.resistance_range);
            peripheral
                .write(&control_point, &command, WriteType::WithResponse)
                .await?;
        }
        Ok(control_point)
    }

    /// What a notification, from a Click v2's `side` if from one, reports.
    fn decode(
        &mut self,
        side: Option<Side>,
        notification: &ValueNotification,
    ) -> Option<DeviceEvent> {
        let name = &self.device.name;
        let (uuid, value) = (notification.uuid, notification.value.as_slice());
        if uuid == uuid_from_u16(ftms::INDOOR_BIKE_DATA) {
            ftms::parse_indoor_bike_data(value)
                .inspect_err(|error| warn!(device = %name, %error, "invalid indoor bike data"))
                .ok()
                .map(DeviceEvent::Telemetry)
        } else if uuid == uuid_from_u16(heart_rate::MEASUREMENT) {
            let heart_rate = heart_rate::parse_measurement(value)
                .inspect_err(|error| warn!(device = %name, %error, "invalid heart rate"))
                .ok()?;
            Some(DeviceEvent::Telemetry(Telemetry {
                heart_rate,
                ..Telemetry::default()
            }))
        } else if uuid == uuid_from_u16(cycling_power::MEASUREMENT) {
            let measurement = cycling_power::parse_measurement(value)
                .inspect_err(|error| warn!(device = %name, %error, "invalid power measurement"))
                .ok()?;
            let cadence = measurement
                .crank
                .and_then(|crank| self.cadence.update(crank, Instant::now()));
            Some(DeviceEvent::Telemetry(Telemetry {
                power: Some(measurement.power),
                cadence,
                ..Telemetry::default()
            }))
        } else if uuid == uuid_from_u16(speed_cadence::MEASUREMENT) {
            let crank = speed_cadence::parse_measurement(value)
                .inspect_err(|error| warn!(device = %name, %error, "invalid cadence measurement"))
                .ok()??;
            let cadence = self.cadence.update(crank, Instant::now())?;
            Some(DeviceEvent::Telemetry(Telemetry {
                cadence: Some(cadence),
                ..Telemetry::default()
            }))
        } else if uuid.to_string().starts_with(di2::BUTTONS_PREFIX) {
            debug!(device = %name, ?value, "D-Fly buttons");
            let presses = self
                .buttons
                .presses(value)
                .inspect_err(|error| warn!(device = %name, %error, "invalid D-Fly buttons"))
                .ok()?;
            (!presses.is_empty()).then_some(DeviceEvent::Buttons(presses))
        } else if uuid == uuid_from_u16(battery::LEVEL) {
            let level = battery::parse_level(value)?;
            self.battery_told(side, level)
        } else if uuid.to_string() == zwift_click::BUTTONS {
            if let Some(level) = zwift_click::battery_level(value) {
                return self.battery_told(side, level);
            }
            let presses = if self.device.model == Some(ControllerModel::ZwiftClickV2) {
                let side = side?;
                debug!(device = %name, pod = side.name(), ?value, "Click v2 buttons");
                self.click_v2
                    .presses(side, value, Instant::now())
                    .inspect_err(|error| warn!(device = %name, %error, "invalid Click v2 buttons"))
                    .ok()?
            } else {
                debug!(device = %name, ?value, "Click buttons");
                self.click
                    .presses(value, Instant::now())
                    .inspect_err(|error| warn!(device = %name, %error, "invalid Click buttons"))
                    .ok()?
            };
            (!presses.is_empty()).then_some(DeviceEvent::Buttons(presses))
        } else {
            if uuid.to_string() == zwift_click::RESPONSE {
                self.log_zwift_answer(side, value);
            } else if uuid == uuid_from_u16(ftms::CONTROL_POINT) {
                match ftms::parse_control_response(value) {
                    Some(r) if r.result == ControlResult::Success => {
                        debug!(device = %name, request = r.request, "control accepted");
                    }
                    Some(r) => {
                        warn!(device = %name, request = r.request, result = ?r.result, "control rejected");
                    }
                    None => {}
                }
            }
            None
        }
    }

    /// Tells in the log what a Zwift Click's or a Click v2 pod's answer to the handshake says.
    fn log_zwift_answer(&mut self, side: Option<Side>, value: &[u8]) {
        let name = &self.device.name;
        if self.device.model == Some(ControllerModel::ZwiftClickV2) {
            let pod = side.map_or("pod", Side::name);
            match zwift_click_v2::answer(value) {
                Answer::Started => match side {
                    Some(side) if !self.answered.contains(&side) => {
                        self.answered.push(side);
                        debug!(device = %name, pod, "Click v2 session started");
                    }
                    _ => trace!(device = %name, pod, "Click v2 keepalive answered"),
                },
                Answer::Locked => warn!(
                    device = %name, pod,
                    "locked by Zwift: it tells of no buttons until it was in a Zwift session again"
                ),
                Answer::Stopped => warn!(device = %name, pod, "stopped telling of its buttons"),
                Answer::Unknown => {
                    warn!(device = %name, pod, ?value, "unexpected Click v2 answer");
                }
            }
        } else if value == zwift_click::HANDSHAKE {
            debug!(device = %name, "Click session started");
        } else {
            // An old firmware answers the handshake with its key for an encrypted session,
            // which ORQWA does not speak (ADR 0003).
            warn!(device = %name, ?value, "unexpected Click handshake answer");
        }
    }
}

/// Looks a device up again by its identifier after it dropped. The Bluetooth stack may have
/// forgotten the old peripheral (macOS does at once), so connecting it again would fail
/// forever: ask the system for it by identifier, else scan for it.
async fn find_peripheral(
    adapter: &Adapter,
    id: btleplug::platform::PeripheralId,
    name: &str,
) -> Result<Peripheral, DeviceError> {
    let options = RetrievePeripheralsOptions {
        identifiers: Some(vec![id.clone()]),
        services: None,
    };
    // Not every platform can retrieve; scanning finds the device there.
    let retrieved = adapter
        .retrieve_peripherals(options)
        .await
        .inspect_err(|error| debug!(device = %name, %error, "cannot retrieve"))
        .unwrap_or_default()
        .into_iter()
        .find(|p| p.id() == id);
    if let Some(peripheral) = retrieved {
        return Ok(peripheral);
    }
    if let Ok(peripheral) = adapter.peripheral(&id).await {
        return Ok(peripheral);
    }
    debug!(device = %name, "scanning for the device");
    adapter.start_scan(ScanFilter::default()).await?;
    sleep(RECONNECT_SCAN).await;
    adapter.stop_scan().await?;
    adapter
        .peripheral(&id)
        .await
        .map_err(|_| DeviceError::NotFound)
}

/// Whether `service` is one Zwift's shifters put their characteristics under: the original
/// Click's own, or the Ride's that the Click v2's pods and the Play share.
fn is_zwift_service(service: &str) -> bool {
    service == zwift_click::SERVICE || service == zwift_click::RIDE_SERVICE
}

/// The Zwift characteristic `uuid` among `characteristics`, under either Zwift service.
fn zwift_characteristic(
    characteristics: &BTreeSet<Characteristic>,
    uuid: &str,
) -> Option<Characteristic> {
    characteristics
        .iter()
        .find(|c| is_zwift_service(&c.service_uuid.to_string()) && c.uuid.to_string() == uuid)
        .cloned()
}

fn characteristic(peripheral: &Peripheral, uuid: u16) -> Result<Characteristic, DeviceError> {
    let wanted = uuid_from_u16(uuid);
    peripheral
        .characteristics()
        .into_iter()
        .find(|c| c.uuid == wanted)
        .ok_or(DeviceError::MissingCharacteristic(uuid))
}

#[cfg(test)]
mod tests {
    use btleplug::api::CharPropFlags;

    use super::*;

    fn characteristic(service: &str, uuid: &str) -> Characteristic {
        Characteristic {
            uuid: uuid.parse().unwrap(),
            service_uuid: service.parse().unwrap(),
            properties: CharPropFlags::NOTIFY,
            descriptors: BTreeSet::new(),
        }
    }

    /// A Click v2 pod offers the Click's characteristics under the Ride's service (#271):
    /// both services serve, a stranger's characteristic of the same UUID does not.
    #[test]
    fn the_zwift_characteristics_are_found_under_either_zwift_service() {
        let pod = BTreeSet::from([
            characteristic(zwift_click::RIDE_SERVICE, zwift_click::BUTTONS),
            characteristic(zwift_click::RIDE_SERVICE, zwift_click::CONTROL),
        ]);
        let click = BTreeSet::from([characteristic(zwift_click::SERVICE, zwift_click::BUTTONS)]);
        let stranger = BTreeSet::from([characteristic(
            "0000180f-0000-1000-8000-00805f9b34fb",
            zwift_click::BUTTONS,
        )]);
        assert!(zwift_characteristic(&pod, zwift_click::BUTTONS).is_some());
        assert!(zwift_characteristic(&pod, zwift_click::CONTROL).is_some());
        assert!(zwift_characteristic(&pod, zwift_click::RESPONSE).is_none());
        assert!(zwift_characteristic(&click, zwift_click::BUTTONS).is_some());
        assert!(zwift_characteristic(&stranger, zwift_click::BUTTONS).is_none());
    }
}
