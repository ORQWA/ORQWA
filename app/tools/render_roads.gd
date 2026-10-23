extends SceneTree
## Controlled profile comparison, generated entirely in Rust without map downloads.
## cargo test -p orqwa-world road_profiles_render_fixture -- --ignored
## OUT=<absolute PNG> xvfb-run godot --path app -s res://tools/render_roads.gd


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var fixture_path: String = ProjectSettings.globalize_path(
		"res://../screenshots/road-profiles.json"
	)
	var fixture: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(fixture_path))
	var scene: Node3D = Node3D.new()
	root.add_child(scene)
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
	var ground: MeshInstance3D = MeshInstance3D.new()
	var plane: PlaneMesh = PlaneMesh.new()
	plane.size = Vector2(400.0, 1400.0)
	ground.mesh = plane
	ground.position = Vector3(65.0, -0.2, -500.0)
	var ground_material: StandardMaterial3D = StandardMaterial3D.new()
	ground_material.albedo_color = Color(0.13, 0.19, 0.09)
	ground_material.roughness = 1.0
	ground.material_override = ground_material
	scene.add_child(ground)
	for data: Dictionary in fixture["meshes"]:
		var instance: MeshInstance3D = MeshInstance3D.new()
		instance.mesh = WorldMeshes.from_arrays(_arrays(data))
		var material: ShaderMaterial = ShaderMaterial.new()
		var edged: bool = data["shoulders"]
		RoadMaterials.configure(material, edged)
		instance.material_override = material
		scene.add_child(instance)
	for data: Dictionary in fixture["labels"]:
		var label: Label3D = Label3D.new()
		var text: String = data["text"]
		var east: float = data["east"]
		label.text = text.replace(" ", "\n")
		label.position = Vector3(east, 0.5, -430.0)
		label.rotation_degrees.x = -90.0
		label.pixel_size = 0.065
		label.font_size = 42
		label.outline_size = 6
		scene.add_child(label)
	var camera: Camera3D = Camera3D.new()
	scene.add_child(camera)
	camera.position = Vector3(67.0, 120.0, -365.0)
	camera.look_at(Vector3(67.0, 0.0, -470.0))
	camera.projection = Camera3D.PROJECTION_ORTHOGONAL
	camera.size = 100.0
	camera.current = true
	var title: Label = Label.new()
	title.text = "Road profiles — estimated widths and lane layouts"
	title.position = Vector2(28.0, 22.0)
	title.add_theme_font_size_override("font_size", 26)
	root.add_child(title)
	for frame: int in range(8):
		await process_frame
	var out: String = OS.get_environment("OUT")
	root.get_texture().get_image().save_png(out)
	print("saved road profiles: " + out)
	quit()


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
