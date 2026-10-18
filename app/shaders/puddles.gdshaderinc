// Puddles in rain (#103, ADR 0019), for the road and the streets: irregular patches where water
// stands — two layers of smooth noise laid in world space, cut at a level that lets more of
// them through as it rains harder — only where the surface is flat, mirroring the sky. Needs `world_xz` (the fragment's position east and
// south in metres) and the world-space normal.

// The sky's colour at the horizon, which puddles show.
uniform vec3 puddle_color : source_color = vec3(0.839, 0.925, 0.894);
// 0 dry, 1 raining: how many cells hold water.
uniform float puddles : hint_range(0.0, 1.0) = 0.0;

// Puddles are about this wide (metres); this share of the surface holds water when it rains.
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

// Smooth noise in [0, 1) at `p`, from the hash of the lattice round it.
float puddle_noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(
		mix(puddle_hash(i).x, puddle_hash(i + vec2(1.0, 0.0)).x, u.x),
		mix(puddle_hash(i + vec2(0.0, 1.0)).x, puddle_hash(i + vec2(1.0, 1.0)).x, u.x),
		u.y
	);
}

// 1 in a puddle at `xz` on a surface whose world normal is `normal`, else 0, edges a pixel soft.
// `edge` is 0 along a way's middle and 1 at its edges, where water collects.
float puddle(vec2 xz, vec3 normal, float edge) {
	if (puddles <= 0.0) {
		return 0.0;
	}
	vec2 scaled = xz / PUDDLE_CELL;
	float field = puddle_noise(scaled) * 0.7 + puddle_noise(scaled * 2.7 + 31.0) * 0.3;
	float share = PUDDLE_SHARE * puddles * mix(0.3, 1.7, edge);
	// The noise's values bunch round the middle: this cut lets about `share` of it through.
	float cut = 0.5 + 0.32 * (1.0 - 2.0 * share);
	float soft = fwidth(field) + 1e-4;
	float inside = smoothstep(cut - soft, cut + soft, field);
	// Water runs off slopes; it stands where the surface is level.
	float level = smoothstep(0.992, 0.998, normal.y);
	return inside * level;
}
