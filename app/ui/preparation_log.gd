class_name PreparationLog
extends VBoxContainer
## What ORQWA is doing while it prepares a course (R40), step by step as a build log: each
## step the core reports (`OrqwaApp.loading_progress`) on its own line, the one under way with
## its count ("143 / 412 tiles") and how long it has taken, the finished ones ticked with
## their time, and at the end how long the whole took. A rider waiting minutes for a long
## course's world sees it move rather than a bar alone.

## Seconds between refreshes of the running step's time.
const TICK_S: float = 1.0

## The steps shown, in order: step name → its row.
var _rows: Dictionary[String, PreparationStep] = {}
var _running: PreparationStep
var _started_ms: int = 0
var _since_tick: float = 0.0
var _done_label: Label = Label.new()


func _init() -> void:
	add_theme_constant_override("separation", 4)
	_done_label.add_theme_color_override("font_color", UiTheme.MUTED)
	_done_label.hide()
	add_child(_done_label)
	hide()


func _process(delta: float) -> void:
	_since_tick += delta
	if _since_tick >= TICK_S and _running != null:
		_since_tick = 0.0
		_running.refresh()


## Starts a new log: the old steps go.
func start() -> void:
	for row: PreparationStep in _rows.values():
		row.queue_free()
	_rows.clear()
	_running = null
	_started_ms = Time.get_ticks_msec()
	_done_label.hide()
	show()


## Step `name` (the core's label) stands at `done` of `total` `unit`; a new name finishes the
## step before it.
func step(name: String, unit: String, done: int, total: int) -> void:
	if not visible:
		start()
	if not _rows.has(name):
		if _running != null:
			_running.finish()
		var row: PreparationStep = PreparationStep.new(name)
		_rows[name] = row
		add_child(row)
		move_child(_done_label, -1)
		_running = row
	_running.progress(unit, done, total)


## Everything is done: the last step is ticked and the whole's time shown.
func finish() -> void:
	if _running != null:
		_running.finish()
		_running = null
	var seconds: float = float(Time.get_ticks_msec() - _started_ms) / 1000.0
	_done_label.text = tr("Ready in %s") % UiTheme.duration(seconds)
	_done_label.show()


## Clears the log, e.g. when the course could not be prepared.
func clear() -> void:
	start()
	hide()


## The steps' names in order, for checks.
func steps() -> PackedStringArray:
	var names: PackedStringArray = PackedStringArray()
	for row: PreparationStep in _rows.values():
		names.append(row.step_name)
	return names


## One step's line: a spinner while it runs, a tick once done; its name, count and time.
class PreparationStep:
	extends HBoxContainer

	var step_name: String
	var _spinner: Spinner = Spinner.new(16.0)
	var _tick: TextureRect = TextureRect.new()
	var _label: Label = Label.new()
	var _detail: Label = Label.new()
	var _started_ms: int = Time.get_ticks_msec()
	var _finished_ms: int = -1
	var _count: String = ""

	func _init(name: String) -> void:
		step_name = name
		add_theme_constant_override("separation", 8)
		_spinner.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		add_child(_spinner)
		_tick.texture = UiTheme.check_icon()
		_tick.stretch_mode = TextureRect.STRETCH_KEEP_CENTERED
		_tick.custom_minimum_size = Vector2(16, 16)
		_tick.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		_tick.hide()
		add_child(_tick)
		_label.text = tr(name)
		_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		add_child(_label)
		_detail.add_theme_color_override("font_color", UiTheme.MUTED)
		_detail.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		add_child(_detail)

	## The step's count so far.
	func progress(unit: String, done: int, total: int) -> void:
		_count = "%d / %d %s" % [done, total, tr(unit)] if total > 0 else ""
		refresh()

	## The step is done: ticked, with the time it took.
	func finish() -> void:
		_finished_ms = Time.get_ticks_msec()
		_spinner.hide()
		_tick.show()
		_label.add_theme_color_override("font_color", UiTheme.MUTED)
		refresh()

	## Rewrites the count and time.
	func refresh() -> void:
		var until: int = _finished_ms if _finished_ms >= 0 else Time.get_ticks_msec()
		var seconds: float = float(until - _started_ms) / 1000.0
		var parts: PackedStringArray = PackedStringArray()
		if _finished_ms < 0 and not _count.is_empty():
			parts.append(_count)
		parts.append(UiTheme.duration(seconds))
		_detail.text = " · ".join(parts)

	## Whether the step is finished, for checks.
	func is_finished() -> bool:
		return _finished_ms >= 0
