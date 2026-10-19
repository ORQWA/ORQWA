shader_type spatial;
// Railways (#75, R89, ADR 0020): a bed of ballast (the gravel texture) with wooden sleepers
// across it and two steel rails along it, their heads polished by wheels. UV.x runs 0–1 across the bed, UV.y is the
// distance along the line in metres. Where sleepers would be finer than a pixel along the line,
// or the rails across it, each gives way to its average colour, so they never shimmer in the
// distance; each on its own, as a line seen along its length foreshortens the sleepers long
// before the rails.

uniform vec3 ballast_color : source_color = vec3(0.659, 0.635, 0.612);
uniform vec3 sleeper_color : source_color = vec3(0.478, 0.341, 0.333);
uniform vec3 rail_color : source_color = vec3(0.38, 0.263, 0.404);
uniform float bed_width = 3.2;
uniform sampler2D gravel_texture : source_color, filter_linear_mipmap_anisotropic, repeat_enable;

// Standard gauge between the rails' middles, rail head width, sleepers (length, width, spacing).
const float GAUGE = 1.5;
const float RAIL = 0.08;
const float SLEEPER_LENGTH = 2.5;
const float SLEEPER_WIDTH = 0.24;
const float SLEEPER_SPACING = 0.6;

// 1 inside [from, to], edges one pixel soft.
float band(float x, float from, float to) {
	float soft = max(fwidth(x), 1e-4);
	return smoothstep(from - soft, from + soft, x) * (1.0 - smoothstep(to - soft, to + soft, x));
}

void fragment() {
	float across = (UV.x - 0.5) * bed_width;
	float along = UV.y;
	float sleeper = band(abs(across), -1.0, SLEEPER_LENGTH * 0.5)
		* band(mod(along, SLEEPER_SPACING), 0.0, SLEEPER_WIDTH);
	float rails = band(abs(across), GAUGE * 0.5 - RAIL * 0.5, GAUGE * 0.5 + RAIL * 0.5);
	// Sleepers hold until a pixel spans a good part of their spacing along the line, rails
	// until it spans a good part of the gauge across it: their edges are already a pixel
	// soft, so they thin out before they go.
	float sleepers_kept = 1.0 - smoothstep(0.15, 0.4, fwidth(along));
	float rails_kept = 1.0 - smoothstep(0.25, 0.6, fwidth(across));
	// The averages: sleepers cover about a third of the middle, rails a little.
	float sleeper_mix = mix(0.3 * band(abs(across), -1.0, SLEEPER_LENGTH * 0.5), sleeper, sleepers_kept);
	float rail_mix = mix(0.06, rails, rails_kept);
	vec3 ballast = texture(gravel_texture, vec2(across, mod(along, 384.0)) / 1.5).rgb;
	ALBEDO = mix(mix(ballast * ballast_color * 2.2, sleeper_color, sleeper_mix), rail_color, rail_mix);
	ROUGHNESS = mix(0.92, 0.3, rail_mix);
	METALLIC = 0.8 * rail_mix;
	SPECULAR = 0.4;
}
