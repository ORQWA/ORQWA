class_name GraphicsBenchmark
extends Window
## An isolated, full-resolution preview, timed only after uploads finish (R150).

signal finished(result: Dictionary)

const TIMEOUT_MS: int = 120000

var _viewport: SubViewport = SubViewport.new()
var _world: BenchmarkWorld
var _run: OrqwaGraphicsBenchmark
var _status: Label = Label.new()
var _active: bool = false
var _saved_fps: int
var _saved_vsync: DisplayServer.VSyncMode
var _saved_atlas: int
var _started: int
var _last: int = 0
var _elapsed: float = 0.0


func _ready() -> void:
	title = tr("Graphics benchmark")
	theme = UiTheme.build()
	exclusive = true
	unresizable = true
	transient = true
	close_requested.connect(cancel)
	visibility_changed.connect(_visibility_changed)
	var content: VBoxContainer = VBoxContainer.new()
	content.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(content)
	var preview: TextureRect = TextureRect.new()
	preview.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	preview.stretch_mode = TextureRect.STRETCH_SCALE
	preview.size_flags_vertical = Control.SIZE_EXPAND_FILL
	content.add_child(preview)
	_viewport.own_world_3d = true
	_viewport.render_target_update_mode = SubViewport.UPDATE_ALWAYS
	add_child(_viewport)
	preview.texture = _viewport.get_texture()
	var bar: HBoxContainer = HBoxContainer.new()
	bar.add_theme_constant_override("separation", 16)
	content.add_child(bar)
	_status.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	bar.add_child(_status)
	var stop: Button = Button.new()
	stop.text = tr("Cancel")
	stop.pressed.connect(cancel)
	bar.add_child(stop)
	set_process(false)


## Returns false without changing anything when there is no real renderer.
func start() -> bool:
	if DisplayServer.get_name() == "headless" or _active:
		return false
	_saved_fps = Engine.max_fps
	_saved_vsync = DisplayServer.window_get_vsync_mode()
	_saved_atlas = RideWorld.shadow_atlas_size
	_viewport.size = get_tree().root.get_texture().get_size()
	_run = OrqwaGraphicsBenchmark.new()
	_world = (load("res://scenes/benchmark_world.tscn") as PackedScene).instantiate()
	# Scene resources otherwise share the ride's environment, including its sky.
	var environment: WorldEnvironment = _world.get_node("Environment")
	environment.environment = environment.environment.duplicate(true)
	_viewport.add_child(_world)
	_world.show_benchmark(_run)
	_active = true
	_started = Time.get_ticks_msec()
	_select_preset()
	RenderingServer.frame_post_draw.connect(_drawn)
	get_tree().root.size_changed.connect(cancel)
	set_process(true)
	UiScale.popup(self, get_tree().root.size)
	return true


func _process(_delta: float) -> void:
	if Time.get_ticks_msec() - _started > TIMEOUT_MS:
		cancel()


func _notification(what: int) -> void:
	if what == NOTIFICATION_APPLICATION_FOCUS_OUT:
		cancel()


func _visibility_changed() -> void:
	if not visible:
		cancel()


func _unhandled_key_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_cancel"):
		cancel()
		set_input_as_handled()


func _exit_tree() -> void:
	_restore()


func _select_preset() -> void:
	var quality: String = _run.quality()
	_world.apply_quality(quality, {"fps_limit": "unlimited", "vsync": "off"})
	_status.text = tr("Testing %s… Keep this window in front.") % tr(quality.capitalize())
	_elapsed = 0.0
	_last = 0
	_world.benchmark_camera(0.0)


func _drawn() -> void:
	if not _active or not _world.uploads_complete():
		_last = 0
		return
	var now: int = Time.get_ticks_usec()
	if _last == 0:
		_last = now
		return
	var seconds: float = (now - _last) / 1000000.0
	_last = now
	_elapsed += seconds
	_world.benchmark_camera(_elapsed)
	if not _run.frame(seconds):
		return
	var result: Dictionary = _run.result()
	if result.is_empty():
		_select_preset()
	else:
		_finish.call_deferred(result)


## Escape, closing, a display resize or losing focus leaves settings untouched.
func cancel() -> void:
	if _active:
		_finish({})


func _finish(result: Dictionary) -> void:
	if not _active:
		return
	_restore()
	hide()
	finished.emit(result)
	queue_free()


func _restore() -> void:
	if not _active:
		return
	_active = false
	set_process(false)
	RenderingServer.frame_post_draw.disconnect(_drawn)
	get_tree().root.size_changed.disconnect(cancel)
	Engine.max_fps = _saved_fps
	DisplayServer.window_set_vsync_mode(_saved_vsync)
	RenderingServer.directional_shadow_atlas_set_size(_saved_atlas, true)
	RideWorld.shadow_atlas_size = _saved_atlas
	_viewport.render_target_update_mode = SubViewport.UPDATE_DISABLED
