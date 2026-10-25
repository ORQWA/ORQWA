class_name TerrainMaterial
extends RefCounted
## The ground's material (terrain.gdshader, R89, ADR 0020): baked textures by land cover, the
## colours that identify the covers, and how near forest ground turns into crowns.

const KINDS: Array[String] = ["grass", "dirt", "gravel", "rock", "canopy"]
const COVERS: Array[String] = [
	"meadow", "farmland", "orchard", "forest", "town", "industrial", "rock"
]
## How far from the route woods keep their dense trees (R103).
const DENSE_BELT: float = 300.0


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


## Forest ground turns into crowns over 150 m and is crowns from where the generator thins the
## woods out, 300 m from the route (vegetation.rs, NEAR_DISTANCE): the camera stays on the
## route, so no thinned wood shows its floor. The fewer trees are drawn (R108, R109), the
## nearer: from 150 m at an eighth of them to 300 m at half or more, never starting within 60 m.
static func set_tree_density(material: ShaderMaterial, density: float) -> void:
	var full: float = lerpf(150.0, DENSE_BELT, clampf((density - 0.125) / 0.375, 0.0, 1.0))
	material.set_shader_parameter("canopy_from", maxf(full - 150.0, 60.0))
	material.set_shader_parameter("canopy_full", full)
