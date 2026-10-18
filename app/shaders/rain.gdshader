shader_type spatial;
// Rain (#103, R89, ADR 0020): faint streaks, as falling drops look with the eye's own motion
// blur — thin, see-through, fading out at both ends, lit by the sky; the particles align them
// with their fall.
render_mode cull_disabled, depth_draw_never, shadows_disabled;

uniform vec3 color : source_color = vec3(0.79, 0.824, 0.851);
uniform float opacity : hint_range(0.0, 1.0) = 0.3;

void fragment() {
	// UV.y runs along the streak: densest in the middle.
	float along = 1.0 - abs(UV.y - 0.5) * 2.0;
	ALBEDO = color;
	ALPHA = opacity * along * along;
	ROUGHNESS = 0.2;
	SPECULAR = 0.5;
}
