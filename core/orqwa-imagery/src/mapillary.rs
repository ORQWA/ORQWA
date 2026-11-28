//! Mapillary: street-level images under CC BY-SA 4.0, searched by bounding box and cached on
//! disk (R307, [ADR 0046](../../../docs/adr/0046-street-imagery-mapillary.md)).
//!
//! Only metadata is kept: where an image was taken, which way it looked and when. The images
//! themselves are fetched later, by the code that derives facts from them.
//!
//! © Mapillary contributors, CC BY-SA 4.0 — attribution must be shown where the facts are.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use orqwa_domain::imagery::{BoxFuture, Corridor, ImageryError, StreetImage, StreetImagery};
use serde::{Deserialize, Serialize};

use crate::corridor::{self, Cell};

/// Version of the cache layout: raised when what is kept or how it is named changes.
pub const CACHE_VERSION: &str = "mapillary-metadata-v1";

/// Where the API lives.
const ENDPOINT: &str = "https://graph.mapillary.com/images";

/// What is asked for: enough to place an image and choose between them, nothing more.
const FIELDS: &str = "id,geometry,compass_angle,captured_at";

/// The most images one response may hold.
const PAGE_LIMIT: usize = 2_000;

/// Pages per cell before the rest is left for another run, so a dense city stays bounded.
const MAX_PAGES: usize = 10;

/// How long a cached cell is used before the source is asked again.
const FRESH: Duration = Duration::from_hours(30 * 24);

/// Mapillary, cached under a directory.
pub struct Mapillary {
    cache_dir: PathBuf,
    endpoint: String,
    token: Option<String>,
    client: reqwest::Client,
    online: bool,
    refresh: bool,
    requests: AtomicUsize,
    paging: AtomicUsize,
    cached_cells: AtomicUsize,
    fetched_cells: AtomicUsize,
}

impl Mapillary {
    /// Creates a client caching under `cache_dir`.
    ///
    /// # Panics
    /// If the HTTP client cannot be initialised (no TLS backend), which is a build error.
    #[must_use]
    pub fn new(cache_dir: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!(
                "ORQWA/",
                env!("CARGO_PKG_VERSION"),
                " (+https://github.com/ORQWA/ORQWA)"
            ))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("HTTP client with TLS");
        Self {
            cache_dir,
            endpoint: ENDPOINT.to_owned(),
            token: None,
            client,
            online: true,
            refresh: false,
            requests: AtomicUsize::new(0),
            paging: AtomicUsize::new(0),
            cached_cells: AtomicUsize::new(0),
            fetched_cells: AtomicUsize::new(0),
        }
    }

    /// Uses this access token for requests.
    #[must_use]
    pub fn with_token(mut self, token: Option<String>) -> Self {
        self.token = token.filter(|token| !token.is_empty());
        self
    }

    /// Uses only the cache and never downloads.
    #[must_use]
    pub fn offline(mut self) -> Self {
        self.online = false;
        self
    }

    /// Asks the source again even where the cache is fresh.
    #[must_use]
    pub fn refreshing(mut self) -> Self {
        self.refresh = true;
        self
    }

    /// HTTP requests made so far, paging included.
    #[must_use]
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::Relaxed)
    }

    /// Requests made after the first of a cell.
    #[must_use]
    pub fn paging(&self) -> usize {
        self.paging.load(Ordering::Relaxed)
    }

    /// Cells answered from the cache.
    #[must_use]
    pub fn cached_cells(&self) -> usize {
        self.cached_cells.load(Ordering::Relaxed)
    }

    /// Cells answered by the source.
    #[must_use]
    pub fn fetched_cells(&self) -> usize {
        self.fetched_cells.load(Ordering::Relaxed)
    }

    /// Where a cell is cached.
    fn path(&self, cell: Cell) -> PathBuf {
        self.cache_dir
            .join(CACHE_VERSION)
            .join(format!("{}.json", cell.key()))
    }

    /// The images of one cell, from the cache or the source.
    async fn images_in(&self, cell: Cell) -> Result<Vec<StreetImage>, ImageryError> {
        let path = self.path(cell);
        let cached = read_cache(&path).await?;
        if let Some((images, fresh)) = cached
            && (!self.online || (fresh && !self.refresh))
        {
            self.cached_cells.fetch_add(1, Ordering::Relaxed);
            return Ok(images);
        }
        if !self.online {
            return Err(ImageryError::Unavailable(format!(
                "no cached street imagery for cell {}: connect once with a Mapillary token",
                cell.key()
            )));
        }
        let Some(token) = self.token.as_deref() else {
            return Err(ImageryError::Unavailable(
                "set MAPILLARY_TOKEN to a Mapillary access token".to_owned(),
            ));
        };
        let images = self.fetch(cell, token).await?;
        write_cache(&path, &images).await?;
        self.fetched_cells.fetch_add(1, Ordering::Relaxed);
        Ok(images)
    }

    /// Asks the source for one cell, following its paging.
    async fn fetch(&self, cell: Cell, token: &str) -> Result<Vec<StreetImage>, ImageryError> {
        let (west, south, east, north) = cell.bounds();
        let mut url = format!(
            "{}?fields={FIELDS}&limit={PAGE_LIMIT}&bbox={west},{south},{east},{north}",
            self.endpoint
        );
        let mut images = Vec::new();
        for page in 0..MAX_PAGES {
            self.requests.fetch_add(1, Ordering::Relaxed);
            if page > 0 {
                self.paging.fetch_add(1, Ordering::Relaxed);
            }
            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("OAuth {token}"))
                .send()
                .await
                .map_err(|error| ImageryError::Unavailable(describe(&error)))?;
            let status = response.status();
            let body = response
                .text()
                .await
                .map_err(|error| ImageryError::Unavailable(describe(&error)))?;
            match status.as_u16() {
                200 => {}
                401 | 403 => {
                    return Err(ImageryError::Unauthorized(
                        "Mapillary turned the token down".to_owned(),
                    ));
                }
                429 => {
                    return Err(ImageryError::Rejected(
                        "Mapillary's rate limit is reached: try again later".to_owned(),
                    ));
                }
                _ => {
                    return Err(ImageryError::Rejected(format!(
                        "Mapillary answered {status}: {}",
                        snippet(&body)
                    )));
                }
            }
            let page: Page = serde_json::from_str(&body).map_err(|error| {
                ImageryError::Rejected(format!("unreadable Mapillary response: {error}"))
            })?;
            images.extend(page.data.into_iter().map(StreetImage::from));
            let Some(next) = page.paging.and_then(|paging| paging.next) else {
                break;
            };
            url = next;
        }
        Ok(images)
    }
}

impl StreetImagery for Mapillary {
    fn id(&self) -> &'static str {
        "mapillary"
    }

    fn name(&self) -> &'static str {
        "Mapillary"
    }

    fn attribution(&self) -> &'static str {
        "© Mapillary contributors, CC BY-SA 4.0"
    }

    fn along<'a>(
        &'a self,
        corridor: Corridor<'a>,
        progress: &'a mut (dyn FnMut(usize, usize) + Send),
    ) -> BoxFuture<'a, Result<Vec<StreetImage>, ImageryError>> {
        Box::pin(async move {
            let cells = corridor::cells(corridor.points, corridor.width);
            let total = cells.len();
            let mut found = BTreeMap::new();
            for (done, cell) in cells.into_iter().enumerate() {
                for image in self.images_in(cell).await? {
                    found.entry(image.id.clone()).or_insert(image);
                }
                progress(done + 1, total);
            }
            let mut images: Vec<StreetImage> = found
                .into_values()
                .filter(|image| {
                    corridor::distance_to_route(corridor.points, image.position)
                        .is_some_and(|(distance, _)| distance <= corridor.width)
                })
                .collect();
            images.sort_by(|a, b| a.id.cmp(&b.id));
            Ok(images)
        })
    }
}

/// A page of the images endpoint.
#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    data: Vec<Raw>,
    paging: Option<Paging>,
}

/// Where the next page is.
#[derive(Deserialize)]
struct Paging {
    next: Option<String>,
}

/// One image as the endpoint describes it.
#[derive(Deserialize)]
struct Raw {
    id: String,
    geometry: Geometry,
    compass_angle: Option<f64>,
    /// Milliseconds since the Unix epoch.
    captured_at: Option<i64>,
}

/// A `GeoJSON` point.
#[derive(Deserialize)]
struct Geometry {
    /// Longitude and latitude, in that order.
    coordinates: [f64; 2],
}

impl From<Raw> for StreetImage {
    fn from(raw: Raw) -> Self {
        Self {
            id: raw.id,
            position: (raw.geometry.coordinates[1], raw.geometry.coordinates[0]),
            heading: raw.compass_angle.unwrap_or(0.0).to_radians(),
            // Mapillary gives milliseconds; StreetImage keeps seconds.
            captured_at: raw.captured_at.map_or(0, |millis| millis / 1000),
        }
    }
}

/// What a cache file holds.
#[derive(Serialize, Deserialize)]
struct Cached {
    /// When it was fetched, in seconds since the Unix epoch.
    fetched_at: i64,
    images: Vec<CachedImage>,
}

/// An image as the cache keeps it.
#[derive(Serialize, Deserialize)]
struct CachedImage {
    id: String,
    lat: f64,
    lon: f64,
    heading: f64,
    captured_at: i64,
}

impl From<&StreetImage> for CachedImage {
    fn from(image: &StreetImage) -> Self {
        Self {
            id: image.id.clone(),
            lat: image.position.0,
            lon: image.position.1,
            heading: image.heading,
            captured_at: image.captured_at,
        }
    }
}

impl From<CachedImage> for StreetImage {
    fn from(image: CachedImage) -> Self {
        Self {
            id: image.id,
            position: (image.lat, image.lon),
            heading: image.heading,
            captured_at: image.captured_at,
        }
    }
}

/// The cached images of a cell, and whether they are still fresh.
async fn read_cache(path: &Path) -> Result<Option<(Vec<StreetImage>, bool)>, ImageryError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ImageryError::Cache(error)),
    };
    let cached: Cached = serde_json::from_slice(&bytes).map_err(|error| {
        ImageryError::Cache(std::io::Error::new(std::io::ErrorKind::InvalidData, error))
    })?;
    let fresh = now().saturating_sub(cached.fetched_at)
        < i64::try_from(FRESH.as_secs()).unwrap_or(i64::MAX);
    let images = cached.images.into_iter().map(StreetImage::from).collect();
    Ok(Some((images, fresh)))
}

/// Writes a cell's images to the cache.
async fn write_cache(path: &Path, images: &[StreetImage]) -> Result<(), ImageryError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(ImageryError::Cache)?;
    }
    let cached = Cached {
        fetched_at: now(),
        images: images.iter().map(CachedImage::from).collect(),
    };
    let bytes = serde_json::to_vec(&cached)
        .map_err(|error| ImageryError::Cache(std::io::Error::other(error)))?;
    tokio::fs::write(path, bytes)
        .await
        .map_err(ImageryError::Cache)
}

/// Seconds since the Unix epoch.
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_secs()).unwrap_or(i64::MAX)
        })
}

/// The first part of a response body, for an error message.
fn snippet(body: &str) -> String {
    body.chars().take(200).collect()
}

/// The error and everything that caused it, for a message a rider can act on.
fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use super::*;

    /// A cache directory of this test alone.
    fn cache_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "orqwa-imagery-{}-{name}-{}",
            std::process::id(),
            now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A stand-in for Mapillary serving `responses` in order; `{base}` is replaced with its
    /// address. Returns the address and the requests it saw.
    fn serve(responses: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let responses: Vec<(u16, String)> = responses
            .into_iter()
            .map(|(status, body)| (status, body.replace("{base}", &base)))
            .collect();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            let mut responses = responses.into_iter();
            for socket in listener.incoming() {
                let mut socket = socket.unwrap();
                // Keep-alive: a client may send its next request on the same connection.
                let mut pending = Vec::new();
                while let Some(head) = read_head(&mut socket, &mut pending) {
                    log.lock()
                        .unwrap()
                        .push(String::from_utf8_lossy(&head).into_owned());
                    let (status, body) = responses.next().unwrap_or((500, "{}".to_owned()));
                    let reply = format!(
                        "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n",
                        body.len()
                    );
                    if socket
                        .write_all(reply.as_bytes())
                        .and_then(|()| socket.write_all(body.as_bytes()))
                        .is_err()
                    {
                        break;
                    }
                }
            }
        });
        (base, seen)
    }

    /// One request head from `socket`, keeping bytes that belong to the next one.
    fn read_head(socket: &mut std::net::TcpStream, pending: &mut Vec<u8>) -> Option<Vec<u8>> {
        let mut buffer = [0; 4096];
        while !pending.windows(4).any(|window| window == b"\r\n\r\n") {
            let read = socket.read(&mut buffer).ok()?;
            if read == 0 {
                return None;
            }
            pending.extend_from_slice(&buffer[..read]);
        }
        let end = pending
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .expect("head found above")
            + 4;
        Some(pending.drain(..end).collect())
    }

    fn page(id: &str, next: Option<&str>) -> String {
        let paging = next.map_or_else(String::new, |next| {
            format!(r#","paging":{{"next":"{next}"}}"#)
        });
        format!(
            r#"{{"data":[{{"id":"{id}","geometry":{{"type":"Point","coordinates":[7.0025,46.0025]}},"compass_angle":90.0,"captured_at":1700000000000}}]{paging}}}"#
        )
    }

    fn client(cache: &Path, endpoint: &str) -> Mapillary {
        let mut client = Mapillary::new(cache.to_path_buf());
        client.endpoint = endpoint.to_owned();
        client
    }

    #[tokio::test]
    async fn paging_is_followed_and_every_request_is_counted() {
        let cache = cache_dir("paging");
        let (base, seen) = serve(vec![
            (200, page("a", Some("{base}/images?after=x"))),
            (200, page("b", None)),
        ]);
        let mapillary = client(&cache, &format!("{base}/images")).with_token(Some("t".into()));
        let images = mapillary
            .along(
                Corridor {
                    points: &[(46.0025, 7.0025), (46.003, 7.0025)],
                    width: 30.0,
                },
                &mut |_, _| {},
            )
            .await
            .unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(
            images[0].captured_at, 1_700_000_000,
            "milliseconds become seconds"
        );
        assert_eq!(mapillary.requests(), 2);
        assert_eq!(mapillary.paging(), 1);
        assert_eq!(mapillary.fetched_cells(), 1);
        assert_eq!(seen.lock().unwrap().len(), 2);
        let token = seen.lock().unwrap()[0].to_lowercase();
        assert!(token.contains("authorization: oauth t"), "{token}");
    }

    #[tokio::test]
    async fn a_cached_cell_is_not_asked_for_again() {
        let cache = cache_dir("cached");
        let (base, _) = serve(vec![(200, page("a", None))]);
        let endpoint = format!("{base}/images");
        client(&cache, &endpoint)
            .with_token(Some("t".into()))
            .along(
                Corridor {
                    points: &[(46.0025, 7.0025)],
                    width: 30.0,
                },
                &mut |_, _| {},
            )
            .await
            .unwrap();

        let again = client(&cache, &endpoint);
        let images = again
            .along(
                Corridor {
                    points: &[(46.0025, 7.0025)],
                    width: 30.0,
                },
                &mut |_, _| {},
            )
            .await
            .unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(again.requests(), 0);
        assert_eq!(again.cached_cells(), 1);
    }

    #[tokio::test]
    async fn offline_without_a_cache_says_so() {
        let cache = cache_dir("offline");
        let mapillary = client(&cache, "http://127.0.0.1:1/images").offline();
        let error = mapillary
            .along(
                Corridor {
                    points: &[(46.0025, 7.0025)],
                    width: 30.0,
                },
                &mut |_, _| {},
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ImageryError::Unavailable(_)), "{error}");
    }

    #[tokio::test]
    async fn a_stale_cache_is_asked_for_again() {
        let cache = cache_dir("stale");
        let (base, seen) = serve(vec![(200, page("a", None)), (200, page("b", None))]);
        let endpoint = format!("{base}/images");
        let mapillary = client(&cache, &endpoint).with_token(Some("t".into()));
        let corridor = Corridor {
            points: &[(46.0025, 7.0025)],
            width: 30.0,
        };
        mapillary.along(corridor, &mut |_, _| {}).await.unwrap();

        // Age the cache past its freshness.
        let path = mapillary.path(Cell::of((46.0025, 7.0025)));
        let mut cached: Cached = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        cached.fetched_at = 0;
        std::fs::write(&path, serde_json::to_vec(&cached).unwrap()).unwrap();

        let images = mapillary.along(corridor, &mut |_, _| {}).await.unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(seen.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_rejected_token_is_named() {
        let cache = cache_dir("token");
        let (base, _) = serve(vec![(401, "{\"error\":{}}".to_owned())]);
        let mapillary = client(&cache, &format!("{base}/images")).with_token(Some("bad".into()));
        let error = mapillary
            .along(
                Corridor {
                    points: &[(46.0025, 7.0025)],
                    width: 30.0,
                },
                &mut |_, _| {},
            )
            .await
            .unwrap_err();
        assert!(matches!(error, ImageryError::Unauthorized(_)), "{error}");
    }

    #[test]
    fn an_image_without_a_heading_or_time_is_still_placed() {
        let raw: Raw = serde_json::from_str(
            r#"{"id":"a","geometry":{"type":"Point","coordinates":[7.5,46.5]},"compass_angle":null,"captured_at":null}"#,
        )
        .unwrap();
        let image = StreetImage::from(raw);
        assert_eq!(image.position, (46.5, 7.5));
        assert!(image.heading.abs() < f64::EPSILON);
        assert_eq!(image.captured_at, 0);
    }
}
