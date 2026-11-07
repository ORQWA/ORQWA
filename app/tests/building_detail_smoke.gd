extends SceneTree
## Ultra is an upload/presentation choice: lighter presets and kept worlds retain their data.

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var benchmark: OrqwaGraphicsBenchmark = OrqwaGraphicsBenchmark.new()
	var ordinary: Dictionary = benchmark.chunk(1.0)
	var detailed: Dictionary = benchmark.chunk_with_building_detail(1.0, true)
	_check(not ordinary.has("building_details"), "ordinary upload allocates no façade details")
	_check(detailed.has("building_details"), "Ultra upload derives façade details")
	var saved_buildings: Dictionary = ordinary["buildings"]
	var uploaded_buildings: Dictionary = detailed["buildings"]
	var saved_models: Array = ordinary["modelled"]
	var uploaded_models: Array = detailed["modelled"]
	_check(saved_buildings == uploaded_buildings, "saved shell data stays unchanged")
	_check(saved_models == uploaded_models, "saved model names and buffers stay unchanged")
	var world: BenchmarkWorld = (
		(load("res://scenes/benchmark_world.tscn") as PackedScene).instantiate()
	)
	root.add_child(world)
	var terrain: Node3D = world.get_node("Terrain")
	for preset: String in ["low", "medium", "high", "ultra"]:
		world.apply_quality(preset, {"view_distance": "ultra", "tree_density": "low"})
		world.show_benchmark(benchmark)
		world.call("_build_some_chunks")
		var detail_count: int = _details(terrain)
		_check(
			detail_count > 0 if preset == "ultra" else detail_count == 0,
			"only Ultra uploads additional geometry, independently of View distance"
		)
	world.apply_quality("ultra", {"view_distance": "low", "tree_density": "low"})
	_check(_details(terrain) > 0, "Ultra retains its close detail under a shorter view range")
	world.apply_quality("ultra", {"view_distance": "ultra", "tree_density": "low"})
	_check(_details(terrain) > 0, "increasing View distance cannot extend close detail")
	world.apply_quality("medium", {"tree_density": "low"})
	_check(terrain.get_child_count() == 0, "downgrading refreshes already uploaded Ultra chunks")
	var next: int = world.get("_next_chunk")
	_check(next == 0, "the same kept course can upload with its new detail budget")
	world.free()
	_models()
	if not _failed:
		print("BUILDING DETAIL SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _details(node: Node) -> int:
	var count: int = 0
	if node.has_meta("building_detail"):
		count = 1
		var instance: GeometryInstance3D = node as GeometryInstance3D
		_check(
			instance.visibility_range_end <= BuildingDetails.RANGE,
			"extended View distance cannot extend close building details"
		)
	for child: Node in node.get_children():
		count += _details(child)
	return count


func _models() -> void:
	var manifest: Dictionary = JSON.parse_string(
		FileAccess.get_file_as_string(BuildingModels.DIRECTORY + "models.json")
	)
	var models: Dictionary = manifest["models"]
	for name: String in models:
		_check(
			ResourceLoader.exists(BuildingModels.DIRECTORY + "detail/" + name + ".glb"),
			"every saved model name has its Ultra appearance: " + name
		)
	var house: ArrayMesh = BuildingModels.mesh("house_gable_2_m")
	var close: ArrayMesh = BuildingModels.mesh("house_gable_2_m", true)
	_check(house != close, "Ultra selects an independent variant")
	_check(_triangles(close) > _triangles(house), "the close house contains extra geometry")
	var glass: bool = false
	var looks: Array[String] = []
	var room_seeds: Dictionary = {}
	for surface: int in close.get_surface_count():
		var material: ShaderMaterial = close.surface_get_material(surface)
		var pattern: int = material.get_shader_parameter("pattern")
		looks.append("%s:%d" % [material.resource_name, pattern])
		if pattern == 21:
			glass = true
			var arrays: Array = close.surface_get_arrays(surface)
			var coordinates: PackedVector2Array = arrays[Mesh.ARRAY_TEX_UV2]
			_check(not coordinates.is_empty(), "Ultra windows retain their stable room UV channel")
			for coordinate: Vector2 in coordinates:
				room_seeds[coordinate.x] = true
	_check(glass, "simulated window interiors belong to the Ultra model: " + ", ".join(looks))
	_check(room_seeds.size() > 1, "different windows can show different interiors")
	for surface: int in house.get_surface_count():
		var material: ShaderMaterial = house.surface_get_material(surface)
		var pattern: int = material.get_shader_parameter("pattern")
		_check(pattern != 21, "ordinary glass stays inexpensive")
	var plaster: ShaderMaterial = BuildingModels.material("plaster")
	var mapped: bool = plaster.get_shader_parameter("roughness_mapped")
	_check(mapped, "shared scanned plaster has its physical roughness map")


func _triangles(mesh: ArrayMesh) -> int:
	var count: int = 0
	for surface: int in mesh.get_surface_count():
		count += mesh.surface_get_array_index_len(surface) / 3
	return count


func _check(ok: bool, message: String) -> void:
	if not ok:
		_failed = true
		push_error(message)
