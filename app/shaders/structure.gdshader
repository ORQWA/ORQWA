shader_type spatial;

#include "res://shaders/ground.gdshaderinc"
#include "res://shaders/bridge_surface.gdshaderinc"

uniform sampler2D masonry_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D masonry_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform float wetness = 0.0;

varying vec3 world_position;
varying vec3 world_normal;

vec3 to_linear(vec3 srgb) {
	return mix(srgb / 12.92, pow((srgb + 0.055) / 1.055, vec3(2.4)), step(0.04045, srgb));
}

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec3 albedo;
	vec3 normal;
	float roughness;
	bridge_concrete(world_position, world_normal, albedo, normal, roughness);
	// Existing worlds store sRGB tints, including darker tunnel floors. Warm stone tints
	// retain their material without new style flags or a stored-world format change.
	float stone = smoothstep(0.09, 0.13, COLOR.r - COLOR.b);
	vec2 uv = ground_tiles(bridge_uv(world_position, world_normal), 3.0);
	vec3 masonry = texture(masonry_albedo, uv).rgb * to_linear(COLOR.rgb) * 2.0;
	vec3 tint = clamp(to_linear(COLOR.rgb) / max(concrete_color, vec3(0.01)), vec3(0.1), vec3(1.15));
	albedo = mix(albedo * tint, masonry, stone);
	normal = normalize(mix(normal, bridge_bend(world_normal, ground_unpack(texture(masonry_normal, uv))), stone));
	roughness = mix(roughness, 0.92, stone);
	float steel = 1.0 - step(0.025, distance(COLOR.rgb, steel_srgb));
	albedo = mix(albedo, steel_color, steel);
	ALBEDO = albedo * (1.0 - wetness * 0.22);
	ROUGHNESS = mix(mix(roughness, 0.45, steel), 0.4, wetness * 0.5);
	METALLIC = 0.65 * steel;
	SPECULAR = 0.35;
	NORMAL = normalize((VIEW_MATRIX * vec4(mix(normal, world_normal, steel), 0.0)).xyz);
}
