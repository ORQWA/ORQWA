//! How full a device's batteries are (R97).

use std::fmt;

/// How full a device's batteries are, in percent, as far as the device told (R97): one level
/// for most devices, one per side for a pair such as the Zwift Click v2's pods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Battery {
    /// The device's own battery.
    pub level: Option<u8>,
    /// The left one of a pair's batteries.
    pub left: Option<u8>,
    /// The right one of a pair's batteries.
    pub right: Option<u8>,
}

impl Battery {
    /// Whether the device told of any of its batteries.
    #[must_use]
    pub fn is_known(&self) -> bool {
        self.level.is_some() || self.left.is_some() || self.right.is_some()
    }
}

/// `80 %`, or `left 80 %, right 75 %` for a pair, as far as known.
impl fmt::Display for Battery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(level) = self.level {
            return write!(f, "{level} %");
        }
        let sides: Vec<String> = [("left", self.left), ("right", self.right)]
            .into_iter()
            .filter_map(|(side, level)| level.map(|level| format!("{side} {level} %")))
            .collect();
        f.write_str(&sides.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pair_tells_each_side_it_knows() {
        let left = Battery {
            left: Some(80),
            ..Battery::default()
        };
        let both = Battery {
            right: Some(75),
            ..left
        };

        assert_eq!(left.to_string(), "left 80 %");
        assert_eq!(both.to_string(), "left 80 %, right 75 %");
        assert!(both.is_known());
        assert!(!Battery::default().is_known());
    }
}
