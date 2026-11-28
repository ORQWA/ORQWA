//! Downloading the chosen images and deriving their facts, once, into the cache.
//!
//! A thumbnail is fetched at most once per image id and a fact set is derived at most once, so
//! preparing the same course again is a set of file reads and costs no requests. Downloads run
//! a few at a time and wait out the source's rate limit instead of failing the whole course.

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::{StreamExt, stream};
use orqwa_domain::imagery::{ImageryError, StreetFacts, StreetImage, StreetImagery};
use serde::{Deserialize, Serialize};

use crate::corridor::Side;
use crate::facts::{Pixels, derive, frontage_name, material_name};
use crate::select::{CANDIDATES, Candidate, Chosen, FrameSide};

/// Where thumbnails are cached, under the app's cache directory.
pub const THUMBNAIL_CACHE: &str = "imagery-thumbnails-v1";

/// Where derived facts are cached, under the app's cache directory. Raise the version when the
/// extractor changes what it reads.
pub const FACTS_CACHE: &str = "imagery-facts-v5";

/// How many thumbnails are fetched at once.
const PARALLEL: usize = 6;

/// One attempt at one picture: its facts, whether they came from the cache, and whether the
/// thumbnail was fetched.
type Attempt = Result<(Derived, bool, bool), ImageryError>;

/// How often a rate-limited download is tried again.
const RATE_LIMIT_TRIES: u32 = 3;

/// One image's facts, with enough of where it was taken to place them.
#[derive(Debug, Clone, PartialEq)]
pub struct Derived {
    /// The image the facts come from.
    pub image: StreetImage,
    /// Distance from the start of the route, in metres.
    pub along: f64,
    /// Distance from the route, in metres.
    pub offset: f64,
    /// Which side of the road the image lies on.
    pub side: Side,
    /// Which half of the picture the facts were read from.
    pub frame_side: FrameSide,
    /// What the picture says.
    pub facts: StreetFacts,
}

/// What a preparation did.
#[derive(Debug, Default)]
pub struct Prepared {
    /// The facts derived, in the order the images were chosen.
    pub derived: Vec<Derived>,
    /// Fact sets read from the cache instead of derived again.
    pub cached: usize,
    /// Thumbnails fetched from the source.
    pub downloaded: usize,
    /// Images that could not be fetched or read, with why.
    pub failed: Vec<(String, ImageryError)>,
}

/// Fetches and derives the facts of `chosen`, using `cache` for thumbnails and facts.
///
/// Each stretch and side is tried with its best-aligned picture first, then the alternatives,
/// until one shows a wall. A stretch whose pictures show none keeps the first picture's facts,
/// which still describe the road, the kerb and the greenery.
///
/// `progress` is called with (stretches done, stretches total). `online` false uses only the
/// cache and reports an uncached image as failed rather than reaching for the network.
pub async fn prepare(
    source: &dyn StreetImagery,
    chosen: &[Chosen<'_>],
    cache: &Path,
    online: bool,
    progress: &mut (dyn FnMut(usize, usize) + Send),
) -> Prepared {
    let total = chosen.len();
    let mut prepared = Prepared::default();
    // One result per stretch and side, and whether that stretch has been settled.
    let mut results: Vec<Option<Attempt>> = (0..total).map(|_| None).collect();
    let mut settled = vec![false; total];
    let mut done = 0;
    for depth in 0..CANDIDATES {
        let pending: Vec<usize> = (0..total)
            .filter(|&index| !settled[index] && chosen[index].candidates.len() > depth)
            .collect();
        if pending.is_empty() {
            break;
        }
        let mut attempts = stream::iter(pending)
            .map(|index| {
                let item = Item::of(&chosen[index].candidates[depth]);
                async move { (index, one(source, cache, online, item).await) }
            })
            .buffer_unordered(PARALLEL);
        while let Some((index, attempt)) = attempts.next().await {
            let improved = match (&results[index], &attempt) {
                (Some(Ok((current, _, _))), Ok((new, _, _))) => {
                    current.facts.facade.is_none() && new.facts.facade.is_some()
                }
                (Some(_), Err(_)) => false,
                // Nothing kept yet, or only a failure: anything that worked is better.
                _ => true,
            };
            let shows_wall =
                matches!(&attempt, Ok((derived, _, _)) if derived.facts.facade.is_some());
            let settled_now = shows_wall || depth + 1 >= chosen[index].candidates.len();
            if improved {
                results[index] = Some(attempt);
            }
            if settled_now {
                settled[index] = true;
                done += 1;
                progress(done, total);
            }
        }
    }
    for (index, result) in results.into_iter().enumerate() {
        match result {
            Some(Ok((derived, cached, downloaded))) => {
                if cached {
                    prepared.cached += 1;
                }
                if downloaded {
                    prepared.downloaded += 1;
                }
                prepared.derived.push(derived);
            }
            Some(Err(error)) => prepared
                .failed
                .push((chosen[index].first().located.image.id.clone(), error)),
            None => {}
        }
    }
    prepared.derived.sort_by(|a, b| {
        a.along
            .total_cmp(&b.along)
            .then(a.image.id.cmp(&b.image.id))
    });
    prepared
}

/// What one image needs, owned so that it can be prepared alongside the others.
struct Item {
    image: StreetImage,
    along: f64,
    offset: f64,
    side: Side,
    frame_side: FrameSide,
}

impl Item {
    fn of(candidate: &Candidate<'_>) -> Self {
        Self {
            image: candidate.located.image.clone(),
            along: candidate.located.along,
            offset: candidate.located.offset,
            side: candidate.located.side,
            frame_side: candidate.frame_side,
        }
    }
}

/// Fetches or reads one image, derives its facts and caches them.
async fn one(source: &dyn StreetImagery, cache: &Path, online: bool, item: Item) -> Attempt {
    let facts_path = cache
        .join(FACTS_CACHE)
        .join(format!("{}.json", safe(&item.image.id)));
    if let Some(facts) = read_facts(&facts_path).await? {
        return Ok((
            Derived {
                image: item.image,
                along: item.along,
                offset: item.offset,
                side: item.side,
                frame_side: item.frame_side,
                facts,
            },
            true,
            false,
        ));
    }

    let thumbnail_path = cache
        .join(THUMBNAIL_CACHE)
        .join(format!("{}.jpg", safe(&item.image.id)));
    let (bytes, downloaded) = match tokio::fs::read(&thumbnail_path).await {
        Ok(bytes) => (bytes, false),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if !online {
                return Err(ImageryError::Unavailable(format!(
                    "image {} is not cached; connect once to prepare it",
                    item.image.id
                )));
            }
            let bytes = download(source, &item.image).await?;
            write(&thumbnail_path, &bytes).await?;
            (bytes, true)
        }
        Err(error) => return Err(ImageryError::Cache(error)),
    };

    let pixels = Pixels::from_jpeg(&bytes)?;
    let facts = derive(&pixels, item.frame_side);
    write_facts(&facts_path, facts).await?;
    Ok((
        Derived {
            image: item.image,
            along: item.along,
            offset: item.offset,
            side: item.side,
            frame_side: item.frame_side,
            facts,
        },
        false,
        downloaded,
    ))
}

/// One thumbnail, waiting out the source's rate limit a few times.
async fn download(
    source: &dyn StreetImagery,
    image: &StreetImage,
) -> Result<Vec<u8>, ImageryError> {
    let mut wait = Duration::from_secs(1);
    let mut attempt = 0;
    loop {
        match source.thumbnail(image).await {
            Err(ImageryError::RateLimited(_)) if attempt < RATE_LIMIT_TRIES => {
                tokio::time::sleep(wait).await;
                wait = wait.saturating_mul(2);
                attempt += 1;
            }
            result => return result,
        }
    }
}

/// A file name that cannot leave the cache directory.
fn safe(id: &str) -> String {
    id.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

async fn write(path: &Path, bytes: &[u8]) -> Result<(), ImageryError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(ImageryError::Cache)?;
    }
    tokio::fs::write(path, bytes)
        .await
        .map_err(ImageryError::Cache)
}

/// What a facts cache file holds.
#[derive(Serialize, Deserialize)]
struct CachedFacts {
    facade: Option<CachedFacade>,
    roof: Option<CachedRoof>,
    frontage: String,
    kerb: bool,
    greenery: bool,
}

#[derive(Serialize, Deserialize)]
struct CachedFacade {
    colour: [u8; 3],
    storeys: u8,
    material: String,
}

#[derive(Serialize, Deserialize)]
struct CachedRoof {
    colour: [u8; 3],
    material: String,
}

impl From<StreetFacts> for CachedFacts {
    fn from(facts: StreetFacts) -> Self {
        Self {
            facade: facts.facade.map(|facade| CachedFacade {
                colour: [facade.colour.red, facade.colour.green, facade.colour.blue],
                storeys: facade.storeys,
                material: material_name(facade.material).to_owned(),
            }),
            roof: facts.roof.map(|roof| CachedRoof {
                colour: [roof.colour.red, roof.colour.green, roof.colour.blue],
                material: material_name(roof.material).to_owned(),
            }),
            frontage: frontage_name(facts.frontage).to_owned(),
            kerb: facts.kerb,
            greenery: facts.greenery,
        }
    }
}

impl From<CachedFacts> for StreetFacts {
    fn from(cached: CachedFacts) -> Self {
        Self {
            facade: cached
                .facade
                .map(|facade| orqwa_domain::imagery::FacadeFacts {
                    colour: colour(facade.colour),
                    storeys: facade.storeys,
                    material: material_from(&facade.material),
                }),
            roof: cached.roof.map(|roof| orqwa_domain::imagery::RoofFacts {
                colour: colour(roof.colour),
                material: material_from(&roof.material),
            }),
            frontage: frontage_from(&cached.frontage),
            kerb: cached.kerb,
            greenery: cached.greenery,
        }
    }
}

fn colour([red, green, blue]: [u8; 3]) -> orqwa_domain::imagery::Rgb {
    orqwa_domain::imagery::Rgb { red, green, blue }
}

fn material_from(name: &str) -> orqwa_domain::imagery::Material {
    use orqwa_domain::imagery::Material;
    match name {
        "brick" => Material::Brick,
        "plaster" => Material::Plaster,
        "concrete" => Material::Concrete,
        "wood" => Material::Wood,
        "stone" => Material::Stone,
        "metal" => Material::Metal,
        "tiles" => Material::Tiles,
        "slate" => Material::Slate,
        _ => Material::Unknown,
    }
}

fn frontage_from(name: &str) -> orqwa_domain::imagery::Frontage {
    use orqwa_domain::imagery::Frontage;
    match name {
        "shop_window" => Frontage::ShopWindow,
        "awning" => Frontage::Awning,
        "garage" => Frontage::Garage,
        "wall" => Frontage::Wall,
        _ => Frontage::Unknown,
    }
}

/// A cached fact set, or `None` when there is none or it cannot be read. A damaged cache file
/// is derived again rather than failing the course.
async fn read_facts(path: &Path) -> Result<Option<StreetFacts>, ImageryError> {
    let bytes = match tokio::fs::read(path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ImageryError::Cache(error)),
    };
    Ok(serde_json::from_slice::<CachedFacts>(&bytes)
        .ok()
        .map(StreetFacts::from))
}

async fn write_facts(path: &Path, facts: StreetFacts) -> Result<(), ImageryError> {
    let cached = CachedFacts::from(facts);
    let bytes = serde_json::to_vec(&cached)
        .map_err(|error| ImageryError::Cache(std::io::Error::other(error)))?;
    write(path, &bytes).await
}

/// Where a thumbnail is cached, for a caller that wants to show it.
#[must_use]
pub fn thumbnail_path(cache: &Path, id: &str) -> PathBuf {
    cache
        .join(THUMBNAIL_CACHE)
        .join(format!("{}.jpg", safe(id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use orqwa_domain::imagery::{BoxFuture, Corridor};
    use orqwa_domain::imagery::{FacadeFacts, Frontage, Material, Rgb, RoofFacts};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn image(id: &str) -> StreetImage {
        StreetImage {
            id: id.to_owned(),
            position: (46.0, 7.0),
            heading: 0.0,
            captured_at: 0,
            thumbnail: Some(format!("https://example.test/{id}.jpg")),
        }
    }

    /// A source that serves one real JPEG per image id and counts fetches.
    struct Fake {
        jpeg: Vec<u8>,
        fetches: AtomicUsize,
    }

    /// A source that serves the real JPEG for every id but the ones it is told to break.
    struct Partly {
        broken: Vec<&'static str>,
        fetches: AtomicUsize,
    }

    impl StreetImagery for Partly {
        fn id(&self) -> &'static str {
            "partly"
        }

        fn name(&self) -> &'static str {
            "Partly"
        }

        fn attribution(&self) -> &'static str {
            "none"
        }

        fn along<'a>(
            &'a self,
            _corridor: Corridor<'a>,
            _progress: &'a mut (dyn FnMut(usize, usize) + Send),
        ) -> BoxFuture<'a, Result<Vec<StreetImage>, ImageryError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn thumbnail<'a>(
            &'a self,
            image: &'a StreetImage,
        ) -> BoxFuture<'a, Result<Vec<u8>, ImageryError>> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            let broken = self.broken.contains(&image.id.as_str());
            Box::pin(async move {
                if broken {
                    Ok(b"not a picture".to_vec())
                } else {
                    Ok(crate::facts::tests_support::JPEG.to_vec())
                }
            })
        }
    }

    fn group(located: &[crate::corridor::Located], depth: usize) -> Chosen<'_> {
        Chosen {
            candidates: located
                .iter()
                .take(depth)
                .map(|located| Candidate {
                    located,
                    frame_side: FrameSide::Right,
                })
                .collect(),
        }
    }

    fn beside(id: &str, along: f64) -> crate::corridor::Located {
        crate::corridor::Located {
            image: image(id),
            along,
            offset: 5.0,
            side: Side::Right,
            road_heading: 0.0,
        }
    }

    impl StreetImagery for Fake {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn name(&self) -> &'static str {
            "Fake"
        }

        fn attribution(&self) -> &'static str {
            "none"
        }

        fn along<'a>(
            &'a self,
            _corridor: Corridor<'a>,
            _progress: &'a mut (dyn FnMut(usize, usize) + Send),
        ) -> BoxFuture<'a, Result<Vec<StreetImage>, ImageryError>> {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn thumbnail<'a>(
            &'a self,
            _image: &'a StreetImage,
        ) -> BoxFuture<'a, Result<Vec<u8>, ImageryError>> {
            self.fetches.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Ok(self.jpeg.clone()) })
        }
    }

    fn cache(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "orqwa-imagery-prepare-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn facts_survive_a_round_trip_through_the_cache() {
        let facts = StreetFacts {
            facade: Some(FacadeFacts {
                colour: Rgb {
                    red: 168,
                    green: 84,
                    blue: 60,
                },
                storeys: 4,
                material: Material::Brick,
            }),
            roof: Some(RoofFacts {
                colour: Rgb {
                    red: 150,
                    green: 70,
                    blue: 55,
                },
                material: Material::Tiles,
            }),
            frontage: Frontage::ShopWindow,
            kerb: true,
            greenery: false,
        };
        let cached = CachedFacts::from(facts);
        let bytes = serde_json::to_vec(&cached).unwrap();
        let back: StreetFacts = serde_json::from_slice::<CachedFacts>(&bytes)
            .unwrap()
            .into();
        assert_eq!(back, facts);
    }

    #[test]
    fn a_damaged_facts_file_is_derived_again() {
        let dir = cache("damaged");
        std::fs::create_dir_all(dir.join(FACTS_CACHE)).unwrap();
        let path = dir.join(FACTS_CACHE).join("a.json");
        std::fs::write(&path, b"{not json").unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let facts = runtime.block_on(read_facts(&path)).unwrap();
        assert!(facts.is_none());
    }

    #[test]
    fn an_id_cannot_leave_the_cache_directory() {
        assert_eq!(safe("../../etc/passwd"), "______etc_passwd");
        assert_eq!(safe("123456"), "123456");
    }

    #[test]
    fn a_second_preparation_reads_the_cache_and_fetches_nothing() {
        let dir = cache("twice");
        let source = Fake {
            jpeg: crate::facts::tests_support::JPEG.to_vec(),
            fetches: AtomicUsize::new(0),
        };
        let located = [beside("1", 10.0)];
        let chosen = [group(&located, 1)];
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let first = runtime.block_on(prepare(&source, &chosen, &dir, true, &mut |_, _| {}));
        assert_eq!(first.derived.len(), 1, "{:?}", first.failed);
        assert_eq!(first.downloaded, 1);
        assert_eq!(source.fetches.load(Ordering::Relaxed), 1);

        let second = runtime.block_on(prepare(&source, &chosen, &dir, true, &mut |_, _| {}));
        assert_eq!(second.derived.len(), 1);
        assert_eq!(second.cached, 1);
        assert_eq!(source.fetches.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_picture_that_cannot_be_read_falls_back_to_the_next_candidate() {
        let dir = cache("fallback");
        let source = Partly {
            broken: vec!["1"],
            fetches: AtomicUsize::new(0),
        };
        let located = [beside("1", 10.0), beside("2", 20.0)];
        let chosen = [group(&located, 2)];
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let prepared = runtime.block_on(prepare(&source, &chosen, &dir, true, &mut |_, _| {}));
        assert_eq!(prepared.derived.len(), 1, "{:?}", prepared.failed);
        assert_eq!(prepared.derived[0].image.id, "2");
        assert_eq!(source.fetches.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_stretch_with_no_wall_stops_after_its_first_picture() {
        let dir = cache("nowall");
        let source = Fake {
            jpeg: crate::facts::tests_support::JPEG.to_vec(),
            fetches: AtomicUsize::new(0),
        };
        let located = [beside("1", 10.0), beside("2", 20.0), beside("3", 30.0)];
        let chosen = [group(&located, 3)];
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let prepared = runtime.block_on(prepare(&source, &chosen, &dir, true, &mut |_, _| {}));
        assert_eq!(prepared.derived.len(), 1);
        assert_eq!(prepared.derived[0].image.id, "1");
        // Every candidate was tried, since none of these pictures shows a wall.
        assert_eq!(source.fetches.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn progress_counts_stretches_not_pictures() {
        let dir = cache("progress");
        let source = Fake {
            jpeg: crate::facts::tests_support::JPEG.to_vec(),
            fetches: AtomicUsize::new(0),
        };
        let located = [beside("1", 10.0), beside("2", 20.0)];
        let chosen = [group(&located, 2)];
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut seen = Vec::new();
        runtime.block_on(prepare(&source, &chosen, &dir, true, &mut |done, total| {
            seen.push((done, total));
        }));
        assert_eq!(seen, [(1, 1)]);
    }
}
