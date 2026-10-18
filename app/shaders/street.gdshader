// Streets, tracks and paths of the map around the route (R86, ADR 0019): textured asphalt for
// paved streets, gravel for tracks and paths, with a strip of grass down the middle of tracks
// where wheels do not run; wet with puddles in rain. UV.x runs 0–1 across, UV.y is the
// distance along in metres.
shader_type spatial;

#include "res://shaders/ground.gdshaderinc"

uniform sampler2D surface_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D surface_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D grass_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform float surface_tile = 2.5;
uniform float surface_roughness = 0.9;
// 1 for tracks: grass in the middle where wheels do not run.
uniform float middle_grass : hint_range(0.0, 1.0) = 0.0;
uniform float wetness : hint_range(0.0, 1.0) = 0.0;

#include "res://shaders/puddles.gdshaderinc"

varying vec3 world_normal;
varying vec2 world_xz;

void vertex() {
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	world_xz = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xz;
}

void fragment() {
	// Laid from above like the ground: asphalt and gravel look alike in every direction, and the
	// draped junctions have no meaningful coordinates across.
	vec2 uv = ground_tiles(world_xz, surface_tile);
	vec3 surface = texture(surface_albedo, uv).rgb;
	vec3 detail = ground_unpack(texture(surface_normal, uv));
	float off_centre = abs(UV.x - 0.5);
	float ragged = (ground_noise(vec2(UV.y * 0.8, 1.0)) - 0.5) * 0.06;
	float middle = middle_grass * (1.0 - smoothstep(0.1, 0.16, off_centre + ragged));
	vec3 grass = texture(grass_albedo, ground_tiles(world_xz, 3.0)).rgb;
	vec3 albedo = mix(surface, grass, middle);
	vec3 normal = ground_bend(world_normal, detail, vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), 1.0 - middle);
	float wet = wetness * (1.0 - middle);
	albedo *= 1.0 - 0.35 * wet;
	float roughness = mix(mix(surface_roughness, 0.92, middle), 0.3, wet);
	float water = puddle(world_xz, world_normal, clamp(off_centre * 2.0, 0.0, 1.0)) * (1.0 - middle);
	ALBEDO = mix(albedo, puddle_color * 0.6, water * 0.6);
	ROUGHNESS = mix(roughness, 0.08, water);
	SPECULAR = mix(0.35, 0.5, max(wet, water));
	NORMAL = normalize((VIEW_MATRIX * vec4(normalize(mix(normal, world_normal, water)), 0.0)).xyz);
}
