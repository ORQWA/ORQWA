//! Choosing which images a world derives facts from: one per stretch of road and side.
//!
//! A corridor can hold hundreds of photographs of the same hundred metres. The world needs one
//! view of each stretch from each side, preferably looking along the road, so that a later
//! preparation chooses the same image again.

use std::collections::BTreeMap;

use crate::corridor::{Located, Side};

/// A stretch and side, as a map key.
type Key = (i64, u8);

/// One candidate's alignment with the road, when it was taken, and where it is in the list.
type Ranked = (f64, i64, usize);

/// How far apart chosen images stand by default, in metres: often enough to see every building,
/// far enough that a course costs a few dozen images rather than thousands.
pub const STRETCH: f64 = 200.0;

/// Which half of a picture shows the side of the road being described.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameSide {
    /// The left half of the picture.
    Left,
    /// The right half of the picture.
    Right,
    /// The whole picture, for a camera on the line looking along it.
    Both,
}

impl FrameSide {
    /// Its name, as written to a report.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Both => "both",
        }
    }
}

/// An image that could stand for a stretch of road, and which half of the picture shows the
/// side it was chosen for.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate<'a> {
    /// The image and where it lies.
    pub located: &'a Located,
    /// Which half of the picture shows the side it was chosen for.
    pub frame_side: FrameSide,
}

/// The images that stand for one stretch of road and one side, best first.
///
/// More than one is kept because the best-aligned picture of a stretch often looks along the
/// road past the buildings, while another picture of the same stretch shows them. A preparation
/// tries them in turn until one shows a wall.
#[derive(Debug, Clone, PartialEq)]
pub struct Chosen<'a> {
    /// The candidates, best first. Never empty.
    pub candidates: Vec<Candidate<'a>>,
}

impl<'a> Chosen<'a> {
    /// The image tried first.
    #[must_use]
    pub fn first(&self) -> &Candidate<'a> {
        &self.candidates[0]
    }
}

/// How many candidates are kept for each stretch and side.
pub const CANDIDATES: usize = 3;

/// One stretch and side per `stretch` metres, with its candidates, the best aligned with the
/// road first and newest on ties.
///
/// An image taken against the direction of travel still counts, with its sides swapped: what is
/// on the picture's left is then the road's right. An image on the line itself describes both.
#[must_use]
#[allow(clippy::cast_possible_truncation)] // a stretch index, not a position
pub fn choose(located: &[Located], stretch: f64) -> Vec<Chosen<'_>> {
    let stretch = if stretch > 0.0 { stretch } else { STRETCH };
    let mut best: BTreeMap<Key, Vec<Ranked>> = BTreeMap::new();
    for (index, image) in located.iter().enumerate() {
        let key = (
            (image.along / stretch).floor() as i64,
            match image.side {
                Side::Left => 0,
                Side::On => 1,
                Side::Right => 2,
            },
        );
        let alignment = (image.image.heading - image.road_heading).cos();
        best.entry(key)
            .or_default()
            .push((alignment, image.image.captured_at, index));
    }
    let mut chosen: Vec<Chosen<'_>> = best
        .into_values()
        .map(|mut candidates| {
            candidates.sort_by(|a, b| {
                b.0.total_cmp(&a.0)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| a.2.cmp(&b.2))
            });
            candidates.truncate(CANDIDATES);
            Chosen {
                candidates: candidates
                    .into_iter()
                    .map(|(alignment, _, index)| {
                        let located = &located[index];
                        Candidate {
                            located,
                            frame_side: match located.side {
                                Side::On => FrameSide::Both,
                                side if alignment >= 0.0 => frame_side(side),
                                side => frame_side(opposite(side)),
                            },
                        }
                    })
                    .collect(),
            }
        })
        .collect();
    chosen.sort_by(|a, b| {
        let (a, b) = (a.first(), b.first());
        a.located
            .along
            .total_cmp(&b.located.along)
            .then_with(|| frame_side_rank(a.frame_side).cmp(&frame_side_rank(b.frame_side)))
    });
    chosen
}

fn frame_side(side: Side) -> FrameSide {
    match side {
        Side::Left => FrameSide::Left,
        Side::Right => FrameSide::Right,
        Side::On => FrameSide::Both,
    }
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
        Side::On => Side::On,
    }
}

fn frame_side_rank(side: FrameSide) -> u8 {
    match side {
        FrameSide::Left => 0,
        FrameSide::Both => 1,
        FrameSide::Right => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corridor::locate;
    use orqwa_domain::imagery::{LatLon, StreetImage};

    fn image(id: &str, at: LatLon, heading: f64, captured_at: i64) -> StreetImage {
        StreetImage {
            id: id.to_owned(),
            position: at,
            heading,
            captured_at,
            thumbnail: None,
        }
    }

    /// A straight road north from (46, 7), twice as long as asked so the stretch lies inside it.
    fn road(length: f64) -> Vec<LatLon> {
        let metres = 111_132.0;
        vec![
            (46.0, 7.0),
            (46.0 + length / metres, 7.0),
            (46.0 + 2.0 * length / metres, 7.0),
        ]
    }

    #[allow(clippy::cast_possible_truncation)] // a test distance, not a position
    fn beside(points: &[LatLon], along: f64, east: f64, heading: f64) -> StreetImage {
        let metres = 111_132.0;
        let east_metres = metres * points[0].0.to_radians().cos();
        image(
            &format!("{along:.0}-{east:.0}"),
            (
                points[0].0 + along / metres,
                points[0].1 + east / east_metres,
            ),
            heading,
            along as i64,
        )
    }

    #[test]
    fn one_image_per_stretch_and_side_is_chosen() {
        let points = road(600.0);
        let mut images = Vec::new();
        for along in [10.0, 110.0, 210.0, 310.0, 410.0, 510.0] {
            for east in [-8.0, 8.0] {
                images.push(beside(&points, along, east, 0.0));
            }
        }
        let located = locate(&points, &images);
        let chosen = choose(&located, 200.0);
        // Three stretches hold the images at 10..510 m, each with both sides.
        assert_eq!(chosen.len(), 6, "{chosen:#?}");
        assert!(chosen.iter().any(|c| c.first().located.side == Side::Left));
        assert!(chosen.iter().any(|c| c.first().located.side == Side::Right));
    }

    #[test]
    fn a_few_alternatives_are_kept_best_first() {
        let points = road(300.0);
        let images: Vec<_> = (0..6)
            .map(|n| beside(&points, 40.0 + f64::from(n), 8.0, 1.5 - f64::from(n) * 0.3))
            .collect();
        let located = locate(&points, &images);
        let chosen = choose(&located, 200.0);
        assert_eq!(chosen.len(), 1);
        let candidates = &chosen[0].candidates;
        assert_eq!(candidates.len(), CANDIDATES);
        // Headings fall towards the road's, so the last image is the straightest and the
        // three best aligned are kept, in that order.
        let ids: Vec<&str> = candidates
            .iter()
            .map(|candidate| candidate.located.image.id.as_str())
            .collect();
        assert_eq!(ids, ["45-8", "44-8", "43-8"]);
    }

    #[test]
    fn the_image_looking_most_along_the_road_wins() {
        let points = road(300.0);
        let aligned = beside(&points, 50.0, 8.0, 0.15);
        let sideways = beside(&points, 90.0, 8.0, 1.4);
        let located = locate(&points, &[sideways.clone(), aligned.clone()]);
        let chosen = choose(&located, 200.0);
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].first().located.image.id, aligned.id);
    }

    #[test]
    fn a_newer_image_wins_when_both_look_along_the_road() {
        let points = road(300.0);
        let older = beside(&points, 50.0, 8.0, 0.0);
        let mut newer = beside(&points, 90.0, 8.0, 0.0);
        newer.captured_at = older.captured_at + 1;
        let located = locate(&points, &[older, newer.clone()]);
        let chosen = choose(&located, 200.0);
        assert_eq!(chosen[0].first().located.image.id, newer.id);
    }

    #[test]
    fn an_image_taken_backwards_describes_the_other_side_of_the_picture() {
        let points = road(300.0);
        let backwards = beside(&points, 50.0, 8.0, std::f64::consts::PI);
        let located = locate(&points, &[backwards]);
        assert_eq!(located[0].side, Side::Right);
        let chosen = choose(&located, 200.0);
        assert_eq!(chosen[0].first().frame_side, FrameSide::Left);
    }

    #[test]
    fn an_image_on_the_line_describes_both_sides() {
        let points = road(300.0);
        let on_line = beside(&points, 50.0, 0.0, 0.0);
        let located = locate(&points, &[on_line]);
        assert_eq!(located[0].side, Side::On);
        let chosen = choose(&located, 200.0);
        assert_eq!(chosen[0].first().frame_side, FrameSide::Both);
    }
}
