extends Control
## Root scene: owns the Torqa node and switches between the start page, the ride, the overlay
## (R55) and the ride summary.

## Video courses are ridden along their video instead of the 3D world (R17).
var _video: VideoView = VideoView.new()
var _overlay: OverlayWindow
## Over everything, the window opaque but the overlay's: other windows showed through the sky
## on Windows (#220, see opaque.gdshader).
var _opaque: ColorRect = ColorRect.new()
var _failures: FailureDetails = FailureDetails.new()

@onready var _torqa: TorqaApp = $Torqa
@onready var _world: RideWorld = $World
@onready var _start: StartPage = $StartPage
@onready var _ride: RideScreen = $RideScreen
@onready var _summary: HistoryScreen = $HistoryScreen


func _ready() -> void:
	print("Torqa %s" % TorqaCore.version())
	theme = UiTheme.build()
	add_child(_failures)
	_failures.set_anchors_and_offsets_preset(Control.PRESET_BOTTOM_RIGHT)
	_failures.offset_left = -584
	_failures.offset_right = -24
	_failures.offset_top = -260
	_failures.offset_bottom = -24
	_torqa.failure_reported.connect(_failures.report)
	add_child(_video)
	move_child(_video, _world.get_index() + 1)
	_world.bind(_torqa)
	_world.apply_quality(_torqa.graphics_quality())
	_start.bind(_torqa)
	_ride.bind(_torqa, _world)
	_summary.bind(_torqa)
	move_child(_failures, get_child_count() - 1)
	_start.ride_started.connect(_on_ride_started)
	_overlay = OverlayWindow.new(get_window())
	# On macOS the window comes up as if transparent (per-pixel transparency is allowed, for
	# the overlay), and a transparent viewport draws no sky: black behind the world until the
	# overlay has been turned on and off once (#156, #149). Say so plainly, as leaving the
	# overlay does.
	get_window().transparent = false
	get_window().transparent_bg = false
	var opaque: ShaderMaterial = ShaderMaterial.new()
	opaque.shader = preload("res://shaders/opaque.gdshader")
	_opaque.material = opaque
	_opaque.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_opaque.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(_opaque)
	_ride.closed.connect(_back_home)
	_ride.summary_requested.connect(_show_summary)
	_ride.overlay_requested.connect(_set_overlay)
	_ride.overlay_zoom_requested.connect(_overlay.zoom)
	get_window().size_changed.connect(_on_window_resized)
	_summary.closed.connect(_back_home)
	# As the rider left it (#218).
	if _torqa.fullscreen():
		get_window().mode = Window.MODE_FULLSCREEN


func _on_ride_started(options: Dictionary) -> void:
	_start.hide()
	var on_its_own: bool = options.has("workout") and not options.get("on_course", false)
	if on_its_own:
		pass  # A workout on its own (R58) is the ride screen alone, over its own backdrop.
	elif not _torqa.riding_along_video():
		_world.apply_quality(_torqa.graphics_quality())
		_world.apply_options(options)
		_world.reset_view()
		_world.show()
	else:
		var video_sound: bool = options.get("video_sound", true)
		_torqa.set_video_sound(video_sound)
		_video.begin(_torqa)
		_video.show()
	_ride.begin(options)
	_ride.show()
	if options.get("overlay", false):
		_set_overlay(true)


## Turns the window into the overlay (R55, R57) or back; the ride goes on either way.
func _set_overlay(on: bool) -> void:
	if on == _overlay.active:
		return
	if on:
		_ride.set_overlay(true)
		_video.hide()
		_opaque.hide()
		# One frame for the overlay's content to take its size.
		await get_tree().process_frame
		_overlay.enter(
			_ride.overlay_content_size(), _torqa.overlay_window(), _torqa.overlay_scale()
		)
		await get_tree().process_frame
		_overlay.set_clickable(_ride.overlay_outline())
	else:
		# Taken first: restoring the window resizes it, which is no choice of the rider's.
		var chosen: float = _overlay.scale
		_torqa.set_overlay_window(_overlay.leave(), chosen)
		_ride.set_overlay(false)
		_video.visible = _ride.visible and _torqa.riding_along_video()
		_opaque.show()


func _on_window_resized() -> void:
	if _overlay.active:
		_overlay.window_resized()
		_overlay.set_clickable(_ride.overlay_outline())


## F11, or Alt+Enter, fills the screen or puts Torqa back in a window (#218), from any page.
func _shortcut_input(event: InputEvent) -> void:
	var key: InputEventKey = event as InputEventKey
	if key == null or not key.pressed or key.echo:
		return
	if key.keycode != KEY_F11 and not (key.keycode == KEY_ENTER and key.alt_pressed):
		return
	# The overlay is a small window of its own; leaving it brings full screen back.
	if not _overlay.active:
		FullScreen.set_on(get_window(), _torqa, not FullScreen.is_on(get_window()))
	get_viewport().set_input_as_handled()


func _notification(what: int) -> void:
	if what != NOTIFICATION_WM_CLOSE_REQUEST or _overlay == null:
		return
	if _overlay.active:
		# Quit from the overlay: it opens there next time.
		_torqa.set_overlay_window(_overlay.geometry(), _overlay.scale)
	else:
		# Full screen may also have been turned on or off by the system (macOS's green button).
		_torqa.set_fullscreen(FullScreen.is_on(get_window()))


## The summary of the ride just saved (R42); closing it returns to the start page.
func _show_summary(path: String) -> void:
	_ride.hide()
	_world.hide()
	_video.hide()
	_summary.open_summary(path)
	_summary.show()


func _back_home() -> void:
	_ride.hide()
	_world.hide()
	_video.hide()
	_summary.hide()
	_start.refresh()
	_start.show()
