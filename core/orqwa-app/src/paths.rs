//! Platform file locations and file names.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The platform's per-user data directory for ORQWA (rides, routes, settings), with Torqa's
/// moved there on the first start after the rename (R102).
#[must_use]
pub fn data_dir() -> PathBuf {
    if cfg!(target_os = "android") {
        return android_dirs(&std::env::temp_dir()).0;
    }
    let env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    let home = env("HOME").or_else(|| env("USERPROFILE"));
    let base = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support"))
    } else if cfg!(windows) {
        env("APPDATA")
    } else {
        env("XDG_DATA_HOME").or_else(|| home.map(|h| h.join(".local/share")))
    };
    let (name, before) = if cfg!(target_os = "macos") || cfg!(windows) {
        ("ORQWA", "Torqa")
    } else {
        ("orqwa", "torqa")
    };
    let base = base.unwrap_or_else(std::env::temp_dir);
    carry_over(&base.join(before), base.join(name))
}

/// The platform's per-user cache directory for ORQWA, with Torqa's moved there on the first start
/// after the rename (R102): it holds this computer's own settings and the kept worlds.
#[must_use]
pub fn cache_dir() -> PathBuf {
    if cfg!(target_os = "android") {
        return android_dirs(&std::env::temp_dir()).1;
    }
    let env = |key: &str| std::env::var_os(key).map(PathBuf::from);
    let home = env("HOME").or_else(|| env("USERPROFILE"));
    let base = if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Caches"))
    } else if cfg!(windows) {
        env("LOCALAPPDATA")
    } else {
        env("XDG_CACHE_HOME").or_else(|| home.map(|h| h.join(".cache")))
    };
    let base = base.unwrap_or_else(std::env::temp_dir);
    carry_over(&base.join("torqa"), base.join("orqwa"))
}

/// The folder to use for what Torqa kept in `before` (R102, ADR 0026): `before` is moved to `new`
/// when only it exists. A move that fails leaves it in use where it is, so a rider loses nothing.
fn carry_over(before: &Path, new: PathBuf) -> PathBuf {
    if new.exists() || !before.exists() {
        return new;
    }
    match std::fs::rename(before, &new) {
        Ok(()) => {
            tracing::info!(from = %before.display(), to = %new.display(), "moved Torqa's folder");
            new
        }
        // Another ORQWA started at the same time may have moved it first.
        Err(_) if new.exists() => new,
        Err(error) => {
            tracing::warn!(%error, folder = %before.display(), "cannot move Torqa's folder; using it");
            before.to_path_buf()
        }
    }
}

/// ORQWA's data and cache directories on Android (ADR 0024), from the app's cache directory that
/// Android gives as the temporary one (`/data/user/0/<app>/cache`): data goes beside it into the
/// app's own `files`, which Android never clears, unlike the cache when space runs short.
fn android_dirs(app_cache: &std::path::Path) -> (PathBuf, PathBuf) {
    let app = app_cache.parent().unwrap_or(app_cache);
    (app.join("files").join("orqwa"), app_cache.join("orqwa"))
}

/// `orqwa-YYYYMMDD-HHMMSS.fit` in UTC.
#[must_use]
pub fn activity_file_name(start: SystemTime) -> PathBuf {
    let secs = start
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    let (year, month, day) = civil_from_days(secs.div_euclid(86_400));
    let time = secs.rem_euclid(86_400);
    PathBuf::from(format!(
        "orqwa-{year:04}{month:02}{day:02}-{:02}{:02}{:02}.fit",
        time / 3600,
        time % 3600 / 60,
        time % 60
    ))
}

/// Gregorian date from days since 1970-01-01 (Howard Hinnant's `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn on_android_data_stays_out_of_the_cache() {
        let (data, cache) = android_dirs(Path::new("/data/user/0/org.orqwa.bike/cache"));

        assert_eq!(
            data,
            PathBuf::from("/data/user/0/org.orqwa.bike/files/orqwa")
        );
        assert_eq!(
            cache,
            PathBuf::from("/data/user/0/org.orqwa.bike/cache/orqwa")
        );
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("orqwa-paths-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn torqas_folder_moves_to_orqwas_with_everything_in_it() {
        let base = scratch("move");
        std::fs::create_dir_all(base.join("Torqa/profiles/rider/rides")).unwrap();
        std::fs::write(base.join("Torqa/profiles/rider/rides/a.fit"), b"ride").unwrap();

        let folder = carry_over(&base.join("Torqa"), base.join("ORQWA"));

        assert_eq!(folder, base.join("ORQWA"));
        assert_eq!(
            std::fs::read(folder.join("profiles/rider/rides/a.fit")).unwrap(),
            b"ride"
        );
        assert!(!base.join("Torqa").exists());
    }

    #[test]
    fn an_existing_orqwa_folder_wins_and_torqas_stays_untouched() {
        let base = scratch("both");
        std::fs::create_dir_all(base.join("Torqa")).unwrap();
        std::fs::create_dir_all(base.join("ORQWA")).unwrap();

        assert_eq!(
            carry_over(&base.join("Torqa"), base.join("ORQWA")),
            base.join("ORQWA")
        );
        assert!(base.join("Torqa").exists());
    }

    #[test]
    fn a_new_rider_gets_orqwas_folder() {
        let base = scratch("new");

        assert_eq!(
            carry_over(&base.join("Torqa"), base.join("ORQWA")),
            base.join("ORQWA")
        );
        assert!(!base.join("ORQWA").exists());
    }

    #[test]
    fn a_folder_that_cannot_move_stays_in_use() {
        let base = scratch("stuck");
        std::fs::create_dir_all(base.join("Torqa")).unwrap();

        let new = base.join("missing-parent/ORQWA");
        assert_eq!(carry_over(&base.join("Torqa"), new), base.join("Torqa"));
        assert!(base.join("Torqa").exists());
    }

    #[test]
    fn names_activity_files_by_utc_start() {
        let leap_day = UNIX_EPOCH + Duration::from_secs(951_782_400 + 13 * 3600 + 5 * 60 + 9);

        assert_eq!(
            activity_file_name(leap_day),
            PathBuf::from("orqwa-20000229-130509.fit")
        );
        assert_eq!(
            activity_file_name(UNIX_EPOCH),
            PathBuf::from("orqwa-19700101-000000.fit")
        );
    }
}
