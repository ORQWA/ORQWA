# Torqa — Requirements

Outcome of the initial requirements-elicitation session (2026-10-02, ~70 questions), extended
2026-10-03 with course previews, start page and realistic graphics (R36–R47) and in-ride
settings, ride names, per-rider HUD layouts, UI quality and direct HUD placement (R48–R54),
plus overlay mode and constant-power / heart-rate workouts (R55–R56), refined 2026-10-06: an
overlay over normal windows only, a Workouts tab and Di2 shifting (R57–R59), extended 2026-10-12
with uploads by one click or automatically (R60) and the analysis of a rider's power across
their rides (R61–R65, #217), and 2026-10-13 with an estimated time to the finish (R66, #231)
and a searchable, sortable and grouped course gallery with the countries courses pass
(R67–R68, #230), and 2026-10-14 with the minimap's land around the whole route (R70, #236)
and power meters and the cadence's source (R71–R72) and a ride chart with cadence and lines to
hide (R73).
Requirement IDs (`R<n>`) are referenced from code, tests and ADRs. The list is **append-only**:
existing requirements are never reworded or renumbered; changes are new requirements that state
what they supersede, and the old entry only gets a short pointer.

## Vision

Torqa is a modern, **offline-first**, open-source indoor cycling app. Import a GPX route and ride it
on a smart trainer (primary: Wahoo KICKR Core 2) through a generated semi-realistic 3D world
*(stylized since 2026-10-05, R44)*, a synced ride video, or street-level imagery.

## General

| ID | Requirement |
|---|---|
| R1 | macOS (Apple Silicon M1+) first; Windows and Linux later. Stack must be cross-platform from day one. |
| R2 | Audience: the author + friends. Open source under **GPL-3.0**. **Zero budget**: free/open data only; users bring their own API keys for paid services. |
| R3 | Offline-first: a prepared ride works without internet. Online extras (DEM correction, world data download, Street View, uploads) are used when connected. |
| R4 | Extensibility is a core principle: interface-driven plugins for route importers, devices, shift inputs, uploaders, ride modes and workout parsers. |

## Devices

| ID | Requirement |
|---|---|
| R5 | Bluetooth LE **FTMS** (MVP). **ANT+ FE-C** later (no ANT+ dongle available for testing yet). *(ANT+ FE-C dropped 2026-10-15.)* |
| R6 | Direct-drive trainers first (KICKR Core 2). Architecture generic enough for any FTMS/FE-C trainer and smart bikes. |
| R7 | Sensors: BLE heart-rate strap. Zwift Click as shift input (no official protocol → reverse-engineered BLE, isolated and optional); also OpenBikeControl and keyboard/gamepad as shift inputs. *(Shift inputs: R59; power meters: R71; cadence: R72.)* |
| R8 | Trainer modes: **SIM** (slope simulation, MVP), **ERG** (target power), **resistance level**; free ride. |
| R9 | Drivetrain: real cassette (trainer handles shifting naturally) and Zwift Cog (app-side **virtual gears** — resistance offset via FTMS). Post-MVP. See ADR 0003. |
| R10 | Software **fake trainer** for development and automated tests. |
| R71 | Extends R7 (2026-10-14): a Bluetooth **power meter** (Cycling Power Service — power pedals such as the Favero Assioma, crank and spider meters) beside the trainer. While it reports, **its power is the rider's**: for the speed, the HUD, the recording and the analysis; its **cadence** too where it reports one (worked out from its crank revolutions), else the trainer's *(choice of source: R72)*. If it drops, the trainer's power and cadence count until it is back, and if the trainer drops the meter's power still moves the rider. It is chosen in Devices & Settings, remembered and reconnected like the other devices (R41, R69). The trainer still holds ERG targets by its own measurement; matching them to the meter is a later step *(matching dropped 2026-10-15)*. |
| R72 | Extends R71 (2026-10-14): the rider chooses **where the cadence comes from**, as Zwift's pairing screen does: **Automatic** (a cadence sensor if one is connected, else the power meter, else the trainer), the **trainer**, the **power meter**, or a Bluetooth **cadence sensor** (Cycling Speed and Cadence Service). While the chosen device reports no cadence, the others stand in, in the automatic order. The choice is kept for the installation; a chosen sensor is remembered and reconnected like the other devices. |

## Routes

| ID | Requirement |
|---|---|
| R11 | Import **GPX** (MVP); importer interface allows TCX/FIT/KML later. Routes are used as-is (no route editor). |
| R12 | Elevation: when online, correct with a terrain model (Mapterhorn, AWS Terrain Tiles fallback — ADR 0005) and smooth; offline, use cached tiles or GPX elevation with smoothing. |

## Physics

| ID | Requirement |
|---|---|
| R13 | Full physics model: rider + bike mass, CdA, Crr, gradient, air density; optional wind and drafting. |
| R14 | Trainer difficulty adjustable 0–100 % (scales the felt grade; virtual speed uses the real grade). |
| R15 | Descent behaviour configurable per ride: simulated coasting vs. clamp to flat. |

## Ride modes

| ID | Requirement |
|---|---|
| R16 | **3D world (MVP)**: semi-realistic style *(now stylized, R44)*. World source user-selectable, default **hybrid** (real DEM + OpenStreetMap when available, procedural fallback). Weather/time-of-day presets. Cameras: first person, chase, drone — user's choice. Optional cadence-synced avatar. 60 fps on an M1 integrated GPU. *(Performance superseded by R43; visual target extended by R44–R47.)* |
| R17 | **Video**: own GoPro/Insta360 footage, downloaded videos, and plain videos without GPS, and Tacx RLV courses (`.rlv` + `.pgmf`, added by #42). Auto-sync from embedded GPS (GoPro GPMF) with manual sync-point fallback. 1080p target; higher resolutions transcoded down on import. Variable playback speed with frame blending. |
| R18 | **Street imagery**: Google Street View (online only, user's API key, no caching per Google ToS) and Mapillary. Smooth crossfade/zoom transitions between panoramas. *(dropped 2026-10-15.)* |
| R19 | Overlays: 2D minimap (OpenStreetMap tiles, cached for offline) and elevation profile with current position. *(Land around the whole route, map style: R70.)* |
| R20 | Ghosts / pacers, selectable per ride: own previous best, fixed W/kg or power pacer, ghost from a GPX/FIT activity. |
| R70 | Extends R19 (2026-10-14, #236): the minimap shows the **land around the whole route**, as the course's map does, for 3D and video courses alike, offline from the course file; the close view shows the route's corridor in detail over it, for video courses too. Each rider chooses whether it is drawn **colored** or **dark**, like the course cards. |

## Training & UX

| ID | Requirement |
|---|---|
| R21 | Structured workouts later, but architected for now: ZWO, ERG/MRC, FIT workouts, built-in editor. No multi-week training plans. *(Constant-power and heart-rate workouts: R56.)* |
| R22 | Rider profile: weight, FTP, max HR, power and HR zones, built-in FTP test. **Multiple user profiles** per installation. |
| R23 | Fully customizable HUD widgets: power (instant/3 s/10 s), cadence, HR, speed, distance, time, elevation gain, current & upcoming gradient, W/kg, NP, TSS, kJ, zone. *(Per-rider layout and editor: R51. Estimated time to go: R66.)* |
| R24 | Metric and imperial units. English UI first, i18n-ready from day one. |
| R25 | Single window / fullscreen, external monitor / TV. Phone companion app is a possible future extension. *(Overlay mode: R55.)* |
| R26 | Audio, selectable per ride: ambient sounds, original video audio, music app control. |
| R27 | Auto-detected climbs with personal KOM-style times; personal records per route. Multiplayer possibly later — keep simulation state separable from rendering. |
| R66 | Extends R23 and R40 (2026-10-13, #231): an **estimated time to the finish**. The course page shows how long the route takes the rider at 75 % of their FTP, with their weight and the descents as chosen in the ride options; the HUD can show the **time to go**, which follows the rider's average power so far (moving over to it from 75 % of FTP during the first ten minutes). Both ride the rest of the route with the ride's own physics, so climbs and descents count as they will. *(Shown to the nearest 5 minutes: R67.)* |

## Data

| ID | Requirement |
|---|---|
| R28 | Export activities as **FIT** (MVP). |
| R29 | Uploaders behind a common interface: Strava, intervals.icu, Garmin Connect, TrainingPeaks, Komoot. (Garmin/TrainingPeaks/Komoot require partner approval → best effort.) |
| R30 | Local storage in a **configurable data directory** that may live in a synced folder (e.g. Nextcloud) → sync-safe design, see ADR 0002. |
| R31 | Detailed ride history and analysis: power/HR/cadence charts, zones, PRs. *(Across rides: R61–R65. Chart lines to hide: R73.)* |
| R73 | Extends R31 (2026-10-14): a ride's chart in the summary and the history also draws **cadence**, and each key of its legend (power, heart rate, cadence, elevation) **hides or shows** that line when clicked, the hidden key dimmed; the others keep their own scales. The choice holds while browsing rides; keys of data the ride lacks are left out. |

## Courses

| ID | Requirement |
|---|---|
| R32 | **Course files** (`.tqc`): a prepared course is saved as one self-contained file that can be stored, copied, shared (mail, Nextcloud, USB, websites) and imported again, fully offline. See ADR 0007. |
| R33 | A course contains the original GPX, the processed route (elevations, bridges/tunnels), the corridor's terrain heights and OpenStreetMap features, the **pre-built 3D world**, a preview image and a manifest (format and generator versions, name, stats, attribution). |
| R34 | **Course library**: a `courses/` folder in the data directory (may be synced, R30). The app lists its courses to pick from, imports `.tqc` files into it and saves newly prepared routes as courses. |
| R35 | Courses built by an older generator still ride instantly from their stored world; they can be rebuilt from the stored inputs with a newer generator. Video courses (R17) use the same format and reference their video file instead of embedding it. |
| R36 | **Course screenshots**: every course carries a small screenshot gallery stored in the `.tqc`. 3D courses render it automatically from the generated world when the course is built (e.g. start, highest point, scenic spots); video courses take frames from the video at fixed distances. The user can pick which image is the **cover**. *(dropped 2026-10-15: the path card, R37, is the course's picture.)* |
| R37 | **Path card**: every course also has a route image: the GPX path in the logo blue **`#2EB0FF`** with start/finish markers, drawn on the course's own map (#192): the land around the whole route in black and greys with its waters, main roads and the names of its towns (#203), kept in the course file as a picture when the course is prepared; or on black for a course without a place. The elevation profile is shown on the course detail page only (review of #28). |

Sharing is file-based for now; a built-in online catalog may follow later.

## App structure & ride flow

| ID | Requirement |
|---|---|
| R38 | A real **start page** separate from the ride view, with a top tab bar: **Courses**, **History**, **Profile**, **Devices & Settings**. The ride view only shows the ride. *(Workouts tab: R58; Analysis tab: R61.)* |
| R39 | **Courses tab**: gallery overview of the course library (R34) — each card shows the cover or path card, distance, elevation gain, a small map, the course type (3D / video) and other key stats. Importing a GPX / preparing a new course starts here. *(Search, sort, filters and groups: R67.)* |
| R40 | **Course detail page** (opened from a card): screenshot gallery *(dropped 2026-10-15, with R36)*, path card, map, elevation profile, stats, personal records on the course, and the per-ride options (difficulty, descent mode, weather, time of day, camera, ghost) → **Ride**. |
| R41 | The last-used trainer and sensors **reconnect automatically** in the background at app start; the user is only prompted if that fails when a ride starts. |
| R42 | At the end of a ride a **summary screen** (stats, charts, PRs, save/discard, upload) is shown, then the app returns to the start page. |
| R48 | **In-ride settings dialog**: the ride view's controls (today's bottom buttons: camera, HUD customization) move into one settings dialog opened from a single button or key. It is shared: the same dialog and settings (camera, HUD, difficulty, descent mode, weather, time of day, …) are used from the course detail page (R40), so options are set the same way before and during a ride. |
| R49 | The in-ride dialog also ends the ride: **finish and save** (→ summary, R42) or **abort without saving**, the latter after a confirmation. |
| R50 | **Ride names**: every ride in the history has a name, defaulting to the course name and date. It can be set on the summary screen (R42) and renamed later in the history; it is used as the activity name in FIT export and uploads. Files on disk keep stable identifiers, so renaming is sync-safe (ADR 0002). |
| R51 | **HUD per rider**: each rider profile has its own HUD layout (R23), editable in the rider's profile settings. The editor shows a separate HUD preview, and widgets are added, removed and **reordered by drag and drop**. It is the same editor as the HUD section of the in-ride settings dialog (R48), so changes made during a ride are saved to the rider's layout. *(Placement refined by R54.)* |
| R52 | **Modern, coherent UI** in a *sleek minimal* style: slim typography, thin translucent panels, generous spacing, few controls visible at once; during a ride the 3D view dominates. All screens and dialogs share one design system (type scale, spacing, colours, corner radii, control styles); no unstyled stock widgets. Course summaries (course cards, detail page, the loaded-course panel) are designed as polished cards with a clear hierarchy of name, path card (R37), key stats and elevation profile. |
| R53 | **Stable, responsive layouts**: loading a course or other content never changes font sizes or lets the layout grow or jump. Resizable windows and dialogs (e.g. rider settings) reflow and resize their inputs with the window instead of leaving them fixed in the middle. Layouts work from 1280×720 up to 4K / TV (R25). |
| R54 | Refines R51: HUD widgets are arranged **directly in the HUD preview**, not only in a list. A widget is dragged from the widget list straight to the position where it should appear, moved around within the HUD, and dragged out to remove it; a drop-position indicator shows where it will land. |
| R67 | Extends R39 (2026-10-13, #230): the Courses tab **searches** courses by name, **sorts** them by name, length, climbing, estimated time (R66), date added or last ridden (each order starting in its natural direction, which a button turns round), **filters** them with chips for the type (3D / video), the terrain (flat, rolling, hilly, mountainous by climbing per km: under 5, 5–10, 10–20, from 20 m) and length bands with round edges in the rider's units (25 / 50 / 100 km or 15 / 30 / 60 mi), and **groups** them by type, terrain or length. Chips of one kind add up, different kinds narrow down. Each card shows the rider's **time estimate** at 75 % of their FTP, worked out from the profile the course file keeps (so it may differ a little from the course page's); both show it to the nearest 5 minutes, which supersedes R66's exact figure on the course page. The order and grouping are remembered; search and chips are not. *(Countries: R68.)* |
| R68 | Extends R67 (2026-10-13, #230): every course knows the **countries its route passes**, in the order it reaches them, from the OpenStreetMap map tiles Torqa already uses (OpenFreeMap): which side of the nearest border between countries the route lies on, a country counting only for at least 200 m of route. Their names come from the tiles' country labels and are kept in the course file, so shared courses name them offline, in the rider's language. Where the tiles leave a border's side without its country (in 2026 often for Germany, Austria and Poland), that stretch is in no known country; a video without a place has none. The gallery **groups** by country (a course under each of its countries), offers **country chips** once the library spans two or more (plus "No country"), **searches** country names, and each card lists the countries by code with their names on hover. Courses prepared before, or offline, learn their countries in the background when Torqa starts. |
| R69 | Extends R41 and R48 (2026-10-13): devices that drop **reconnect by themselves** once back in reach, also after a long pause — on every platform, the device is looked up again rather than its stale link retried. The in-ride settings dialog gets a **Devices** tab: the trainer, heart-rate strap and shifter, each connected or not, and a **Reconnect** for those that are not (e.g. a strap not found at start); connected devices keep their link. A strap that is gone records no heart rate rather than its last value. |
| R76 | Extends R41 and R69 (2026-10-16): a device **connects the moment it is chosen** under Devices & Settings, not when the ride starts, and choosing none lets the device go and forgets it. Each row has a **search of its own kind**: it looks for trainers, power meters, cadence sensors, straps or shifters alone, chooses the nearest one found and connects it, or says *Not found*; a **scan for all devices** stays as the fallback. Each device row shows **how it is doing**: amber *Connecting* while the link is made (also for the devices used last, shown by name while they reconnect at start, and *Reconnecting* after a drop), green *Connected* once the device reports, red *Not found* when the scan missed a device used last (choosing it looks again) and red *Failed* when connecting was refused. The ride settings show the same states. |
| R77 | Extends R75 (2026-10-16): the **Zwift Click v2** — the two pods sold with the Zwift Cog since 2025, a right one with +, Y, Z, A and B, a left one with − and four arrows — shifts the virtual gears directly over Bluetooth too. Its pods are two Bluetooth devices, found and connected as **one shifter**: the right pod first, the left with it when in reach, dropped and reconnected together. Each pod speaks the plain protocol of the Zwift Ride (a `RideOn` handshake, then one bit map of its buttons; ADR 0003), kept awake by the handshake every few seconds. The buttons are 1–10 of the shifter's assignments (#139, R66): + is 1 and − 2, as on the original Click, then Y, Z, A, B, ↑, ↓, ← and →; a press and a hold each, no double press. What any shifter's buttons do is set in a **dialog** opened by the pen beside the shifter's row under Devices & Settings, so the tab stays one screen whether a shifter has two buttons or ten. Zwift **locks the left pod** to its own app: it tells of its buttons only within about a day of a Zwift session (log in, connect it, close Zwift), which Torqa cannot replace; the right pod is never locked, so + and a lettered button given "shift down" work without Zwift at all. No pod was checked by Torqa: the protocol is as BikeControl, QZ and MaximumTrainer drive it, and the driver stays isolated and optional. The Play and the Ride stay out. |

## Graphics

| ID | Requirement |
|---|---|
| R43 | Supersedes the R16 performance target. **Quality presets** Low / Medium / High / Ultra. An M1 (base) integrated GPU holds **60 fps on Medium**; High/Ultra target stronger Apple GPUs and, later, discrete GPUs on Windows/Linux. |
| R44 | **Visual target** *(revised 2026-10-05, [ADR 0011](adr/0011-stylized-look.md): stylized instead of realistic)*: a **stylized, faceted low-poly** world and riders in a **soft pastel palette** — a bright, toy-like diorama of the real place (real terrain, roads and buildings), not a photograph. No racing UI or racing accessories (banners, arches, crowds). |
| R45 | Look priorities, all four: **vegetation** (faceted trees and bushes, grass and flowers moving in the wind); **terrain & road** (faceted ground coloured by land cover, clean roads and tracks, verges); **lighting & atmosphere** (warm soft sun, soft shadows, ambient occlusion, pastel gradient sky, low-poly clouds, haze toward distant terrain); **rider & bike**. |
| R46 | **Rider**: stylized, faceted female or male rider (user's choice) of natural proportions (not toy-like, ADR 0011) with natural, cadence-driven pedaling and body motion on a stylized bike. Customization (bikes, kits, …) may follow; the rider and bike are kept modular for it. |
| R47 | **Assets** must stay redistributable as open source with the GPL-3.0 project: own work, CC0, CC-BY or CC-BY-SA, credited in a credits file. Models are produced by documented Blender Python scripts (riders scripted from scratch and from CC0 stylized bases); scripts and exported models are both committed. See ADR 0009 and ADR 0011. |

## Workouts & overlay

| ID | Requirement |
|---|---|
| R55 | **Overlay mode**: Torqa can be started as an overlay only — a transparent, always-on-top HUD window over any other app, so a video can be streamed full-screen while the rider's HUD (R51) shows on top. It also stays visible over full-screen apps. The overlay can be moved and resized anywhere on screen, and its position and size are remembered. Only workouts can be selected in the overlay (no courses, no 3D world). An overlay session is recorded as a ride (history, FIT). macOS first; Windows and Linux later (on Wayland only where the compositor allows always-on-top). *(Refined by R57: no full-screen apps; entered from any ride.)* |
| R56 | **Constant-power and heart-rate workouts** as additional workout modes (alongside structured workouts, R21): **constant power** (ERG at a target, e.g. 200 W) and **heart-rate hold** — the target is a zone (e.g. Zone 3, aiming at the middle of the zone) or a specific bpm, and the target power is adjusted continuously from the measured heart rate. The controller stays within user-set minimum and maximum power and ramps gently, since heart rate lags power by 30–60 s. |
| R57 | Refines R55 (2026-10-06): the overlay stays on top of **normal (windowed) apps only**; showing it over full-screen apps would need native macOS window code (a new dependency and unsafe code) and is not done. The overlay is entered with a button during **any ride or workout** (a course ride goes on following its gradient, its world hidden) or by ticking **"start as overlay"** when starting a workout. |
| R58 | Extends R38 (2026-10-06): a **Workouts** tab on the start page, right after Courses. Workouts (R56, later R21) are started there, either on their own (a flat road, the HUD only) or **on a course in the 3D world** (the trainer holds the workout's power while the world goes by). |
| R59 | Supersedes R7's shift inputs (2026-10-06): the virtual gears are shifted from the **keyboard** and from **Shimano Di2** directly over Bluetooth — the buttons the rider assigns to D-Fly channels in E-TUBE shift up and down. Each channel's press, hold and double press can be given an action instead (#139): shift one or two gears, next camera, overlay on/off, or the music (play/pause, next, previous). No dependency on paid bridge apps (BikeControl needs a subscription on macOS and Android), so no OpenBikeControl-through-BikeControl; the Zwift Click (encrypted, the v2 unlocked daily in Zwift) is not supported. |

## Uploads

| ID | Requirement |
|---|---|
| R60 | Extends R29 (2026-10-12): rides go up to a rider's training platforms with **one click** from the summary or the history, or **automatically** to each account the rider set to upload by itself. An automatic upload starts when the summary is closed with **Done**, never before (the ride may still be renamed or discarded there) and never for a discarded ride. A ride goes to a platform at most once; an upload that fails leaves the ride to upload again from the history. |

## Analysis

| ID | Requirement |
|---|---|
| R61 | Extends R38 (2026-10-12): an **Analysis** tab on the start page, right after History: the active rider's power across all their rides, beyond the single ride of R31. |
| R62 | **Power curve**: the rider's best average power over every duration from 1 s to the longest ride, among the rides of a chosen period (last 6 weeks, 90 days, 12 months, all time), drawn against all rides or the period before, in W or in W/kg (with the rider's weight on each ride, where the ride kept it). A ride's summary shows its own curve over the rider's best. The durations 5 s, 1, 5, 20 and 60 min over which a ride holds the rider's best power are marked ★ like the records of R27. Simulated rides (sped up or moved along) count for none of it. |
| R63 | **Power profile** (the "power spider" of #217): a radar of the rider's best 5 s, 1, 5, 20 and 60 min of the chosen period (R62), each as a share of their best of all rides — the rim is the rider's own best, so no age, sex or reference tables are needed. |
| R64 | **Fitness, fatigue and form** over the days of the chosen period, from the rides' TSS (the performance management chart: 42- and 7-day exponentially weighted averages, form the day before's fitness minus fatigue, with form's bands), computed in Torqa offline; and **weekly bars** of the period: time, distance, climbing, TSS, number of rides, and the time in each power and heart-rate zone of the rider's zones today, as the summary counts it. |
| R65 | **Progress** over the chosen period, week by week (month by month for periods over 13 weeks): the best 1, 5 and 20 min of each, the FTP the rides were saved with and the FTP tests. And an **FTP estimated** from the best efforts of the last 90 days (the critical power of Morton's three-parameter model, or 95 % of the best 20 min where the model does not fit), shown beside the rider's FTP with a button to take it — **never applied without asking**. |
| R66 | Extends R59 (2026-10-13): a Di2 channel's press, hold or double press can also **pause and resume** the ride, make the **trainer difficulty** 10 % harder or easier (within 0–100 %, as the ride settings show), and in a workout make its **intensity** 5 % harder or easier (50–150 % of every planned power, for constant-power and structured workouts) or **skip to the next step** of a structured workout (the last one ends it). The keyboard does the same: H / E harder and easier, I / L more and less intensity, N the next step. Heart-rate holds and FTP tests keep their powers and steps: a hold would undo the change, and a test's FTP comes from its protocol. |
| R75 | Extends R59 (2026-10-15, #228): the **Zwift Click** — the original single unit with a + and a − button, sold with the Zwift Cog — shifts the virtual gears directly over Bluetooth, with no app in between. On today's firmware its protocol is plain (a `RideOn` handshake, then the buttons' states), with no encryption and no daily unlock, as checked on a unit (ADR 0003). Its + and − are buttons 1 and 2 of the shifter's assignments (#139, R66): a press and a hold can each be given an action; it has no double press, so a tap shifts the moment the button comes up. It is chosen, remembered and reconnected like a Di2. The two-pod Click v2, the Play and the Ride stay out (R59): they are unlocked daily in Zwift or send other messages, and none was checked. |

## Language and errors

| ID | Requirement |
|---|---|
| R74 | Extends R24 (2026-10-15, #261): the German interface uses the individually approved wording, informal du and Swiss ss spelling. Core failures carry a translatable explanation separately from their original technical details; filenames and device/platform names stay untouched. Details remain accessible after a ride notification fades and can be expanded without interrupting the ride. |
| R78 | Refines R43 and R55 (2026-10-16, #257): what depends on the **computer** — the graphics quality and full screen — is kept in `computer.toml` next to the cache, never in the data directory, which may be shared between computers (R30): each keeps its own choice. The **overlay's place and size** are the **rider's** (`profiles/<rider>/overlay.toml`), so they follow the rider and sync with the data. Values from before are carried over once: the overlay's place to every rider without their own, the quality and full screen to the computer that starts first; then they are gone from `settings.toml`. |

## MVP definition

Import a GPX → generate the 3D world → ride it on the KICKR Core 2 in SIM mode → save a FIT file.

## Test hardware available

KICKR Core 2, Polar H9 heart-rate strap, Garmin Venu 3 (broadcasting heart rate), Favero
Assioma Pro Duo power pedals, Shimano Di2 (RD-R8150), and a Zwift Click + Cog (the original
Click, R75). No Zwift Click v2 (R77): its driver is unchecked.
