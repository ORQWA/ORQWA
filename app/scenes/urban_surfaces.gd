class_name UrbanSurfaces
extends RefCounted
## Materials and presentation of Rust-generated sidewalks, curbs and building access (R105).

static var _material: ShaderMaterial


static func configure_terrain(terrain: ShaderMaterial) -> void:
	for surface: String in ["paving", "asphalt"]:
		for channel: String in ["albedo", "normal"]:
			terrain.set_shader_parameter(
				surface + "_" + channel,
				load("res://assets/textures/%s_%s.webp" % [surface, channel])
			)
	for cover: String in ["urban", "commercial"]:
		var code: Color = Palette.color("ground." + cover)
		terrain.set_shader_parameter("cover_" + cover, Vector3(code.r, code.g, code.b))


static func set_conditions(raining: float, horizon: Color) -> void:
	material().set_shader_parameter("wetness", raining)
	material().set_shader_parameter("puddles", raining)
	material().set_shader_parameter("puddle_color", horizon)


static func material() -> ShaderMaterial:
	if _material == null:
		_material = ShaderMaterial.new()
		_material.shader = preload("res://shaders/urban.gdshader")
		for surface: String in ["concrete", "paving", "asphalt"]:
			for channel: String in ["albedo", "normal", "rough"]:
				_material.set_shader_parameter(
					surface + "_" + channel,
					load("res://assets/textures/%s_%s.webp" % [surface, channel])
				)
	return _material


static func add_to(parent: Node3D, chunk: Dictionary, distance_m: float) -> void:
	var arrays: Dictionary = chunk["urban"]
	var mesh: ArrayMesh = WorldMeshes.from_arrays(arrays)
	if mesh.get_surface_count() == 0:
		return
	var instance: MeshInstance3D = MeshInstance3D.new()
	instance.mesh = mesh
	instance.material_override = material()
	instance.visibility_range_end = distance_m
	instance.visibility_range_end_margin = 100.0
	instance.visibility_range_fade_mode = GeometryInstance3D.VISIBILITY_RANGE_FADE_SELF
	# The same distance on every preset: a sidewalk is as small on screen on all of them.
	instance.set_meta("fixed_range", true)
	parent.add_child(instance)
