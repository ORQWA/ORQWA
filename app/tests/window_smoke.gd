extends SceneTree
## Headless checks of the app's window: full screen (#218).
## Run: godot --headless --path app -s res://tests/window_smoke.gd

const MAIN_SCENE: String = "res://scenes/main.tscn"

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	await _fullscreen()
	if not _failed:
		print("WINDOW SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## Full screen (#218): F11 and the check box in the settings switch it, and the choice is kept
## for the next start; the overlay stays a window of its own.
func _fullscreen() -> void:
	var main: Control = (load(MAIN_SCENE) as PackedScene).instantiate()
	root.add_child(main)
	await process_frame
	var torqa: TorqaApp = main.get_node("Torqa")
	var window: Window = root
	# Headless, the window has no mode of its own: it is checked on a real display.
	var headless: bool = DisplayServer.get_name() == "headless"
	torqa.set_fullscreen(false)

	_press(KEY_F11)
	_check(torqa.fullscreen(), "F11 fills the screen, also next time")
	_check(headless or FullScreen.is_on(window), "the window fills the screen")
	if not headless:
		_press(KEY_F11)
		_check(not torqa.fullscreen() and not FullScreen.is_on(window), "F11 again: a window")
	torqa.set_fullscreen(false)
	_press(KEY_ENTER, true)
	_check(torqa.fullscreen(), "Alt+Enter as well")

	var box: CheckBox = _button_with_text(main, "Full screen (F11)")
	_check(box != null, "a check box in the settings")
	if box != null:
		box.button_pressed = true
		_check(torqa.fullscreen(), "the check box fills the screen")
		box.button_pressed = false
		_check(not torqa.fullscreen(), "and puts it back in a window")

	var ride: RideScreen = main.get_node("RideScreen")
	ride.begin({"workout": {"kind": "zone", "zone": 3}, "on_course": false})
	ride.show()
	ride.overlay_requested.emit(true)
	for i: int in range(3):
		await process_frame
	_press(KEY_F11)
	_check(not torqa.fullscreen(), "the overlay does not fill the screen")
	ride.overlay_requested.emit(false)
	await process_frame
	main.free()


func _press(key: Key, alt: bool = false) -> void:
	for pressed: bool in [true, false]:
		var event: InputEventKey = InputEventKey.new()
		event.keycode = key
		event.alt_pressed = alt
		event.pressed = pressed
		root.push_input(event)


## The first button under `node` showing `text`.
func _button_with_text(node: Node, text: String) -> Button:
	for button: Node in node.find_children("*", "Button", true, false):
		if (button as Button).text == text:
			return button
	return null


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("WINDOW SMOKE TEST FAILED: " + what)
		_failed = true
