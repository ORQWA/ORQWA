# Screenshots

Current app screens and generated scenery, captured from the application with a simulated
trainer and sample ride history. Open an image for its full size. The phone-remote browser
screens remain in the [remote guide](https://github.com/ORQWA/ORQWA/wiki/Devices-and-riding#remote).

## Riding and training

| Riding | Training |
|---|---|
| ![Chase camera](images/app/ride.jpg) | ![Heart-rate workout](images/workouts/heart-rate-hold.png) |
| ![First-person camera](images/app/ride-first-person.jpg) | ![Structured workout](images/workouts/structured-ride.png) |
| ![Overview camera](images/app/ride-overview.jpg) | ![Planned workouts](images/workouts/planned.png) |
| ![Video course](images/readme/video-course.jpg) | ![Workout editor](images/workouts/editor.png) |

Video footage: “Le petit Menglonnais” by [Fred Van Gestel](https://cycling.vangestel.online/indoor/videos/france/le-petit-menglonnais/),
[CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/). That screenshot has the
same licence; the footage is used only in documentation.

## Courses, setup and analysis

| Before riding | After riding |
|---|---|
| ![Course library](images/app/courses.png) | ![Ride summary](images/app/summary.png) |
| ![Course detail](images/app/course.png) | ![Ride history](images/app/history.png) |
| ![Connected devices](images/issues/devices-readings.png) | ![Power curve and profile](images/app/analysis.png) |
| ![Rider profile](images/app/profile.png) | ![Fitness and fatigue](images/app/analysis-fitness.png) |
| ![HUD editor](images/app/hud-editor.jpg) | ![Weekly training](images/app/analysis-weekly.png) |
| ![Graphics settings](images/app/settings-graphics.png) | ![Progress over time](images/app/analysis-progress.png) |

## Cities and woodland

Graphics quality controls the forest's density and distance; automatic tree LODs retain
nearby detail and use simpler distant canopies and shadows. These matching views show
the lowest and highest presets. [Tree settings](https://github.com/ORQWA/ORQWA/wiki/Settings#trees-and-forests)
apply from the next ride without regenerating the course.

| Low | Ultra |
|---|---|
| ![Fewer nearby trees](images/views/forest-low.jpg) | ![Dense woods drawn farther](images/views/forest-ultra.jpg) |

| City streets | Woodland |
|---|---|
| ![City paving and sidewalks](images/views/city-street.jpg) | ![Wooded hairpin](images/views/hairpin-chase.jpg) |
| ![City overview](images/views/city-overview.jpg) | ![Forest floor and ferns](images/views/forest-floor.jpg) |
| ![City bench](images/views/city-benches.jpg) | ![Hairpin overview](images/views/hairpin-drone.jpg) |

## Rocks and cliffs

Broad irregular fractures remain visible at overview distance; mineral detail comes from
a credited CC0 rock scan. Kept worlds receive the appearance without regeneration.

| Cliff overview | Closer rock faces |
|---|---|
| ![Fractured cliff](images/issues/rock-cliff-overview.jpg) | ![Rock faces on a wooded slope](images/issues/rock-cliff-close.jpg) |

Repeat with `VIEWS="rock-cliff-overview rock-cliff-close"` in `scripts/render-views.sh`.
See the [boulder gallery](../art/README.md#vegetation).

## Standard route views

The fixture routes exercise villages, climbs, junctions, bridges, tunnels and weather. Their
fixed camera positions make it possible to compare future changes. These are the current
renders; historical before/after images remain with their original reviews.

| Villages and roads | Lake and junction views |
|---|---|
| ![Village](images/views/village-chase.jpg) | ![Lakeside road](images/views/lake-chase.jpg) |
| ![Climb](images/views/climb-chase.jpg) | ![Lake overview](images/views/lake-drone.jpg) |
| ![Evening village](images/views/village-evening.jpg) | ![Rain](images/views/lake-rain.jpg) |
| ![Morning climb](images/views/climb-morning-drone.jpg) | ![Morning lake](images/views/lake-morning.jpg) |
| ![Morning valley](images/views/valley-morning.jpg) | ![Lakeside village](images/views/lakeside-village.jpg) |
| ![Shops](images/views/shops-side.jpg) | ![Building block](images/views/block-chase.jpg) |
| ![Junction](images/views/junction-chase.jpg) | ![Junction overview](images/views/junction-drone.jpg) |
| ![Railway tunnel](images/views/railway-tunnel-drone.jpg) | |

| Bern | Oberalp |
|---|---|
| ![River](images/views/river-drone.jpg) | ![Road tunnel](images/views/oberalp-tunnel-chase.jpg) |
| ![Old town](images/views/oldtown-drone.jpg) | ![Railway](images/views/oberalp-railway-chase.jpg) |
| ![Bridge](images/views/bridge-chase.jpg) | ![Mountain hairpins](images/views/oberalp-hairpins-drone.jpg) |
| ![Roundabout](images/views/roundabout-drone.jpg) | ![Alpine lake](images/views/oberalp-lake-drone.jpg) |
| ![Bridge from the side](images/views/bridge-side.jpg) | ![Tunnel portal](images/views/oberalp-portal-drone.jpg) |
| | ![Mountain village](images/views/oberalp-village-chase.jpg) |

Every building, plant, roadside prop and rider also has a refreshed image in the
[model galleries](../art/README.md#gallery). See [Development](development.md#refreshing-screenshots)
for the capture and review workflow.
