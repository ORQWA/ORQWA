class_name FailureDetails
extends VBoxContainer
## Keeps failures available after a notification fades, without interrupting the ride.

const HISTORY_LIMIT: int = 20

var _failures: Array[Dictionary] = []
var _toggle: Button = Button.new()
var _dismiss: Button = Button.new()
var _panel: PanelContainer = PanelContainer.new()
var _rows: VBoxContainer = VBoxContainer.new()


func _init() -> void:
	alignment = BoxContainer.ALIGNMENT_END
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_panel.add_theme_stylebox_override("panel", UiTheme.bar())
	var scroll: ScrollContainer = ScrollContainer.new()
	scroll.custom_minimum_size.y = 180
	scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_rows.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_rows.add_theme_constant_override("separation", 12)
	scroll.add_child(_rows)
	_panel.add_child(scroll)
	add_child(_panel)
	_panel.hide()
	var buttons: HBoxContainer = HBoxContainer.new()
	buttons.size_flags_horizontal = Control.SIZE_SHRINK_END
	_dismiss.text = tr("Dismiss")
	_dismiss.pressed.connect(clear)
	buttons.add_child(_dismiss)
	_toggle.text = tr("Technical details")
	_toggle.toggle_mode = true
	_toggle.toggled.connect(_panel.set_visible)
	buttons.add_child(_toggle)
	add_child(buttons)
	hide()


## Arguments are substituted after translation and remain untouched, even if they contain %s.
static func summary(failure: Dictionary) -> String:
	var message: String = failure.get("message", "An unexpected error occurred.")
	var values: PackedStringArray = failure.get("args", PackedStringArray())
	var parts: PackedStringArray = TranslationServer.translate(message).split("%s")
	var text: String = parts[0]
	for i: int in range(1, parts.size()):
		text += (values[i - 1] if i <= values.size() else "%s") + parts[i]
	return text


## Stores a snapshot of the key, arguments and original diagnostic, newest first.
func report(failure: Dictionary) -> void:
	if failure.is_empty():
		return
	_failures.push_front(failure.duplicate(true))
	if _failures.size() > HISTORY_LIMIT:
		_failures.resize(HISTORY_LIMIT)
	_refresh()
	show()


## Forgets the failures shown and hides until the next one.
func clear() -> void:
	_failures.clear()
	_toggle.button_pressed = false
	_refresh()
	hide()


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_instance_valid(_toggle):
		_toggle.text = tr("Technical details")
		_refresh()


func _refresh() -> void:
	for child: Node in _rows.get_children():
		_rows.remove_child(child)
		child.queue_free()
	for failure: Dictionary in _failures:
		var explanation: Label = Label.new()
		explanation.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		explanation.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		explanation.text = summary(failure)
		_rows.add_child(explanation)
		var details: Label = Label.new()
		details.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		details.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		details.add_theme_color_override("font_color", UiTheme.MUTED)
		details.text = failure.get("details", "")
		_rows.add_child(details)
