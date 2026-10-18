class_name VegetationModels
extends RefCounted
## The vegetation models made in Blender (art/vegetation, ADR 0019): trees, bushes and rocks.
## Loads them and gives each material name its look: bark textured on trunks
## (app/shaders/vegetation.gdshader), foliage cards of leaves or needles cut out and tinted per
## plant (app/shaders/foliage.gdshader), palm fronds and rocks tinted per plant.

const DIRECTORY: String = "res://assets/models/vegetation/"
## Per material name: shader, its `kind` (vegetation.gdshader), texture, texture size in metres,
## how much the plant's own colour tints it, and sway in the wind (metres at 10 m height).
const LOOKS: Dictionary[String, Array] = {
	"trunk": ["vegetation", 0, "bark", 1.2, 0.0, 0.12],
	"leaves": ["foliage", 0, "leaves", 1.0, 1.0, 0.12],
	"needles": ["foliage", 0, "needles", 1.0, 1.0, 0.1],
	"frond": ["vegetation", 2, "", 1.0, 1.0, 0.12],
	"rock": ["vegetation", 3, "rock", 1.6, 1.0, 0.0],
}

static var _materials: Dictionary[String, ShaderMaterial] = {}
static var _meshes: Dictionary[String, Mesh] = {}


## The mesh of model `name` with its materials replaced by the shared looks.
static func mesh(name: String) -> Mesh:
	if _meshes.has(name):
		return _meshes[name]
	var scene: PackedScene = load(DIRECTORY + name + ".glb")
	var root: Node = scene.instantiate()
	var found: ArrayMesh = _find_mesh(root).mesh
	root.free()
	for surface: int in found.get_surface_count():
		var imported: Material = found.surface_get_material(surface)
		found.surface_set_material(surface, material(imported.resource_name))
	_meshes[name] = found
	return found


## The shared material for a material name.
static func material(name: String) -> ShaderMaterial:
	if _materials.has(name):
		return _materials[name]
	var look: Array = LOOKS.get(name, LOOKS["leaves"])
	var shader_name: String = look[0]
	var texture_name: String = look[2]
	var shader_material: ShaderMaterial = ShaderMaterial.new()
	shader_material.shader = load("res://shaders/%s.gdshader" % shader_name)
	if not texture_name.is_empty():
		shader_material.set_shader_parameter(
			"albedo_texture", load("res://assets/textures/%s_albedo.webp" % texture_name)
		)
		var normal_path: String = "res://assets/textures/%s_normal.webp" % texture_name
		if ResourceLoader.exists(normal_path):
			shader_material.set_shader_parameter("normal_texture", load(normal_path))
	if shader_name == "vegetation":
		shader_material.set_shader_parameter("kind", look[1])
		shader_material.set_shader_parameter("tile", look[3])
		shader_material.set_shader_parameter("tint_strength", look[4])
	shader_material.set_shader_parameter("sway", look[5])
	_materials[name] = shader_material
	return shader_material


## How strongly the wind moves trees (0 calm, 1 storm), for the weather.
static func set_wind(strength: float) -> void:
	for name: String in LOOKS:
		material(name).set_shader_parameter("wind_strength", strength)


static func _find_mesh(node: Node) -> MeshInstance3D:
	if node is MeshInstance3D:
		return node
	for child: Node in node.get_children():
		var found: MeshInstance3D = _find_mesh(child)
		if found != null:
			return found
	return null
