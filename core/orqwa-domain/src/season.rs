//! Rider-selected seasons, independent of the calendar and the ride's weather (#350).

/// The season used to present a 3D ride. Older settings and unknown future seasons use summer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    /// Green foliage and meadows, the original appearance.
    #[default]
    Summer,
}

impl Season {
    /// Seasons available in the ride options, with the default first.
    pub const ALL: &[Self] = &[Self::Summer];

    /// Stable, untranslated name used by settings and the presentation boundary.
    #[must_use]
    pub const fn name(self) -> &'static str {
        // i18n-begin
        match self {
            Self::Summer => "Summer",
        }
        // i18n-end
    }

    /// Looks up a season without accepting an unknown setting.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|season| season.name() == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summer_is_the_default_and_names_round_trip() {
        assert_eq!(Season::default(), Season::Summer);
        for season in Season::ALL {
            assert_eq!(Season::from_name(season.name()), Some(*season));
        }
        assert_eq!(Season::from_name("unknown"), None);
    }
}
