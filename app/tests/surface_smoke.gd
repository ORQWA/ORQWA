extends SceneTree
## R121: actual rendering must keep roads, streets, paths and railway beds opaque from below.

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var world: RideWorld = (load("res://scenes/world.tscn") as PackedScene).instantiate()
	root.add_child(world)
	world.hide()
	world.set_process(false)
	var viewport: SubViewport = SubViewport.new()
	viewport.size = Vector2i(800, 240)
	viewport.own_world_3d = true
	viewport.transparent_bg = true
	viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	root.add_child(viewport)
	var environment_node: WorldEnvironment = WorldEnvironment.new()
	environment_node.environment = Environment.new()
	environment_node.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	environment_node.environment.ambient_light_color = Color.WHITE
	environment_node.environment.ambient_light_energy = 1.0
	viewport.add_child(environment_node)
	var arrays: Array = []
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = PackedVector3Array(
		[Vector3(-1, 0, 1), Vector3(-1, 0, -1), Vector3(1, 0, -1), Vector3(1, 0, 1)]
	)
	arrays[Mesh.ARRAY_NORMAL] = PackedVector3Array([Vector3.UP, Vector3.UP, Vector3.UP, Vector3.UP])
	arrays[Mesh.ARRAY_TEX_UV] = PackedVector2Array(
		[Vector2(0, 0), Vector2(0, 2), Vector2(1, 2), Vector2(1, 0)]
	)
	arrays[Mesh.ARRAY_INDEX] = PackedInt32Array([0, 1, 2, 0, 2, 3])
	var styles: Array[Color] = [
		Color(0.0, 6.0 / 32.0, 0.25, 0.25),
		Color(0.0, 4.0 / 32.0, 0.0, 0.125),
		Color(0.125, 3.0 / 32.0, 0.0, 0.625)
	]
	var names: Array[String] = [
		"_road_material", "_street_material", "_track_material", "_rail_material"
	]
	for index: int in names.size():
		arrays[Mesh.ARRAY_COLOR] = (
			PackedColorArray([styles[index], styles[index], styles[index], styles[index]])
			if index < styles.size()
			else null
		)
		var mesh: ArrayMesh = ArrayMesh.new()
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		var instance: MeshInstance3D = MeshInstance3D.new()
		instance.mesh = mesh
		instance.material_override = world.get(names[index])
		instance.position.x = -4.5 + index * 3.0
		viewport.add_child(instance)
	var road_material: ShaderMaterial = world.get("_road_material")
	var paving: Texture2D = road_material.get_shader_parameter("concrete_albedo")
	var bridge: Texture2D = road_material.get_shader_parameter("bridge_concrete_albedo")
	if paving == null or bridge == null or paving == bridge:
		push_error("Road paving and bridge concrete must retain separate texture bindings")
		_failed = true
	var camera: Camera3D = Camera3D.new()
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 4.0
	viewport.add_child(camera)
	for side: float in [1.0, -1.0]:
		camera.look_at_from_position(Vector3(0.0, side * 6.0, 0.0), Vector3.ZERO, Vector3.FORWARD)
		for frame: int in range(3):
			await process_frame
		await RenderingServer.frame_post_draw
		var picture: Image = viewport.get_texture().get_image()
		var captures: String = OS.get_environment("SURFACE_CAPTURES")
		if not captures.is_empty():
			DirAccess.make_dir_recursive_absolute(captures)
			picture.save_png(captures.path_join("above.png" if side > 0.0 else "below.png"))
		for index: int in names.size():
			var pixel: Vector2 = camera.unproject_position(Vector3(-4.5 + index * 3.0, 0.0, 0.0))
			if picture.get_pixelv(Vector2i(pixel)).a < 0.95:
				push_error("%s transparent from side %s" % [names[index], side])
				_failed = true
	if not _failed:
		print("SURFACE SMOKE TEST PASSED")
	quit(1 if _failed else 0)
