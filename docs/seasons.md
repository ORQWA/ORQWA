# Seasons

Season is a 3D ride option, beside time of day and weather (#350, R154). The Rust domain
owns the available season names; storage remembers the selection in `computer.toml`.
Missing or unknown future names fall back to Summer without losing other settings.
Both options panels use the same control and restore the saved choice.

Summer preserves the green broadleaf, conifer, grass and distant-canopy palette and the
existing 2,400 m temperate snow line (4,500 m in the tropics). Rust also supplies the
1,200 m evergreen lowland ceiling used by seasonal materials. Choosing a season applies presentation to
the same course, including kept worlds: neither the generator nor stored format changes.
Season does not change trainer resistance, ride speed or the map.

Summer keeps a stable 10% of the course's existing flower clumps in bloom and replaces the
others with full grass tufts at the same transforms. Rust filters the presentation buffers
when uploading chunks; switching season refreshes them without changing the kept world.

Render fixed route views with `SEASON=Summer scripts/render-views.sh` in the dev container.
The same cameras and quality preset allow seasonal comparisons. Check road height and
overviews, distant woods and Low as well as Medium/Ultra. Native GPU frame time remains
the performance evidence; software Vulkan checks appearance and shader compilation.

`scripts/render-seasons.sh` renders the real materials on a fixed set of trees and flowers
at 700 m, without preparing a course. Set `SEASON` and `OUT_DIR` inside the container.
Pair these close comparisons with [Summer on the Gurten](images/seasons/summer-climb.jpg),
the [hairpin](images/seasons/summer-hairpin.jpg) and [Low](images/seasons/summer-low.jpg).
