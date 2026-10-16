//! Bluetooth LE discovery and drivers for FTMS trainers, power meters, cadence sensors,
//! heart-rate sensors, Shimano Di2 shifters and the Zwift Click.

use std::time::{Duration, Instant};

use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::{
    Central, CentralEvent, CharPropFlags, Characteristic, Manager as _, Peripheral as _,
    PeripheralProperties, RetrievePeripheralsOptions, ScanFilter, ValueNotification, WriteType,
};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures::StreamExt;
use tokio::time::{sleep, timeout};
use torqa_domain::telemetry::{Telemetry, TrainerControl};
use tracing::{debug, info, warn};

use crate::cycling_power;
use crate::di2;
use crate::ftms::{self, ControlResult, ResistanceRange};
use crate::handle::{DeviceError, DeviceEvent, DeviceHandle, DriverChannels};
use crate::heart_rate;
use crate::speed_cadence;
use crate::zwift_click;

const RECONNECT_DELAY: Duration = Duration::from_secs(2);
/// How long a reconnect scans for a device the system no longer knows.
const RECONNECT_SCAN: Duration = Duration::from_secs(5);
/// How long connecting and setting up may take before the attempt starts over.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// What a discovered device can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Which shifter a controller is: their buttons and their protocols differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerModel {
    /// Shimano Di2 (R59): four D-Fly channels, each telling presses, holds and double presses.
    Di2,
    /// The Zwift Click (R75): a + and a − button, each telling presses and holds.
    ZwiftClick,
}

impl ControllerModel {
    /// The model's name as the front end knows it.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Di2 => "di2",
            Self::ZwiftClick => "zwift_click",
        }
    }
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
            let (kind, model) = if properties.services.contains(&uuid_from_u16(ftms::SERVICE)) {
                (DeviceKind::Trainer, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(cycling_power::SERVICE))
            {
                (DeviceKind::PowerMeter, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(heart_rate::SERVICE))
            {
                (DeviceKind::HeartRateSensor, None)
            } else if properties
                .services
                .contains(&uuid_from_u16(speed_cadence::SERVICE))
            {
                (DeviceKind::CadenceSensor, None)
            } else if is_zwift_click(&properties, &name) {
                (DeviceKind::Controller, Some(ControllerModel::ZwiftClick))
            } else if properties.services.iter().any(|s| s.to_string() == di2::SERVICE)
                || properties.manufacturer_data.contains_key(&di2::MANUFACTURER)
                // Rear derailleurs advertise as their model, e.g. "RDR9250".
                || name.starts_with("RDR")
            {
                (DeviceKind::Controller, Some(ControllerModel::Di2))
            } else {
                continue;
            };
            devices.push(DiscoveredDevice {
                name,
                kind,
                model,
                rssi: properties.rssi,
                peripheral,
            });
        }
        devices.sort_by_key(|d| std::cmp::Reverse(d.rssi));
        Ok(devices)
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
                cadence: cycling_power::Cadence::default(),
            }
            .run()
        })
    }
}

/// Whether an advertisement is a Zwift Click's. Zwift's advertising data names the model;
/// without it (some stacks pass it on, some not) the unit's service or its name tells, which
/// the Play and the Ride share, so those are only kept out where the data is there.
fn is_zwift_click(properties: &PeripheralProperties, name: &str) -> bool {
    match properties
        .manufacturer_data
        .get(&zwift_click::MANUFACTURER)
        .and_then(|data| data.first())
    {
        Some(&device_type) => device_type == zwift_click::DEVICE_TYPE,
        None => {
            properties
                .services
                .iter()
                .any(|s| s.to_string() == zwift_click::SERVICE)
                || name == zwift_click::NAME
        }
    }
}

type Notifications = std::pin::Pin<Box<dyn futures::Stream<Item = ValueNotification> + Send>>;

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
            match end {
                Ok(SessionEnd::HandleDropped) => break,
                Ok(SessionEnd::Disconnected) => info!(device = %self.device.name, "disconnected"),
                Err(error) => warn!(device = %self.device.name, %error, "connection failed"),
            }
            if self
                .channels
                .events
                .send(DeviceEvent::Disconnected)
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
    }

    /// Looks the device up again after it dropped. The Bluetooth stack may have forgotten the
    /// old peripheral (macOS does at once), so connecting it again would fail forever: ask the
    /// system for it by identifier, else scan for it.
    async fn find_again(&mut self) -> Result<(), DeviceError> {
        let id = self.device.peripheral.id();
        let options = RetrievePeripheralsOptions {
            identifiers: Some(vec![id.clone()]),
            services: None,
        };
        // Not every platform can retrieve; scanning finds the device there.
        let retrieved = self
            .adapter
            .retrieve_peripherals(options)
            .await
            .inspect_err(|error| debug!(device = %self.device.name, %error, "cannot retrieve"))
            .unwrap_or_default()
            .into_iter()
            .find(|p| p.id() == id);
        if let Some(peripheral) = retrieved {
            self.device.peripheral = peripheral;
            return Ok(());
        }
        if let Ok(peripheral) = self.adapter.peripheral(&id).await {
            self.device.peripheral = peripheral;
            return Ok(());
        }
        debug!(device = %self.device.name, "scanning for the device");
        self.adapter.start_scan(ScanFilter::default()).await?;
        sleep(RECONNECT_SCAN).await;
        self.adapter.stop_scan().await?;
        self.device.peripheral = self
            .adapter
            .peripheral(&id)
            .await
            .map_err(|_| DeviceError::NotFound)?;
        Ok(())
    }

    /// One connection, from connecting until it drops or the handle goes away.
    async fn session(&mut self) -> Result<SessionEnd, DeviceError> {
        let peripheral = self.device.peripheral.clone();
        let mut central_events = self.adapter.events().await?;
        // A separate sender, so waiting for closure does not borrow `channels` during `next_control`.
        let events = self.channels.events.clone();

        info!(device = %self.device.name, "connecting");
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
        info!(device = %self.device.name, "connected");
        if events.send(DeviceEvent::Connected).await.is_err() {
            return Ok(SessionEnd::HandleDropped);
        }

        loop {
            tokio::select! {
                notification = notifications.next() => {
                    let Some(notification) = notification else {
                        return Ok(SessionEnd::Disconnected);
                    };
                    if let Some(event) = self.decode(&notification)
                        && events.send(event).await.is_err()
                    {
                        return Ok(SessionEnd::HandleDropped);
                    }
                }
                event = central_events.next() => match event {
                    Some(CentralEvent::DeviceDisconnected(id)) if id == peripheral.id() => {
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
                () = events.closed() => return Ok(SessionEnd::HandleDropped),
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
        let notifications = peripheral.notifications().await?;

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
                self.set_up_zwift_click(peripheral).await?;
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
        Ok((notifications, control_point))
    }

    /// Subscribes to a Zwift Click's buttons and starts its session (R75): it only tells of
    /// its buttons after the handshake, which it answers on the response characteristic.
    async fn set_up_zwift_click(&mut self, peripheral: &Peripheral) -> Result<(), DeviceError> {
        let characteristic = |uuid: &'static str| {
            peripheral
                .characteristics()
                .into_iter()
                .find(|c| {
                    c.service_uuid.to_string() == zwift_click::SERVICE && c.uuid.to_string() == uuid
                })
                .ok_or(DeviceError::MissingService(zwift_click::SERVICE))
        };
        let buttons = characteristic(zwift_click::BUTTONS)?;
        let response = characteristic(zwift_click::RESPONSE)?;
        let control = characteristic(zwift_click::CONTROL)?;
        // A button down when the link dropped is let go of as far as Torqa knows.
        self.click = zwift_click::Buttons::default();
        peripheral.subscribe(&response).await?;
        peripheral.subscribe(&buttons).await?;
        peripheral
            .write(&control, zwift_click::HANDSHAKE, WriteType::WithoutResponse)
            .await?;
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

    fn decode(&mut self, notification: &ValueNotification) -> Option<DeviceEvent> {
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
        } else if uuid.to_string() == zwift_click::BUTTONS {
            debug!(device = %name, ?value, "Click buttons");
            let presses = self
                .click
                .presses(value, Instant::now())
                .inspect_err(|error| warn!(device = %name, %error, "invalid Click buttons"))
                .ok()?;
            (!presses.is_empty()).then_some(DeviceEvent::Buttons(presses))
        } else {
            if uuid.to_string() == zwift_click::RESPONSE {
                if value == zwift_click::HANDSHAKE {
                    debug!(device = %name, "Click session started");
                } else {
                    // An old firmware answers the handshake with its key for an encrypted
                    // session, which Torqa does not speak (ADR 0003).
                    warn!(device = %name, ?value, "unexpected Click handshake answer");
                }
            }
            if uuid == uuid_from_u16(ftms::CONTROL_POINT) {
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
}

fn characteristic(peripheral: &Peripheral, uuid: u16) -> Result<Characteristic, DeviceError> {
    let wanted = uuid_from_u16(uuid);
    peripheral
        .characteristics()
        .into_iter()
        .find(|c| c.uuid == wanted)
        .ok_or(DeviceError::MissingCharacteristic(uuid))
}
