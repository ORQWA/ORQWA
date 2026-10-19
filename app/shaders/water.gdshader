shader_type spatial;
// Lakes and rivers (R89, ADR 0020): dark, clear water that reflects the sky — glossy, with the
// Fresnel effect of physically based light, so it mirrors more at a low angle — rippled by small
// waves drifting with the wind, larger swell further out. Shallow water shows a lighter green.

#include "res://shaders/ground.gdshaderinc"

uniform vec3 deep_color : source_color = vec3(0.071, 0.141, 0.153);
uniform vec3 shallow_color : source_color = vec3(0.157, 0.29, 0.282);
// How strong the ripples are.
uniform float wave_strength = 0.35;

varying vec3 world_position;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

// Height of the water's surface at `p` (metres east and south), moving with time: layers of
// noise drifting in different directions at different scales.
float waves(vec2 p) {
	float t = TIME;
	return ground_noise(p / 9.0 + vec2(t * 0.05, t * 0.03)) * 0.6
		+ ground_noise(p / 3.1 - vec2(t * 0.11, -t * 0.07)) * 0.3
		+ ground_noise(p / 1.1 + vec2(-t * 0.21, t * 0.17)) * 0.12;
}

void fragment() {
	// Wrapped, as for the ground textures: precision tens of kilometres out.
	vec2 p = mod(world_position.xz, 1024.0);
	float e = 0.08;
	float h = waves(p);
	vec2 slope = vec2(waves(p + vec2(e, 0.0)) - h, waves(p + vec2(0.0, e)) - h) / e;
	// Ripples fade with distance, where they would only shimmer.
	float near = 1.0 - smoothstep(60.0, 400.0, length(VERTEX));
	vec3 world_normal = normalize(vec3(-slope.x * wave_strength * near, 1.0, -slope.y * wave_strength * near));
	NORMAL = normalize((VIEW_MATRIX * vec4(world_normal, 0.0)).xyz);
	float facing = clamp(dot(normalize(VIEW), NORMAL), 0.0, 1.0);
	ALBEDO = mix(deep_color, shallow_color, (1.0 - facing) * 0.4);
	// Not lower: the sun on a nearly perfect mirror is a pinpoint bright enough to bloom.
	ROUGHNESS = 0.08;
	SPECULAR = 0.5;
}
