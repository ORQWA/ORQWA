class_name CourseSettings
extends VBoxContainer
## How this computer keeps its courses (R78), on the settings' Courses tab: their 3D worlds
## kept once built, so that riding a course again starts at once (R101), and the room they
## take.

## Keeping the courses' worlds, on this computer.
var _keep: CheckBox = CheckBox.new()
var _note: Label = Label.new()
var _torqa: TorqaApp


func _init() -> void:
	add_theme_constant_override("separation", 16)
	_keep.toggled.connect(_on_keep_toggled)
	add_child(_keep)
	_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	# A width to wrap at before the dialog lays it out, as on the Graphics tab.
	_note.custom_minimum_size = Vector2(600, 0)
	add_child(_note)
	_translate()


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	refresh()


## Shows the setting and the room the kept worlds take now: they grow with every course ridden.
func refresh() -> void:
	if _torqa != null:
		_keep.set_pressed_no_signal(_torqa.keeps_worlds())
	_translate()


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		_translate()


func _on_keep_toggled(on: bool) -> void:
	if _torqa != null:
		_torqa.set_keeps_worlds(on)
	_translate()


func _translate() -> void:
	_keep.text = tr("Keep courses ready to ride")
	var note: String = (
		tr(
			"Each course's 3D world is kept on this computer once built: the next ride starts at once."
		)
		+ " "
		+ tr("A world takes about 10 MB per kilometre of route.")
	)
	# Turned off, they are being deleted.
	if _torqa != null and _keep.button_pressed:
		var size: String = String.humanize_size(_torqa.kept_worlds_size())
		note += " " + tr("The kept worlds take %s.") % size
	_note.text = note
