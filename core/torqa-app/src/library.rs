//! The course library as the Courses tab shows it (R67): searched by name, filtered by type,
//! terrain and length, sorted and grouped.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};

use torqa_domain::profile::UnitSystem;
use torqa_domain::units::Meters;

use crate::CourseEntry;

/// Metres in a mile.
const MILE: f64 = 1_609.344;

/// Gives an enum `ALL`, `name` and `from_name`: the names the front end and the settings file
/// know its values by.
macro_rules! named {
    ($type:ident { $($variant:ident => $name:literal),+ $(,)? }) => {
        impl $type {
            /// Every value, in the order they are listed.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The name the front end and the settings use.
            #[must_use]
            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }

            /// The value called `name`, if any.
            #[must_use]
            pub fn from_name(name: &str) -> Option<Self> {
                Self::ALL.iter().copied().find(|value| value.name() == name)
            }
        }
    };
}

/// Whether a course is ridden in the 3D world or along a video (R17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CourseKind {
    /// Ridden in the 3D world.
    ThreeD,
    /// Ridden along a video, with or without a place (Tacx RLV).
    Video,
}

named!(CourseKind {
    ThreeD => "3d",
    Video => "video",
});

/// How hilly a course is, by how much it climbs per kilometre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Terrain {
    /// Under 5 m per km.
    Flat,
    /// 5 to 10 m per km.
    Rolling,
    /// 10 to 20 m per km.
    Hilly,
    /// 20 m per km and more.
    Mountainous,
}

named!(Terrain {
    Flat => "flat",
    Rolling => "rolling",
    Hilly => "hilly",
    Mountainous => "mountainous",
});

impl Terrain {
    /// Climbing per kilometre, in metres, from which each terrain after flat begins.
    const FROM: [f64; 3] = [5.0, 10.0, 20.0];

    /// The terrain of a course `length` long that climbs `gain` in all.
    #[must_use]
    pub fn of(length: Meters, gain: Meters) -> Self {
        let per_km = if length.0 > 0.0 {
            gain.0 / (length.0 / 1000.0)
        } else {
            0.0
        };
        match Self::FROM.iter().filter(|&&from| per_km >= from).count() {
            0 => Self::Flat,
            1 => Self::Rolling,
            2 => Self::Hilly,
            _ => Self::Mountainous,
        }
    }
}

/// How long a course is, in four bands with round edges in the rider's units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LengthBand {
    /// Up to the first edge.
    Short,
    /// Between the first and second edge.
    Medium,
    /// Between the second and third edge.
    Long,
    /// From the third edge.
    VeryLong,
}

named!(LengthBand {
    Short => "short",
    Medium => "medium",
    Long => "long",
    VeryLong => "very_long",
});

impl LengthBand {
    /// Where the bands meet: 25, 50 and 100 km, or 15, 30 and 60 miles, rather than kilometres
    /// converted to odd miles.
    #[must_use]
    pub fn edges(units: UnitSystem) -> [Meters; 3] {
        match units {
            UnitSystem::Metric => [25_000.0, 50_000.0, 100_000.0].map(Meters),
            UnitSystem::Imperial => [15.0, 30.0, 60.0].map(|miles| Meters(miles * MILE)),
        }
    }

    /// The band of a course `length` long; an edge belongs to the band above it.
    #[must_use]
    pub fn of(length: Meters, units: UnitSystem) -> Self {
        let passed = Self::edges(units)
            .iter()
            .filter(|edge| length.0 >= edge.0)
            .count();
        Self::ALL[passed]
    }
}

/// What the gallery is sorted by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    /// The course's name.
    #[default]
    Name,
    /// Its length.
    Length,
    /// Its total climbing.
    Climbing,
    /// The time the rider is estimated to take (R66).
    Time,
    /// When it was prepared or imported.
    Added,
    /// When the rider last rode it.
    Ridden,
}

named!(SortKey {
    Name => "name",
    Length => "length",
    Climbing => "climbing",
    Time => "time",
    Added => "added",
    Ridden => "ridden",
});

impl SortKey {
    /// Whether this order is wanted from the top down when first chosen: the latest courses
    /// and rides are the ones looked for, the rest from A, short, flat and quick.
    #[must_use]
    pub fn natural_descending(self) -> bool {
        matches!(self, Self::Added | Self::Ridden)
    }
}

/// What the gallery is grouped by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GroupKey {
    /// One gallery, no groups.
    #[default]
    None,
    /// 3D and video courses.
    Kind,
    /// Flat to mountainous.
    Terrain,
    /// Length bands.
    Length,
}

named!(GroupKey {
    None => "none",
    Kind => "type",
    Terrain => "terrain",
    Length => "length",
});

/// How the gallery is ordered and grouped, remembered between starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct View {
    /// What it is sorted by.
    pub sort: SortKey,
    /// Whether from the largest, latest or last letter down.
    pub descending: bool,
    /// What it is grouped by.
    pub group: GroupKey,
}

/// What the rider asks the gallery for: the view, a search and the chips chosen. An empty
/// list of chips allows every value; chips of one kind add to each other, kinds narrow down.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CourseQuery {
    /// Order and groups.
    pub view: View,
    /// Words the course's name contains, of any case; empty for all.
    pub search: String,
    /// The kinds of course shown.
    pub kinds: Vec<CourseKind>,
    /// The terrains shown.
    pub terrains: Vec<Terrain>,
    /// The length bands shown.
    pub lengths: Vec<LengthBand>,
}

/// A course with what the gallery sorts and filters it by beyond its manifest.
#[derive(Debug, Clone, PartialEq)]
pub struct LibraryCourse {
    /// The course file and its manifest.
    pub entry: CourseEntry,
    /// How long the rider would take at the planned share of their FTP (R66), estimated from
    /// the profile the course file keeps; `None` if that power would not get them there.
    pub estimated: Option<Duration>,
    /// When the rider last started a ride on it; `None` if never.
    pub last_ridden: Option<SystemTime>,
}

impl LibraryCourse {
    /// Whether it is ridden in the 3D world or along a video.
    #[must_use]
    pub fn kind(&self) -> CourseKind {
        if self.entry.manifest.video.is_some() {
            CourseKind::Video
        } else {
            CourseKind::ThreeD
        }
    }

    /// How hilly it is.
    #[must_use]
    pub fn terrain(&self) -> Terrain {
        Terrain::of(
            Meters(self.entry.manifest.length_m),
            Meters(self.entry.manifest.elevation_gain_m),
        )
    }

    /// Its length band in `units`.
    #[must_use]
    pub fn length_band(&self, units: UnitSystem) -> LengthBand {
        LengthBand::of(Meters(self.entry.manifest.length_m), units)
    }
}

/// One group of the gallery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Group {
    /// Every course shown, when not grouped.
    All,
    /// The courses of one kind.
    Kind(CourseKind),
    /// The courses of one terrain.
    Terrain(Terrain),
    /// The courses of one length band.
    Length(LengthBand),
}

impl Group {
    /// The name of the kind, terrain or band it holds; empty for [`Group::All`].
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::All => "",
            Self::Kind(kind) => kind.name(),
            Self::Terrain(terrain) => terrain.name(),
            Self::Length(band) => band.name(),
        }
    }
}

/// A group of the gallery with its courses in order.
#[derive(Debug, Clone, PartialEq)]
pub struct CourseGroup {
    /// Which group.
    pub group: Group,
    /// Its courses, sorted as asked.
    pub courses: Vec<LibraryCourse>,
}

/// The gallery as asked for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Gallery {
    /// How many courses the library holds, shown or not.
    pub total: usize,
    /// The groups with courses shown, in their own order (3D before video, flat to
    /// mountainous, short to long).
    pub groups: Vec<CourseGroup>,
}

/// The gallery of `courses` that `query` asks for, length bands in `units`.
#[must_use]
pub fn gallery(courses: Vec<LibraryCourse>, query: &CourseQuery, units: UnitSystem) -> Gallery {
    let total = courses.len();
    let search = query.search.trim().to_lowercase();
    let mut groups: BTreeMap<Group, Vec<LibraryCourse>> = BTreeMap::new();
    for course in courses {
        let shown = course.entry.manifest.name.to_lowercase().contains(&search)
            && allows(&query.kinds, &course.kind())
            && allows(&query.terrains, &course.terrain())
            && allows(&query.lengths, &course.length_band(units));
        if !shown {
            continue;
        }
        let group = match query.view.group {
            GroupKey::None => Group::All,
            GroupKey::Kind => Group::Kind(course.kind()),
            GroupKey::Terrain => Group::Terrain(course.terrain()),
            GroupKey::Length => Group::Length(course.length_band(units)),
        };
        groups.entry(group).or_default().push(course);
    }
    let groups = groups
        .into_iter()
        .map(|(group, mut courses)| {
            courses.sort_by(|a, b| order(a, b, query.view));
            CourseGroup { group, courses }
        })
        .collect();
    Gallery { total, groups }
}

/// Whether chips `chosen` let `value` through: all of them do when none is chosen.
fn allows<T: PartialEq>(chosen: &[T], value: &T) -> bool {
    chosen.is_empty() || chosen.contains(value)
}

/// The order of two courses in `view`, by name where they tie.
fn order(a: &LibraryCourse, b: &LibraryCourse, view: View) -> Ordering {
    let (left, right) = (&a.entry.manifest, &b.entry.manifest);
    let directed = |ordering: Ordering| {
        if view.descending {
            ordering.reverse()
        } else {
            ordering
        }
    };
    let first = match view.sort {
        SortKey::Name => directed(by_name(a, b)),
        SortKey::Length => directed(left.length_m.total_cmp(&right.length_m)),
        SortKey::Climbing => directed(left.elevation_gain_m.total_cmp(&right.elevation_gain_m)),
        SortKey::Added => directed(left.created_unix_s.cmp(&right.created_unix_s)),
        SortKey::Time => known_first(a.estimated, b.estimated, directed),
        SortKey::Ridden => known_first(a.last_ridden, b.last_ridden, directed),
    };
    first.then_with(|| by_name(a, b))
}

/// Known values in `directed` order, then the courses without one, whichever the direction:
/// a course never ridden is not the one ridden longest ago.
fn known_first<T: Ord>(
    a: Option<T>,
    b: Option<T>,
    directed: impl Fn(Ordering) -> Ordering,
) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => directed(a.cmp(&b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Alphabetical of any case, then by file so that the order is always the same.
fn by_name(a: &LibraryCourse, b: &LibraryCourse) -> Ordering {
    a.entry
        .manifest
        .name
        .to_lowercase()
        .cmp(&b.entry.manifest.name.to_lowercase())
        .then_with(|| a.entry.path.cmp(&b.entry.path))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use torqa_storage::course::{Manifest, VideoReference};

    use super::*;

    const KM: f64 = 1000.0;

    fn course(name: &str, km: f64, gain_m: f64) -> LibraryCourse {
        LibraryCourse {
            entry: CourseEntry {
                path: PathBuf::from(format!("/courses/{name}.tqc")),
                manifest: Manifest {
                    format: 1,
                    generator: "Torqa test".to_owned(),
                    name: name.to_owned(),
                    length_m: km * KM,
                    elevation_gain_m: gain_m,
                    max_grade_percent: 8.0,
                    created_unix_s: 0,
                    attribution: Vec::new(),
                    route_key: None,
                    track: Vec::new(),
                    profile: Vec::new(),
                    video: None,
                },
            },
            estimated: None,
            last_ridden: None,
        }
    }

    fn video(mut course: LibraryCourse) -> LibraryCourse {
        course.entry.manifest.video = Some(VideoReference {
            path: "/videos/ride.mp4".to_owned(),
            file_name: "ride.mp4".to_owned(),
            size: 1,
            offset_s: 0.0,
            end_s: None,
            marks: Vec::new(),
            located: true,
        });
        course
    }

    fn added(mut course: LibraryCourse, unix_s: u64) -> LibraryCourse {
        course.entry.manifest.created_unix_s = unix_s;
        course
    }

    fn timed(mut course: LibraryCourse, minutes: Option<u64>) -> LibraryCourse {
        course.estimated = minutes.map(Duration::from_mins);
        course
    }

    fn ridden(mut course: LibraryCourse, days_ago: Option<u64>) -> LibraryCourse {
        let now = SystemTime::UNIX_EPOCH + Duration::from_hours(24 * 365);
        course.last_ridden = days_ago.map(|days| now - Duration::from_hours(24 * days));
        course
    }

    fn library() -> Vec<LibraryCourse> {
        vec![
            course("Gurten loop", 31.0, 380.0),
            course("aare flats", 42.0, 60.0),
            video(course("Jura ridge", 48.0, 720.0)),
            course("Grimsel", 120.0, 2_600.0),
        ]
    }

    fn names(gallery: &Gallery) -> Vec<Vec<&str>> {
        gallery
            .groups
            .iter()
            .map(|group| {
                group
                    .courses
                    .iter()
                    .map(|c| c.entry.manifest.name.as_str())
                    .collect()
            })
            .collect()
    }

    fn sorted(courses: Vec<LibraryCourse>, sort: SortKey, descending: bool) -> Vec<String> {
        let query = CourseQuery {
            view: View {
                sort,
                descending,
                group: GroupKey::None,
            },
            ..CourseQuery::default()
        };
        gallery(courses, &query, UnitSystem::Metric).groups[0]
            .courses
            .iter()
            .map(|c| c.entry.manifest.name.clone())
            .collect()
    }

    #[test]
    fn terrain_follows_the_climbing_per_kilometre() {
        let terrain = |gain| Terrain::of(Meters(10.0 * KM), Meters(gain));

        assert_eq!(terrain(49.0), Terrain::Flat);
        assert_eq!(terrain(50.0), Terrain::Rolling);
        assert_eq!(terrain(99.0), Terrain::Rolling);
        assert_eq!(terrain(100.0), Terrain::Hilly);
        assert_eq!(terrain(200.0), Terrain::Mountainous);
        assert_eq!(Terrain::of(Meters(0.0), Meters(10.0)), Terrain::Flat);
    }

    #[test]
    fn length_bands_have_round_edges_in_the_riders_units() {
        let metric = |km: f64| LengthBand::of(Meters(km * KM), UnitSystem::Metric);
        let imperial = |miles: f64| LengthBand::of(Meters(miles * MILE), UnitSystem::Imperial);

        assert_eq!(metric(24.9), LengthBand::Short);
        assert_eq!(metric(25.0), LengthBand::Medium);
        assert_eq!(metric(50.0), LengthBand::Long);
        assert_eq!(metric(100.0), LengthBand::VeryLong);
        assert_eq!(imperial(14.9), LengthBand::Short);
        assert_eq!(imperial(15.0), LengthBand::Medium);
        assert_eq!(imperial(59.9), LengthBand::Long);
        assert_eq!(imperial(60.0), LengthBand::VeryLong);
    }

    #[test]
    fn the_search_finds_names_of_any_case() {
        let query = CourseQuery {
            search: "  GURTEN ".to_owned(),
            ..CourseQuery::default()
        };

        let found = gallery(library(), &query, UnitSystem::Metric);

        assert_eq!(names(&found), [["Gurten loop"]]);
        assert_eq!(found.total, 4, "the library still holds all four");
    }

    #[test]
    fn chips_of_one_kind_add_up_and_kinds_narrow_down() {
        let both_kinds = CourseQuery {
            kinds: vec![CourseKind::ThreeD, CourseKind::Video],
            ..CourseQuery::default()
        };
        assert_eq!(
            gallery(library(), &both_kinds, UnitSystem::Metric).groups[0]
                .courses
                .len(),
            4
        );

        let hilly_and_medium = CourseQuery {
            terrains: vec![Terrain::Hilly, Terrain::Mountainous],
            lengths: vec![LengthBand::Medium],
            ..CourseQuery::default()
        };
        assert_eq!(
            names(&gallery(library(), &hilly_and_medium, UnitSystem::Metric)),
            [["Gurten loop", "Jura ridge"]]
        );

        let nothing = CourseQuery {
            kinds: vec![CourseKind::Video],
            terrains: vec![Terrain::Flat],
            ..CourseQuery::default()
        };
        let empty = gallery(library(), &nothing, UnitSystem::Metric);
        assert_eq!(empty.groups, []);
        assert_eq!(empty.total, 4);
    }

    #[test]
    fn names_sort_alphabetically_of_any_case() {
        assert_eq!(
            sorted(library(), SortKey::Name, false),
            ["aare flats", "Grimsel", "Gurten loop", "Jura ridge"]
        );
        assert_eq!(
            sorted(library(), SortKey::Name, true),
            ["Jura ridge", "Gurten loop", "Grimsel", "aare flats"]
        );
    }

    #[test]
    fn length_and_climbing_sort_by_their_figures() {
        assert_eq!(
            sorted(library(), SortKey::Length, false),
            ["Gurten loop", "aare flats", "Jura ridge", "Grimsel"]
        );
        assert_eq!(
            sorted(library(), SortKey::Climbing, true),
            ["Grimsel", "Jura ridge", "Gurten loop", "aare flats"]
        );
    }

    #[test]
    fn the_latest_courses_come_first() {
        assert!(SortKey::Added.natural_descending());
        let courses = vec![
            added(course("Old", 30.0, 0.0), 100),
            added(course("New", 30.0, 0.0), 300),
            added(course("Middle", 30.0, 0.0), 200),
        ];

        assert_eq!(
            sorted(courses, SortKey::Added, true),
            ["New", "Middle", "Old"]
        );
    }

    #[test]
    fn courses_without_an_estimate_come_last_either_way() {
        let courses = || {
            vec![
                timed(course("Wall", 5.0, 900.0), None),
                timed(course("Long", 90.0, 900.0), Some(200)),
                timed(course("Quick", 20.0, 100.0), Some(40)),
            ]
        };

        assert_eq!(
            sorted(courses(), SortKey::Time, false),
            ["Quick", "Long", "Wall"]
        );
        assert_eq!(
            sorted(courses(), SortKey::Time, true),
            ["Long", "Quick", "Wall"]
        );
    }

    #[test]
    fn the_courses_ridden_last_come_first_and_those_never_ridden_last() {
        assert!(SortKey::Ridden.natural_descending());
        assert!(!SortKey::Time.natural_descending());
        let courses = || {
            vec![
                ridden(course("Never", 30.0, 0.0), None),
                ridden(course("Last week", 30.0, 0.0), Some(7)),
                ridden(course("Yesterday", 30.0, 0.0), Some(1)),
            ]
        };

        assert_eq!(
            sorted(courses(), SortKey::Ridden, true),
            ["Yesterday", "Last week", "Never"]
        );
        assert_eq!(
            sorted(courses(), SortKey::Ridden, false),
            ["Last week", "Yesterday", "Never"]
        );
    }

    #[test]
    fn groups_come_in_their_own_order_and_only_with_courses() {
        let by = |group| CourseQuery {
            view: View {
                group,
                ..View::default()
            },
            ..CourseQuery::default()
        };

        let terrains = gallery(library(), &by(GroupKey::Terrain), UnitSystem::Metric);
        assert_eq!(
            terrains.groups.iter().map(|g| g.group).collect::<Vec<_>>(),
            [
                Group::Terrain(Terrain::Flat),
                Group::Terrain(Terrain::Hilly),
                Group::Terrain(Terrain::Mountainous),
            ]
        );
        assert_eq!(
            names(&terrains),
            [
                vec!["aare flats"],
                vec!["Gurten loop", "Jura ridge"],
                vec!["Grimsel"]
            ]
        );

        let kinds = gallery(library(), &by(GroupKey::Kind), UnitSystem::Metric);
        assert_eq!(
            names(&kinds),
            [
                vec!["aare flats", "Grimsel", "Gurten loop"],
                vec!["Jura ridge"]
            ]
        );

        let lengths = gallery(library(), &by(GroupKey::Length), UnitSystem::Metric);
        assert_eq!(
            lengths
                .groups
                .iter()
                .map(|g| g.group.name())
                .collect::<Vec<_>>(),
            ["medium", "very_long"]
        );
    }

    #[test]
    fn every_choice_is_known_by_its_name() {
        for &key in SortKey::ALL {
            assert_eq!(SortKey::from_name(key.name()), Some(key));
        }
        for &key in GroupKey::ALL {
            assert_eq!(GroupKey::from_name(key.name()), Some(key));
        }
        for &terrain in Terrain::ALL {
            assert_eq!(Terrain::from_name(terrain.name()), Some(terrain));
        }
        assert_eq!(CourseKind::from_name("3d"), Some(CourseKind::ThreeD));
        assert_eq!(
            LengthBand::from_name("very_long"),
            Some(LengthBand::VeryLong)
        );
        assert_eq!(SortKey::from_name("colour"), None);
    }
}
