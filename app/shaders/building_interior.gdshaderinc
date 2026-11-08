// Opaque window interiors retain sky reflections without transparent surfaces or room meshes.
uniform vec3 interior_dark : source_color;
uniform vec3 interior_light : source_color;
uniform vec3 curtain_color : source_color;

vec3 building_interior(vec2 pane, float seed) {
	pane = clamp(pane, vec2(0.0), vec2(1.0));
	float side = min(pane.x, 1.0 - pane.x);
	float curtains = 1.0 - smoothstep(0.12, 0.22 + seed * 0.12, side);
	float blinds = smoothstep(0.68 + seed * 0.28, 0.70 + seed * 0.28, pane.y);
	float fine = 1.0 - smoothstep(0.015, 0.05, fwidth(pane.x));
	float pleat = 0.87 + fine * 0.13 * sin(pane.x * 95.0);
	vec3 room = mix(interior_dark, interior_light, seed) * 0.55;
	room *= mix(0.7, 1.0, smoothstep(0.0, 0.3, side));
	return mix(room, curtain_color * pleat * 0.65, max(curtains * step(0.2, seed), blinds));
}
