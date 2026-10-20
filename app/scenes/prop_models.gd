class_name PropModels
extends RefCounted
## The props made in Blender that stand by the road (art/props, R92): guard rails, delineator
## posts and snow poles, street lamps, benches, bins and bus shelters, pasture fences, hay
## bales and wayside crosses, road signs and power poles; the power lines' wires too. Loads
## them and gives each material name its look from the palette's `props` section: the shelters'
## glass see-through,
## the lamps' glass faintly lit.

const DIRECTORY: String = "res://assets/models/props/"
## Per material name: roughness, metallic, opacity and how brightly it glows (its own colour).
const LOOKS: Dictionary[String, Array] = {
	"galvanised": [0.45, 0.55, 1.0, 0.0],
	"post": [0.6, 0.0, 1.0, 0.0],
	"band": [0.7, 0.0, 1.0, 0.0],
	"reflector": [0.25, 0.3, 1.0, 0.0],
	"pole_red": [0.5, 0.0, 1.0, 0.0],
	"pole_white": [0.5, 0.0, 1.0, 0.0],
	"metal_dark": [0.5, 0.6, 1.0, 0.0],
	"wood": [0.8, 0.0, 1.0, 0.0],
	"lamp": [0.3, 0.0, 1.0, 0.6],
	"glass": [0.05, 0.0, 0.3, 0.0],
	"wood_weathered": [0.9, 0.0, 1.0, 0.0],
	"hay": [0.95, 0.0, 1.0, 0.0],
	"wrap": [0.35, 0.0, 1.0, 0.0],
	"stone": [0.9, 0.0, 1.0, 0.0],
	"sign_blue": [0.55, 0.0, 1.0, 0.0],
	"sign_white": [0.55, 0.0, 1.0, 0.0],
	"sign_red": [0.55, 0.0, 1.0, 0.0],
	"sign_black": [0.7, 0.0, 1.0, 0.0],
	"insulator": [0.3, 0.0, 1.0, 0.0],
	"wire": [0.5, 0.6, 1.0, 0.0],
}

static var _materials: Dictionary[String, StandardMaterial3D] = {}
static var _meshes: Dictionary[String, Mesh] = {}


## The mesh of model `name` with its materials replaced by the shared looks.
static func mesh(name: String) -> Mesh:
	if _meshes.has(name):
		return _meshes[name]
	var path: String = DIRECTORY + name + ".glb"
	# A world kept from an older Torqa may name a model this one no longer has (R101): it is
	# left out rather than breaking its chunk.
	if not ResourceLoader.exists(path):
		return null
	var scene: PackedScene = load(path)
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
	var opacity: float = look[2]
	if opacity < 1.0:
		created.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
		created.albedo_color.a = opacity
		created.cull_mode = BaseMaterial3D.CULL_DISABLED
	var glow: float = look[3]
	if glow > 0.0:
		created.emission_enabled = true
		created.emission = created.albedo_color
		created.emission_energy_multiplier = glow
	_materials[name] = created
	return created


## The power lines' wires: dark, seen from both sides.
static func wire_material() -> StandardMaterial3D:
	var wire: StandardMaterial3D = material("wire").duplicate()
	wire.cull_mode = BaseMaterial3D.CULL_DISABLED
	return wire


## A sign's text (`world_chunk()["labels"]`): white on the place signs' blue, black on the
## warnings' white, as wide as the sign's face at most.
static func label(entry: Dictionary) -> Label3D:
	var text: String = entry["text"]
	var values: PackedFloat32Array = entry["transform"]
	var made: Label3D = Label3D.new()
	made.text = text
	made.font_size = 72
	made.outline_size = 0
	made.double_sided = false
	made.shaded = true
	# Place names are words, grades figures.
	var place: bool = not text.ends_with("%")
	made.modulate = Color.WHITE if place else Color(0.08, 0.08, 0.08)
	var face: float = 1.25 if place else 0.32
	made.pixel_size = 0.0036 if place else 0.003
	var font: Font = made.font if made.font != null else ThemeDB.fallback_font
	var width: float = font.get_string_size(text, HORIZONTAL_ALIGNMENT_CENTER, -1, made.font_size).x
	if width * made.pixel_size > face:
		made.pixel_size = face / width
	made.transform = Transform3D(
		Basis(
			Vector3(values[0], values[4], values[8]),
			Vector3(values[1], values[5], values[9]),
			Vector3(values[2], values[6], values[10])
		),
		Vector3(values[3], values[7], values[11])
	)
	return made


static func _find_mesh(node: Node) -> MeshInstance3D:
	if node is MeshInstance3D:
		return node
	for child: Node in node.get_children():
		var found: MeshInstance3D = _find_mesh(child)
		if found != null:
			return found
	return null
