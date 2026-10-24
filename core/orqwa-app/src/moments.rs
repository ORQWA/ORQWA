//! The moments of a ride worth telling the rider of (R124): what the ride screen shows on a
//! card while it happens and the summary lists as the ride's highlights.

use std::time::Duration;

use orqwa_domain::profile::UnitSystem;
use orqwa_domain::units::{GradePercent, Meters};

/// Distance between the "so far" milestones: 25 of the rider's kilometres or miles.
const MILESTONE_STEPS: f64 = 25.0;
const METERS_PER_MILE: f64 = 1_609.344;
/// Halfway and "1 km to go" need a route longer than this, or they crowd its start.
const MIN_MILESTONE_ROUTE: Meters = Meters(2_000.0);
/// A "so far" milestone this close to halfway or the last kilometre would only repeat it.
const MILESTONE_GAP: Meters = Meters(1_000.0);

/// Something that happened on the ride.
#[derive(Debug, Clone, PartialEq)]
pub enum Moment {
    /// The rider topped a climb, not faster than before.
    ClimbTopped {
        /// Index in [`Route::climbs`](orqwa_routes::Route::climbs).
        index: usize,
        /// Time from foot to top.
        elapsed: Duration,
        /// The climb's average gradient.
        grade: GradePercent,
    },
    /// The rider topped a climb faster than ever before (R27).
    ClimbRecord {
        /// Index in [`Route::climbs`](orqwa_routes::Route::climbs).
        index: usize,
        /// Time from foot to top.
        elapsed: Duration,
        /// How much faster than the best time before.
        gain: Duration,
    },
    /// Half the route lies behind the rider.
    Halfway {
        /// The distance ridden.
        distance: Meters,
    },
    /// The finish is a kilometre away, or a mile for a rider in miles.
    OneToGo,
    /// Another 25 km ridden, or 25 miles for a rider in miles: the distance so far.
    Distance(Meters),
    /// The rider has never ridden this course before.
    FirstRide {
        /// The course's name.
        course: String,
    },
}

/// Finds the milestones of a ride as the rider passes them.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Milestones {
    /// The route's length; a ride without a route only has the "so far" milestones.
    length: Option<Meters>,
    /// Where the rider was at the last look.
    passed: Meters,
    /// The rider's kilometre or mile.
    unit: Meters,
}

impl Milestones {
    pub(crate) fn new(length: Option<Meters>, units: UnitSystem) -> Self {
        Self {
            length,
            passed: Meters(0.0),
            unit: Meters(match units {
                UnitSystem::Metric => 1_000.0,
                UnitSystem::Imperial => METERS_PER_MILE,
            }),
        }
    }

    /// The milestones between the last look and `distance`, in the order the rider met them.
    pub(crate) fn reach(&mut self, distance: Meters) -> Vec<Moment> {
        let from = self.passed.0;
        let to = distance.0;
        if to <= from {
            return Vec::new();
        }
        self.passed = distance;
        let crossed = |at: f64| from < at && at <= to;
        let mut found: Vec<(f64, Moment)> = Vec::new();
        let unit = self.unit.0;
        let every = MILESTONE_STEPS * unit;
        let length = self.length.filter(|l| l.0 > MIN_MILESTONE_ROUTE.0);
        if let Some(length) = length {
            let half = length.0 / 2.0;
            if crossed(half) {
                found.push((
                    half,
                    Moment::Halfway {
                        distance: Meters(half),
                    },
                ));
            }
            let last = length.0 - unit;
            if crossed(last) {
                found.push((last, Moment::OneToGo));
            }
        }
        // Whole multiples only: the steps between looks are far shorter than the gap.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let first = (from / every).floor() as u64 + 1;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let last = (to / every).floor() as u64;
        for n in first..=last {
            #[allow(clippy::cast_precision_loss)] // a few multiples of 25 km
            let at = n as f64 * every;
            let crowded = length.is_some_and(|length| {
                (at - length.0 / 2.0).abs() < MILESTONE_GAP.0
                    || at > length.0 - unit - MILESTONE_GAP.0
            });
            if !crowded {
                found.push((at, Moment::Distance(Meters(at))));
            }
        }
        found.sort_by(|a, b| a.0.total_cmp(&b.0));
        found.into_iter().map(|(_, moment)| moment).collect()
    }

    /// The rider jumped to `distance`: what lies between counts as passed, untold.
    pub(crate) fn skip_to(&mut self, distance: Meters) {
        self.passed = distance;
    }
}

/// The moment of topping a climb in `elapsed`, with the best time before this ride; a ride
/// that does not count towards records (`real` false) sets none.
pub(crate) fn climb_topped(
    index: usize,
    elapsed: Duration,
    grade: GradePercent,
    previous_best: Option<Duration>,
    real: bool,
) -> Moment {
    match previous_best.filter(|best| real && elapsed < *best) {
        Some(best) => Moment::ClimbRecord {
            index,
            elapsed,
            gain: best.saturating_sub(elapsed),
        },
        None => Moment::ClimbTopped {
            index,
            elapsed,
            grade,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rides `milestones` from the start to `to` in steps of `step`, collecting what it tells.
    fn ride(milestones: &mut Milestones, to: f64, step: f64) -> Vec<Moment> {
        let mut told = Vec::new();
        let mut at = 0.0;
        while at < to {
            at = (at + step).min(to);
            told.extend(milestones.reach(Meters(at)));
        }
        told
    }

    #[test]
    fn a_long_route_tells_each_milestone_once_in_order() {
        let mut milestones = Milestones::new(Some(Meters(80_000.0)), UnitSystem::Metric);

        let told = ride(&mut milestones, 80_000.0, 7.5);

        assert_eq!(
            told,
            vec![
                Moment::Distance(Meters(25_000.0)),
                Moment::Halfway {
                    distance: Meters(40_000.0)
                },
                Moment::Distance(Meters(50_000.0)),
                Moment::Distance(Meters(75_000.0)),
                Moment::OneToGo,
            ]
        );
    }

    #[test]
    fn a_distance_milestone_next_to_halfway_or_the_finish_is_left_out() {
        let mut milestones = Milestones::new(Some(Meters(50_400.0)), UnitSystem::Metric);

        let told = ride(&mut milestones, 50_400.0, 10.0);

        // 25 km is 200 m from halfway, 50 km lies in the last kilometre.
        assert_eq!(
            told,
            vec![
                Moment::Halfway {
                    distance: Meters(25_200.0)
                },
                Moment::OneToGo,
            ]
        );
    }

    #[test]
    fn a_short_route_only_tells_of_halfway_and_the_last_kilometre_when_long_enough() {
        let mut short = Milestones::new(Some(Meters(1_500.0)), UnitSystem::Metric);
        assert_eq!(ride(&mut short, 1_500.0, 5.0), []);

        let mut three = Milestones::new(Some(Meters(3_000.0)), UnitSystem::Metric);
        assert_eq!(
            ride(&mut three, 3_000.0, 5.0),
            vec![
                Moment::Halfway {
                    distance: Meters(1_500.0)
                },
                Moment::OneToGo,
            ]
        );
    }

    #[test]
    fn a_ride_without_a_route_counts_the_distance_only() {
        let mut milestones = Milestones::new(None, UnitSystem::Metric);

        let told = ride(&mut milestones, 60_000.0, 9.0);

        assert_eq!(
            told,
            vec![
                Moment::Distance(Meters(25_000.0)),
                Moment::Distance(Meters(50_000.0)),
            ]
        );
    }

    #[test]
    fn a_rider_in_miles_gets_milestones_in_miles() {
        let mile = 1_609.344;
        let every = 25.0 * mile;
        let mut milestones = Milestones::new(Some(Meters(100.0 * mile)), UnitSystem::Imperial);

        let told = ride(&mut milestones, 100.0 * mile, 10.0);

        assert_eq!(
            told,
            vec![
                Moment::Distance(Meters(every)),
                Moment::Halfway {
                    distance: Meters(50.0 * mile)
                },
                Moment::Distance(Meters(3.0 * every)),
                Moment::OneToGo,
            ],
            "50 mi is halfway, and 1 mi to go comes at 99 mi"
        );
        let mut again = Milestones::new(Some(Meters(100.0 * mile)), UnitSystem::Imperial);
        let last_mile = ride(&mut again, 99.0 * mile - 5.0, 10.0);
        assert!(!last_mile.contains(&Moment::OneToGo));
    }

    #[test]
    fn jumping_passes_milestones_without_telling_them() {
        let mut milestones = Milestones::new(Some(Meters(80_000.0)), UnitSystem::Metric);
        milestones.skip_to(Meters(60_000.0));

        let told = ride_from(&mut milestones, 60_000.0, 80_000.0);

        assert_eq!(
            told,
            vec![Moment::Distance(Meters(75_000.0)), Moment::OneToGo]
        );
    }

    fn ride_from(milestones: &mut Milestones, from: f64, to: f64) -> Vec<Moment> {
        let mut told = Vec::new();
        let mut at = from;
        while at < to {
            at = (at + 10.0).min(to);
            told.extend(milestones.reach(Meters(at)));
        }
        told
    }

    #[test]
    fn a_climb_is_a_record_only_when_faster_than_a_real_earlier_time() {
        let grade = GradePercent(8.0);
        let minutes = |m: u64| Duration::from_secs(m * 60);

        assert_eq!(
            climb_topped(1, minutes(14), grade, Some(minutes(15)), true),
            Moment::ClimbRecord {
                index: 1,
                elapsed: minutes(14),
                gain: minutes(1)
            }
        );
        let topped = Moment::ClimbTopped {
            index: 1,
            elapsed: minutes(14),
            grade,
        };
        assert_eq!(
            climb_topped(1, minutes(14), grade, None, true),
            topped,
            "a first time"
        );
        assert_eq!(
            climb_topped(1, minutes(14), grade, Some(minutes(13)), true),
            topped,
            "slower"
        );
        assert_eq!(
            climb_topped(1, minutes(14), grade, Some(minutes(15)), false),
            topped,
            "simulated"
        );
    }
}
