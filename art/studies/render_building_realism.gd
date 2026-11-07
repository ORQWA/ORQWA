extends SceneTree
## Fixed camera and lighting comparisons; the study never loads or changes a rider's library.

var _camera: Camera3D = Camera3D.new()
var _building: MultiMeshInstance3D = MultiMeshInstance3D.new()
var _stage: Node3D = Node3D.new()
var _out: String = "res://../renders"


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	if not OS.get_environment("OUT_DIR").is_empty():
		_out = OS.get_environment("OUT_DIR")
	DirAccess.make_dir_recursive_absolute(_out)
	if OS.get_environment("COMPOSE_ONLY") == "1":
		await _comparisons()
		_camera.free()
		_building.free()
		_stage.free()
		quit(0)
		return
	_stage_scene()
	for kind: String in ["house", "town"]:
		if not OS.get_environment("KINDS").is_empty() and kind != OS.get_environment("KINDS"):
			continue
		_place_stage(kind)
		for version: String in ["before", "after"]:
			_show(kind, version)
			for view: String in ["street", "detail"]:
				_camera_view(kind, view)
				for frame: int in range(6):
					await process_frame
				await RenderingServer.frame_post_draw
				var picture: Image = root.get_texture().get_image()
				var name: String = "%s-%s-%s.png" % [kind, view, version]
				picture.save_png(_out.path_join(name))
				print("STUDY rendered ", name)
	await _comparisons()
	quit(0)


func _comparisons() -> void:
	var gallery: String = OS.get_environment("GALLERY")
	if gallery.is_empty():
		gallery = _out.path_join("comparisons")
	DirAccess.make_dir_recursive_absolute(gallery)
	for kind: String in ["house", "town"]:
		if not OS.get_environment("KINDS").is_empty() and kind != OS.get_environment("KINDS"):
			continue
		for view: String in ["street", "detail"]:
			var canvas: SubViewport = SubViewport.new()
			canvas.disable_3d = true
			canvas.render_target_update_mode = SubViewport.UPDATE_ALWAYS
			root.add_child(canvas)
			for side: int in range(2):
				var version: String = "before" if side == 0 else "after"
				var file: String = "%s-%s-%s.png" % [kind, view, version]
				var picture: Image = Image.load_from_file(_out.path_join(file))
				canvas.size = Vector2i(picture.get_width() * 2, picture.get_height() + 68)
				var panel: TextureRect = TextureRect.new()
				panel.texture = ImageTexture.create_from_image(picture)
				panel.position = Vector2(side * picture.get_width(), 68)
				canvas.add_child(panel)
				var label: Label = Label.new()
				label.position = Vector2(side * picture.get_width() + 28, 16)
				label.add_theme_font_size_override("font_size", 28)
				if side == 1:
					label.text = "PROTOTYPE"
				else:
					label.text = "CURRENT MODEL" if kind == "house" else "CURRENT SHELL STYLE"
				canvas.add_child(label)
			for frame: int in range(3):
				await process_frame
			await RenderingServer.frame_post_draw
			var board: Image = canvas.get_texture().get_image()
			var suffix: String = "" if view == "detail" else "-street"
			board.save_jpg(gallery.path_join(kind + suffix + "-comparison.jpg"), 0.9)
			canvas.queue_free()
			print("STUDY compared ", kind, " ", view)
	await process_frame


func _stage_scene() -> void:
	root.add_child(_stage)
	root.add_child(_building)
	root.add_child(_camera)
	_camera.make_current()
	_camera.fov = 53.0
	var environment: Environment = Environment.new()
	var sky: Sky = Sky.new()
	var sky_material: ProceduralSkyMaterial = ProceduralSkyMaterial.new()
	sky_material.sky_top_color = Palette.color("sky.top")
	sky_material.sky_horizon_color = Palette.color("sky.horizon")
	sky_material.ground_horizon_color = Palette.color("sky.horizon")
	sky.sky_material = sky_material
	environment.background_mode = Environment.BG_SKY
	environment.sky = sky
	environment.ambient_light_source = Environment.AMBIENT_SOURCE_SKY
	environment.ambient_light_energy = 0.55
	environment.ambient_light_sky_contribution = 0.55
	environment.tonemap_mode = Environment.TONE_MAPPER_FILMIC
	environment.ssao_enabled = true
	environment.ssao_radius = 1.0
	environment.ssao_intensity = 1.5
	var world: WorldEnvironment = WorldEnvironment.new()
	world.environment = environment
	root.add_child(world)
	var sun: DirectionalLight3D = DirectionalLight3D.new()
	sun.rotation_degrees = Vector3(-42.0, -28.0, 0.0)
	sun.light_color = Palette.color("light.sun")
	sun.light_energy = 1.6
	sun.shadow_enabled = true
	sun.directional_shadow_max_distance = 70.0
	root.add_child(sun)
	root.msaa_3d = Viewport.MSAA_4X


func _texture_material(texture: String, scale: float) -> StandardMaterial3D:
	var material: StandardMaterial3D = StandardMaterial3D.new()
	material.albedo_texture = load("res://assets/textures/%s_albedo.webp" % texture)
	material.normal_enabled = true
	material.normal_texture = load("res://assets/textures/%s_normal.webp" % texture)
	material.uv1_triplanar = true
	material.uv1_world_triplanar = true
	material.uv1_scale = Vector3.ONE / scale
	material.roughness = 0.90
	return material


func _box(size: Vector3, position: Vector3, material: Material) -> void:
	var instance: MeshInstance3D = MeshInstance3D.new()
	var box: BoxMesh = BoxMesh.new()
	box.size = size
	box.material = material
	instance.mesh = box
	instance.position = position
	_stage.add_child(instance)


func _place_stage(kind: String) -> void:
	for child: Node in _stage.get_children():
		_stage.remove_child(child)
		child.queue_free()
	var lawn: StandardMaterial3D = _texture_material("grass", 2.0)
	lawn.albedo_color = Color(0.35, 0.40, 0.27)
	_box(Vector3(180, 0.15, 180), Vector3(0, -0.20, 0), lawn)
	var road: StandardMaterial3D = _texture_material("asphalt", 2.0)
	road.albedo_color = Color(0.32, 0.34, 0.36)
	var paving: StandardMaterial3D = _texture_material("paving", 2.0)
	paving.albedo_color = Color(0.72, 0.72, 0.68)
	var concrete: StandardMaterial3D = _texture_material("concrete", 3.0)
	var edge: float = 5.0 if kind == "town" else 4.5
	_box(Vector3(120, 0.13, 2.5), Vector3(0, -0.06, edge + 1.22), paving)
	_box(Vector3(120, 0.20, 0.14), Vector3(0, -0.10, edge + 2.5), concrete)
	_box(Vector3(120, 0.12, 7.0), Vector3(0, -0.14, edge + 6.07), road)
	_box(Vector3(120, 0.13, 2.5), Vector3(0, -0.06, edge + 10.9), paving)
	if kind == "house":
		_box(Vector3(1.5, 0.08, 4.0), Vector3(-2.0, -0.01, edge + 0.4), paving)


func _show(kind: String, version: String) -> void:
	var mesh: ArrayMesh = BuildingModels.mesh(kind + "_" + version).duplicate()
	var wall_tint: Color = Palette.colors("buildings.walls")[0]
	var roof_tint: Color = Palette.colors("buildings.tiles")[1]
	for surface: int in mesh.get_surface_count():
		var material: ShaderMaterial = mesh.surface_get_material(surface)
		if kind == "town" and version == "before" and material.get_shader_parameter("pattern") == 0:
			material = ShaderMaterial.new()
			material.shader = load("res://shaders/shell.gdshader")
			material.set_shader_parameter("study_tint", wall_tint)
			for name: String in ["plaster", "boards", "tiles", "gravel"]:
				var texture: String = {"boards": "planks", "tiles": "roof_tiles"}.get(name, name)
				material.set_shader_parameter(
					name + "_texture", load("res://assets/textures/%s_albedo.webp" % texture)
				)
			mesh.surface_set_material(surface, material)
		elif version == "after":
			material = material.duplicate()
			material.shader = load("res://shaders/study.gdshader")
			var pattern: int = material.get_shader_parameter("pattern")
			if pattern == 0 or pattern == 8:
				var scan: String = "scan_plaster" if pattern == 0 else "scan_roof"
				for channel: String in ["albedo", "normal"]:
					material.set_shader_parameter(
						channel + "_texture",
						load("res://assets/textures/%s_%s.webp" % [scan, channel])
					)
				material.set_shader_parameter(
					"roughness_texture", load("res://assets/textures/%s_rough.webp" % scan)
				)
				material.set_shader_parameter("study_scan", true)
				material.set_shader_parameter("tile", 2.0)
			mesh.surface_set_material(surface, material)
	var multi: MultiMesh = MultiMesh.new()
	multi.transform_format = MultiMesh.TRANSFORM_3D
	multi.use_colors = true
	multi.use_custom_data = true
	multi.mesh = mesh
	multi.instance_count = 1
	multi.set_instance_transform(0, Transform3D.IDENTITY)
	multi.set_instance_color(0, wall_tint)
	multi.set_instance_custom_data(0, Color(roof_tint.r, roof_tint.g, roof_tint.b, 0.5))
	_building.multimesh = multi


func _camera_view(kind: String, view: String) -> void:
	if kind == "house":
		if view == "street":
			_camera.look_at_from_position(Vector3(17.5, 1.75, 24), Vector3(0, 4.2, 0))
		else:
			_camera.look_at_from_position(Vector3(3.0, 1.75, 12), Vector3(0, 3.2, 4.5))
	elif view == "street":
		_camera.look_at_from_position(Vector3(20, 1.75, 29), Vector3(0, 6.3, 0))
	else:
		_camera.look_at_from_position(Vector3(2, 1.75, 14), Vector3(0, 4.4, 5))
