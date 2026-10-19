extends SceneTree
## Headless checks of the app's window: full screen (#218), and the interface keeping its size
## whatever the window's while the layouts reflow (R94).
## Run: godot --headless --path app -s res://tests/window_smoke.gd

const MAIN_SCENE: String = "res://scenes/main.tscn"

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	await _fullscreen()
	await _fixed_scale()
	await _reflow_grid()
	var ride: String = await _record_ride()
	for locale: String in ["en", "de"]:
		await _small_window(locale, ride)
	TranslationServer.set_locale("en")
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


## The interface keeps the size the system gives it on the screen, whatever the window's (R94):
## a larger window shows more rather than larger text, a smaller one less rather than smaller.
func _fixed_scale() -> void:
	var main: Control = (load(MAIN_SCENE) as PackedScene).instantiate()
	root.add_child(main)
	await process_frame
	var scale: float = root.content_scale_factor
	_check(
		is_equal_approx(scale, UiScale.of_screen(root.current_screen)),
		"as large as the system draws on the screen: %s" % scale
	)
	_check(root.content_scale_mode == Window.CONTENT_SCALE_MODE_DISABLED, "not fit to the window")
	for units: Vector2 in [UiScale.MIN_WINDOW_SIZE, Vector2(1600, 900), Vector2(2560, 1440)]:
		root.size = Vector2i((units * scale).round())
		await process_frame
		var room: Vector2 = root.get_visible_rect().size
		_check(
			is_equal_approx(root.content_scale_factor, scale) and room.is_equal_approx(units),
			"a window of %s units lays out %s, at the same scale" % [units, room]
		)
	_check(
		root.min_size == Vector2i((UiScale.MIN_WINDOW_SIZE * scale).ceil()),
		"no smaller than the layouts are made for: %s" % root.min_size
	)
	main.free()


## As many of a reflowing grid's children side by side as fit its parent (R94), the rest below
## them, and back beside each other once there is room again; an exact fit is too tight.
func _reflow_grid() -> void:
	var parent: VBoxContainer = VBoxContainer.new()
	root.add_child(parent)
	var grid: ReflowGrid = ReflowGrid.new()
	grid.max_columns = 3
	grid.add_theme_constant_override("h_separation", 10)
	parent.add_child(grid)
	for i: int in range(3):
		var cell: Control = Control.new()
		cell.custom_minimum_size = Vector2(100, 20)
		grid.add_child(cell)
	# Three cells side by side need 320, two 210.
	for case: Array in [[400.0, 3], [320.0, 2], [250.0, 2], [150.0, 1], [321.0, 3]]:
		var width: float = case[0]
		var expected: int = case[1]
		# As a container would give it: the parent first grows to the grid's least size.
		for i: int in range(3):
			parent.size = Vector2(width, 200)
			await process_frame
		_check(
			grid.columns == expected and grid.size.x <= width,
			"%d side by side in %s: %d, %s wide" % [expected, width, grid.columns, grid.size.x]
		)
	# A child that expands, e.g. a map, needs room beside the others.
	(grid.get_child(0) as Control).size_flags_horizontal = Control.SIZE_EXPAND_FILL
	grid.expand_room = 100.0
	for i: int in range(3):
		parent.size = Vector2(400, 200)
		await process_frame
	_check(grid.columns == 2, "the map's room counts: %d side by side" % grid.columns)
	parent.free()


## A short ride, so the history, its summary and the analysis have something to lay out.
func _record_ride() -> String:
	var torqa: TorqaApp = TorqaApp.new()
	root.add_child(torqa)
	var saved: Array[String] = []
	torqa.ride_saved.connect(func(path: String) -> void: saved.append(path))
	_check(torqa.connect_fake_trainer(250.0, 90.0), "fake trainer")
	var workout: Dictionary = {"kind": "power", "power": 200.0, "name": "Window smoke test"}
	_check(torqa.start_workout(workout, false, false), "a ride")
	await create_timer(1.5).timeout
	torqa.finish_ride()
	torqa.free()
	_check(saved.size() == 1, "the ride saved: %s" % [saved])
	return saved[0] if saved.size() == 1 else ""


## Every page and dialog fits the smallest window the layouts are made for (R94, R53), in
## English and in German, whose words run longer: nothing reaches past the window's edges, and
## what is taller than the window scrolls.
func _small_window(locale: String, ride: String) -> void:
	TranslationServer.set_locale(locale)
	var settings: TorqaApp = TorqaApp.new()
	root.add_child(settings)
	var rider: Dictionary = settings.profile()
	var rider_id: String = rider["id"]
	var language: String = rider.get("language", "")
	rider["language"] = locale
	settings.save_profile(rider_id, rider)
	settings.free()
	var main: Control = (load(MAIN_SCENE) as PackedScene).instantiate()
	root.add_child(main)
	await process_frame
	root.size = Vector2i((UiScale.MIN_WINDOW_SIZE * root.content_scale_factor).ceil())
	var room: Vector2 = root.get_visible_rect().size
	var torqa: TorqaApp = main.get_node("Torqa")
	var start: StartPage = main.get_node("StartPage")
	# The gallery's toolbar as with courses in it, and the rider's whole setup.
	var courses: CoursesTab = start.get("_courses")
	var toolbar: Control = courses.get("_toolbar")
	toolbar.show()
	var profile: ProfileTab = start.get("_profile")
	var unfolded: Dictionary[String, bool] = profile.get("_unfolded")
	unfolded[rider_id] = true
	profile.refresh()
	var tabs: TabContainer = start.get("_tabs")
	for tab: int in range(tabs.get_tab_count()):
		tabs.current_tab = tab
		await _settle()
		_fits(start, room, "%s, the %s tab" % [locale, tabs.get_tab_control(tab).name])
	tabs.current_tab = StartPage.Tab.COURSES
	(
		start
		. call(
			"_open_course",
			{
				"path": "",
				"name": "Gurtenstrasse",
				"length_m": 2400.0,
				"elevation_gain_m": 149.4,
				"max_grade": 17.6,
			}
		)
	)
	await _settle()
	_fits(start, room, "%s, a course's page" % locale)
	var settings_dialog: SettingsDialog = start.get("_settings")
	settings_dialog.open()
	await _dialog_fits(settings_dialog, room, locale)
	var rider_dialog: ProfileDialog = start.find_children("*", "ProfileDialog", true, false)[0]
	rider_dialog.edit(torqa.profile(), torqa.hud_layout())
	await _dialog_fits(rider_dialog, room, locale)
	var editor: WorkoutEditor = start.find_children("*", "WorkoutEditor", true, false)[0]
	editor.edit({}, 200.0)
	await _dialog_fits(editor, room, locale)
	var buttons: ButtonDialog = start.find_children("*", "ButtonDialog", true, false)[0]
	buttons.open(ButtonDialog.PHONE_REMOTE, TorqaApp.phone_remote_name())
	await _dialog_fits(buttons, room, locale)
	start.hide()
	var summary: HistoryScreen = main.get_node("HistoryScreen")
	if not ride.is_empty():
		main.call("_show_summary", ride)
		await _settle()
		_fits(summary, room, "%s, a ride's summary" % locale)
	summary.hide()
	var ride_screen: RideScreen = main.get_node("RideScreen")
	ride_screen.begin({"workout": {"kind": "zone", "zone": 3}, "on_course": false})
	ride_screen.show()
	await _settle()
	_fits(ride_screen, room, "%s, the ride's screen" % locale)
	ride_screen.call("_open_settings")
	var ride_settings: RideSettingsDialog = (
		ride_screen.find_children("*", "RideSettingsDialog", true, false)[0]
	)
	await _dialog_fits(ride_settings, room, locale)
	main.free()
	rider["language"] = language
	settings = TorqaApp.new()
	root.add_child(settings)
	settings.save_profile(rider_id, rider)
	settings.free()


## Whether `page` and what is in it need no more room than `room`.
func _fits(page: Control, room: Vector2, what: String) -> void:
	var need: Vector2 = _need(page)
	_check(need.x <= room.x and need.y <= room.y, "%s fits %s: needs %s" % [what, room, need])


## The room a control needs: a container's own least size, or what its children need.
func _need(control: Control) -> Vector2:
	if control is Container:
		return control.get_combined_minimum_size()
	var need: Vector2 = control.get_combined_minimum_size()
	for child: Node in control.get_children():
		if child is Control and (child as Control).visible:
			need = need.max(_need(child as Control))
	return need


## Whether `dialog`, just opened, fits `room` with its title and lies inside it; closes it.
func _dialog_fits(dialog: Window, room: Vector2, locale: String) -> void:
	await _settle()
	var what: String = "%s, %s" % [locale, dialog.title]
	var title: float = dialog.get_theme_constant("title_height")
	var need: Vector2 = Vector2(dialog.min_size).max(dialog.get_contents_minimum_size())
	_check(
		dialog.visible and need.x <= room.x and need.y + title <= room.y,
		"%s fits %s: needs %s" % [what, room, need]
	)
	var frame: Rect2 = Rect2(
		Vector2(dialog.position) - Vector2(0, title), Vector2(dialog.size) + Vector2(0, title)
	)
	_check(Rect2(Vector2.ZERO, room).encloses(frame), "%s inside the window: %s" % [what, frame])
	dialog.hide()


func _settle() -> void:
	for i: int in range(4):
		await process_frame


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
