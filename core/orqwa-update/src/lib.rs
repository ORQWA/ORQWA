//! Finding a newer ORQWA release and installing it (R99, ADR 0023): GitHub's latest release,
//! its version against the running one, and on Windows its installer, downloaded and run over
//! the installed ORQWA.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::AsyncWriteExt;

/// GitHub's latest published release of ORQWA; drafts and pre-releases are never "latest".
pub const LATEST_RELEASE: &str = "https://api.github.com/repos/ORQWA/ORQWA/releases/latest";

/// The release asset that installs ORQWA on Windows.
pub const INSTALLER: &str = "ORQWA-windows-x86_64.msi";

/// Something went wrong looking for, downloading or installing an update.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// GitHub could not be reached, or answered with an error.
    #[error("cannot reach GitHub: {0}")]
    Unreachable(String),
    /// GitHub's answer is not a release ORQWA understands.
    #[error("unexpected answer from GitHub: {0}")]
    Unexpected(String),
    /// The installer could not be downloaded or saved.
    #[error("cannot download the installer: {0}")]
    Download(String),
    /// The installer could not be started.
    #[error("cannot start the installer: {0}")]
    Install(String),
}

/// A release version, `major.minor.patch`, as the tags name them (`v0.4.0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    /// Breaking changes.
    pub major: u64,
    /// Features.
    pub minor: u64,
    /// Fixes.
    pub patch: u64,
}

impl Version {
    /// Reads `0.4.0` or `v0.4.0`; anything else, such as a pre-release suffix, is `None`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut parts = text.trim().trim_start_matches('v').split('.');
        let mut number = || parts.next()?.parse::<u64>().ok();
        let version = Self {
            major: number()?,
            minor: number()?,
            patch: number()?,
        };
        parts.next().is_none().then_some(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A published release.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Its version.
    pub version: Version,
    /// The release's page, with its notes.
    pub page: String,
    /// The Windows installer, when the release carries one.
    pub installer: Option<Installer>,
}

impl Release {
    /// Whether this release is newer than the running `current` version (`0.4.0`); a running
    /// version that is no release version, e.g. a development build's, is never older.
    #[must_use]
    pub fn is_newer_than(&self, current: &str) -> bool {
        Version::parse(current).is_some_and(|current| self.version > current)
    }
}

/// A release's installer, to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installer {
    /// Where to download it.
    pub url: String,
    /// Its size in bytes, to tell a complete download.
    pub size: u64,
}

fn client(timeout: Duration) -> Result<reqwest::Client, UpdateError> {
    reqwest::Client::builder()
        .user_agent(concat!(
            "ORQWA/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/ORQWA/ORQWA)"
        ))
        .timeout(timeout)
        .build()
        .map_err(|e| UpdateError::Unreachable(format!("no HTTP client: {e}")))
}

/// Asks `url` (normally [`LATEST_RELEASE`]) for the latest release.
///
/// # Errors
/// [`UpdateError::Unreachable`] without an answer or with an error status,
/// [`UpdateError::Unexpected`] when the answer is no release.
pub async fn latest_release(url: &str) -> Result<Release, UpdateError> {
    let response = client(Duration::from_secs(20))?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| UpdateError::Unreachable(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(UpdateError::Unreachable(format!("HTTP {status}")));
    }
    let body = response
        .text()
        .await
        .map_err(|e| UpdateError::Unreachable(e.to_string()))?;
    parse_release(&body)
}

/// A release from GitHub's JSON for one.
///
/// # Errors
/// [`UpdateError::Unexpected`] when it is no JSON, or its tag no version.
pub fn parse_release(json: &str) -> Result<Release, UpdateError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| UpdateError::Unexpected(e.to_string()))?;
    let tag = value["tag_name"].as_str().unwrap_or_default();
    let version = Version::parse(tag)
        .ok_or_else(|| UpdateError::Unexpected(format!("tag {tag:?} is no version")))?;
    let installer = value["assets"].as_array().and_then(|assets| {
        assets
            .iter()
            .find(|asset| asset["name"] == INSTALLER)
            .and_then(|asset| {
                Some(Installer {
                    url: asset["browser_download_url"].as_str()?.to_owned(),
                    size: asset["size"].as_u64()?,
                })
            })
    });
    Ok(Release {
        version,
        page: value["html_url"].as_str().unwrap_or_default().to_owned(),
        installer,
    })
}

/// Downloads `installer` into `dir` as `name`, replacing an earlier download, and returns
/// its path. A download that ends early never lies there under that name.
///
/// # Errors
/// [`UpdateError::Download`] if it cannot be fetched or saved, or arrives incomplete.
pub async fn download(
    installer: &Installer,
    dir: &Path,
    name: &str,
) -> Result<PathBuf, UpdateError> {
    let failed = |e: &dyn fmt::Display| UpdateError::Download(e.to_string());
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| failed(&e))?;
    let path = dir.join(name);
    let partial = dir.join(format!("{name}.part"));
    // A whole installer over a slow line takes minutes; only a stalled one fails.
    let mut response = client(Duration::from_mins(30))?
        .get(&installer.url)
        .send()
        .await
        .map_err(|e| failed(&e))?;
    if !response.status().is_success() {
        return Err(UpdateError::Download(format!("HTTP {}", response.status())));
    }
    let mut file = tokio::fs::File::create(&partial)
        .await
        .map_err(|e| failed(&e))?;
    let mut received = 0_u64;
    while let Some(chunk) = response.chunk().await.map_err(|e| failed(&e))? {
        received += chunk.len() as u64;
        file.write_all(&chunk).await.map_err(|e| failed(&e))?;
    }
    file.flush().await.map_err(|e| failed(&e))?;
    drop(file);
    if received != installer.size {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(UpdateError::Download(format!(
            "{received} of {} bytes arrived",
            installer.size
        )));
    }
    tokio::fs::rename(&partial, &path)
        .await
        .map_err(|e| failed(&e))?;
    Ok(path)
}

/// Whether this system can install an update itself; elsewhere the rider downloads it.
#[must_use]
pub const fn can_install() -> bool {
    cfg!(windows)
}

/// Starts installing `msi` over the running ORQWA and then starting `app` again; the caller
/// quits at once, so the installer can replace ORQWA's files. Windows asks for permission, as
/// ORQWA is installed for all users.
///
/// # Errors
/// [`UpdateError::Install`] if the installer cannot be started, or on another system.
pub fn install(msi: &Path, app: &Path) -> Result<(), UpdateError> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// Runs without a console window flashing up.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        // `ping` waits a few seconds without needing a console (unlike `timeout`), so ORQWA
        // has quit before the installer replaces its files. `/S` keeps cmd's quoting simple.
        let script = format!(
            "/S /C \"ping -n 4 127.0.0.1 >nul & msiexec /i \"{}\" /passive /norestart && start \"\" \"{}\"\"",
            msi.display(),
            app.display()
        );
        std::process::Command::new("cmd")
            .raw_arg(script)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map(drop)
            .map_err(|e| UpdateError::Install(e.to_string()))
    }
    #[cfg(not(windows))]
    {
        let _ = (msi, app);
        Err(UpdateError::Install(
            "ORQWA installs updates itself only on Windows".to_owned(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    /// GitHub's answer for a release, cut to what ORQWA reads.
    fn release_json(tag: &str, assets: &str) -> String {
        format!(
            r#"{{"tag_name":"{tag}","html_url":"https://github.com/ORQWA/ORQWA/releases/tag/{tag}","draft":false,"prerelease":false,"assets":[{assets}]}}"#
        )
    }

    fn asset(name: &str, url: &str, size: u64) -> String {
        format!(r#"{{"name":"{name}","size":{size},"browser_download_url":"{url}"}}"#)
    }

    /// Answers each connection with the next of `answers`, an HTTP status and body.
    async fn serve(answers: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let site = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            for (status, body) in answers {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(head.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            }
        });
        site
    }

    #[test]
    fn versions_compare_by_number_not_by_text() {
        let v = |text| Version::parse(text).unwrap();

        assert!(v("v0.10.0") > v("v0.9.9"));
        assert!(v("1.0.0") > v("0.42.7"));
        assert_eq!(v("v0.4.0"), v("0.4.0"));
        assert_eq!(v("0.4.0").to_string(), "0.4.0");
        assert_eq!(
            Version::parse("v0.5.0-rc.1"),
            None,
            "pre-releases are not offered"
        );
        assert_eq!(Version::parse("0.5"), None);
        assert_eq!(Version::parse("0.5.0.0"), None);
    }

    #[test]
    fn only_a_newer_release_is_an_update() {
        let release = parse_release(&release_json("v0.5.0", "")).unwrap();

        assert!(release.is_newer_than("0.4.0"));
        assert!(!release.is_newer_than("0.5.0"), "the running version");
        assert!(
            !release.is_newer_than("0.6.0"),
            "a build ahead of the release"
        );
        assert!(!release.is_newer_than("dev"), "not a release version");
    }

    #[test]
    fn the_windows_installer_is_found_among_the_assets() {
        let json = release_json(
            "v0.5.0",
            &[
                asset("ORQWA-windows-x86_64.zip", "https://x/zip", 1),
                asset(INSTALLER, "https://x/msi", 130_000_000),
            ]
            .join(","),
        );

        let release = parse_release(&json).unwrap();

        assert_eq!(release.version, Version::parse("0.5.0").unwrap());
        assert!(release.page.ends_with("/releases/tag/v0.5.0"));
        assert_eq!(
            release.installer,
            Some(Installer {
                url: "https://x/msi".to_owned(),
                size: 130_000_000
            })
        );
    }

    #[test]
    fn a_release_without_an_installer_still_counts() {
        let json = release_json("v0.5.0", &asset("ORQWA-macos-arm64.zip", "https://x/m", 1));

        let release = parse_release(&json).unwrap();

        assert_eq!(
            release.installer, None,
            "offered as a download page instead"
        );
    }

    #[test]
    fn answers_that_are_no_release_are_errors() {
        assert!(matches!(
            parse_release("not json"),
            Err(UpdateError::Unexpected(_))
        ));
        assert!(matches!(
            parse_release(&release_json("nightly", "")),
            Err(UpdateError::Unexpected(_))
        ));
    }

    #[tokio::test]
    async fn the_latest_release_comes_from_github() {
        let json = release_json("v0.5.0", &asset(INSTALLER, "https://x/msi", 3));
        let site = serve(vec![("200 OK", json.into_bytes())]).await;

        let release = latest_release(&site).await.unwrap();

        assert_eq!(release.version.to_string(), "0.5.0");
    }

    #[tokio::test]
    async fn an_error_status_is_unreachable() {
        let site = serve(vec![("403 Forbidden", b"{}".to_vec())]).await;

        assert!(matches!(
            latest_release(&site).await,
            Err(UpdateError::Unreachable(_))
        ));
    }

    #[tokio::test]
    async fn the_installer_is_saved_only_when_complete() {
        let dir = std::env::temp_dir().join(format!("orqwa-update-{}", std::process::id()));
        let site = serve(vec![
            ("200 OK", b"MSI!".to_vec()),
            ("200 OK", b"MS".to_vec()),
        ])
        .await;
        let installer = Installer {
            url: site.clone(),
            size: 4,
        };

        let path = download(&installer, &dir, "ORQWA-0.5.0.msi").await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"MSI!");
        std::fs::remove_file(&path).unwrap();

        let cut = download(&installer, &dir, "ORQWA-0.5.0.msi").await;
        assert!(matches!(cut, Err(UpdateError::Download(_))), "{cut:?}");
        assert!(
            !path.exists(),
            "nothing incomplete under the installer's name"
        );
        assert!(!dir.join("ORQWA-0.5.0.msi.part").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
