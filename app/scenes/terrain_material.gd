class_name TerrainMaterial
extends RefCounted
## The ground's material (terrain.gdshader, R89, ADR 0020): baked textures by land cover, the
## colours that identify the covers, and how near forest ground turns into crowns.

const KINDS: Array[String] = ["grass", "dirt", "gravel", "rock", "canopy"]
const COVERS: Array[String] = [
	"meadow", "farmland", "orchard", "forest", "town", "industrial", "rock"
]
## How far from the camera, at the most, forest ground turns into crowns.
const CANOPY_FULL: float = 300.0


## Gives `material` the ground shader, the `<kind>_albedo` and `<kind>_normal` maps of each of
## KINDS from `textures`, and the covers' colours.
static func configure(material: ShaderMaterial, textures: Dictionary[String, Texture2D]) -> void:
	material.shader = preload("res://shaders/terrain.gdshader")
	for kind: String in KINDS:
		material.set_shader_parameter(kind + "_albedo", textures[kind + "_albedo"])
		material.set_shader_parameter(kind + "_normal", textures[kind + "_normal"])
	UrbanSurfaces.configure_terrain(material)
	for cover: String in COVERS:
		var colour: Color = Palette.color("ground." + cover)
		material.set_shader_parameter("cover_" + cover, Vector3(colour.r, colour.g, colour.b))
	material.set_shader_parameter("snow_color", Palette.color("ground.snow"))


## Near the camera the drawn trees carry the woods: from there forest ground turns into crowns
## over 150 m, complete by CANOPY_FULL, the nearer the fewer trees are drawn (R108, R109): by
## 150 m at an eighth of them. Woods the generator thins out are crowns wherever they are
## (`set_route_distances`).
static func set_tree_density(material: ShaderMaterial, density: float) -> void:
	var full: float = lerpf(150.0, CANOPY_FULL, clampf((density - 0.125) / 0.375, 0.0, 1.0))
	material.set_shader_parameter("canopy_from", maxf(full - 150.0, 60.0))
	material.set_shader_parameter("canopy_full", full)


## Tells the ground where the woods are thinned (R145): `distances` as
## `OrqwaApp.route_distances()` gives them, empty without a route.
static func set_route_distances(material: ShaderMaterial, distances: Dictionary) -> void:
	if distances.is_empty():
		material.set_shader_parameter("route_size", Vector2.ZERO)
		return
	var image: Image = distances["image"]
	material.set_shader_parameter("route_distance", ImageTexture.create_from_image(image))
	material.set_shader_parameter("route_origin", distances["origin"])
	material.set_shader_parameter("route_size", distances["size"])
	material.set_shader_parameter("route_step", distances["step"])
	material.set_shader_parameter("dense_forest", distances["dense"])
