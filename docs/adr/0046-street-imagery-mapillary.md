# ADR 0046 — Street imagery from Mapillary

- Status: proposed (step 3 of 6: thumbnails, decoding and the fact extractor)
- Date: 2026-11-28
- Requirements: R307, R3, R33, R89, R101
- Supersedes: the imagery source of R18 (dropped 2026-10-15), which stays dropped for Google

## Context

R18 asked for Google Street View and Mapillary panoramas in the ride, online only, and was
dropped on 2026-10-15. Google's terms allow no lasting storage and no derivative work, while a
course keeps its pre-built world in its file (R33, [ADR 0042](0042-world-files.md)) and rides
offline (R3). A world that must work in ten years cannot be made of Google's pixels.

The realistic look ([ADR 0020](0020-realistic-look.md), R89) is still mostly guessed where the
map is thin: only 8.9 % of buildings in the DACH download carry a shape tag, 5.6 % in
Switzerland ([ADR 0041](0041-building-shape.md)). Walls, storeys, materials and what stands at
street level come from recipes. Street-level photographs are the one open source that shows
them.

Mapillary publishes street-level images under CC BY-SA 4.0 through a versioned API. Coverage
is uneven by country and city, and images carry capture time, position and compass heading.
KartaView offers a similar corpus under the same licence if Mapillary proves too thin.

## Decision

- **Mapillary is the only street imagery source.** Google Street View stays out of worlds and
  of the app: R18's Google half is not revived. KartaView remains an option behind the same
  trait, not a second implementation now.
- **Facts are kept, pixels are not.** A world may keep what a photograph *says* about a place —
  facade base colour (quantised), storeys, wall and roof material class, ground-floor frontage
  (shop window, awning, garage), kerb and street-furniture class — as values in an optional
  world extension. It never keeps the image, a crop, a traced outline or a texture made from
  one. Photographs live only in the download cache and are deleted with it.
- **Opt-in, with the rider's own token**, like the Geofabrik regions (R142) and the Strava
  application (ADR 0013). Off by default; a course prepared without it looks exactly as it does
  today, and nothing in the ride path waits for the network (R3).
- **The corridor is queried in bulk, once.** Mapillary's image search takes a bounding box, so
  the route's corridor is swept as a few hundred-metre tiles rather than one request per route
  point. For each stretch of road and side, the newest image looking along the road is chosen;
  the rest of the metadata is discarded.
- **Only chosen images are fetched, once, and only at thumbnail size.** Downloads are bounded
  (six at a time, at most 4 MiB each), respect `429` with backoff, and are cached by image id,
  so rebuilding a course costs no requests. The derived facts are cached beside the images by
  image id, so a second preparation is a file read. A stretch keeps up to three candidates and
  preparation tries them in order until one shows a wall; a stretch none of them shows keeps
  the first picture's facts, which still describe the road.
- **Facts stream into the world as its chunks build.** The world builder already fills chunks
  in order; hints for a chunk are derived while the next chunk's images download, so the first
  streets appear within seconds of the route being matched instead of after the whole corridor.
  Hints that arrive later regenerate their own chunk only.
- **The world file says what it used.** The facts go in an optional extension with the
  Mapillary image ids and capture dates they came from, the licence and the attribution
  (`© Mapillary contributors`, CC BY-SA 4.0). A release that does not know the extension ignores
  it, as [ADR 0042](0042-world-files.md) requires; one that does reads it. Changing what the
  facts do to the look raises `orqwa_world::GENERATOR` and `stored::FORMAT` as any other
  visual change does.
- **The app names the server.** `graph.mapillary.com` joins the wiki's list of servers ORQWA
  contacts, in the step that first calls it.

## Why facts and not pixels

CC BY-SA 4.0 lets ORQWA store and derive from the images, but a derived work must be shared
under the same licence. Worlds are sold ([ADR 0043](0043-signed-licensed-worlds.md)), and a
world that is an adaptation of Mapillary images could not be. Facts — how many storeys a house
has, what colour its wall is, whether a shopfront faces the road — are not copyrightable
expression, and a file that keeps only those is not an adaptation. That is the line the
implementation must not cross: no image data, no traced geometry, no photogrammetry, no
texture baked from a photograph.

Where the boundary is close (a colour sampled from a facade is still a fact; a facade *photo*
is not), the code keeps the quantised value and discards the sample.

## Steps

Each step is its own PR:

1. This ADR, R307 and the roadmap entry.
2. `StreetImagery` behind a trait in `orqwa-domain`; the Mapillary metadata client, corridor
   sweep and cache; `orqwa-cli imagery-scan <route>` measuring coverage, images found and
   requests per kilometre on real courses. No pixels yet, so no decoder dependency.
3. Thumbnail download, decoding and the fact extractor, with the decoder crate agreed first
   (`zune-jpeg` is the candidate; the workspace has `png` and `image-webp` only).
4. The world uses the facts for facades, storeys and street-level detail; generator bump and
   the standard render views for review.
5. The app: the setting, the token, progress and cancel, and the offline fallback.
6. The course-file extension, its attribution in the manifest and About, and the wiki's server
   list and usage guide.

## Measurements

`orqwa-cli imagery-scan <route>` in the dev container (2026-11-28), corridor 30 m, one
Mapillary token:

| Course | Length | Cells | Requests | Images within 30 m | Coverage | Requests per km |
|---|---:|---:|---:|---:|---:|---:|
| Bielersee | 7.1 km | 24 | 24 | 3,451 | 99 % (left 98 %, right 98 %) | 3.4 |
| London cycleway | 0.9 km | 3 | 3 | 1,154 | 81 % (left 51 %, right 77 %) | 3.4 |
| Oberalp | 33.2 km | 87 | 78 | 6,214 | 69 % (left 54 %, right 42 %) | 2.3 |

A 0.005° cell is about 550 m, so a course costs 2–4 requests per kilometre, and a second scan
of the same corridor makes none: every cell it needs is already cached. Coverage is the share
of route samples 10 m apart with an image within 30 m and 25 m along; the alpine route shows
the expected gap where the pass road has fewer photographs, and the city lane shows the other
side photographed more often than the left.

The Oberalp scan first stopped at the source's rate limit after nine cells and finished on the
next run; the cache kept what had arrived. Pacing across a long corridor belongs with the
extractor, not the measurement.

### What the photographs said

`orqwa-cli imagery-scan <route> --facts` on the same courses, one image per 40 m stretch and
side, decoded with `zune-jpeg` (pure Rust, no C dependency):

| Course | Stretches | With a wall |
|---|---:|---:|
| Bielersee | 73 | 34 |
| Gurtenstrasse | 11 | 2 |
| Kirchenfeldbrücke | 8 | 7 |
| London cycleway | 9 | 8 |

On the Bielersee course the walls came out concrete 25, plaster 4, stone 2 and unknown 3; the
ground floor showed a plain wall in 23 stretches, a garage door in 8 and an awning in 3, and
was not visible in 39.

The extractor reduces a picture to a 64 by 48 grid, finds the largest even, upright, solid
region as the facade, and reads its colour, storeys and material from that region only. It
keeps no pixel: the facts above are all that leaves. City streets and bridges yield a wall in
most stretches; a country road yields one where houses stand close to it, which is the
expected limit of a photograph looking along the road.

The thumbnail and fact caches (`imagery-thumbnails-v1`, `imagery-facts-v5`) are keyed by image
id, so a second run downloads and decodes nothing. Thumbnails are untrusted input: a body over
4 MiB is refused before it is buffered, and the decoder refuses pictures over 2048 pixels on a
side.

## Consequences

- Street imagery costs nothing but the rider's token and a few thousand requests per course;
  the cache makes re-preparation free.
- Coverage decides usefulness: step 2 measures it on the authors' courses before step 3 spends
  effort on the extractor. A country with no imagery keeps today's look.
- The world file grows by a few hundred bytes per building at most, in an extension older
  releases ignore.
- The imagery is credited wherever the facts are shown, and the licence text ships with the
  notices.

## Open questions

- Whether hint-derived worlds may be sold is the maintainer's call, on the reasoning above.
  Until it is made, the extension is written for every course and the paid-world pipeline
  (ADR 0043) does not seal a world that carries it.
- How much the facts improve the look, and whether the simple extractor is good enough on
  streets where a facade is partly hidden, is what step 4's render review decides.
