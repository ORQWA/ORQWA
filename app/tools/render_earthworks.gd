extends SceneTree
## Offline road-earthworks review, using meshes exported by earthworks_render_fixture.


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var source: String = OS.get_environment("FIXTURE")
	if source.is_empty():
		source = ProjectSettings.globalize_path("res://../screenshots/earthworks.json")
	var scenes: Array = JSON.parse_string(FileAccess.get_file_as_string(source))
	var out: String = OS.get_environment("OUT_DIR")
	for data: Dictionary in scenes:
		var scene: Node3D = Node3D.new()
		root.add_child(scene)
		var slope: float = data["slope"]
		var environment: WorldEnvironment = WorldEnvironment.new()
		environment.environment = Environment.new()
		environment.environment.background_mode = Environment.BG_COLOR
		environment.environment.background_color = Color(0.49, 0.62, 0.72)
		environment.environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
		environment.environment.ambient_light_color = Color(0.7, 0.77, 0.85)
		environment.environment.ambient_light_energy = 0.6
		environment.environment.tonemap_mode = Environment.TONE_MAPPER_AGX
		scene.add_child(environment)
		var sun: DirectionalLight3D = DirectionalLight3D.new()
		sun.rotation_degrees = Vector3(-52.0, -35.0, 0.0)
		sun.light_energy = 2.0
		sun.shadow_enabled = true
		scene.add_child(sun)
		for mesh_data: Dictionary in data["meshes"]:
			var instance: MeshInstance3D = MeshInstance3D.new()
			instance.mesh = WorldMeshes.from_arrays(_arrays(mesh_data))
			var material: ShaderMaterial = ShaderMaterial.new()
			if mesh_data["terrain"]:
				_terrain(material)
			else:
				RoadMaterials.configure(material, false)
			instance.material_override = material
			scene.add_child(instance)
		var camera: Camera3D = Camera3D.new()
		scene.add_child(camera)
		camera.position = Vector3(90.0, slope * 320.0 + 200.0, -290.0)
		camera.look_at(Vector3(320.0, slope * 320.0, -490.0))
		camera.projection = Camera3D.PROJECTION_ORTHOGONAL
		camera.size = 350.0
		camera.current = true
		await _save(out.path_join("slope-%s-overview.png" % slope))
		camera.projection = Camera3D.PROJECTION_PERSPECTIVE
		camera.position = Vector3(178.0, slope * 200.0 + 13.0, -465.0)
		camera.look_at(Vector3(202.0, slope * 200.0, -495.0))
		await _save(out.path_join("slope-%s-close.png" % slope))
		camera.position = Vector3(302.0, slope * 320.0 + 10.0, -465.0)
		camera.look_at(Vector3(321.0, slope * 320.0, -495.0))
		await _save(out.path_join("slope-%s-track.png" % slope))
		scene.queue_free()
		await process_frame
	quit()


func _save(path: String) -> void:
	for frame: int in range(8):
		await process_frame
	root.get_texture().get_image().save_png(path)
	print("saved earthworks: " + path)


func _terrain(material: ShaderMaterial) -> void:
	material.shader = preload("res://shaders/terrain.gdshader")
	for surface: String in ["grass", "dirt", "gravel", "rock"]:
		for channel: String in ["albedo", "normal"]:
			material.set_shader_parameter(
				surface + "_" + channel,
				load("res://assets/textures/%s_%s.webp" % [surface, channel])
			)
	UrbanSurfaces.configure_terrain(material)
	for cover: String in ["meadow", "farmland", "orchard", "forest", "town", "industrial", "rock"]:
		var colour: Color = Palette.color("ground." + cover)
		material.set_shader_parameter("cover_" + cover, Vector3(colour.r, colour.g, colour.b))
	material.set_shader_parameter("snow_color", Palette.color("ground.snow"))


func _arrays(data: Dictionary) -> Dictionary:
	var vertices: PackedVector3Array = PackedVector3Array()
	var normals: PackedVector3Array = PackedVector3Array()
	var uvs: PackedVector2Array = PackedVector2Array()
	var colors: PackedColorArray = PackedColorArray()
	var indices: Array = data["indices"]
	for v: Array in data["vertices"]:
		var p: PackedFloat32Array = PackedFloat32Array(v)
		vertices.append(Vector3(p[0], p[1], p[2]))
	for v: Array in data["normals"]:
		var p: PackedFloat32Array = PackedFloat32Array(v)
		normals.append(Vector3(p[0], p[1], p[2]))
	for v: Array in data["uvs"]:
		var p: PackedFloat32Array = PackedFloat32Array(v)
		uvs.append(Vector2(p[0], p[1]))
	for v: Array in data["colors"]:
		var p: PackedFloat32Array = PackedFloat32Array(v)
		colors.append(Color(p[0], p[1], p[2], p[3]))
	return {
		"vertices": vertices,
		"normals": normals,
		"uvs": uvs,
		"colors": colors,
		"indices": PackedInt32Array(indices),
	}
