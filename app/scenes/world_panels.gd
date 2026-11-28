class_name WorldPanels
extends Control
## The panels a world's script shows on screen (R302): text, values, bars, grids and images,
## in ORQWA's own style and places (`top`, `side`, `centre`, `bottom`). The world gives the
## content; how it looks is ORQWA's. It follows the ride of the screen it is on, and takes
## World action (W) for the script.

const METERS_PER_MILE: float = 1609.344
const METERS_PER_FOOT: float = 0.3048

var imperial: bool = false

var _orqwa: OrqwaApp
var _answers: int = -1
var _places: Dictionary[String, BoxContainer] = {}
## Shown panels by id, and when each goes.
var _shown: Dictionary[String, Control] = {}
var _until: Dictionary[String, float] = {}
var _clock: float = 0.0
var _notice: Label = Label.new()
var _images: Dictionary[String, Texture2D] = {}
var _asked_images: Dictionary[String, bool] = {}


func _init() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	var top: HBoxContainer = HBoxContainer.new()
	top.set_anchors_preset(Control.PRESET_CENTER_TOP)
	top.position.y = 72.0
	var side: VBoxContainer = VBoxContainer.new()
	side.set_anchors_preset(Control.PRESET_CENTER_LEFT)
	side.position.x = 16.0
	var centre: VBoxContainer = VBoxContainer.new()
	centre.set_anchors_preset(Control.PRESET_CENTER)
	var bottom: HBoxContainer = HBoxContainer.new()
	bottom.set_anchors_preset(Control.PRESET_CENTER_BOTTOM)
	bottom.position.y = -120.0
	for place: BoxContainer in [top, side, centre, bottom]:
		place.mouse_filter = Control.MOUSE_FILTER_IGNORE
		place.grow_horizontal = Control.GROW_DIRECTION_BOTH
		place.grow_vertical = Control.GROW_DIRECTION_BOTH
		place.add_theme_constant_override("separation", 8)
		add_child(place)
	_places = {"top": top, "side": side, "centre": centre, "bottom": bottom}
	_notice.add_theme_color_override("font_color", UiTheme.MUTED)
	_notice.hide()
	bottom.add_child(_notice)


## Shows `world` (`ride_state()["world"]`, or `null` without a script).
func _process(delta: float) -> void:
	var screen: Node = get_parent()
	var app: OrqwaApp = screen.get("_orqwa") if screen != null else null
	if app == null:
		return
	var finished: bool = screen.get("_finished")
	imperial = screen.get("_imperial")
	var riding: bool = is_visible_in_tree() and not finished
	var state: Dictionary = app.ride_state() if riding else {}
	show_world(state.get("world"), delta, app)


func _unhandled_input(event: InputEvent) -> void:
	var key: InputEventKey = event as InputEventKey
	if key == null or not key.pressed or key.echo or key.keycode != KEY_W:
		return
	var screen: Node = get_parent()
	var finished: bool = screen.get("_finished") if screen != null else true
	if _orqwa == null or finished or not is_visible_in_tree():
		return
	# World action (R302): what it does is the world's script's to decide.
	_orqwa.world_action()
	get_viewport().set_input_as_handled()


func show_world(world: Variant, delta: float, orqwa: OrqwaApp) -> void:
	_orqwa = orqwa
	_clock += delta
	if world == null:
		if _answers >= 0:
			_clear()
		return
	var info: Dictionary = world
	var stopped: String = info["stopped"]
	_notice.visible = not stopped.is_empty()
	if _notice.visible:
		_notice.text = tr("The world's script has stopped; the ride goes on without it.")
	var answers: int = info["answers"]
	if answers != _answers:
		_answers = answers
		var text: String = info["output"]
		var parsed: Variant = JSON.parse_string(text)
		var output: Dictionary = parsed if parsed is Dictionary else {}
		var panels: Array = output.get("ui", [])
		for panel: Dictionary in panels:
			_show_panel(panel)
	_take_images()
	for id: String in _until.keys():
		if _clock >= _until[id]:
			_drop(id)


func _clear() -> void:
	for id: String in _shown.keys():
		_drop(id)
	_answers = -1
	_notice.hide()


func _drop(id: String) -> void:
	if _shown.has(id):
		_shown[id].queue_free()
	_shown.erase(id)
	_until.erase(id)


## Shows `panel` to this rider, replacing the one of its id.
func _show_panel(panel: Dictionary) -> void:
	var to: Variant = panel.get("to", "all")
	if to is float and int(to) != 0:
		return
	var id: String = panel.get("id", "")
	var place: String = panel.get("slot", "")
	if not _places.has(place):
		return
	_drop(id)
	var box: PanelContainer = PanelContainer.new()
	box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var rows: VBoxContainer = VBoxContainer.new()
	rows.add_theme_constant_override("separation", 4)
	var items: Array = panel.get("items", [])
	for item: Dictionary in items:
		var shown: Control = _item(item)
		if shown != null:
			rows.add_child(shown)
	box.add_child(rows)
	_places[place].add_child(box)
	_shown[id] = box
	var seconds: Variant = panel.get("seconds")
	if seconds != null:
		_until[id] = _clock + float(seconds)


func _item(item: Dictionary) -> Control:
	var text: Dictionary = item.get("text", {})
	var label: String = WorldPanels.pick(text)
	match item.get("kind", ""):
		"text":
			var emphasis: bool = item.get("emphasis", false)
			return _text(label, emphasis)
		"value":
			var unit: String = item.get("unit", "")
			var decimals: int = int(item.get("decimals", 0))
			var shown: String = _value(float(item.get("value", 0.0)), unit, decimals)
			return _text(("%s  %s" % [label, shown]) if not label.is_empty() else shown, true)
		"bar":
			return _bar(label, float(item.get("value", 0.0)))
		"grid":
			return _grid(item)
		"image":
			var file: String = item.get("image", "")
			return _image(file)
	return null


static func _text(text: String, emphasis: bool) -> Label:
	var shown: Label = UiTheme.value(28) if emphasis else Label.new()
	shown.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	shown.text = text
	return shown


static func _bar(label: String, value: float) -> Control:
	var bar: ProgressBar = ProgressBar.new()
	bar.min_value = 0.0
	bar.max_value = 1.0
	bar.step = 0.0
	bar.show_percentage = false
	bar.custom_minimum_size = Vector2(220.0, 12.0)
	bar.value = clampf(value, 0.0, 1.0)
	if label.is_empty():
		return bar
	var column: VBoxContainer = VBoxContainer.new()
	var caption: Label = UiTheme.caption(label)
	caption.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	column.add_child(caption)
	column.add_child(bar)
	return column


func _image(file: String) -> TextureRect:
	var image: TextureRect = TextureRect.new()
	image.expand_mode = TextureRect.EXPAND_FIT_WIDTH_PROPORTIONAL
	image.custom_minimum_size = Vector2(0.0, 64.0)
	image.set_meta("file", file)
	if _images.has(file):
		image.texture = _images[file]
	elif not _asked_images.has(file):
		_asked_images[file] = true
		_orqwa.request_world_part(file)
	return image


func _grid(item: Dictionary) -> GridContainer:
	var rows: Array = item.get("rows", [])
	var columns: int = 1
	for row: Array in rows:
		columns = maxi(columns, row.size())
	var grid: GridContainer = GridContainer.new()
	grid.columns = columns
	grid.add_theme_constant_override("h_separation", 12)
	var highlight: Variant = item.get("highlight")
	for index: int in rows.size():
		var row: Array = rows[index]
		for column: int in columns:
			var cell: Label = Label.new()
			cell.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
			if column < row.size():
				var text: Dictionary = row[column]
				cell.text = WorldPanels.pick(text)
			if highlight != null and int(highlight) == index:
				cell.add_theme_color_override("font_color", UiTheme.ACCENT)
			grid.add_child(cell)
	return grid


## `value` in `unit` (SI) as the rider reads it: speeds and distances in their units.
func _value(value: float, unit: String, decimals: int) -> String:
	var shown: float = value
	var shown_unit: String = unit
	match unit:
		"m/s":
			shown = value * (2.236936 if imperial else 3.6)
			shown_unit = "mph" if imperial else "km/h"
		"m":
			if imperial:
				shown = value / METERS_PER_FOOT
				shown_unit = "ft"
			elif absf(value) >= 1000.0:
				shown = value / 1000.0
				shown_unit = "km"
		"s":
			var whole: int = roundi(value)
			return "%d:%02d" % [floori(whole / 60.0), whole % 60]
	return ("%." + str(clampi(decimals, 0, 6)) + "f %s") % [shown, shown_unit]


## Images arrive from the world file; panels showing them get them as they do.
func _take_images() -> void:
	for file: String in _asked_images.keys():
		var part: Variant = _orqwa.take_world_part(file)
		if part == null:
			continue
		_asked_images.erase(file)
		if not part is PackedByteArray:
			push_warning("world script image %s: %s" % [file, part])
			continue
		var bytes: PackedByteArray = part
		var image: Image = Image.new()
		var loaded: Error = image.load_png_from_buffer(bytes)
		if loaded != OK:
			loaded = image.load_webp_from_buffer(bytes)
		if loaded != OK:
			loaded = image.load_jpg_from_buffer(bytes)
		if loaded != OK:
			continue
		_images[file] = ImageTexture.create_from_image(image)
		for shown: Control in _shown.values():
			for rect: Node in shown.find_children("*", "TextureRect", true, false):
				var texture_rect: TextureRect = rect as TextureRect
				if texture_rect.get_meta("file", "") == file:
					texture_rect.texture = _images[file]


## The text of `text` (language code → text) for the rider's language: theirs, its language
## without the region, the default (empty key), or any.
static func pick(text: Dictionary) -> String:
	var locale: String = TranslationServer.get_locale()
	var language: String = locale.get_slice("_", 0)
	for key: String in [locale, language, ""]:
		if text.has(key):
			return str(text[key])
	return str(text.values()[0]) if not text.is_empty() else ""
