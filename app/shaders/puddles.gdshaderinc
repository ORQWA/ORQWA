// Puddles in rain (#103), for the road and the streets: polygonal patches where water stands —
// cells of a pattern laid in world space, low-poly like the ground — only where the surface is
// flat, the sky's colour with a soft sheen. Needs `world_xz` (the fragment's position east and
// south in metres) and the world-space normal.

// The sky's colour at the horizon, which puddles show.
uniform vec3 puddle_color : source_color = vec3(0.839, 0.925, 0.894);
// 0 dry, 1 raining: how many cells hold water.
uniform float puddles : hint_range(0.0, 1.0) = 0.0;

// Cells are about this wide (metres); this share of them hold water when it rains.
const float PUDDLE_CELL = 1.6;
const float PUDDLE_SHARE = 0.12;

// Two numbers in [0, 1) for the cell at whole-metre-cell `p`. An integer hash (PCG 2D): the usual
// fract(sin(…)) loses all precision tens of kilometres from the origin, where some GPUs return 0
// for every cell, and every cell held water on dry roads (#229).
vec2 puddle_hash(vec2 p) {
	uvec2 v = uvec2(ivec2(p)) * 1664525u + 1013904223u;
	v.x += v.y * 1664525u;
	v.y += v.x * 1664525u;
	v ^= v >> 16u;
	v.x += v.y * 1664525u;
	v.y += v.x * 1664525u;
	v ^= v >> 16u;
	return vec2(v >> 8u) / 16777216.0;
}

// 1 in a puddle at `xz` on a surface whose world normal is `normal`, else 0, edges a pixel soft.
// `edge` is 0 along a way's middle and 1 at its edges, where water collects.
float puddle(vec2 xz, vec3 normal, float edge) {
	if (puddles <= 0.0) {
		return 0.0;
	}
	vec2 scaled = xz / PUDDLE_CELL;
	vec2 cell = floor(scaled);
	vec2 local = scaled - cell;
	float nearest = 8.0;
	float second = 8.0;
	vec2 owner = cell;
	for (int y = -1; y <= 1; y++) {
		for (int x = -1; x <= 1; x++) {
			vec2 offset = vec2(float(x), float(y));
			float d = length(offset + puddle_hash(cell + offset) - local);
			if (d < nearest) {
				second = nearest;
				nearest = d;
				owner = cell + offset;
			} else if (d < second) {
				second = d;
			}
		}
	}
	float share = PUDDLE_SHARE * puddles * mix(0.3, 1.7, edge);
	float wet = puddle_hash(owner + 17.0).x < share ? 1.0 : 0.0;
	// Inset from the cells' edges, so neighbouring puddles stay apart.
	float gap = second - nearest;
	float soft = fwidth(gap) + 1e-4;
	float inside = smoothstep(0.1 - soft, 0.1 + soft, gap);
	// Water runs off slopes; it stands where the surface is level.
	float level = smoothstep(0.992, 0.998, normal.y);
	return wet * inside * level;
}
