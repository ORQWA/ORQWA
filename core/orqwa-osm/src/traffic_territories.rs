//! Territories whose traffic side differs from the country named by the tiles.

use std::sync::LazyLock;

use crate::LatLon;

#[derive(serde::Deserialize)]
struct Territory {
    code: String,
    polygons: Vec<Vec<Vec<[f64; 2]>>>,
}

static TERRITORIES: LazyLock<Vec<Territory>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../data/traffic-territories.json"))
        .expect("bundled territory outlines are valid JSON")
});

/// ISO code for Hong Kong, Macau or the US Virgin Islands at `place`, whose traffic keeps
/// left while the country reported by the tiles keeps right. Uses bundled outlines offline.
#[must_use]
pub fn traffic_territory(place: LatLon) -> Option<&'static str> {
    TERRITORIES
        .iter()
        .find(|territory| {
            territory.polygons.iter().any(|polygon| {
                polygon.first().is_some_and(|outer| contains(outer, place))
                    && !polygon.iter().skip(1).any(|hole| contains(hole, place))
            })
        })
        .map(|territory| territory.code.as_str())
}

fn contains(ring: &[[f64; 2]], (lat, lon): LatLon) -> bool {
    let mut inside = false;
    for pair in ring.windows(2) {
        let ([ax, ay], [bx, by]) = (pair[0], pair[1]);
        if (ay > lat) != (by > lat) && lon < ax + (lat - ay) * (bx - ax) / (by - ay) {
            inside = !inside;
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn territory_rules_do_not_spill_over_the_mainland_border() {
        for (place, code) in [
            ((22.3193, 114.1694), "HK"),
            ((22.201, 113.546), "MO"),
            ((18.3419, -64.9307), "VI"),
            ((17.7466, -64.7032), "VI"),
        ] {
            assert_eq!(traffic_territory(place), Some(code));
        }
        for place in [
            (22.55, 114.10),
            (22.25, 113.52),
            (18.4655, -66.1057),
            (51.5, -0.1),
        ] {
            assert_eq!(traffic_territory(place), None);
        }
    }
}
