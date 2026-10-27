// Broadleaves change colour; evergreen lowlands and needles keep their own palette.
uniform vec4 autumn_colors[4] : source_color;
uniform float autumn_strength : hint_range(0.0, 1.0) = 0.0;
uniform float autumn_from = -10000.0;

float autumn_amount(float height) {
	return autumn_strength * smoothstep(autumn_from, autumn_from + 80.0, height);
}

vec3 autumn_colour(vec2 place) {
	float choice = fract(sin(dot(place, vec2(12.9898, 78.233))) * 43758.5453);
	return autumn_colors[int(choice * 3.999)].rgb;
}
