class_name SoundSettings
extends VBoxContainer
## The app's own sounds (R125), on the settings' Sound tab: the chime that comes with each
## moment of a ride (R124). The ride's wind, bicycle and video sound stay in its options.

var _sounds: CheckBox = CheckBox.new()
var _note: Label = Label.new()
var _preview: Button = Button.new()
var _chime: MomentChime = MomentChime.new()
var _orqwa: OrqwaApp


func _init() -> void:
	add_theme_constant_override("separation", 16)
	_sounds.text = tr("Play a chime with the ride's moments")
	_sounds.toggled.connect(_on_toggled)
	add_child(_sounds)
	_note.text = tr("A short chime as a card tells of a climb topped, a milestone or a record.")
	_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	# A width to wrap at before the dialog lays it out, as on the Graphics tab.
	_note.custom_minimum_size = Vector2(600, 0)
	add_child(_note)
	_preview.text = tr("Play the chime")
	_preview.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_preview.pressed.connect(_chime.play)
	add_child(_preview)
	add_child(_chime)


func bind(orqwa: OrqwaApp) -> void:
	_orqwa = orqwa
	refresh()


## Shows the setting as it is now.
func refresh() -> void:
	if _orqwa != null:
		_sounds.set_pressed_no_signal(_orqwa.sounds())


## Whether the chime is on, as the box shows it.
func is_on() -> bool:
	return _sounds.button_pressed


func _on_toggled(on: bool) -> void:
	if _orqwa != null:
		_orqwa.set_sounds(on)
