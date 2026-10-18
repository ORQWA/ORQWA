shader_type spatial;
// The road ridden (R86, ADR 0019): textured asphalt, slightly darker and smoother where wheels
// run, white markings worn by traffic — solid edge lines and a dashed centre line, anti-aliased
// so they hold in the distance — and its skirts below the edges: a narrow, ragged strip of
// gravel the asphalt frays into, then the verge's grass, mapped like the ground's so the two
// meet without a seam. Wet in rain: darker, glossier, with puddles. UV.x runs 0–1 across the
// road (beyond, the skirts), UV.y is the distance along the route in metres.

#include "res://shaders/ground.gdshaderinc"

uniform sampler2D asphalt_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D asphalt_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D asphalt_rough : hint_roughness_r, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D gravel_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D gravel_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D grass_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D grass_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform vec3 marking_color : source_color = vec3(0.85, 0.85, 0.82);
uniform float road_width = 6.0;
uniform float line_width = 0.12;
uniform float dash_length = 3.0;
uniform float dash_gap = 6.0;
uniform float asphalt_tile = 2.5;
uniform float gravel_tile = 1.5;
// 0 dry, 1 soaked: darker asphalt with a sheen.
uniform float wetness : hint_range(0.0, 1.0) = 0.0;

#include "res://shaders/puddles.gdshaderinc"

varying vec3 world_position;
varying vec3 world_normal;
varying vec2 world_xz;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	world_xz = world_position.xz;
}

// 1 inside [from, to] across the road, with edges one pixel soft.
float band(float across, float from, float to) {
	float soft = max(fwidth(across), 1e-4);
	return smoothstep(from - soft, from + soft, across) * (1.0 - smoothstep(to - soft, to + soft, across));
}

void fragment() {
	float across = UV.x * road_width;
	float along = UV.y;
	// Metres across and along, wrapped along so tens of kilometres keep their precision.
	vec2 metres = vec2(across, mod(along, 256.0 * asphalt_tile * gravel_tile));

	vec2 asphalt_uv = metres / asphalt_tile;
	vec3 asphalt = texture(asphalt_albedo, asphalt_uv).rgb;
	vec3 asphalt_detail = ground_unpack(texture(asphalt_normal, asphalt_uv));
	float asphalt_roughness = texture(asphalt_rough, asphalt_uv).r;
	// Wheel tracks: darker and polished, a metre and a half in from either edge.
	float track = band(across, 1.0, 2.1) + band(across, road_width - 2.1, road_width - 1.0);
	track *= 0.5 + 0.5 * ground_noise(vec2(across * 2.0, along / 7.0));
	asphalt *= 1.0 - 0.12 * track;
	asphalt_roughness -= 0.1 * track;

	// Markings, worn away in flecks and patches.
	float edges = band(across, 0.25, 0.25 + line_width)
		+ band(across, road_width - 0.25 - line_width, road_width - 0.25);
	float half_line = line_width * 0.5;
	float centre_line = band(across, road_width * 0.5 - half_line, road_width * 0.5 + half_line);
	float dash = mod(along, dash_length + dash_gap);
	float soft = max(fwidth(along), 1e-4);
	centre_line *= 1.0 - smoothstep(dash_length - soft, dash_length + soft, dash);
	float wear = smoothstep(0.25, 0.55, dot(asphalt, vec3(0.6)) + ground_noise(vec2(across * 3.0, along * 0.6)) * 0.5);
	float marking = clamp(edges + centre_line, 0.0, 1.0) * mix(0.55, 1.0, wear);
	vec3 albedo = mix(asphalt, marking_color, marking);
	float roughness = mix(asphalt_roughness, 0.6, marking);

	// Gravel on the skirts; the asphalt's edge frays into it.
	vec2 gravel_uv = metres / gravel_tile;
	vec3 gravel = texture(gravel_albedo, gravel_uv).rgb;
	vec3 gravel_detail = ground_unpack(texture(gravel_normal, gravel_uv));
	float outside = max(-UV.x, UV.x - 1.0) * road_width;
	float fray = (ground_noise(vec2(along * 1.5, UV.x > 0.5 ? 7.0 : 3.0)) - 0.5) * 0.12;
	float shoulder = smoothstep(-0.02, 0.02, outside + fray);
	albedo = mix(albedo, gravel * 0.85, shoulder);
	roughness = mix(roughness, 0.95, shoulder);
	vec3 detail = normalize(mix(asphalt_detail, gravel_detail, shoulder));
	vec3 normal = ground_bend_along(world_normal, detail, world_position, metres, 1.0);
	// Beyond half a metre or so the verge is grass, its edge ragged.
	vec2 grass_uv = ground_tiles(world_xz, 3.0);
	float ragged = (ground_noise(vec2(along * 0.9, UV.x > 0.5 ? 11.0 : 5.0)) - 0.5) * 0.5;
	float verge = smoothstep(0.35, 0.7, outside + ragged);
	vec3 grass_bent = ground_bend(world_normal, ground_unpack(texture(grass_normal, grass_uv)),
		vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), 0.8);
	albedo = mix(albedo, texture(grass_albedo, grass_uv).rgb, verge);
	normal = normalize(mix(normal, grass_bent, verge));
	roughness = mix(roughness, 0.92, verge);

	// Rain darkens and smooths the asphalt, gravel less.
	float wet = wetness * mix(1.0, 0.5, shoulder) * (1.0 - verge);
	albedo *= 1.0 - 0.4 * wet;
	roughness = mix(roughness, 0.25, wet);
	float water = puddle(world_xz, world_normal, clamp(abs(UV.x - 0.5) * 2.0, 0.0, 1.0)) * (1.0 - shoulder);
	ALBEDO = mix(albedo, puddle_color * 0.6, water * 0.6);
	ROUGHNESS = mix(roughness, 0.08, water);
	SPECULAR = mix(0.4, 0.5, max(wet, water));
	NORMAL = normalize((VIEW_MATRIX * vec4(normalize(mix(normal, world_normal, water)), 0.0)).xyz);
}
