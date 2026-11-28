class_name WorldScriptScene
extends Node3D
## What a world's script places in the scene (R302, `docs/world-files.md`, "`ORQWA_script`"):
## its models on their anchors, playing their own clips, and lines between them. The core works
## out where each anchor stands (`ride_state()["world"]`); this only places and draws.

## Where a line point is drawn when a node it names is not placed.
const NOWHERE: Vector3 = Vector3.INF

var _orqwa: OrqwaApp
var _answers: int = -1
var _nodes: Array = []
var _lines: Array = []
var _hidden_riders: Dictionary[int, bool] = {}
var _camera: Variant = null
## Placed nodes by id, and the model each shows.
var _placed: Dictionary[String, Node3D] = {}
var _placed_model: Dictionary[String, String] = {}
## Model scenes made from the world's files, by model name, and those asked for.
var _scenes: Dictionary[String, Node3D] = {}
var _asked: Dictionary[String, String] = {}
var _line_mesh: ImmediateMesh = ImmediateMesh.new()
var _line_material: StandardMaterial3D = StandardMaterial3D.new()


func _init() -> void:
	var lines: MeshInstance3D = MeshInstance3D.new()
	lines.mesh = _line_mesh
	lines.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	_line_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_line_material.albedo_color = Palette.color("road.rail")
	add_child(lines)


## Lets go of everything placed, at the end of a ride.
func clear() -> void:
	for node: Node3D in _placed.values():
		node.queue_free()
	_placed.clear()
	_placed_model.clear()
	for scene: Node3D in _scenes.values():
		scene.queue_free()
	_scenes.clear()
	_asked.clear()
	_answers = -1
	_nodes = []
	_lines = []
	_hidden_riders.clear()
	_camera = null
	_line_mesh.clear_surfaces()


## Whether ORQWA's own rider is shown in slot `slot`: unless the script hides it.
func rider_shown(slot: int = 0) -> bool:
	return not _hidden_riders.has(slot)


## The script's camera hint: `{x, y, elevation_m, heading, back_m, up_m}`, or `null`.
func camera_hint() -> Variant:
	return _camera


## Follows `world` (`ride_state()["world"]`, or `null` without a script).
func show_world(world: Variant, orqwa: OrqwaApp) -> void:
	_orqwa = orqwa
	if world == null:
		if _answers >= 0:
			clear()
		return
	var info: Dictionary = world
	var answers: int = info["answers"]
	if answers != _answers:
		_answers = answers
		_read_answer(info)
	_take_models()
	var poses: Array = info["poses"]
	_camera = info["camera"]
	var transforms: Dictionary[String, Transform3D] = {}
	for index: int in mini(_nodes.size(), poses.size()):
		var node: Dictionary = _nodes[index]
		var id: String = node.get("id", "")
		var model: String = node.get("model", "")
		var placed: Node3D = _node_for(id, model)
		if placed == null:
			continue
		var pose: Variant = poses[index]
		var visible_wanted: bool = node.get("visible", true)
		placed.visible = pose != null and visible_wanted
		if not placed.visible:
			continue
		var where: Dictionary = pose
		placed.transform = _transform(node, where)
		transforms[id] = placed.global_transform
		_play(placed, node.get("clip"))
	_draw_lines(transforms)


func _read_answer(info: Dictionary) -> void:
	var text: String = info["output"]
	var parsed: Variant = JSON.parse_string(text)
	var output: Dictionary = parsed if parsed is Dictionary else {}
	var scene: Dictionary = output.get("scene", {})
	_nodes = scene.get("nodes", [])
	_lines = scene.get("lines", [])
	_hidden_riders.clear()
	var riders: Array = scene.get("riders", [])
	for rider: Dictionary in riders:
		if not rider.get("shown", true):
			_hidden_riders[int(rider.get("slot", 0))] = true
	var models: Dictionary = info["models"]
	var wanted: Dictionary[String, bool] = {}
	for node: Dictionary in _nodes:
		var model: String = node.get("model", "")
		wanted[node.get("id", "")] = true
		if models.has(model) and not _scenes.has(model) and not _asked.has(model):
			var file: String = models[model]
			_asked[model] = file
			_orqwa.request_world_part(file)
	# Nodes the answer no longer names are let go of.
	for id: String in _placed.keys():
		if not wanted.has(id):
			_placed[id].queue_free()
			_placed.erase(id)
			_placed_model.erase(id)


## Makes the scenes of the models the core has read.
func _take_models() -> void:
	for model: String in _asked.keys():
		var part: Variant = _orqwa.take_world_part(_asked[model])
		if part == null:
			continue
		_asked.erase(model)
		if not part is PackedByteArray:
			push_warning("world script model %s: %s" % [model, part])
			continue
		var document: GLTFDocument = GLTFDocument.new()
		var state: GLTFState = GLTFState.new()
		var bytes: PackedByteArray = part
		if document.append_from_buffer(bytes, "", state) != OK:
			push_warning("world script model %s is not readable glTF" % model)
			continue
		var scene: Node3D = document.generate_scene(state) as Node3D
		if scene != null:
			_scenes[model] = scene


## The node placed as `id`, showing `model`: made once its model's scene is there.
func _node_for(id: String, model: String) -> Node3D:
	if _placed.has(id) and _placed_model[id] == model:
		return _placed[id]
	if _placed.has(id):
		_placed[id].queue_free()
		_placed.erase(id)
	var placed: Node3D
	if model == "rider":
		var avatar: RiderAvatar = RiderAvatar.new()
		var look: String = _orqwa.profile().get("avatar", RiderAvatar.RIDERS[0])
		avatar.rider = look
		placed = avatar
	elif _scenes.has(model):
		placed = _scenes[model].duplicate() as Node3D
	else:
		return null
	add_child(placed)
	_placed[id] = placed
	_placed_model[id] = model
	return placed


## Right, up and forward of the anchor along the road; Godot looks along −z (north), headings run
## clockwise from north.
static func _transform(node: Dictionary, pose: Dictionary) -> Transform3D:
	var heading: float = pose["heading"]
	var x: float = pose["x"]
	var y: float = pose["y"]
	var elevation: float = pose["elevation_m"]
	var yaw: Basis = Basis(Vector3.UP, -heading)
	var offset: Array = node.get("offset_m", [0.0, 0.0, 0.0])
	var local: Vector3 = Vector3(float(offset[0]), float(offset[1]), -float(offset[2]))
	var angles: Array = node.get("rotation_deg", [0.0, 0.0, 0.0])
	var turned: Basis = Basis.from_euler(
		Vector3(
			deg_to_rad(float(angles[0])),
			-deg_to_rad(float(angles[1])),
			deg_to_rad(float(angles[2]))
		)
	)
	var size: float = node.get("scale", 1.0)
	var origin: Vector3 = Vector3(x, elevation, -y) + yaw * local
	return Transform3D((yaw * turned).scaled(Vector3.ONE * size), origin)


## Plays the model's own clip `clip` (`{name, rate, phase}`), if it has one by that name.
static func _play(placed: Node3D, clip: Variant) -> void:
	if clip == null:
		return
	var info: Dictionary = clip
	var players: Array[Node] = placed.find_children("*", "AnimationPlayer", true, false)
	if players.is_empty():
		return
	var player: AnimationPlayer = players[0] as AnimationPlayer
	var clip_name: String = info.get("name", "")
	if not player.has_animation(clip_name):
		return
	if player.current_animation != clip_name:
		player.play(clip_name)
		player.get_animation(clip_name).loop_mode = Animation.LOOP_LINEAR
	var phase: Variant = info.get("phase")
	if phase != null:
		var length: float = player.get_animation(clip_name).length
		player.speed_scale = 0.0
		player.seek(clampf(float(phase), 0.0, 1.0) * length, true)
	else:
		player.speed_scale = float(info.get("rate", 1.0))


func _draw_lines(transforms: Dictionary[String, Transform3D]) -> void:
	_line_mesh.clear_surfaces()
	var drawn: PackedVector3Array = PackedVector3Array()
	for line: Dictionary in _lines:
		var start: Dictionary = line.get("from", {})
		var finish: Dictionary = line.get("to", {})
		var from: Vector3 = _line_point(start, transforms)
		var to: Vector3 = _line_point(finish, transforms)
		if from != NOWHERE and to != NOWHERE:
			drawn.append(from)
			drawn.append(to)
	if drawn.is_empty():
		return
	_line_mesh.surface_begin(Mesh.PRIMITIVE_LINES, _line_material)
	for point: Vector3 in drawn:
		_line_mesh.surface_add_vertex(to_local(point))
	_line_mesh.surface_end()


static func _line_point(end: Dictionary, transforms: Dictionary[String, Transform3D]) -> Vector3:
	var id: String = end.get("node", "")
	if not transforms.has(id):
		return NOWHERE
	var point: Array = end.get("point", [0.0, 0.0, 0.0])
	return transforms[id] * Vector3(float(point[0]), float(point[1]), -float(point[2]))
