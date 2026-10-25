class_name TerrainMaterial
extends RefCounted
## The ground's material (terrain.gdshader, R89, ADR 0020): baked textures by land cover, the
## colours that identify the covers, and how near forest ground turns into crowns.

const KINDS: Array[String] = ["grass", "dirt", "gravel", "rock", "canopy"]
const COVERS: Array[String] = [
	"meadow", "farmland", "orchard", "forest", "town", "industrial", "rock"
]


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


## The fewer trees are drawn (R108, R109), the nearer forest ground turns into crowns: from
## 100 m at an eighth of them to 250 m at half or more.
static func set_tree_density(material: ShaderMaterial, density: float) -> void:
	var from: float = lerpf(100.0, 250.0, clampf((density - 0.125) / 0.375, 0.0, 1.0))
	material.set_shader_parameter("canopy_from", from)
	material.set_shader_parameter("canopy_full", from + 400.0)
