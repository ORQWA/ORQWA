//! The Zwift Click v2 over Bluetooth (R77): the two pods sold with the Zwift Cog since
//! September 2025, a right one with **+**, **Y**, **Z**, **A** and **B**, a left one with
//! **−** and four arrows. Each pod is a Bluetooth device of its own, with the original Click's
//! characteristics ([`crate::zwift_click`]) under the Zwift Ride's service and the Ride's plain
//! messages: the app writes `RideOn`, the pod answers `RideOn` and a version, and from then on
//! notifies the state of the buttons as one bit map whenever it changes. The left pod's map
//! holds the right pod's buttons too: it relays them, as it does for Zwift's own app.
//!
//! Two things set the pods apart from the original Click, as the community found out
//! (ADR 0003):
//!
//! - Zwift **locks the left pod** to its own app, as the community reports: unless it was in a
//!   Zwift session within about a day, it answers the handshake with a challenge Torqa cannot
//!   answer, and tells nothing of its buttons. An unlocked pod stays unlocked while an [`UNLOCK_ACK`] follows the handshake.
//!   The right pod is never locked.
//! - A pod **falls asleep** after about a minute without a message, dropping the link, so the
//!   handshake is written again every [`KEEPALIVE`].
//!
//! The protocol is not published by Zwift; it is as the community documented it and as
//! `BikeControl`, QZ and `MaximumTrainer` drive the pods, and as a maintainer's pair answered
//! it (R96, ADR 0003): the handshake, the buttons' bits and the keepalive hold. That pair's left
//! pod was not locked, so the lock is the community's account alone.

use std::time::{Duration, Instant};

use orqwa_domain::shifting::ButtonPress;

use crate::bytes::ParseError;
use crate::zwift_click::{Button, HANDSHAKE, varint_fields};

/// The right pod's device type, the first byte of Zwift's advertising data.
pub const RIGHT_POD: u8 = 0x0A;
/// The left pod's device type.
pub const LEFT_POD: u8 = 0x0B;
/// What Torqa calls the pair: the pods advertise the original Click's name.
pub const NAME: &str = "Zwift Click v2";
/// Written to the left pod's control characteristic after the handshake: keeps an unlocked
/// pod unlocked across connections, and does nothing to a locked one.
pub const UNLOCK_ACK: &[u8] = &[0xFF, 0x04, 0x00];
/// How often the handshake is written again, so a pod does not fall asleep.
pub const KEEPALIVE: Duration = Duration::from_secs(3);

/// A notification of the buttons' states.
const BUTTON_STATES: u8 = 0x23;
/// Its field: the map of all buttons, a bit clear while its button is down. Other fields
/// (the Ride's analog paddles) are nested messages and follow it.
const MAP_FIELD: u64 = 1;
/// The first byte of an answer that is no `RideOn`: a locked pod's challenge or a stop.
const VENDOR: u8 = 0xFF;
const CHALLENGE: u8 = 0x03;
const STOPPED: u8 = 0x05;

/// Which pod of the pair a device is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// The pod with + and the lettered buttons; never locked.
    Right,
    /// The pod with − and the arrows; locked unless unlocked in Zwift lately.
    Left,
}

impl Side {
    /// The pod a device type names, if a Click v2's.
    #[must_use]
    pub fn from_device_type(device_type: u8) -> Option<Self> {
        match device_type {
            RIGHT_POD => Some(Self::Right),
            LEFT_POD => Some(Self::Left),
            _ => None,
        }
    }

    /// The pod's name for the log.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Right => "right pod",
            Self::Left => "left pod",
        }
    }
}

/// The buttons as the shifter's assignments number them (#139), each with its pod and its bit
/// in the map: + and − first, so the default map (1 shifts up, 2 down) fits as it fits a Di2
/// and the original Click; then the right pod's Y, Z, A and B and the left pod's ↑, ↓, ← and →.
/// The bits are those of `BikeControl`'s code, as a pair confirmed (R96).
const BUTTONS: [(Side, u32); 10] = [
    (Side::Right, 0x1000),
    (Side::Left, 0x0100),
    (Side::Right, 0x0040),
    (Side::Right, 0x0080),
    (Side::Right, 0x0010),
    (Side::Right, 0x0020),
    (Side::Left, 0x0002),
    (Side::Left, 0x0008),
    (Side::Left, 0x0001),
    (Side::Left, 0x0004),
];

/// What a pod answers on the response characteristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The session is up: the pod tells of its buttons from now on.
    Started,
    /// The pod is locked to Zwift and tells nothing of its buttons until unlocked there.
    Locked,
    /// The pod stops telling of its buttons.
    Stopped,
    /// Something the community has not documented.
    Unknown,
}

/// What an answer on the response characteristic says.
#[must_use]
pub fn answer(value: &[u8]) -> Answer {
    match value {
        _ if value.starts_with(HANDSHAKE) => Answer::Started,
        [VENDOR, CHALLENGE, ..] => Answer::Locked,
        [VENDOR, STOPPED, ..] => Answer::Stopped,
        _ => Answer::Unknown,
    }
}

/// Turns the pods' button-state notifications into presses, each once, as
/// [`crate::zwift_click::Buttons`] does for the original Click: a hold once a button has been
/// down for [`crate::zwift_click::HOLD`], a press when it comes up before that.
#[derive(Debug, Clone, Default)]
pub struct Buttons {
    buttons: [Button; BUTTONS.len()],
}

impl Buttons {
    /// The presses one notification from `side`, received at `now`, tells of. Only that pod's
    /// own buttons count: the left pod reports the right pod's too, which would otherwise count
    /// each of them twice (R96).
    ///
    /// # Errors
    /// [`ParseError`] if the notification is empty or cut short.
    pub fn presses(
        &mut self,
        side: Side,
        data: &[u8],
        now: Instant,
    ) -> Result<Vec<ButtonPress>, ParseError> {
        let (&kind, fields) = data.split_first().ok_or(ParseError::TooShort {
            needed: 1,
            actual: 0,
        })?;
        if kind != BUTTON_STATES {
            return Ok(Vec::new());
        }
        let map = varint_fields(fields)?
            .into_iter()
            .find(|&(field, _)| field == MAP_FIELD)
            .map(|(_, value)| value);
        let Some(map) = map else {
            return Ok(Vec::new());
        };
        let mut presses = Vec::new();
        for ((button, &(pod, bit)), number) in self.buttons.iter_mut().zip(&BUTTONS).zip(1..) {
            if pod != side {
                continue;
            }
            let down = map & u64::from(bit) == 0;
            if let Some(press) = button.update(down, now) {
                presses.push(ButtonPress {
                    channel: number,
                    press,
                });
            }
        }
        Ok(presses)
    }
}

#[cfg(test)]
mod tests {
    use orqwa_domain::shifting::Press;

    use super::*;
    use crate::zwift_click::HOLD;

    const ALL_UP: u32 = 0xFFFF_FFFF;
    const PLUS: u32 = 0x1000;
    const MINUS: u32 = 0x0100;
    const Y: u32 = 0x0040;
    const B: u32 = 0x0020;
    const UP: u32 = 0x0002;

    /// A button-state notification with the buttons of `down` down.
    fn frame(down: u32) -> Vec<u8> {
        let mut map = ALL_UP & !down;
        let mut frame = vec![BUTTON_STATES, 0x08];
        while map >= 0x80 {
            frame.push((map & 0x7F) as u8 | 0x80);
            map >>= 7;
        }
        frame.push(u8::try_from(map).expect("below 0x80"));
        frame
    }

    fn press(channel: u8, press: Press) -> ButtonPress {
        ButtonPress { channel, press }
    }

    #[test]
    fn the_released_state_is_the_frame_the_community_documented() {
        assert_eq!(frame(0), [0x23, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn a_tap_of_plus_is_a_press_of_button_1_when_it_comes_up() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        let down = buttons.presses(Side::Right, &frame(PLUS), start).unwrap();
        let up = buttons
            .presses(Side::Right, &frame(0), start + Duration::from_millis(150))
            .unwrap();
        let again = buttons
            .presses(Side::Right, &frame(0), start + Duration::from_millis(250))
            .unwrap();

        assert_eq!(down, []);
        assert_eq!(up, [press(1, Press::Short)]);
        assert_eq!(again, [], "the state repeated is no new press");
    }

    #[test]
    fn minus_held_on_the_left_pod_is_a_hold_of_button_2_once() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        buttons.presses(Side::Left, &frame(MINUS), start).unwrap();
        let held = buttons
            .presses(Side::Left, &frame(MINUS), start + HOLD)
            .unwrap();
        let longer = buttons
            .presses(Side::Left, &frame(MINUS), start + HOLD * 2)
            .unwrap();
        let up = buttons
            .presses(Side::Left, &frame(0), start + HOLD * 3)
            .unwrap();

        assert_eq!(held, [press(2, Press::Long)]);
        assert_eq!(longer, []);
        assert_eq!(up, [], "a hold is not also a press when let go");
    }

    #[test]
    fn each_pod_tells_of_its_own_buttons_only() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        // A frame from the left pod shows + up: that is not + let go on the right pod.
        buttons.presses(Side::Right, &frame(PLUS), start).unwrap();
        let left = buttons
            .presses(Side::Left, &frame(UP), start + Duration::from_millis(100))
            .unwrap();
        let left_up = buttons
            .presses(Side::Left, &frame(0), start + Duration::from_millis(200))
            .unwrap();
        let right_up = buttons
            .presses(Side::Right, &frame(0), start + Duration::from_millis(300))
            .unwrap();

        assert_eq!(left, []);
        assert_eq!(left_up, [press(7, Press::Short)], "↑ is button 7");
        assert_eq!(right_up, [press(1, Press::Short)], "+ was down all along");
    }

    #[test]
    fn the_lettered_buttons_are_3_to_6_and_two_at_once_both_count() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        buttons.presses(Side::Right, &frame(Y | B), start).unwrap();
        let b_up = buttons
            .presses(Side::Right, &frame(Y), start + Duration::from_millis(100))
            .unwrap();
        let y_up = buttons
            .presses(Side::Right, &frame(0), start + Duration::from_millis(200))
            .unwrap();

        assert_eq!(b_up, [press(6, Press::Short)]);
        assert_eq!(y_up, [press(3, Press::Short)]);
    }

    /// The released frame and + down, as a maintainer's pair sent them (R96).
    const PAIR_UP: [u8; 7] = [0x23, 0x08, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
    const PAIR_PLUS: [u8; 7] = [0x23, 0x08, 0xFF, 0xDF, 0xFF, 0xFF, 0x0F];

    #[test]
    fn each_button_of_a_real_pair_is_its_number() {
        // The map's first two bytes in the pair's frames, one button down at a time; the
        // rest of each frame is as in `PAIR_UP`.
        let pressed = [
            ("+", Side::Right, [0xFF, 0xDF], 1),
            ("−", Side::Left, [0xFF, 0xFD], 2),
            ("Y", Side::Right, [0xBF, 0xFF], 3),
            ("Z", Side::Right, [0xFF, 0xFE], 4),
            ("A", Side::Right, [0xEF, 0xFF], 5),
            ("B", Side::Right, [0xDF, 0xFF], 6),
            ("↑", Side::Left, [0xFD, 0xFF], 7),
            ("↓", Side::Left, [0xF7, 0xFF], 8),
            ("←", Side::Left, [0xFE, 0xFF], 9),
            ("→", Side::Left, [0xFB, 0xFF], 10),
        ];
        for (name, side, [low, high], number) in pressed {
            let mut buttons = Buttons::default();
            let start = Instant::now();

            let frame = [0x23, 0x08, low, high, 0xFF, 0xFF, 0x0F];
            buttons.presses(side, &frame, start).unwrap();
            let up = buttons
                .presses(side, &PAIR_UP, start + Duration::from_millis(100))
                .unwrap();

            assert_eq!(up, [press(number, Press::Short)], "{name}");
        }
    }

    #[test]
    fn a_right_pod_press_the_left_pod_relays_counts_once() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        // A tap of + as the pair sent it: both pods report it down, then both up.
        let frames = [
            (Side::Left, PAIR_PLUS, 0),
            (Side::Right, PAIR_PLUS, 28),
            (Side::Left, PAIR_PLUS, 90),
            (Side::Right, PAIR_PLUS, 118),
            (Side::Right, PAIR_UP, 208),
            (Side::Left, PAIR_UP, 241),
        ];
        let presses: Vec<_> = frames
            .into_iter()
            .flat_map(|(side, frame, ms)| {
                buttons
                    .presses(side, &frame, start + Duration::from_millis(ms))
                    .unwrap()
            })
            .collect();

        assert_eq!(presses, [press(1, Press::Short)]);
    }

    #[test]
    fn other_messages_and_frames_without_the_map_press_nothing() {
        let mut buttons = Buttons::default();
        let now = Instant::now();

        assert_eq!(buttons.presses(Side::Right, &[0x15], now).unwrap(), []);
        assert_eq!(
            buttons
                .presses(Side::Right, &[0x19, 0x10, 0x5D], now)
                .unwrap(),
            []
        );
        // The left pod's status and vendor messages, between its button frames (R96).
        let status = [
            0x2A, 0x08, 0x03, 0x12, 0x12, 0x12, 0x10, 0x08, 0x18, 0x10, 0x00, 0x18, 0x48, 0x22,
            0x05, 0xFF, 0xFF, 0xFF, 0xFF, 0x1F, 0x28, 0xFB, 0x01,
        ];
        assert_eq!(buttons.presses(Side::Left, &status, now).unwrap(), []);
        let vendor = [0xFF, 0x05, 0x00, 0xFA, 0x05, 0x18, 0x0A, 0x0C];
        assert_eq!(buttons.presses(Side::Left, &vendor, now).unwrap(), []);
        assert_eq!(buttons.presses(Side::Right, &[0x23], now).unwrap(), []);
        assert!(buttons.presses(Side::Right, &[], now).is_err());
        assert!(
            buttons
                .presses(Side::Right, &[0x23, 0x08, 0xFF], now)
                .is_err()
        );
    }

    #[test]
    fn the_answers_tell_a_session_from_a_lock() {
        assert_eq!(answer(b"RideOn\x02\x03"), Answer::Started);
        assert_eq!(answer(b"RideOn"), Answer::Started);
        assert_eq!(answer(&[0xFF, 0x03, 0x12, 0x34]), Answer::Locked);
        assert_eq!(answer(&[0xFF, 0x05, 0x00, 0xEA, 0x05]), Answer::Stopped);
        assert_eq!(answer(&[0x00]), Answer::Unknown);
        assert_eq!(answer(&[]), Answer::Unknown);
    }

    #[test]
    fn the_sides_come_from_the_device_types() {
        assert_eq!(Side::from_device_type(RIGHT_POD), Some(Side::Right));
        assert_eq!(Side::from_device_type(LEFT_POD), Some(Side::Left));
        assert_eq!(Side::from_device_type(9), None, "the original Click");
        assert_eq!(Side::from_device_type(8), None, "a Ride's pod");
    }
}
