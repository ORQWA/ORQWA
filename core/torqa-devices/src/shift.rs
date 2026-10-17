//! Shift inputs (R7) behind [`ShiftInput`]: the keyboard, whose keys the front end passes on,
//! and the buttons of a shifter — a Shimano Di2's D-Fly channels or a Zwift Click's + and − —
//! which do what the rider gave them (#139).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, mpsc};

use torqa_domain::shifting::{
    ButtonAction, ButtonMap, Control, ControllerModel, Shift, ShiftInput,
};

use crate::{DeviceEvent, DeviceHandle};

/// What each shifter model's buttons do (R84), shared with the settings, so a change applies
/// at once. A model not given any shifts with its first two buttons.
#[derive(Debug, Default)]
pub struct Assignments(Mutex<HashMap<ControllerModel, ButtonMap>>);

impl Assignments {
    /// Each model's buttons doing what `map` gives them for it.
    #[must_use]
    pub fn new(map: impl Fn(ControllerModel) -> ButtonMap) -> Self {
        let assignments = Self::default();
        assignments.set_all(map);
        assignments
    }

    /// Gives the buttons of a `model` what `map` says.
    pub fn set(&self, model: ControllerModel, map: ButtonMap) {
        self.maps().insert(model, map);
    }

    /// Gives each model's buttons what `map` says for it, e.g. another rider's.
    pub fn set_all(&self, map: impl Fn(ControllerModel) -> ButtonMap) {
        *self.maps() = ControllerModel::ALL
            .into_iter()
            .map(|model| (model, map(model)))
            .collect();
    }

    /// What the buttons of a `model` do.
    #[must_use]
    pub fn get(&self, model: ControllerModel) -> ButtonMap {
        self.maps().get(&model).copied().unwrap_or_default()
    }

    fn maps(&self) -> MutexGuard<'_, HashMap<ControllerModel, ButtonMap>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Shifts, and the other controls its buttons were given, from a shifter connected with
/// [`crate::ble::Bluetooth::connect`].
#[derive(Debug)]
pub struct Controller {
    handle: DeviceHandle,
    /// Which shifter it is, whose assignments its buttons follow.
    model: ControllerModel,
    assignments: Arc<Assignments>,
    connected: bool,
    /// Why the last attempt to connect failed, while the driver tries again (R80).
    failure: Option<String>,
    /// Controls pressed for, until [`ShiftInput::controls`] takes them.
    controls: Vec<Control>,
}

impl Controller {
    /// Shifts from the `model` of shifter behind `handle`, its buttons doing what
    /// `assignments` say for that model.
    #[must_use]
    pub fn new(
        handle: DeviceHandle,
        model: ControllerModel,
        assignments: Arc<Assignments>,
    ) -> Self {
        Self {
            handle,
            model,
            assignments,
            connected: false,
            failure: None,
            controls: Vec::new(),
        }
    }
}

impl ShiftInput for Controller {
    fn name(&self) -> &str {
        self.handle.name()
    }

    fn poll(&mut self) -> Vec<Shift> {
        let mut shifts = Vec::new();
        while let Ok(Some(event)) = self.handle.try_next_event() {
            match event {
                DeviceEvent::Connected => {
                    self.connected = true;
                    self.failure = None;
                }
                DeviceEvent::Disconnected => self.connected = false,
                DeviceEvent::Failed(reason) => self.failure = Some(reason),
                DeviceEvent::Buttons(presses) => {
                    for action in self.assignments.get(self.model).actions(&presses) {
                        shifts.extend_from_slice(action.shifts());
                        if let ButtonAction::Control(control) = action {
                            self.controls.push(control);
                        }
                    }
                }
                DeviceEvent::Telemetry(_) => {}
            }
        }
        shifts
    }

    fn controls(&mut self) -> Vec<Control> {
        std::mem::take(&mut self.controls)
    }

    fn connected(&self) -> bool {
        self.connected
    }

    fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
}

/// Shifts from the keyboard, as passed on through its [`KeyboardKeys`].
#[derive(Debug)]
pub struct Keyboard {
    shifts: mpsc::Receiver<Shift>,
}

/// Passes key presses on to a [`Keyboard`].
#[derive(Debug, Clone)]
pub struct KeyboardKeys {
    shifts: mpsc::Sender<Shift>,
}

/// A keyboard shift input and the handle the front end presses its keys through.
#[must_use]
pub fn keyboard() -> (KeyboardKeys, Keyboard) {
    let (shifts, received) = mpsc::channel();
    (KeyboardKeys { shifts }, Keyboard { shifts: received })
}

impl KeyboardKeys {
    /// A shift key was pressed.
    pub fn press(&self, shift: Shift) {
        // Only fails once the keyboard input is gone, when no one shifts any more.
        let _ = self.shifts.send(shift);
    }
}

impl ShiftInput for Keyboard {
    fn name(&self) -> &'static str {
        "Keyboard"
    }

    fn poll(&mut self) -> Vec<Shift> {
        self.shifts.try_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use torqa_domain::shifting::{ButtonPress, Press};

    use super::*;

    /// A shifter called `name` that connects and sends `presses`.
    fn shifter(name: &str, presses: Vec<ButtonPress>) -> DeviceHandle {
        DeviceHandle::spawn(name.to_owned(), false, |channels| async move {
            let _ = channels.events.send(DeviceEvent::Connected).await;
            let _ = channels.events.send(DeviceEvent::Buttons(presses)).await;
        })
    }

    fn press(channel: u8, press: Press) -> ButtonPress {
        ButtonPress { channel, press }
    }

    #[tokio::test]
    async fn a_shifter_that_cannot_be_connected_says_why_until_it_is() {
        let handle =
            crate::fake::spawn_failing("Zwift Click", "device not found", Duration::from_millis(5));
        let mut controller = Controller::new(
            handle,
            ControllerModel::ZwiftClick,
            Arc::new(Assignments::default()),
        );
        assert_eq!(controller.failure(), None);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while controller.failure().is_none() {
            assert!(
                std::time::Instant::now() < deadline,
                "the failure was never told"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
            assert_eq!(controller.poll(), []);
        }
        assert!(!controller.connected());
        assert_eq!(controller.failure(), Some("device not found"));
    }

    #[tokio::test]
    async fn presses_shift_or_ask_for_the_controls_their_buttons_were_given() {
        let mut map = ButtonMap::shifting(2, 1);
        map.assign(
            3,
            Press::Short,
            Some(ButtonAction::Control(Control::NextCamera)),
        );
        let presses = vec![
            press(2, Press::Short),
            press(3, Press::Short),
            press(1, Press::Double),
            press(4, Press::Short),
        ];
        let assignments = Arc::new(Assignments::new(|_| map));
        let mut controller = Controller::new(
            shifter("RDR8150", presses),
            ControllerModel::Di2,
            assignments,
        );
        tokio::task::yield_now().await;

        let shifts = controller.poll();

        assert_eq!(shifts, [Shift::Up, Shift::Down, Shift::Down]);
        assert_eq!(controller.controls(), [Control::NextCamera]);
        assert_eq!(controller.controls(), [], "taken once");
        assert!(controller.connected());
    }

    #[tokio::test]
    async fn a_shifters_buttons_do_what_they_were_given_for_its_model_alone() {
        let assignments = Arc::new(Assignments::default());
        let mut di2 = ButtonMap::NONE;
        di2.assign(1, Press::Short, Some(ButtonAction::Control(Control::Pause)));
        assignments.set(ControllerModel::Di2, di2);
        let mut click = Controller::new(
            shifter("Zwift Click", vec![press(1, Press::Short)]),
            ControllerModel::ZwiftClick,
            Arc::clone(&assignments),
        );
        tokio::task::yield_now().await;

        assert_eq!(click.poll(), [Shift::Up], "the Click's + shifts as it did");
        assert_eq!(click.controls(), []);
        assert_eq!(assignments.get(ControllerModel::Di2), di2);
        assert_eq!(
            assignments.get(ControllerModel::ZwiftClickV2),
            ButtonMap::default()
        );
    }

    #[test]
    fn keys_pressed_come_out_in_order_once() {
        let (keys, mut keyboard) = keyboard();

        keys.press(Shift::Up);
        keys.press(Shift::Up);
        keys.press(Shift::Down);

        assert_eq!(keyboard.poll(), [Shift::Up, Shift::Up, Shift::Down]);
        assert_eq!(keyboard.poll(), []);
    }
}
