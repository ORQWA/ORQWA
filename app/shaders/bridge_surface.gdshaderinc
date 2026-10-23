// Shared concrete for bridge decks, piers and exposed backs of road surfaces. Projection
// uses metres on the dominant face, including vertical faces, so aggregate never stretches.
uniform sampler2D bridge_concrete_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D bridge_concrete_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D bridge_concrete_rough : filter_linear_mipmap_anisotropic, repeat_enable;
uniform vec3 concrete_color : source_color;
uniform vec3 steel_color : source_color;
uniform vec3 steel_srgb;

vec2 bridge_uv(vec3 p, vec3 n) {
	vec3 a = abs(n);
	if (a.y >= max(a.x, a.z)) { return p.xz; }
	if (a.x >= a.z) { return p.zy; }
	return p.xy;
}

vec3 bridge_bend(vec3 n, vec3 detail) {
	vec3 a = abs(n);
	if (a.y >= max(a.x, a.z)) {
		return ground_bend(n, detail, vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), 0.55);
	}
	if (a.x >= a.z) {
		return ground_bend(n, detail, vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0), 0.55);
	}
	return ground_bend(n, detail, vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), 0.55);
}

void bridge_concrete(vec3 p, vec3 n, out vec3 albedo, out vec3 normal, out float roughness) {
	// Photographed cast concrete spans 2.16 m, preserving pores and formboard marks.
	vec2 uv = ground_tiles(bridge_uv(p, n), 2.16);
	float stains = ground_noise(vec2((p.x + p.z) * 1.6, p.y * 0.14));
	float weather = mix(0.83, 1.04, stains);
	albedo = texture(bridge_concrete_albedo, uv).rgb * mix(vec3(1.0), concrete_color, 0.1) * weather;
	normal = bridge_bend(n, ground_unpack(texture(bridge_concrete_normal, uv)));
	roughness = texture(bridge_concrete_rough, uv).r;
}
