//! Bluetooth Battery Service (R97), and keeping what a device told of its batteries.

use torqa_domain::battery::Battery;

use crate::zwift_click_v2::Side;

/// Battery service.
pub const SERVICE: u16 = 0x180F;
/// Battery Level characteristic (read, notify where the device offers it).
pub const LEVEL: u16 = 0x2A19;

/// Decodes a Battery Level, in percent; `None` if it is empty or above 100 %.
#[must_use]
pub fn parse_level(data: &[u8]) -> Option<u8> {
    data.first().copied().filter(|&level| level <= 100)
}

/// Notes a `level` the device told, of its own battery or of its `side` of a pair: the
/// batteries as they are now if that changed them, so that each change is told once while a
/// device repeating its level (a Click v2 pod every few seconds) is not.
pub fn note(battery: &mut Battery, side: Option<Side>, level: u8) -> Option<Battery> {
    let known = match side {
        None => &mut battery.level,
        Some(Side::Left) => &mut battery.left,
        Some(Side::Right) => &mut battery.right,
    };
    (known.replace(level) != Some(level)).then_some(*battery)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_level_is_one_byte_in_percent() {
        assert_eq!(parse_level(&[85]), Some(85));
        assert_eq!(parse_level(&[100, 0]), Some(100));
        assert_eq!(parse_level(&[101]), None);
        assert_eq!(parse_level(&[]), None);
    }

    #[test]
    fn each_change_is_told_once_and_a_pair_keeps_both_sides() {
        let mut battery = Battery::default();

        let left = note(&mut battery, Some(Side::Left), 80);
        let again = note(&mut battery, Some(Side::Left), 80);
        let right = note(&mut battery, Some(Side::Right), 75);
        let lower = note(&mut battery, Some(Side::Left), 79);

        let pair = |left, right| Battery {
            left: Some(left),
            right: Some(right),
            level: None,
        };
        assert_eq!(left.map(|b| b.left), Some(Some(80)));
        assert_eq!(again, None, "the same level repeated is no news");
        assert_eq!(right, Some(pair(80, 75)));
        assert_eq!(lower, Some(pair(79, 75)));
    }

    #[test]
    fn a_device_of_one_battery_tells_its_level() {
        let mut battery = Battery::default();

        assert_eq!(
            note(&mut battery, None, 64),
            Some(Battery {
                level: Some(64),
                ..Battery::default()
            })
        );
    }
}
