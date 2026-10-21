shader_type spatial;
// Urban mesh colours carry material codes, not albedo: paving 0, concrete .25,
// asphalt .5, curb .75. UVs are metres, including on vertical curb and garden-wall faces.

#include "res://shaders/ground.gdshaderinc"

uniform sampler2D concrete_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D concrete_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D concrete_rough : filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D paving_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D paving_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D paving_rough : filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D asphalt_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D asphalt_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D asphalt_rough : filter_linear_mipmap_anisotropic, repeat_enable;
uniform float wetness = 0.0;
#include "res://shaders/puddles.gdshaderinc"

varying float material_code;
varying vec3 world_normal;
varying vec3 world_position;

void vertex() {
	material_code = COLOR.r * 4.0;
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec3 albedo;
	vec3 detail;
	float roughness;
	if (material_code < 0.5) {
		albedo = texture(paving_albedo, UV / 2.0).rgb;
		detail = ground_unpack(texture(paving_normal, UV / 2.0));
		roughness = texture(paving_rough, UV / 2.0).r;
	} else if (material_code > 1.5 && material_code < 2.5) {
		albedo = texture(asphalt_albedo, UV / 2.5).rgb;
		detail = ground_unpack(texture(asphalt_normal, UV / 2.5));
		roughness = texture(asphalt_rough, UV / 2.5).r;
	} else {
		albedo = texture(concrete_albedo, UV / 3.0).rgb;
		detail = ground_unpack(texture(concrete_normal, UV / 3.0));
		roughness = texture(concrete_rough, UV / 3.0).r;
	}
	vec3 normal = ground_bend_along(world_normal, detail, world_position, UV, 1.0);
	float water = puddle(world_position.xz, world_normal, 0.5);
	ALBEDO = mix(albedo * (1.0 - 0.3 * wetness), puddle_color * 0.6, water * 0.6);
	ROUGHNESS = mix(mix(roughness, 0.35, wetness), 0.08, water);
	SPECULAR = mix(0.3, 0.45, wetness);
	NORMAL = normalize((VIEW_MATRIX * vec4(normal, 0.0)).xyz);
}
