//! Newer versions of Torqa (R99, ADR 0023): looked for at start-up unless this computer turned
//! that off, or when the rider asks; never during a ride. On Windows the new installer is
//! downloaded and run over Torqa when the rider agrees; elsewhere the rider gets its page.

use std::path::PathBuf;

use torqa_storage::computer;
use torqa_update::{Release, UpdateError};
use tracing::{info, warn};

use crate::failure::{Failure, Kind};
use crate::{App, AppError, AppEvent, JobResult};

/// What the app knows about updates.
pub(crate) struct UpdateState {
    /// Where the latest release is asked for: GitHub, except in tests.
    pub(crate) source: String,
    /// The newer release found, if any.
    release: Option<Release>,
    /// A check or download is under way.
    busy: bool,
    /// The newer release's installer, downloaded.
    downloaded: Option<PathBuf>,
}

impl Default for UpdateState {
    fn default() -> Self {
        Self {
            source: torqa_update::LATEST_RELEASE.to_owned(),
            release: None,
            busy: false,
            downloaded: None,
        }
    }
}

/// Background work on updates, done.
pub(crate) enum Done {
    /// The latest release, looked for at start-up or because the rider `asked`.
    Checked {
        asked: bool,
        result: Result<Release, UpdateError>,
    },
    /// The installer, downloaded.
    Downloaded(Result<PathBuf, UpdateError>),
}

impl App {
    /// Whether this computer looks for a newer Torqa at start-up (R99); yes unless turned off.
    #[must_use]
    pub fn checks_for_updates(&self) -> bool {
        computer::check_updates(&self.computer)
    }

    /// Turns looking for a newer Torqa at start-up on or off for this computer.
    ///
    /// # Errors
    /// [`AppError::Reported`] if it cannot be saved.
    pub fn set_checks_for_updates(&mut self, on: bool) -> Result<(), AppError> {
        computer::set_check_updates(&self.computer, on)
            .map_err(|e| AppError::Reported(Failure::new(Kind::SaveSetting, e)))
    }

    /// Looks for a newer release in the background; whether it does. At start-up (`asked`
    /// false) only if this computer looks for updates, and then only a newer version is told:
    /// a failure goes to the log, as the rider asked for nothing. When the rider `asked`, being
    /// up to date and failures are told too. Never during a ride or while already looking.
    pub fn check_for_updates(&mut self, asked: bool) -> bool {
        if self.updates.busy || self.ride.is_some() || (!asked && !self.checks_for_updates()) {
            return false;
        }
        self.updates.busy = true;
        let source = self.updates.source.clone();
        let tx = self.jobs_tx.clone();
        self.runtime.spawn(async move {
            let result = torqa_update::latest_release(&source).await;
            let _ = tx.send(JobResult::Update(Done::Checked { asked, result }));
        });
        true
    }

    /// Downloads the newer release's installer in the background; whether it does. Only with
    /// one found that this system installs itself, and never during a ride.
    pub fn download_update(&mut self) -> bool {
        let Some(installer) = self
            .updates
            .release
            .as_ref()
            .and_then(|r| r.installer.clone())
        else {
            return false;
        };
        if self.updates.busy || self.ride.is_some() || !torqa_update::can_install() {
            return false;
        }
        let name = format!(
            "Torqa-{}.msi",
            self.updates
                .release
                .as_ref()
                .map_or_else(String::new, |release| release.version.to_string(),)
        );
        self.updates.busy = true;
        let dir = self.cache_dir.join("updates");
        let tx = self.jobs_tx.clone();
        self.runtime.spawn(async move {
            let result = torqa_update::download(&installer, &dir, &name).await;
            let _ = tx.send(JobResult::Update(Done::Downloaded(result)));
        });
        true
    }

    /// Starts installing the downloaded update over Torqa, which then starts again; the front
    /// end quits at once after this succeeds, so the installer can replace Torqa's files.
    ///
    /// # Errors
    /// [`AppError::Reported`] without a downloaded update, during a ride, or if the installer
    /// cannot be started.
    pub fn install_update(&mut self) -> Result<(), AppError> {
        let failed = |cause: &dyn std::fmt::Display| {
            AppError::Reported(Failure::new(Kind::InstallUpdate, cause))
        };
        if self.ride.is_some() {
            return Err(failed(&"not during a ride"));
        }
        let msi = self
            .updates
            .downloaded
            .clone()
            .ok_or_else(|| failed(&"nothing downloaded"))?;
        let app = std::env::current_exe().map_err(|e| failed(&e))?;
        torqa_update::install(&msi, &app).map_err(|e| failed(&e))?;
        info!(installer = %msi.display(), "installing the update");
        Ok(())
    }

    /// Tells what a background update job found.
    pub(crate) fn update_done(&mut self, done: Done) -> Option<AppEvent> {
        self.updates.busy = false;
        match done {
            Done::Checked {
                asked,
                result: Ok(release),
            } => {
                if !release.is_newer_than(torqa_domain::version()) {
                    return asked.then_some(AppEvent::UpToDate);
                }
                let event = AppEvent::UpdateAvailable {
                    version: release.version.to_string(),
                    page: release.page.clone(),
                    installable: torqa_update::can_install() && release.installer.is_some(),
                };
                self.updates.release = Some(release);
                Some(event)
            }
            Done::Checked {
                asked,
                result: Err(error),
            } => {
                warn!(%error, "cannot look for updates");
                asked.then(|| AppEvent::UpdateFailed(Failure::new(Kind::CheckUpdate, error)))
            }
            Done::Downloaded(Ok(path)) => {
                self.updates.downloaded = Some(path);
                Some(AppEvent::UpdateDownloaded)
            }
            Done::Downloaded(Err(error)) => {
                warn!(%error, "cannot download the update");
                Some(AppEvent::UpdateFailed(Failure::new(
                    Kind::DownloadUpdate,
                    error,
                )))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    use super::*;
    use crate::tests::temp_dir;

    /// A stand-in for GitHub answering each request with the next of `answers`.
    fn github(answers: Vec<(&'static str, String)>) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let site = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for (status, body) in answers {
                let (mut socket, _) = listener.accept().unwrap();
                let _ = socket.read(&mut [0; 4096]).unwrap();
                let reply = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                socket.write_all(reply.as_bytes()).unwrap();
            }
        });
        site
    }

    fn release(tag: &str) -> (&'static str, String) {
        let json = format!(
            r#"{{"tag_name":"{tag}","html_url":"https://example.org/{tag}","assets":[{{"name":"{}","size":4,"browser_download_url":"https://example.org/msi"}}]}}"#,
            torqa_update::INSTALLER
        );
        ("200 OK", json)
    }

    /// The events until the check or download is over.
    fn settle(app: &mut App) -> Vec<AppEvent> {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut events = Vec::new();
        while Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
            events.extend(app.update(Duration::from_millis(10)));
            if !app.updates.busy {
                return events;
            }
        }
        panic!("the update job never finished");
    }

    fn app(name: &str, answers: Vec<(&'static str, String)>) -> App {
        let dir = temp_dir(name);
        let mut app = App::new(dir.join("data"), dir.join("cache")).unwrap();
        app.updates.source = github(answers);
        app
    }

    #[test]
    fn a_newer_release_is_offered_at_start_up() {
        let mut app = app("update-newer", vec![release("v99.0.0")]);

        assert!(app.check_for_updates(false));
        let events = settle(&mut app);

        assert!(
            events.iter().any(|e| matches!(
                e,
                AppEvent::UpdateAvailable { version, page, installable }
                    if version == "99.0.0"
                        && page.ends_with("v99.0.0")
                        && *installable == torqa_update::can_install()
            )),
            "{events:?}"
        );
        app.shutdown();
    }

    #[test]
    fn at_start_up_only_a_newer_version_is_told() {
        let mut app = app(
            "update-quiet",
            vec![
                release(&format!("v{}", torqa_domain::version())),
                ("500 Oops", String::new()),
            ],
        );

        assert!(app.check_for_updates(false));
        let same = settle(&mut app);
        assert!(app.check_for_updates(false));
        let failed = settle(&mut app);

        assert!(same.is_empty(), "the running version: {same:?}");
        assert!(
            failed.is_empty(),
            "a failure the rider did not ask about: {failed:?}"
        );
        app.shutdown();
    }

    #[test]
    fn asked_the_rider_hears_of_being_up_to_date_and_of_failures() {
        let mut app = app(
            "update-asked",
            vec![
                release(&format!("v{}", torqa_domain::version())),
                ("500 Oops", String::new()),
            ],
        );

        assert!(app.check_for_updates(true));
        let same = settle(&mut app);
        assert!(app.check_for_updates(true));
        let failed = settle(&mut app);

        assert!(matches!(same.as_slice(), [AppEvent::UpToDate]), "{same:?}");
        assert!(
            matches!(failed.as_slice(), [AppEvent::UpdateFailed(f)] if f.kind == Kind::CheckUpdate),
            "{failed:?}"
        );
        app.shutdown();
    }

    #[test]
    fn turned_off_only_asking_looks() {
        let mut app = app("update-off", vec![release("v99.0.0")]);

        app.set_checks_for_updates(false).unwrap();

        assert!(!app.checks_for_updates());
        assert!(!app.check_for_updates(false), "not at start-up");
        assert!(app.check_for_updates(true), "the rider's button still does");
        settle(&mut app);
        app.shutdown();
    }

    #[test]
    fn nothing_is_installed_without_a_download() {
        let mut app = app("update-install", vec![release("v99.0.0")]);

        assert!(app.install_update().is_err(), "nothing downloaded");
        assert!(!app.download_update(), "no newer release known yet");
        app.check_for_updates(true);
        settle(&mut app);

        assert_eq!(
            app.download_update(),
            torqa_update::can_install(),
            "elsewhere the rider gets the release page"
        );
        app.shutdown();
    }
}
