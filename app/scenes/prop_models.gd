class_name PropModels
extends RefCounted
## The props made in Blender that stand by the road (art/props, R89): guard rails, delineator
## posts and snow poles. Loads them and gives each material name its look from the palette's
## `props` section.

const DIRECTORY: String = "res://assets/models/props/"
## Per material name: roughness and metallic.
const LOOKS: Dictionary[String, Array] = {
	"galvanised": [0.45, 0.55],
	"post": [0.6, 0.0],
	"band": [0.7, 0.0],
	"reflector": [0.25, 0.3],
	"pole_red": [0.5, 0.0],
	"pole_white": [0.5, 0.0],
}

static var _materials: Dictionary[String, StandardMaterial3D] = {}
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
static func material(name: String) -> StandardMaterial3D:
	if _materials.has(name):
		return _materials[name]
	var look: Array = LOOKS.get(name, LOOKS["post"])
	var created: StandardMaterial3D = StandardMaterial3D.new()
	created.albedo_color = Palette.color("props." + name)
	created.roughness = look[0]
	created.metallic = look[1]
	_materials[name] = created
	return created


static func _find_mesh(node: Node) -> MeshInstance3D:
	if node is MeshInstance3D:
		return node
	for child: Node in node.get_children():
		var found: MeshInstance3D = _find_mesh(child)
		if found != null:
			return found
	return null
