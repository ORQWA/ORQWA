//! The Zwift Click over Bluetooth (R75): the original single unit with a + and a − button,
//! sold with the Zwift Cog. On today's firmware its protocol is plain: the app writes
//! `RideOn` to the control characteristic, the unit answers `RideOn` on the response
//! characteristic, and from then on notifies the state of both buttons whenever it changes,
//! about ten times a second while a button is held. No key exchange, no pairing, no unlock.
//!
//! The unit only tells whether a button is down. Torqa makes the presses of a Di2 out of that
//! (#139): a hold once the button has been down for [`HOLD`], else a press when it is let go.
//! There is no double press, so a tap shifts the moment the button comes up.
//!
//! The protocol is not published by Zwift; it is as the community documented it and as a
//! unit answered when checked (ADR 0003).

use std::time::{Duration, Instant};

use torqa_domain::shifting::{ButtonPress, Press};

use crate::bytes::ParseError;

/// Zwift's own service, shared by the Click, Play and Ride.
pub const SERVICE: &str = "00000001-19ca-4651-86e5-fa29dcdd09d1";
/// Notifications: the buttons' states, and a status every few seconds.
pub const BUTTONS: &str = "00000002-19ca-4651-86e5-fa29dcdd09d1";
/// Written without response: the handshake.
pub const CONTROL: &str = "00000003-19ca-4651-86e5-fa29dcdd09d1";
/// Indications: the handshake's answer.
pub const RESPONSE: &str = "00000004-19ca-4651-86e5-fa29dcdd09d1";
/// Zwift's Bluetooth company identifier, in the advertising data.
pub const MANUFACTURER: u16 = 0x094A;
/// The Click's device type, the first byte of Zwift's advertising data (the Play's pads are
/// 2 and 3, the Ride's 7 and 8, the two-pod Click v2's 10 and 11).
pub const DEVICE_TYPE: u8 = 9;
/// The unit's advertised name, for stacks that pass neither its service nor its maker on.
pub const NAME: &str = "Zwift Click";
/// Written to [`CONTROL`] to start the session; the unit answers the same on [`RESPONSE`].
pub const HANDSHAKE: &[u8] = b"RideOn";
/// Down this long, a button press is a hold.
pub const HOLD: Duration = Duration::from_millis(500);
/// The + button's number in the shifter's assignments (#139).
pub const PLUS: u8 = 1;
/// The − button's number in the shifter's assignments.
pub const MINUS: u8 = 2;

/// A notification of the buttons' states; others (0x19, the status) say nothing of them.
const BUTTON_STATES: u8 = 0x37;
/// Protocol-buffer fields of a button-state notification: the + and the − button, `0` down.
const PLUS_FIELD: u64 = 1;
const MINUS_FIELD: u64 = 2;

/// Turns the unit's button-state notifications into presses, each once.
#[derive(Debug, Clone, Default)]
pub struct Buttons {
    buttons: [Button; 2],
}

#[derive(Debug, Clone, Copy, Default)]
struct Button {
    /// Since when it has been down, if it is.
    down_since: Option<Instant>,
    /// Whether its current press was already a hold, which is one press however long.
    held: bool,
}

impl Buttons {
    /// The presses one notification, received at `now`, tells of: a hold once a button has
    /// been down for [`HOLD`], a press when it comes up before that.
    ///
    /// # Errors
    /// [`ParseError`] if the notification is empty or cut short.
    pub fn presses(&mut self, data: &[u8], now: Instant) -> Result<Vec<ButtonPress>, ParseError> {
        let (&kind, fields) = data.split_first().ok_or(ParseError::TooShort {
            needed: 1,
            actual: 0,
        })?;
        if kind != BUTTON_STATES {
            return Ok(Vec::new());
        }
        let mut down = [None; 2];
        for (field, value) in varint_fields(fields)? {
            match field {
                PLUS_FIELD => down[0] = Some(value == 0),
                MINUS_FIELD => down[1] = Some(value == 0),
                _ => {}
            }
        }
        let mut presses = Vec::new();
        for ((button, down), number) in self.buttons.iter_mut().zip(down).zip(PLUS..) {
            let Some(down) = down else { continue };
            match (button.down_since, down) {
                (None, true) => {
                    button.down_since = Some(now);
                    button.held = false;
                }
                (Some(since), true) => {
                    if !button.held && now.duration_since(since) >= HOLD {
                        button.held = true;
                        presses.push(ButtonPress {
                            channel: number,
                            press: Press::Long,
                        });
                    }
                }
                (Some(_), false) => {
                    if !button.held {
                        presses.push(ButtonPress {
                            channel: number,
                            press: Press::Short,
                        });
                    }
                    button.down_since = None;
                    button.held = false;
                }
                (None, false) => {}
            }
        }
        Ok(presses)
    }
}

/// The varint fields of a protocol-buffer message as `(field number, value)`, in order.
/// Reading stops at the first field of another wire type: the unit sends none.
fn varint_fields(mut data: &[u8]) -> Result<Vec<(u64, u64)>, ParseError> {
    let mut fields = Vec::new();
    while !data.is_empty() {
        let (tag, rest) = varint(data)?;
        if tag & 0b111 != 0 {
            break;
        }
        let (value, rest) = varint(rest)?;
        fields.push((tag >> 3, value));
        data = rest;
    }
    Ok(fields)
}

/// A varint and what follows it.
fn varint(data: &[u8]) -> Result<(u64, &[u8]), ParseError> {
    let mut value = 0u64;
    for (i, &byte) in data.iter().enumerate().take(10) {
        value |= u64::from(byte & 0x7F) << (7 * i);
        if byte & 0x80 == 0 {
            return Ok((value, &data[i + 1..]));
        }
    }
    Err(ParseError::TooShort {
        needed: data.len() + 1,
        actual: data.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Notifications as a Click sent them, both buttons up, + down, − down.
    const BOTH_UP: [u8; 5] = [0x37, 0x08, 0x01, 0x10, 0x01];
    const PLUS_DOWN: [u8; 5] = [0x37, 0x08, 0x00, 0x10, 0x01];
    const MINUS_DOWN: [u8; 5] = [0x37, 0x08, 0x01, 0x10, 0x00];
    const STATUS: [u8; 3] = [0x19, 0x10, 0x5D];

    fn press(channel: u8, press: Press) -> ButtonPress {
        ButtonPress { channel, press }
    }

    #[test]
    fn a_tap_is_a_press_when_the_button_comes_up() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        let down = buttons.presses(&PLUS_DOWN, start).unwrap();
        let still = buttons
            .presses(&PLUS_DOWN, start + Duration::from_millis(100))
            .unwrap();
        let up = buttons
            .presses(&BOTH_UP, start + Duration::from_millis(180))
            .unwrap();
        let again = buttons
            .presses(&BOTH_UP, start + Duration::from_millis(280))
            .unwrap();

        assert_eq!(down, [], "not until it is known to be a tap");
        assert_eq!(still, []);
        assert_eq!(up, [press(PLUS, Press::Short)]);
        assert_eq!(again, [], "the state repeated is no new press");
    }

    #[test]
    fn a_button_down_for_the_hold_time_is_held_once() {
        let mut buttons = Buttons::default();
        let start = Instant::now();

        buttons.presses(&MINUS_DOWN, start).unwrap();
        let early = buttons
            .presses(&MINUS_DOWN, start + Duration::from_millis(499))
            .unwrap();
        let held = buttons.presses(&MINUS_DOWN, start + HOLD).unwrap();
        let longer = buttons.presses(&MINUS_DOWN, start + HOLD * 3).unwrap();
        let up = buttons.presses(&BOTH_UP, start + HOLD * 4).unwrap();
        let tapped = buttons.presses(&MINUS_DOWN, start + HOLD * 5).unwrap();
        let tap_up = buttons
            .presses(&BOTH_UP, start + HOLD * 5 + Duration::from_millis(100))
            .unwrap();

        assert_eq!(early, []);
        assert_eq!(held, [press(MINUS, Press::Long)]);
        assert_eq!(longer, []);
        assert_eq!(up, [], "a hold is not also a press when let go");
        assert_eq!(tapped, []);
        assert_eq!(tap_up, [press(MINUS, Press::Short)], "a new press after it");
    }

    #[test]
    fn both_buttons_count_on_their_own() {
        let mut buttons = Buttons::default();
        let start = Instant::now();
        let both_down = [0x37, 0x08, 0x00, 0x10, 0x00];

        buttons.presses(&both_down, start).unwrap();
        let minus_up = buttons
            .presses(&PLUS_DOWN, start + Duration::from_millis(100))
            .unwrap();
        let plus_up = buttons
            .presses(&BOTH_UP, start + Duration::from_millis(200))
            .unwrap();

        assert_eq!(minus_up, [press(MINUS, Press::Short)]);
        assert_eq!(plus_up, [press(PLUS, Press::Short)]);
    }

    #[test]
    fn the_status_and_unknown_fields_press_nothing() {
        let mut buttons = Buttons::default();
        let start = Instant::now();
        // A later firmware might add fields, or use a longer varint for a value.
        let more_fields = [0x37, 0x18, 0x05, 0x08, 0x81, 0x00, 0x10, 0x01];

        assert_eq!(buttons.presses(&STATUS, start).unwrap(), []);
        assert_eq!(buttons.presses(&more_fields, start).unwrap(), []);
        assert_eq!(
            buttons.presses(&PLUS_DOWN, start).unwrap(),
            [],
            "+ went down"
        );
        assert_eq!(
            buttons.presses(&STATUS, start + HOLD).unwrap(),
            [],
            "the status says nothing of the buttons"
        );
        assert_eq!(
            buttons.presses(&PLUS_DOWN, start + HOLD).unwrap(),
            [press(PLUS, Press::Long)],
            "held, as the next state tells"
        );
        assert_eq!(buttons.presses(&BOTH_UP, start + HOLD * 2).unwrap(), []);
    }

    #[test]
    fn empty_and_cut_short_notifications_are_errors() {
        let mut buttons = Buttons::default();
        let now = Instant::now();

        assert!(buttons.presses(&[], now).is_err());
        assert!(buttons.presses(&[0x37, 0x08], now).is_err());
        assert!(buttons.presses(&[0x37, 0x08, 0x80], now).is_err());
    }
}
