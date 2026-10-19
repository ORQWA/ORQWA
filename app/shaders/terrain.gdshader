shader_type spatial;
// The ground (R89, ADR 0020): smooth terrain textured by land cover. The vertex colour names the
// cover from OpenStreetMap (one palette colour per kind, alpha marking the bed under water);
// meadows, orchards and gardens are grass, fields grass gone golden, forests a floor of earth
// with some grass, industrial land gravel, rock rock. Covers blend over a grid cell; steep
// ground is bare rock, mapped from three sides so cliffs do not smear; high ground is snow.
// Textures are mapped in metres on the world's x and z, at two scales mixed by large patches,
// so the tile does not show.

#include "res://shaders/ground.gdshaderinc"

uniform sampler2D grass_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D grass_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D dirt_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D dirt_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D gravel_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D gravel_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D rock_albedo : source_color, filter_linear_mipmap_anisotropic, repeat_enable;
uniform sampler2D rock_normal : hint_normal, filter_linear_mipmap_anisotropic, repeat_enable;

// The vertex colours of the covers (palette `ground.*`, sRGB): they identify, not colour.
uniform vec3 cover_meadow = vec3(0.698, 0.757, 0.471);
uniform vec3 cover_farmland = vec3(0.749, 0.776, 0.498);
uniform vec3 cover_orchard = vec3(0.647, 0.71, 0.416);
uniform vec3 cover_forest = vec3(0.482, 0.498, 0.267);
uniform vec3 cover_town = vec3(0.761, 0.769, 0.604);
uniform vec3 cover_industrial = vec3(0.788, 0.765, 0.706);
uniform vec3 cover_rock = vec3(0.765, 0.733, 0.706);

uniform vec3 snow_color : source_color = vec3(0.86, 0.88, 0.9);
uniform float snow_line = 2400.0;
// Ground steeper than this (its normal's vertical part below it, ~53°) is bare rock; somewhat
// less steep ground shows earth through the grass.
uniform float rock_slope = 0.6;
// Tile sizes in metres.
uniform float grass_tile = 3.0;
uniform float dirt_tile = 2.5;
uniform float gravel_tile = 1.5;
uniform float rock_tile = 7.0;

varying vec3 world_position;
varying vec3 world_normal;
varying vec4 cover;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
	cover = COLOR;
}

float distance2(vec3 colour, vec3 kind) {
	vec3 d = colour - kind;
	return dot(d, d);
}

// How much the interpolated cover colour is `kind`'s, against the closest cover's distance
// `nearest`: 1 for the closest, fading towards the neighbouring cover across the cell. Relative
// to the closest, so a colour halfway between two far-apart covers still belongs to both.
float weight(vec3 colour, vec3 kind, float nearest) {
	return exp(-(distance2(colour, kind) - nearest) * 400.0);
}

// A texture laid on the ground from above: albedo out, the bent normal returned.
vec3 from_above(sampler2D albedo, sampler2D normal_map, float tile, float strength, out vec3 colour) {
	vec2 uv = ground_tiles(world_position.xz, tile);
	float patches = ground_noise(world_position.xz / 23.0);
	vec3 near = texture(albedo, uv).rgb;
	// Wrapped on its own, unturned: a turned copy would break at the wrap.
	vec3 far = texture(albedo, ground_tiles(world_position.xz, tile * 2.7) + 0.31).rgb;
	colour = mix(near, far, 0.25 + 0.35 * patches);
	vec3 detail = ground_unpack(texture(normal_map, uv));
	return ground_bend(world_normal, detail, vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), strength);
}

// Rock from three sides, weighted by which way the ground faces.
vec3 rock_sides(out vec3 colour) {
	vec3 blend = pow(abs(world_normal), vec3(4.0));
	blend /= blend.x + blend.y + blend.z;
	vec3 p = ground_tiles3(world_position, rock_tile);
	vec3 x_colour = texture(rock_albedo, p.zy).rgb;
	vec3 y_colour = texture(rock_albedo, p.xz).rgb;
	vec3 z_colour = texture(rock_albedo, p.xy).rgb;
	colour = x_colour * blend.x + y_colour * blend.y + z_colour * blend.z;
	vec3 n = world_normal;
	vec3 x_normal = ground_bend(n, ground_unpack(texture(rock_normal, p.zy)), vec3(0.0, 0.0, 1.0), vec3(0.0, 1.0, 0.0), 1.0);
	vec3 y_normal = ground_bend(n, ground_unpack(texture(rock_normal, p.xz)), vec3(1.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0), 1.0);
	vec3 z_normal = ground_bend(n, ground_unpack(texture(rock_normal, p.xy)), vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), 1.0);
	return normalize(x_normal * blend.x + y_normal * blend.y + z_normal * blend.z);
}

void fragment() {
	vec3 colour = cover.rgb;
	float nearest = min(min(min(distance2(colour, cover_meadow), distance2(colour, cover_orchard)),
		min(distance2(colour, cover_town), distance2(colour, cover_farmland))),
		min(min(distance2(colour, cover_forest), distance2(colour, cover_industrial)),
		distance2(colour, cover_rock)));
	float w_meadow = weight(colour, cover_meadow, nearest) + weight(colour, cover_orchard, nearest)
		+ weight(colour, cover_town, nearest);
	float w_farmland = weight(colour, cover_farmland, nearest);
	float w_forest = weight(colour, cover_forest, nearest);
	float w_industrial = weight(colour, cover_industrial, nearest);
	float w_rock = weight(colour, cover_rock, nearest);
	float total = w_meadow + w_farmland + w_forest + w_industrial + w_rock;

	vec3 grass;
	vec3 grass_n = from_above(grass_albedo, grass_normal, grass_tile, 0.8, grass);
	vec3 dirt;
	vec3 dirt_n = from_above(dirt_albedo, dirt_normal, dirt_tile, 1.0, dirt);
	vec3 gravel;
	vec3 gravel_n = from_above(gravel_albedo, gravel_normal, gravel_tile, 1.0, gravel);
	vec3 rock;
	vec3 rock_n = rock_sides(rock);

	// Fields: grass ripened towards straw, patch by patch.
	float ripe = ground_noise(world_position.xz / 60.0);
	vec3 field = mix(grass, grass * vec3(1.45, 1.25, 0.6), 0.45 + 0.4 * ripe);
	// Forest floor: dark moss and grass, earth and needles showing through in patches.
	float bare = smoothstep(0.5, 0.8, ground_noise(world_position.xz / 7.0));
	vec3 moss = grass * vec3(0.6, 0.68, 0.5);
	vec3 floor_colour = mix(moss, dirt * 0.7, bare * 0.7);
	vec3 floor_normal = normalize(mix(grass_n, dirt_n, bare * 0.7));

	// Meadows vary as growth does: lusher and drier patches over a hundred metres and more,
	// drier on steep ground, yellower above the tree line.
	float lush = ground_noise(world_position.xz / 90.0) * 0.6 + ground_noise(world_position.xz / 310.0) * 0.4;
	vec3 meadow = grass * mix(vec3(0.98, 0.92, 0.7), vec3(0.7, 0.82, 0.76), lush);
	float dry = smoothstep(0.97, 0.8, world_normal.y) * 0.5
		+ smoothstep(1700.0, 2300.0, world_position.y) * 0.6;
	meadow = mix(meadow, meadow * vec3(1.25, 1.08, 0.62), clamp(dry, 0.0, 1.0));
	vec3 albedo = (meadow * w_meadow + field * w_farmland + floor_colour * w_forest
		+ gravel * w_industrial + rock * w_rock) / total;
	vec3 normal = normalize(grass_n * (w_meadow + w_farmland) + floor_normal * w_forest
		+ gravel_n * w_industrial + rock_n * w_rock);
	float roughness = 0.92;

	// The bed under water: dark wet earth.
	float bed = smoothstep(0.3, 0.7, cover.a);
	albedo = mix(albedo, dirt * 0.45, bed);
	normal = normalize(mix(normal, dirt_n, bed));

	// Steep banks show earth through the grass, steeper ground is bare rock; the edges broken up
	// by the textures themselves.
	float bank = 1.0 - smoothstep(rock_slope + 0.02, rock_slope + 0.12, world_normal.y + (dirt.r - 0.3) * 0.2);
	albedo = mix(albedo, dirt * 0.8, bank * 0.6 * (1.0 - bed));
	normal = normalize(mix(normal, dirt_n, bank * 0.6 * (1.0 - bed)));
	float steep = 1.0 - smoothstep(rock_slope - 0.03, rock_slope + 0.01, world_normal.y + (rock.r - 0.4) * 0.1);
	steep *= 1.0 - bed;
	albedo = mix(albedo, rock, steep);
	normal = normalize(mix(normal, rock_n, steep));

	// Snow above the snow line, thinning out over a hundred metres, gone from steep faces.
	float snow = smoothstep(snow_line - 60.0, snow_line + 60.0, world_position.y + (ground_noise(world_position.xz / 30.0) - 0.5) * 120.0);
	snow *= smoothstep(0.6, 0.8, world_normal.y) * (1.0 - bed);
	albedo = mix(albedo, snow_color, snow);
	normal = normalize(mix(normal, world_normal, snow * 0.7));
	roughness = mix(roughness, 0.7, snow);

	// Large, soft variations in brightness and hue, as soil and growth vary.
	float tone = ground_noise(world_position.xz / 140.0) * 0.6 + ground_noise(world_position.xz / 35.0) * 0.4;
	albedo *= mix(vec3(0.88, 0.9, 0.86), vec3(1.08, 1.06, 1.0), tone);

	ALBEDO = albedo;
	NORMAL = normalize((VIEW_MATRIX * vec4(normal, 0.0)).xyz);
	ROUGHNESS = roughness;
	SPECULAR = 0.3;
}
