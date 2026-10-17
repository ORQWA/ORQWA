class_name GraphicsSettings
extends VBoxContainer
## How the app looks on this computer (R78), on the settings' Graphics tab (R83): how
## detailed the 3D world is drawn (R43) and full screen or a window (#218).

## How detailed the 3D world is drawn on this computer (R43).
var _quality: OptionButton = OptionButton.new()
var _quality_note: Label = Label.new()
## Full screen or a window (#218), as F11 switches it too.
var _fullscreen: CheckBox = CheckBox.new()
var _torqa: TorqaApp


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	var current: String = _torqa.graphics_quality()
	for i: int in range(_quality.item_count):
		if _quality.get_item_metadata(i) == current:
			_quality.select(i)
	_show_quality_note()


func _init() -> void:
	add_theme_constant_override("separation", 16)
	var row: HBoxContainer = HBoxContainer.new()
	row.add_theme_constant_override("separation", 16)
	var caption: Label = Label.new()
	caption.text = tr("Quality")
	caption.custom_minimum_size = Vector2(180, 0)
	row.add_child(caption)
	# i18n-begin
	for preset: Array in [
		["low", "Low"], ["medium", "Medium"], ["high", "High"], ["ultra", "Ultra"]
	]:
		# i18n-end
		var key: String = preset[0]
		var label: String = preset[1]
		_quality.add_item(tr(label))
		_quality.set_item_metadata(_quality.item_count - 1, key)
	_quality.custom_minimum_size = Vector2(240, 0)
	_quality.item_selected.connect(_on_quality_selected)
	row.add_child(_quality)
	add_child(row)
	_quality_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_quality_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	# A width to wrap at before the dialog lays it out: without, the label asks for a line per
	# word, and the dialog grows that tall.
	_quality_note.custom_minimum_size = Vector2(600, 0)
	add_child(_quality_note)
	var screen_row: HBoxContainer = HBoxContainer.new()
	screen_row.add_theme_constant_override("separation", 16)
	var screen_caption: Label = Label.new()
	screen_caption.text = tr("Screen")
	screen_caption.custom_minimum_size = Vector2(180, 0)
	screen_row.add_child(screen_caption)
	_fullscreen.text = tr("Full screen (F11)")
	_fullscreen.toggled.connect(
		func(on: bool) -> void: FullScreen.set_on(_app_window(), _torqa, on)
	)
	screen_row.add_child(_fullscreen)
	add_child(screen_row)


func _ready() -> void:
	# F11 or the system may switch it while the settings are open.
	_app_window().size_changed.connect(
		func() -> void: _fullscreen.set_pressed_no_signal(FullScreen.is_on(_app_window()))
	)
	_fullscreen.set_pressed_no_signal(FullScreen.is_on(_app_window()))


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		_show_quality_note()


## The app's own window: the settings are a dialog, a window of their own.
func _app_window() -> Window:
	return get_tree().root


func _on_quality_selected(index: int) -> void:
	var key: String = _quality.get_item_metadata(index)
	_torqa.set_graphics_quality(key)
	_show_quality_note()


func _show_quality_note() -> void:
	# i18n-begin
	var notes: Dictionary[String, String] = {
		"low": "For weaker computers: shorter view, simpler shadows and sky.",
		"medium": "A balance of visual quality and performance.",
		"high": "For stronger GPUs: softer shadows, bounced light, light fog, more distance.",
		"ultra": "For powerful GPUs: all effects, including global illumination.",
	}
	# i18n-end
	var key: String = _torqa.graphics_quality() if _torqa != null else "medium"
	var note: String = notes.get(key, notes["medium"])
	_quality_note.text = tr(note) + " " + tr("Applies from the next ride.")
