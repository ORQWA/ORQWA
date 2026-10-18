// Shared by the ground, road and street shaders (R89, ADR 0020): tileable PBR textures baked in
// Blender (art/textures), sampled so they keep their size in metres, read without visible
// repeats, and bend the surface's own smooth normal by their normal maps.

// A number in [0, 1) for the lattice point `p`. An integer hash (PCG 2D), as for the puddles:
// fract(sin(…)) loses its precision tens of kilometres from the origin (#229).
float ground_hash(vec2 p) {
	uvec2 v = uvec2(ivec2(p)) * 1664525u + 1013904223u;
	v.x += v.y * 1664525u;
	v.y += v.x * 1664525u;
	v ^= v >> 16u;
	v.x += v.y * 1664525u;
	return float(v.x >> 8u) / 16777216.0;
}

// Texture coordinates in tiles for world positions `p`, wrapped first: tens of kilometres out,
// dividing the raw position would leave too few bits for a texel.
vec2 ground_tiles(vec2 p, float tile) {
	return mod(p, 256.0 * tile) / tile;
}

vec3 ground_tiles3(vec3 p, float tile) {
	return mod(p, 256.0 * tile) / tile;
}

// Smooth value noise, 0–1: large patches that keep tiled textures from repeating visibly.
float ground_noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(
		mix(ground_hash(i), ground_hash(i + vec2(1.0, 0.0)), u.x),
		mix(ground_hash(i + vec2(0.0, 1.0)), ground_hash(i + vec2(1.0, 1.0)), u.x),
		u.y
	);
}

// A tangent-space normal from a normal map, its z rebuilt: VRAM-compressed normal maps keep
// only red and green.
vec3 ground_unpack(vec4 texel) {
	vec2 xy = texel.rg * 2.0 - 1.0;
	return vec3(xy, sqrt(max(0.0, 1.0 - dot(xy, xy))));
}

// The world normal `n` bent by tangent-space `detail`, for a texture whose u runs along `u_axis`
// and whose v runs along `v_axis` in the world. Blender bakes OpenGL normal maps (green along
// the image's up), and Godot's v runs down the image, so green bends against `v_axis`.
vec3 ground_bend(vec3 n, vec3 detail, vec3 u_axis, vec3 v_axis, float strength) {
	vec3 t = normalize(u_axis - n * dot(n, u_axis));
	vec3 b = normalize(v_axis - n * dot(n, v_axis));
	return normalize(n * detail.z + (t * detail.x - b * detail.y) * strength);
}

// The same for a surface with its own texture coordinates (`uv`, in metres) such as a road
// along its curve: the axes come from how position and coordinates change across the pixel.
vec3 ground_bend_along(vec3 n, vec3 detail, vec3 position, vec2 uv, float strength) {
	vec3 dp1 = dFdx(position);
	vec3 dp2 = dFdy(position);
	vec2 duv1 = dFdx(uv);
	vec2 duv2 = dFdy(uv);
	vec3 dp2perp = cross(dp2, n);
	vec3 dp1perp = cross(n, dp1);
	vec3 u_axis = dp2perp * duv1.x + dp1perp * duv2.x;
	vec3 v_axis = dp2perp * duv1.y + dp1perp * duv2.y;
	if (dot(u_axis, u_axis) < 1e-12 || dot(v_axis, v_axis) < 1e-12) {
		return n;
	}
	return ground_bend(n, detail, u_axis, v_axis, strength);
}
