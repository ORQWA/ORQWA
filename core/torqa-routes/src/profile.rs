//! A course's length and gradients alone: what simulating a ride along it needs.

use torqa_domain::units::{GradePercent, Meters};

use crate::Route;

/// How long a course is and how steep where: all a steady ride along it needs (R66), so a
/// course can be simulated from its full route or from the profile a course file keeps.
pub trait Gradients {
    /// Total length.
    fn length(&self) -> Meters;
    /// The gradient at a distance from the start, clamped to the course.
    fn grade_at(&self, distance: Meters) -> GradePercent;
}

impl Gradients for Route {
    fn length(&self) -> Meters {
        Route::length(self)
    }

    fn grade_at(&self, distance: Meters) -> GradePercent {
        Route::grade_at(self, distance)
    }
}

/// Elevations at distances along a course, with straight slopes between them: e.g. the
/// thinned profile a course file keeps for its card, enough to estimate a ride's time without
/// loading the route.
#[derive(Debug, Clone, PartialEq)]
pub struct ElevationProfile {
    /// `(distance, elevation)`, by distance.
    points: Vec<(Meters, Meters)>,
}

impl ElevationProfile {
    /// The profile through `points`, `(distance from the start, elevation)`; `None` without
    /// two points of different distance. Points out of order are sorted.
    #[must_use]
    pub fn new(points: impl IntoIterator<Item = (Meters, Meters)>) -> Option<Self> {
        let mut points: Vec<_> = points
            .into_iter()
            .filter(|(d, e)| d.0.is_finite() && e.0.is_finite())
            .collect();
        points.sort_by(|a, b| a.0.0.total_cmp(&b.0.0));
        let span = points.last()?.0.0 - points.first()?.0.0;
        (span > 0.0).then_some(Self { points })
    }
}

impl Gradients for ElevationProfile {
    fn length(&self) -> Meters {
        self.points.last().map_or(Meters(0.0), |p| p.0)
    }

    fn grade_at(&self, distance: Meters) -> GradePercent {
        let index = self
            .points
            .partition_point(|p| p.0.0 <= distance.0)
            .clamp(1, self.points.len() - 1);
        let ((d0, e0), (d1, e1)) = (self.points[index - 1], self.points[index]);
        let span = d1.0 - d0.0;
        GradePercent(if span > 0.0 {
            (e1.0 - e0.0) / span * 100.0
        } else {
            0.0
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(points: &[(f64, f64)]) -> Option<ElevationProfile> {
        ElevationProfile::new(points.iter().map(|&(d, e)| (Meters(d), Meters(e))))
    }

    #[test]
    fn slopes_run_straight_between_the_points() {
        let profile = profile(&[(0.0, 500.0), (1_000.0, 550.0), (3_000.0, 530.0)]).unwrap();

        assert_eq!(profile.length(), Meters(3_000.0));
        assert_eq!(profile.grade_at(Meters(500.0)), GradePercent(5.0));
        assert_eq!(profile.grade_at(Meters(2_000.0)), GradePercent(-1.0));
        // Clamped to the course at either end.
        assert_eq!(profile.grade_at(Meters(-10.0)), GradePercent(5.0));
        assert_eq!(profile.grade_at(Meters(9_000.0)), GradePercent(-1.0));
    }

    #[test]
    fn a_profile_needs_some_length() {
        assert!(profile(&[]).is_none());
        assert!(profile(&[(0.0, 500.0)]).is_none());
        assert!(profile(&[(100.0, 500.0), (100.0, 510.0)]).is_none());
    }
}
