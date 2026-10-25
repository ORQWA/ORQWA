//! Geofabrik's download server (<https://download.geofabrik.de>): the regions it offers and
//! their downloads, fetched only when a rider asks (ADR 0033).

use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::StatusCode;
use reqwest::header::{CONTENT_LENGTH, RANGE};
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use super::regions::{DOWNLOADS, download_path, partial_path, source_path};
use crate::OsmError;

/// Where Geofabrik lists its regions, without their outlines.
pub const INDEX: &str = "https://download.geofabrik.de/index-v1-nogeom.json";
/// The list as last fetched, in the map cache: the regions' names offline.
const INDEX_FILE: &str = "geofabrik-index.json";

/// A region Geofabrik offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Its name on the server, e.g. `switzerland`, `bayern` or `us/california`.
    pub id: String,
    /// Its name to show, e.g. `Switzerland`.
    pub name: String,
    /// The region it is a part of, e.g. `europe`; `None` for a continent.
    pub parent: Option<String>,
    /// Its latest download, e.g. `…/europe/switzerland-latest.osm.pbf`.
    pub url: String,
}

/// The latest download of a region: the file its `-latest` address leads to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Latest {
    /// The file's own address.
    pub url: String,
    /// Its name, dated, e.g. `switzerland-261024.osm.pbf`: another name is a newer download.
    pub name: String,
    /// Its size in bytes.
    pub size: u64,
}

#[derive(Deserialize)]
struct Index {
    features: Vec<Feature>,
}

#[derive(Deserialize)]
struct Feature {
    properties: Properties,
}

#[derive(Deserialize)]
struct Properties {
    id: String,
    name: String,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default)]
    urls: Urls,
}

#[derive(Default, Deserialize)]
struct Urls {
    #[serde(default)]
    pbf: Option<String>,
}

/// Geofabrik's server, or one standing in for it.
pub struct Geofabrik {
    index: String,
    client: reqwest::Client,
}

impl Default for Geofabrik {
    fn default() -> Self {
        Self::at(INDEX)
    }
}

impl Geofabrik {
    /// The server whose region list is at `index`.
    ///
    /// # Panics
    /// If the HTTP client cannot be initialised (no TLS backend), which is a build error.
    #[must_use]
    pub fn at(index: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "ORQWA/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/ORQWA/ORQWA)"
            ))
            .connect_timeout(Duration::from_secs(30))
            // A country's download takes minutes; only a stalled one fails.
            .read_timeout(Duration::from_mins(1))
            .build()
            .expect("HTTP client with TLS");
        Self {
            index: index.into(),
            client,
        }
    }

    /// The regions it offers, as it lists them; the list is kept in the map cache `osm_dir`
    /// for [`cached_offers`].
    ///
    /// # Errors
    /// [`OsmError::Unavailable`] if the list cannot be fetched, [`OsmError::Invalid`] if it is
    /// not one.
    pub async fn offers(&self, osm_dir: &Path) -> Result<Vec<Offer>, OsmError> {
        let response = self
            .client
            .get(&self.index)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(unavailable)?;
        let bytes = response.bytes().await.map_err(unavailable)?;
        let offers = parse(&bytes)?;
        tokio::fs::create_dir_all(osm_dir).await?;
        let aside = osm_dir.join(format!("{INDEX_FILE}.part"));
        tokio::fs::write(&aside, &bytes).await?;
        tokio::fs::rename(&aside, osm_dir.join(INDEX_FILE)).await?;
        Ok(offers)
    }

    /// The latest download of `offer`.
    ///
    /// # Errors
    /// [`OsmError::Unavailable`] if the server does not tell it.
    pub async fn latest(&self, offer: &Offer) -> Result<Latest, OsmError> {
        // Its `-latest` address leads on to the dated file.
        let response = self
            .client
            .head(&offer.url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(unavailable)?;
        let url = response.url().clone();
        let name = url
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| OsmError::Unavailable(format!("no file at {url}")))?
            .to_owned();
        let size = response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|length| length.to_str().ok()?.parse().ok())
            .ok_or_else(|| OsmError::Unavailable(format!("no size for {url}")))?;
        Ok(Latest {
            url: url.into(),
            name,
            size,
        })
    }

    /// Downloads `latest` as region `id`'s download in the map cache `osm_dir` (see
    /// [`download_path`]), replacing an earlier one only once it is whole. A download of the
    /// same file cut short goes on where it stopped. `progress` hears the bytes there of all.
    ///
    /// # Errors
    /// [`OsmError::Unavailable`] if it cannot be fetched or does not arrive whole (what
    /// arrived stays, to go on with), [`OsmError::Cache`] if it cannot be saved.
    pub async fn download(
        &self,
        osm_dir: &Path,
        id: &str,
        latest: &Latest,
        progress: &mut (dyn FnMut(u64, u64) + Send),
    ) -> Result<PathBuf, OsmError> {
        tokio::fs::create_dir_all(osm_dir.join(DOWNLOADS)).await?;
        let partial = partial_path(osm_dir, id);
        let partial_source = source_path(&partial);
        let same = tokio::fs::read_to_string(&partial_source)
            .await
            .is_ok_and(|name| name == latest.name);
        let mut have = if same {
            tokio::fs::metadata(&partial).await.map_or(0, |m| m.len())
        } else {
            tokio::fs::write(&partial_source, &latest.name).await?;
            0
        };
        if have > latest.size {
            have = 0;
        }
        progress(have, latest.size);
        if have < latest.size {
            let mut request = self.client.get(&latest.url);
            if have > 0 {
                request = request.header(RANGE, format!("bytes={have}-"));
            }
            let mut response = request
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(unavailable)?;
            // A server that sends it all again is answered by starting again.
            if response.status() != StatusCode::PARTIAL_CONTENT {
                have = 0;
            }
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(have > 0)
                .truncate(have == 0)
                .open(&partial)
                .await?;
            // What arrived before a failure is kept whole, to go on from.
            let mut failed = None;
            loop {
                match response.chunk().await {
                    Ok(Some(chunk)) => {
                        file.write_all(&chunk).await?;
                        have += chunk.len() as u64;
                        progress(have, latest.size);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        failed = Some(unavailable(error));
                        break;
                    }
                }
            }
            file.flush().await?;
            if let Some(error) = failed {
                return Err(error);
            }
        }
        if have != latest.size {
            return Err(OsmError::Unavailable(format!(
                "{have} of {} bytes of {} arrived",
                latest.size, latest.name
            )));
        }
        let download = download_path(osm_dir, id);
        tokio::fs::rename(&partial, &download).await?;
        tokio::fs::rename(&partial_source, source_path(&download)).await?;
        Ok(download)
    }
}

/// The regions as last fetched into the map cache `osm_dir`; none before.
#[must_use]
pub fn cached_offers(osm_dir: &Path) -> Vec<Offer> {
    std::fs::read(osm_dir.join(INDEX_FILE))
        .ok()
        .and_then(|bytes| parse(&bytes).ok())
        .unwrap_or_default()
}

fn parse(bytes: &[u8]) -> Result<Vec<Offer>, OsmError> {
    let index: Index =
        serde_json::from_slice(bytes).map_err(|e| OsmError::Invalid(format!("regions: {e}")))?;
    Ok(index
        .features
        .into_iter()
        .filter_map(|feature| {
            let p = feature.properties;
            Some(Offer {
                url: p.urls.pbf?,
                id: p.id,
                name: p.name,
                parent: p.parent,
            })
        })
        .collect())
}

#[allow(clippy::needless_pass_by_value)] // used as map_err(unavailable)
fn unavailable(error: reqwest::Error) -> OsmError {
    OsmError::Unavailable(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// The download the stand-in serves: 100 000 bytes of a counting pattern.
    fn file() -> Vec<u8> {
        (0..100_000u32).map(|n| (n % 251) as u8).collect()
    }

    /// A stand-in for Geofabrik serving Bern: its list, its `-latest` address leading to
    /// `bern-261024.osm.pbf`, and that file, in parts when asked; the first `cut` answers with
    /// the file stop after 30 000 bytes. The requests it had, by line and range.
    fn geofabrik(cut: usize) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let site = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&requests);
        let base = site.clone();
        std::thread::spawn(move || {
            let mut cut = cut;
            for socket in listener.incoming() {
                let mut socket = socket.unwrap();
                let mut request = [0; 4096];
                let read = socket.read(&mut request).unwrap();
                let request = String::from_utf8_lossy(&request[..read]).to_lowercase();
                let line = request.lines().next().unwrap_or_default().to_owned();
                let range: Option<usize> = request
                    .lines()
                    .find_map(|l| l.strip_prefix("range: bytes="))
                    .and_then(|r| r.trim_end_matches('-').parse().ok());
                seen.lock().unwrap().push(format!("{line} {range:?}"));
                let head = line.starts_with("head");
                let (status, headers, body): (&str, String, Vec<u8>) = if line.contains("index") {
                    let index = format!(
                        r#"{{"features":[{{"properties":{{"id":"europe","name":"Europe","urls":{{"pbf":"{base}/europe-latest.osm.pbf"}}}}}},{{"properties":{{"id":"bern","parent":"europe","name":"Bern","urls":{{"pbf":"{base}/europe/bern-latest.osm.pbf"}}}}}},{{"properties":{{"id":"gone","name":"No download","urls":{{}}}}}}]}}"#
                    );
                    ("200 OK", String::new(), index.into_bytes())
                } else if line.contains("latest") {
                    let to = format!("Location: {base}/europe/bern-261024.osm.pbf\r\n");
                    ("307 Temporary Redirect", to, Vec::new())
                } else {
                    let file = file();
                    let from = range.unwrap_or(0);
                    let status = if range.is_some() {
                        "206 Partial Content"
                    } else {
                        "200 OK"
                    };
                    (status, String::new(), file[from..].to_vec())
                };
                let mut sent = body.clone();
                if !head && line.contains("261024") && cut > 0 {
                    cut -= 1;
                    sent.truncate(30_000);
                }
                let reply = format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(reply.as_bytes());
                if !head {
                    let _ = socket.write_all(&sent);
                }
            }
        });
        (site, requests)
    }

    fn cache(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("orqwa-geofabrik-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[tokio::test]
    async fn lists_its_regions_and_keeps_the_list_for_their_names_offline() {
        let (site, _) = geofabrik(0);
        let osm = cache("offers");

        let offers = Geofabrik::at(format!("{site}/index-v1-nogeom.json"))
            .offers(&osm)
            .await
            .unwrap();

        let ids: Vec<_> = offers
            .iter()
            .map(|o| (o.id.as_str(), o.parent.as_deref()))
            .collect();
        // One without a download is no offer.
        assert_eq!(ids, [("europe", None), ("bern", Some("europe"))]);
        assert_eq!(cached_offers(&osm), offers);
        std::fs::remove_dir_all(osm).unwrap();
    }

    #[tokio::test]
    async fn downloads_the_latest_file_and_goes_on_with_one_cut_short() {
        let (site, requests) = geofabrik(1);
        let osm = cache("download");
        let geofabrik = Geofabrik::at(format!("{site}/index-v1-nogeom.json"));
        let offers = geofabrik.offers(&osm).await.unwrap();

        let latest = geofabrik.latest(&offers[1]).await.unwrap();
        assert_eq!(latest.name, "bern-261024.osm.pbf");
        assert_eq!(latest.size, 100_000);
        let mut heard = Vec::new();
        let cut = geofabrik
            .download(&osm, "bern", &latest, &mut |done, total| {
                heard.push((done, total));
            })
            .await;
        assert!(matches!(cut, Err(OsmError::Unavailable(_))), "{cut:?}");
        assert!(!download_path(&osm, "bern").exists(), "never there half");
        let path = geofabrik
            .download(&osm, "bern", &latest, &mut |done, total| {
                heard.push((done, total));
            })
            .await
            .unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), file());
        assert_eq!(
            super::super::download_source(&osm, "bern").as_deref(),
            Some("bern-261024.osm.pbf")
        );
        assert!(!partial_path(&osm, "bern").exists());
        let gets: Vec<String> = requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.starts_with("get /europe/bern-261024"))
            .cloned()
            .collect();
        assert_eq!(gets.len(), 2);
        assert!(
            gets[1].ends_with("Some(30000)"),
            "goes on from where it stopped: {gets:?}"
        );
        assert_eq!(heard.last(), Some(&(100_000, 100_000)));
        std::fs::remove_dir_all(osm).unwrap();
    }

    #[tokio::test]
    async fn a_newer_file_starts_afresh() {
        let (site, requests) = geofabrik(0);
        let osm = cache("newer");
        let geofabrik = Geofabrik::at(format!("{site}/index-v1-nogeom.json"));
        let offers = geofabrik.offers(&osm).await.unwrap();
        let latest = geofabrik.latest(&offers[1]).await.unwrap();
        // What is left of yesterday's download.
        std::fs::create_dir_all(osm.join(DOWNLOADS)).unwrap();
        std::fs::write(partial_path(&osm, "bern"), [1; 500]).unwrap();
        std::fs::write(
            source_path(&partial_path(&osm, "bern")),
            "bern-261023.osm.pbf",
        )
        .unwrap();

        let path = geofabrik
            .download(&osm, "bern", &latest, &mut |_, _| {})
            .await
            .unwrap();

        assert_eq!(std::fs::read(path).unwrap(), file());
        let last = requests.lock().unwrap().last().cloned().unwrap();
        assert!(last.ends_with("None"), "from the start: {last}");
        std::fs::remove_dir_all(osm).unwrap();
    }
}
