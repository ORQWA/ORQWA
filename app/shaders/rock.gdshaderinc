// Shared cliff and boulder weathering (R112). Broad broken faces survive mipmapping
// without forming masonry rows; the field lives in metres rather than in the texture.
float rock_field(vec3 p, out float shade) {
	vec2 face = vec2(p.x + p.z * 0.73, p.y + p.z * 0.19);
	vec2 warp = vec2(ground_noise(face * 0.43), ground_noise(face * 0.37 + 17.3));
	face += (warp - 0.5) * 1.4;
	float mass = ground_noise(face * 0.82);
	float grain = ground_noise(face * 2.3 + 9.1);
	// Oblique, interrupted fracture families avoid a grid of enclosed paving cells.
	float bedding = ground_noise(vec2(face.x * 0.38 + face.y * 0.21, face.y * 1.7));
	float joint = ground_noise(vec2(face.x * 1.2 - face.y * 0.37, face.y * 0.55) + 31.7);
	float aa = max(fwidth(bedding), fwidth(joint));
	float split = 1.0 - smoothstep(0.018, 0.065 + aa, abs(bedding - 0.5));
	split *= smoothstep(0.28, 0.6, grain);
	float cross_split = 1.0 - smoothstep(0.012, 0.045 + aa, abs(joint - 0.52));
	cross_split *= smoothstep(0.38, 0.64, mass);
	float fissure = max(split, cross_split);
	shade = mix(0.65, 1.22, smoothstep(0.23, 0.77, mass)) * (1.0 - fissure * 0.38);
	return mass * 0.42 + grain * 0.08 - fissure * 0.12;
}

// Bend only the normal: road contact, shorelines and kept-world geometry stay exact.
// Broad relief remains visible after the scanned grain becomes subpixel.
void rock_weathering(vec3 position, vec3 surface_normal, float scale,
		inout vec3 colour, inout vec3 normal) {
	float shade;
	float height = rock_field(position / scale, shade) * scale;
	vec3 dx = dFdx(position);
	vec3 dy = dFdy(position);
	vec3 across = cross(dy, surface_normal);
	vec3 along = cross(surface_normal, dx);
	float determinant = dot(dx, across);
	if (abs(determinant) > 1e-10) {
		vec3 gradient = (across * dFdx(height) + along * dFdy(height)) / determinant;
		float footprint = max(length(dx), length(dy)) / scale;
		float relief = 1.0 - smoothstep(0.12, 0.5, footprint);
		normal = normalize(normal - gradient * relief);
	}
	colour *= shade;
}
