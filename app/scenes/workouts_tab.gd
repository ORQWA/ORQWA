class_name WorkoutsTab
extends HBoxContainer
## The Workouts tab (R58): a constant power or a heart rate to hold (R56), or a structured
## workout from the library (R21) with its steps shown, ridden on its own (the HUD only) or on
## a course in its 3D world → Start. A course's world is built once Start is pressed, as on the
## course page. Workout files are imported into the library here. With an account on a
## platform that holds the rider's plan, today's planned workouts are listed to choose (R81).

## Start was pressed for `workout` (`WorkoutOptions.workout()` with its `name`), on the course
## at `course_path` or on its own if empty.
signal start_requested(workout: Dictionary, course_path: String)
## The course asked for with `prepare()` is ready: the workout can start.
signal ready_to_start

var _torqa: TorqaApp
var _options: WorkoutOptions = WorkoutOptions.new()
## The structured workout chosen: what it is about and its steps.
var _about: Label = Label.new()
var _plan_chart: WorkoutChart = WorkoutChart.new()
var _import_dialog: FileDialog = FileDialog.new()
var _editor: WorkoutEditor = WorkoutEditor.new()
var _plan_buttons: HBoxContainer = HBoxContainer.new()
var _edit_button: Button = Button.new()
var _where: OptionButton = OptionButton.new()
## The course of each entry of `_where` after the first ("on its own").
var _course_paths: PackedStringArray = PackedStringArray()
var _overlay: CheckBox = CheckBox.new()
var _start_button: Button = Button.new()
var _loading_bar: ProgressBar = ProgressBar.new()
var _status: Label = Label.new()
## Waiting for the chosen course's route, or for its world.
var _loading_route: bool = false
var _building: bool = false
## Today's workouts from the platform holding the rider's plan (R81): a row per workout with
## a button that chooses it; shown only with such an account connected.
var _planned_panel: PanelContainer = PanelContainer.new()
var _planned_rows: VBoxContainer = VBoxContainer.new()
var _planned_note: Label = Label.new()


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	_torqa.route_loaded.connect(_on_route_loaded)
	_torqa.loading_progress.connect(_on_loading_progress)
	_torqa.world_ready.connect(_on_world_ready)
	_torqa.failed.connect(_on_failed)
	_torqa.planned_workouts.connect(_on_planned_workouts)
	_torqa.planned_workouts_failed.connect(_on_planned_workouts_failed)
	_editor.bind(torqa)
	refresh()


## Brings the rider's zones and the course list up to date, e.g. after another rider was chosen.
func refresh() -> void:
	var ftp: float = _torqa.profile().get("ftp_w", 200.0)
	_options.configure(_torqa.heart_rate_zones(), ftp)
	_options.set_plans(_torqa.workouts())
	_show_plan()
	var chosen: String = course_path()
	_where.clear()
	_course_paths.clear()
	_where.add_item(tr("On its own (HUD only)"))
	for course: Dictionary in _torqa.courses():
		var located: bool = course.get("located", true)
		if not located:
			continue
		var course_name: String = course["name"]
		_where.add_item(course_name)
		var path: String = course["path"]
		_course_paths.append(path)
	var index: int = _course_paths.find(chosen)
	_where.select(index + 1 if index >= 0 else 0)
	_fetch_planned()


## The chosen course's file, or "" to ride on its own.
func course_path() -> String:
	return _course_paths[_where.selected - 1] if _where.selected > 0 else ""


## Whether the workout starts as the overlay (R57), only the HUD over other windows.
func start_as_overlay() -> bool:
	return _overlay.button_pressed


## The workout chosen here, as `start_requested` hands it out: `WorkoutOptions.workout()` with
## its `name` (for the history and on screen) and, for a structured workout, its `plan`
## (`TorqaApp.workouts()` entry).
func workout() -> Dictionary:
	var chosen: Dictionary = _options.workout()
	chosen["name"] = _options.title()
	chosen["plan"] = _options.plan() if chosen["kind"] != "ftp_test" else _ftp_test_plan()
	return chosen


## Gets the chosen course's route and world ready; `ready_to_start` follows, at once without
## a course or with its world built already.
func prepare() -> void:
	var path: String = course_path()
	if path.is_empty():
		ready_to_start.emit()
		return
	_start_button.disabled = true
	_status.text = tr("Loading the course") + " …"
	_loading_route = true
	if _torqa.loaded_course() != path:
		_torqa.open_course(path)
	elif _torqa.has_route():
		_build()


## Shows why the workout cannot start (e.g. no trainer); empty clears it.
func show_status(message: String) -> void:
	_status.text = message


func _init() -> void:
	add_theme_constant_override("separation", 24)
	var left: VBoxContainer = VBoxContainer.new()
	left.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	left.add_theme_constant_override("separation", 14)
	var heading: Label = Label.new()
	heading.text = tr("Workout")
	heading.add_theme_font_size_override("font_size", 26)
	left.add_child(heading)
	var intro: Label = Label.new()
	intro.text = tr("Hold a power or a heart rate, or ride a structured workout step by step.")
	intro.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	intro.add_theme_color_override("font_color", UiTheme.MUTED)
	left.add_child(intro)
	var options_panel: PanelContainer = PanelContainer.new()
	var options_rows: VBoxContainer = VBoxContainer.new()
	options_rows.add_theme_constant_override("separation", 12)
	options_rows.add_child(_options)
	_about.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_about.add_theme_color_override("font_color", UiTheme.MUTED)
	# Descriptions are the authors' own.
	_about.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	options_rows.add_child(_about)
	_plan_chart.custom_minimum_size = Vector2(0, 140)
	options_rows.add_child(_plan_chart)
	_plan_buttons.add_theme_constant_override("separation", 8)
	_edit_button.pressed.connect(func() -> void: _editor.edit(_options.plan(), _ftp()))
	_plan_buttons.add_child(_edit_button)
	var new_button: Button = Button.new()
	new_button.text = tr("New workout…")
	new_button.pressed.connect(func() -> void: _editor.edit({}, _ftp()))
	_plan_buttons.add_child(new_button)
	options_rows.add_child(_plan_buttons)
	_editor.saved.connect(_on_saved)
	_editor.deleted.connect(func() -> void: refresh())
	add_child(_editor)
	options_panel.add_child(options_rows)
	left.add_child(options_panel)
	var planned_rows: VBoxContainer = VBoxContainer.new()
	planned_rows.add_theme_constant_override("separation", 10)
	planned_rows.add_child(UiTheme.caption(tr("Planned for today")))
	_planned_rows.add_theme_constant_override("separation", 8)
	planned_rows.add_child(_planned_rows)
	_planned_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_planned_note.add_theme_color_override("font_color", UiTheme.MUTED)
	planned_rows.add_child(_planned_note)
	_planned_panel.add_child(planned_rows)
	_planned_panel.hide()
	left.add_child(_planned_panel)
	add_child(left)
	_options.changed.connect(_show_plan)
	_options.import_requested.connect(func() -> void: _import_dialog.popup_centered_ratio(0.7))
	_import_dialog.title = tr("Choose a workout file")
	_import_dialog.file_mode = FileDialog.FILE_MODE_OPEN_FILE
	_import_dialog.access = FileDialog.ACCESS_FILESYSTEM
	var patterns: PackedStringArray = PackedStringArray()
	for extension: String in TorqaApp.workout_extensions():
		patterns.append("*." + extension)
	_import_dialog.filters = PackedStringArray([", ".join(patterns) + " ; " + tr("Workouts")])
	_import_dialog.use_native_dialog = true
	_import_dialog.file_selected.connect(_import)
	add_child(_import_dialog)

	var panel: PanelContainer = PanelContainer.new()
	panel.custom_minimum_size = Vector2(520, 0)
	var right: VBoxContainer = VBoxContainer.new()
	right.add_theme_constant_override("separation", 14)
	right.add_child(UiTheme.caption(tr("Where")))
	# Course names are never translated.
	_where.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	_where.tooltip_text = tr(
		"On a course, the trainer holds the workout's power while you ride through its 3D world"
	)
	right.add_child(_where)
	_overlay.text = tr("Start as overlay")
	_overlay.tooltip_text = tr("Only the HUD, on top of other windows, e.g. over a video")
	right.add_child(_overlay)
	var fill: Control = Control.new()
	fill.size_flags_vertical = Control.SIZE_EXPAND_FILL
	right.add_child(fill)
	_loading_bar.max_value = 1.0
	_loading_bar.show_percentage = false
	_loading_bar.custom_minimum_size = Vector2(0, 8)
	_loading_bar.hide()
	right.add_child(_loading_bar)
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.add_theme_color_override("font_color", UiTheme.MUTED)
	right.add_child(_status)
	_start_button.text = tr("Start")
	_start_button.custom_minimum_size = Vector2(0, 56)
	_start_button.add_theme_font_size_override("font_size", 22)
	_start_button.add_theme_stylebox_override("normal", UiTheme.accent_button())
	_start_button.pressed.connect(_on_start_pressed)
	right.add_child(_start_button)
	panel.add_child(right)
	add_child(panel)


## The chosen FTP test's steps, to show it before and behind it while riding; for the ramp
## test as far as most riders get, as it goes on until the rider gives way.
func _ftp_test_plan() -> Dictionary:
	var test: String = _options.workout()["test"]
	var steps: Array = _torqa.ftp_test_steps(test)
	var duration: float = 0.0
	for step: Dictionary in steps:
		var seconds: float = step["duration_s"]
		duration += seconds
	var description: String = tr(
		"Power rises each minute until you give way. FTP: 75 % of your best minute."
	)
	if test == "twenty_minutes":
		description = tr(
			"Warm-up and activation, then 20 minutes all out. FTP: 95 % of their average."
		)
	elif test == "two_by_eight":
		description = tr("Twice 8 minutes all out, 10 easy between. FTP: 90 % of their average.")
	return {
		"name": _options.title(),
		"description": description,
		"steps": steps,
		"duration_s": duration,
	}


## The chosen structured workout's (or FTP test's) description and steps; nothing for the
## other kinds.
func _show_plan() -> void:
	var plan: Dictionary = workout()["plan"]
	_about.visible = not plan.is_empty()
	_plan_chart.visible = not plan.is_empty()
	var kind: String = _options.workout()["kind"]
	_plan_buttons.visible = kind == "plan"
	var builtin: bool = plan.get("builtin", false)
	_edit_button.text = tr("Edit a copy…") if builtin else tr("Edit…")
	_edit_button.visible = not plan.is_empty()
	if plan.is_empty():
		return
	var description: String = plan["description"]
	_about.text = description
	var steps: Array = plan["steps"]
	_plan_chart.set_steps(steps, _ftp())


func _ftp() -> float:
	if _torqa == null:
		return 200.0
	var ftp: float = _torqa.profile().get("ftp_w", 200.0)
	return ftp


## Asks the platforms holding the rider's plan for today's workouts (R81), by this computer's
## date; the section shows only when there is such an account.
func _fetch_planned() -> void:
	var asked: bool = _torqa.fetch_planned_workouts(Time.get_date_string_from_system())
	_planned_panel.visible = asked
	if asked:
		_planned_note.text = tr("Looking up today's plan…")
		_planned_note.add_theme_color_override("font_color", UiTheme.MUTED)
		_planned_note.show()


func _on_planned_workouts(_platform: String, _day: String) -> void:
	var plans: Array = _torqa.workouts()
	_options.set_plans(plans)
	_show_planned(plans)


func _on_planned_workouts_failed(_platform: String, message: String) -> void:
	_planned_note.text = message
	_planned_note.add_theme_color_override("font_color", UiTheme.DANGER)
	_planned_note.show()


## Lists the planned workouts among `plans` (`TorqaApp.workouts()` entries), each with a
## button that chooses it as the structured workout to ride.
func _show_planned(plans: Array) -> void:
	for child: Node in _planned_rows.get_children():
		_planned_rows.remove_child(child)
		child.queue_free()
	var listed: int = 0
	for plan: Dictionary in plans:
		var platform: String = plan.get("planned", "")
		if platform.is_empty():
			continue
		var row: HBoxContainer = HBoxContainer.new()
		row.add_theme_constant_override("separation", 12)
		var choose: Button = Button.new()
		choose.text = tr("Choose")
		var id: String = plan["id"]
		choose.pressed.connect(func() -> void: _choose_planned(id))
		row.add_child(choose)
		var label: Label = Label.new()
		# Workout and platform names are never translated.
		label.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		var seconds: float = plan["duration_s"]
		label.text = "%s  ·  %s  ·  %s" % [plan["name"], UiTheme.duration(seconds), platform]
		label.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		row.add_child(label)
		_planned_rows.add_child(row)
		listed += 1
	_planned_note.text = tr("Nothing planned for today.")
	_planned_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_planned_note.visible = listed == 0
	_planned_panel.show()


## A planned workout becomes the structured workout to ride; Start rides it as any other.
func _choose_planned(id: String) -> void:
	_options.set_workout({"kind": "plan", "id": id})
	_show_plan()
	_status.text = ""


func _on_saved(id: String) -> void:
	_options.set_plans(_torqa.workouts(), id)
	_show_plan()
	_status.text = tr("Saved to your workouts.")


func _import(path: String) -> void:
	var id: String = _torqa.import_workout(path)
	if id.is_empty():
		return
	_options.set_plans(_torqa.workouts(), id)
	_show_plan()
	_status.text = tr("Added to your workouts.")


func _on_start_pressed() -> void:
	_status.text = ""
	start_requested.emit(workout(), course_path())


func _on_route_loaded(_route: Dictionary) -> void:
	if _loading_route and _torqa.loaded_course() == course_path():
		_build()


## The course's route is there: its world next, ridden in 3D even if it has a video (R58).
func _build() -> void:
	_loading_route = false
	_torqa.ride_along_video(false)
	if _torqa.build_world():
		_done()
		return
	_building = true
	_loading_bar.value = 0.0
	_loading_bar.show()
	_status.text = tr("Building 3D world") + " …"


func _on_loading_progress(step: String, _unit: String, done: int, total: int) -> void:
	if not _building:
		return
	_loading_bar.value = float(done) / float(maxi(total, 1))
	_status.text = tr(step) + " …"


func _on_world_ready(_info: Dictionary) -> void:
	if _building:
		_done()


func _done() -> void:
	_building = false
	_loading_bar.hide()
	_status.text = ""
	_start_button.disabled = false
	ready_to_start.emit()


func _on_failed(message: String) -> void:
	if _loading_route or _building:
		_loading_route = false
		_building = false
		_loading_bar.hide()
		_start_button.disabled = false
		_status.text = message
