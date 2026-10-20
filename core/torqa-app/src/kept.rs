//! The courses' worlds this computer keeps (R101, ADR 0025): once a library course's world is
//! built it is kept in the cache, and **Ride** reads it instead of building it again. Off until
//! the rider turns it on, as a world takes about 10 MB per kilometre of route.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use torqa_storage::computer;
use torqa_storage::course;
use torqa_storage::worlds::KeptWorlds;
use torqa_world::World;
use tracing::{info, warn};

use crate::failure::{Failure, Kind};
use crate::{App, AppError};

/// What the cache holds of a course's world.
pub(crate) enum Found {
    /// Its world, kept.
    World(Box<World>),
    /// None yet: one built is kept under this fingerprint.
    Missing(String),
    /// Nothing that can be told: the course file cannot be read.
    Unknown,
}

/// Where kept worlds are: the cache, the setting to check once one is written (it may have
/// been turned off meanwhile) and the library whose courses' worlds stay.
#[derive(Clone)]
pub(crate) struct Keeper {
    kept: KeptWorlds,
    computer: PathBuf,
    library: PathBuf,
}

impl Keeper {
    /// The kept world of the library course at `course`, if there is one. A damaged one is
    /// built again.
    pub(crate) fn find(&self, course: &Path) -> Found {
        let fingerprint = match course::fingerprint(course) {
            Ok(fingerprint) => fingerprint,
            Err(error) => {
                warn!(%error, course = %course.display(), "cannot tell the course's kept world");
                return Found::Unknown;
            }
        };
        match self.kept.read(&fingerprint) {
            Ok(Some(world)) => {
                info!(chunks = world.chunks.len(), "riding the kept world");
                Found::World(Box::new(world))
            }
            Ok(None) => Found::Missing(fingerprint),
            Err(error) => {
                warn!(%error, "the kept world is damaged; building it again");
                Found::Missing(fingerprint)
            }
        }
    }

    /// Keeps `world` as that of the course at `course`, found by `fingerprint` (that of a
    /// world just built, not kept yet) or else by the file, and drops the worlds no course of
    /// the library has any more. Blocks for seconds: the world is deflated.
    pub(crate) fn keep(&self, course: &Path, world: &World, fingerprint: Option<String>) {
        let fingerprint = match fingerprint {
            Some(fingerprint) => fingerprint,
            None => match course::fingerprint(course) {
                // Turned on again while it is still kept: writing it again only costs time.
                Ok(fingerprint) if self.kept.has(&fingerprint) => return self.tidy(),
                Ok(fingerprint) => fingerprint,
                Err(error) => {
                    warn!(%error, course = %course.display(), "cannot keep the course's world");
                    return;
                }
            },
        };
        if let Err(error) = self.kept.write(&fingerprint, world) {
            warn!(%error, "cannot keep the course's world");
        }
        self.tidy();
    }

    /// Drops the worlds of other builds and of courses gone from the library, or all of them
    /// once keeping them is turned off.
    pub(crate) fn tidy(&self) {
        let tidied = if computer::keep_worlds(&self.computer) {
            self.kept.keep_only(&library_fingerprints(&self.library))
        } else {
            self.kept.clear()
        };
        if let Err(error) = tidied {
            warn!(%error, "cannot tidy the kept worlds");
        }
    }
}

/// The fingerprints of the courses in `library`.
fn library_fingerprints(library: &Path) -> HashSet<String> {
    std::fs::read_dir(library)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == course::EXTENSION))
        .filter_map(|path| course::fingerprint(&path).ok())
        .collect()
}

impl App {
    /// Whether this computer keeps the courses' worlds once built (R101); no until turned on.
    #[must_use]
    pub fn keeps_worlds(&self) -> bool {
        computer::keep_worlds(&self.computer)
    }

    /// Turns keeping the courses' worlds on or off for this computer. On, the world of the
    /// course loaded is kept at once; off, every kept world is deleted.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot be saved.
    pub fn set_keeps_worlds(&mut self, on: bool) -> Result<(), AppError> {
        computer::set_keep_worlds(&self.computer, on)
            .map_err(|e| AppError::Reported(Failure::new(Kind::SaveSetting, e)))?;
        match (on, &self.world, &self.course) {
            (true, Some(world), Some(course)) => {
                self.keep_world(course.clone(), Arc::clone(world), None);
            }
            _ => self.tidy_kept_worlds(),
        }
        Ok(())
    }

    /// How many bytes the kept worlds take on this computer.
    #[must_use]
    pub fn kept_worlds_size(&self) -> u64 {
        self.keeper().kept.size()
    }

    /// Names the commit this build is made from (R83): a world is kept by the build that made
    /// it, as another may build it differently.
    pub fn set_commit(&mut self, commit: &str) {
        self.build = if commit.is_empty() {
            torqa_domain::version().to_owned()
        } else {
            format!("{}-{commit}", torqa_domain::version())
        };
    }

    /// Drops, in the background, the kept worlds of other builds and of courses gone from the
    /// library (also on another computer sharing it), or all once keeping them is off.
    pub fn tidy_kept_worlds(&self) {
        let keeper = self.keeper();
        self.runtime.spawn_blocking(move || keeper.tidy());
    }

    pub(crate) fn keeper(&self) -> Keeper {
        Keeper {
            kept: KeptWorlds::new(&self.cache_dir, &self.build),
            computer: self.computer.clone(),
            library: self.courses_dir(),
        }
    }

    /// Keeps `world` as that of the library course at `course`, in the background.
    pub(crate) fn keep_world(
        &self,
        course: PathBuf,
        world: Arc<World>,
        fingerprint: Option<String>,
    ) {
        let keeper = self.keeper();
        self.runtime
            .spawn_blocking(move || keeper.keep(&course, &world, fingerprint));
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::AppEvent;
    use crate::tests::{run_until, temp_dir, write_route};

    /// Updates `app` until `done` holds, as the kept worlds are written in the background.
    fn wait_for(app: &mut App, mut done: impl FnMut(&App) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(60);
        while !done(app) {
            assert!(Instant::now() < deadline, "waited in vain");
            std::thread::sleep(Duration::from_millis(16));
            app.update(Duration::from_millis(16));
        }
    }

    /// Prepares the test route as a course of `app`'s library.
    fn prepare(app: &mut App, dir: &Path) -> PathBuf {
        app.load_route(write_route(dir), true);
        let events = run_until(app, |e| matches!(e, AppEvent::CourseAdded(_)));
        let Some(AppEvent::CourseAdded(course)) = events.last() else {
            unreachable!()
        };
        course.clone()
    }

    /// Opens `course` and builds its world for a ride; how many fallback samples it has, which
    /// tells a world marked in its kept file apart from one built again.
    fn ride(app: &mut App, course: &Path) -> usize {
        app.open_course(course.to_owned());
        app.build_world();
        let events = run_until(app, |e| {
            matches!(e, AppEvent::WorldReady { .. } | AppEvent::Error(_))
        });
        match events.last() {
            Some(AppEvent::WorldReady {
                fallback_samples, ..
            }) => *fallback_samples,
            other => panic!("no world: {other:?}"),
        }
    }

    const MARK: usize = 424_242;

    /// Marks the kept world of the course with `fingerprint`.
    fn mark(keeper: &Keeper, fingerprint: &str) {
        let mut world = keeper.kept.read(fingerprint).unwrap().expect("kept");
        world.fallback_samples = MARK;
        keeper.kept.write(fingerprint, &world).unwrap();
    }

    #[test]
    fn a_course_ridden_again_rides_its_kept_world() {
        let dir = temp_dir("kept-ride");
        let mut app = App::new(dir.join("data"), dir.join("cache")).unwrap();
        assert!(!app.keeps_worlds(), "off until turned on (R101)");
        app.set_keeps_worlds(true).unwrap();
        // Prepared from its GPX: the world built for it is kept for the course.
        let course = prepare(&mut app, &dir);
        let keeper = app.keeper();
        let fingerprint = course::fingerprint(&course).unwrap();
        wait_for(&mut app, |_| keeper.kept.has(&fingerprint));
        mark(&keeper, &fingerprint);

        assert_eq!(ride(&mut app, &course), MARK, "ridden from the kept world");
        // Renamed, it is still the same world.
        app.rename_course(&course, "Evening loop").unwrap();
        assert_eq!(ride(&mut app, &course), MARK);

        // Turned off, the kept worlds go and the course is built again.
        app.set_keeps_worlds(false).unwrap();
        wait_for(&mut app, |app| app.kept_worlds_size() == 0);
        assert_ne!(ride(&mut app, &course), MARK);
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(16));
            app.update(Duration::from_millis(16));
        }
        assert_eq!(app.kept_worlds_size(), 0, "and none is kept");
        app.shutdown();
    }

    #[test]
    fn turned_on_the_world_loaded_is_kept_until_its_course_is_deleted() {
        let dir = temp_dir("kept-delete");
        let mut app = App::new(dir.join("data"), dir.join("cache")).unwrap();
        let course = prepare(&mut app, &dir);
        let keeper = app.keeper();
        let fingerprint = course::fingerprint(&course).unwrap();
        for _ in 0..30 {
            std::thread::sleep(Duration::from_millis(16));
            app.update(Duration::from_millis(16));
        }
        assert_eq!(app.kept_worlds_size(), 0, "nothing kept while off");

        app.set_keeps_worlds(true).unwrap();
        wait_for(&mut app, |_| keeper.kept.has(&fingerprint));
        assert!(app.kept_worlds_size() > 0);

        app.delete_course(&course).unwrap();
        wait_for(&mut app, |_| !keeper.kept.has(&fingerprint));
        app.shutdown();
    }

    #[test]
    fn a_damaged_world_or_one_of_another_build_is_built_again() {
        let dir = temp_dir("kept-again");
        let mut app = App::new(dir.join("data"), dir.join("cache")).unwrap();
        app.set_commit("1111111");
        app.set_keeps_worlds(true).unwrap();
        let course = prepare(&mut app, &dir);
        let older = app.keeper();
        let fingerprint = course::fingerprint(&course).unwrap();
        wait_for(&mut app, |_| older.kept.has(&fingerprint));
        mark(&older, &fingerprint);

        // Another build may build it differently: it builds its own, and the older goes.
        app.set_commit("2222222");
        let keeper = app.keeper();
        assert_ne!(ride(&mut app, &course), MARK);
        wait_for(&mut app, |_| {
            keeper.kept.has(&fingerprint) && !older.kept.has(&fingerprint)
        });

        // A damaged file is no error to the rider: the world is built and kept again.
        let file = std::fs::read_dir(dir.join("cache/worlds"))
            .unwrap()
            .flat_map(|shelf| std::fs::read_dir(shelf.unwrap().path()).unwrap())
            .map(|entry| entry.unwrap().path())
            .find(|path| path.extension().is_some_and(|e| e == "world"))
            .expect("the kept world");
        std::fs::write(&file, b"damaged").unwrap();
        assert_ne!(ride(&mut app, &course), MARK);
        wait_for(&mut app, |_| {
            keeper.kept.read(&fingerprint).is_ok_and(|w| w.is_some())
        });
        app.shutdown();
    }
}
