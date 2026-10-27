extends SceneTree
## R151: the rendering boundary, complete results and cancellation without changing settings.

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	TranslationServer.set_locale("en")
	var main: Control = (load("res://scenes/main.tscn") as PackedScene).instantiate()
	root.add_child(main)
	await process_frame
	var orqwa: OrqwaApp = main.get_node("Orqwa")
	var start: StartPage = main.get_node("StartPage")
	var settings: SettingsDialog = start.get("_settings")
	var graphics: GraphicsSettings = settings.get("_graphics")
	var original: String = orqwa.graphics_quality()
	var options: Dictionary = orqwa.graphics_options()
	_check(orqwa.set_graphics_option("shadows", "ultra"), "override before calibration")
	graphics.call("_benchmark_finished", {})
	_check(orqwa.graphics_quality() == original, "cancel keeps the preset")
	var kept_shadows: String = orqwa.graphics_options()["shadows"]
	_check(kept_shadows == "ultra", "cancel keeps details")
	var run: OrqwaGraphicsBenchmark = OrqwaGraphicsBenchmark.new()
	_check(run.result().is_empty(), "no recommendation before drawing")
	_check(not run.road().is_empty(), "road is available without loading a course")
	var chunk: Dictionary = run.chunk(1.0)
	var plants: Dictionary = chunk["plants"]
	_check(not plants.is_empty(), "offline models to exercise density")
	for seconds: float in [0.01, 0.012, 0.025, 0.04]:
		_check(not run.frame(3.0), "shader warm-up")
		while not run.frame(seconds):
			pass
	var result: Dictionary = run.result()
	var recommended: String = result["quality"]
	_check(recommended == "medium", "a complete run chooses the measured preset")
	graphics.call("_benchmark_finished", result)
	var report: Label = graphics.get("_benchmark_note")
	_check(report.text.contains("fps"), "completed calibration reports measured FPS")
	_check(orqwa.graphics_quality() == "medium", "the recommendation is saved")
	for value: String in orqwa.graphics_options().values():
		_check(value == "auto", "all details follow the measured preset")
	var quality: OptionButton = graphics.get("_quality")
	var selected: String = quality.get_item_metadata(quality.selected)
	_check(selected == "medium", "selection refreshes")
	_check(orqwa.world_chunk_count() == 0, "the rider's course is untouched")
	_check(orqwa.ride_state().is_empty(), "no ride is started or recorded")
	if DisplayServer.get_name() == "headless":
		var preview: GraphicsBenchmark = GraphicsBenchmark.new()
		root.add_child(preview)
		_check(not preview.start(), "dummy rendering cannot calibrate hardware")
		preview.free()
	else:
		var original_scale: float = root.content_scale_factor
		var original_size: Vector2i = root.size
		root.content_scale_factor = 1.25
		root.size = Vector2i((UiScale.MIN_WINDOW_SIZE * root.content_scale_factor).ceil())
		await process_frame
		settings.open()
		await process_frame
		await _rendered_cancellation(graphics, orqwa)
		_check(settings.visible, "the settings reopen after cancellation")
		if OS.get_environment("BENCHMARK_COMPLETE") == "1":
			await _rendered_completion(graphics, orqwa)
			_check(settings.visible, "the settings reopen with the completed result")
		settings.hide()
		root.content_scale_factor = original_scale
		root.size = original_size
	orqwa.set_graphics_quality(original)
	for option: String in options:
		var value: String = options[option]
		orqwa.set_graphics_option(option, value)
	main.free()
	await process_frame
	if not _failed:
		print("BENCHMARK SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _rendered_cancellation(graphics: GraphicsSettings, orqwa: OrqwaApp) -> void:
	var fps: int = Engine.max_fps
	var vsync: DisplayServer.VSyncMode = DisplayServer.window_get_vsync_mode()
	var atlas: int = RideWorld.shadow_atlas_size
	var disabled_3d: bool = root.disable_3d
	graphics.call("_benchmark")
	var preview: GraphicsBenchmark
	for node: Node in root.get_children():
		if node is GraphicsBenchmark:
			preview = node
	_check(preview != null, "the settings button starts a preview")
	if preview == null:
		return
	var viewport: SubViewport = preview.get("_viewport")
	_check(
		viewport.size == root.size,
		"the physical render resolution is independent of interface scaling"
	)
	var world: BenchmarkWorld = preview.get("_world")
	while not world.uploads_complete():
		await process_frame
	await RenderingServer.frame_post_draw
	var live_fps: Label = preview.get("_fps")
	_check(live_fps.is_visible_in_tree(), "live FPS is visible beside the preset under test")
	_check(live_fps.text.contains("fps"), "live FPS has its unit")
	_check(live_fps.text != "— fps", "live FPS refreshes while the preview draws")
	_check(viewport.size == root.get_texture().get_image().get_size(), "actual rendered pixels")
	_check(root.disable_3d, "the hidden ride's 3D is suspended during calibration")
	_check(Engine.max_fps == 0, "the frame cap is suspended during measurement")
	_check(
		DisplayServer.window_get_vsync_mode() == DisplayServer.VSYNC_DISABLED,
		"V-Sync is suspended during measurement"
	)
	var out: String = OS.get_environment("BENCHMARK_SCREENSHOT")
	if not out.is_empty():
		root.get_texture().get_image().save_png(out)
	preview.cancel()
	await process_frame
	await process_frame
	_check(Engine.max_fps == fps, "cancelling restores the previous frame cap")
	_check(DisplayServer.window_get_vsync_mode() == vsync, "cancelling restores V-Sync")
	_check(RideWorld.shadow_atlas_size == atlas, "cancelling restores the shared shadow atlas")
	_check(root.disable_3d == disabled_3d, "cancelling restores background 3D")
	_check(orqwa.graphics_quality() == "medium", "cancelling preserves the preset")
	_check(orqwa.world_chunk_count() == 0, "preview never changes the loaded course")


func _rendered_completion(graphics: GraphicsSettings, orqwa: OrqwaApp) -> void:
	var fps: int = Engine.max_fps
	var vsync: DisplayServer.VSyncMode = DisplayServer.window_get_vsync_mode()
	var atlas: int = RideWorld.shadow_atlas_size
	var disabled_3d: bool = root.disable_3d
	_check(orqwa.set_graphics_option("grass", "far"), "override before the complete run")
	graphics.call("_benchmark")
	var result: Array[Dictionary] = [{}]
	var preview: GraphicsBenchmark
	for node: Node in root.get_children():
		if node is GraphicsBenchmark:
			preview = node
	_check(preview != null, "a cancelled run can be started again")
	if preview == null:
		return
	preview.finished.connect(func(value: Dictionary) -> void: result[0] = value)
	while is_instance_valid(preview):
		await process_frame
	await process_frame
	await process_frame
	_check(not result[0].is_empty(), "all four presets complete with real rendered frames")
	if not result[0].is_empty():
		var chosen: String = result[0]["quality"]
		_check(orqwa.graphics_quality() == chosen, "completion applies its actual recommendation")
		var width: int = result[0]["width"]
		var height: int = result[0]["height"]
		_check(Vector2i(width, height) == root.size, "result identifies the tested physical pixels")
		var report: Label = graphics.get("_benchmark_note")
		_check(report.is_visible_in_tree(), "the result is visible after returning to Settings")
		_check(
			(
				report.text.contains("fps")
				and report.text.contains("Tested at %d × %d pixels." % [width, height])
			),
			"the result shows measured FPS and resolution"
		)
		var grass: String = orqwa.graphics_options()["grass"]
		_check(grass == "auto", "completion resets overrides")
	_check(Engine.max_fps == fps, "completion restores the live frame cap")
	_check(DisplayServer.window_get_vsync_mode() == vsync, "completion restores V-Sync")
	_check(RideWorld.shadow_atlas_size == atlas, "completion restores the shared shadow atlas")
	_check(root.disable_3d == disabled_3d, "completion restores background 3D")
	_check(orqwa.ride_state().is_empty(), "completion never starts a ride")


func _check(ok: bool, message: String) -> void:
	if not ok:
		_failed = true
		push_error("BENCHMARK: " + message)
