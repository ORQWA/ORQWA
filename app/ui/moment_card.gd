class_name MomentCard
extends PanelContainer
## A moment of the ride (R124) on a card at the top centre over the HUD: its figure large, its
## line below. It slides in, stays a few seconds and slides out; moments that come meanwhile
## wait their turn, so two never cover each other.

## A card was shown with `text`, e.g. for the overlay to show it in its bar.
signal shown(text: String)

const SHOW_SECONDS: float = 4.0
const SLIDE_SECONDS: float = 0.35
const TOP: float = 80.0
const WIDTH: float = 460.0

## What is still to show: `[figure, line]` pairs.
var _queue: Array[PackedStringArray] = []
var _figure: Label = UiTheme.value(36)
var _line: Label = Label.new()
var _tween: Tween


func _init() -> void:
	hide()
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	custom_minimum_size.x = WIDTH
	var style: StyleBoxFlat = UiTheme.panel(0.88)
	style.border_color = Color(UiTheme.ACCENT, 0.7)
	style.set_border_width_all(0)
	style.border_width_bottom = 3
	style.set_content_margin_all(16)
	add_theme_stylebox_override("panel", style)
	var box: VBoxContainer = VBoxContainer.new()
	box.add_theme_constant_override("separation", 4)
	box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_figure.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_figure.add_theme_color_override("font_color", UiTheme.ACCENT)
	box.add_child(_figure)
	_line.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_line.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_line.add_theme_font_size_override("font_size", 16)
	box.add_child(_line)
	add_child(box)


## Shows `line` under `figure` (none if empty), after the cards before it.
func tell(figure: String, line: String) -> void:
	if line.is_empty():
		return
	_queue.append(PackedStringArray([figure, line]))
	if not is_showing():
		_next()


## Whether a card is on screen or on its way.
func is_showing() -> bool:
	return _tween != null and _tween.is_running()


## The card on screen: `[figure, line]`, empty when none.
func current() -> PackedStringArray:
	if not is_showing():
		return PackedStringArray()
	return PackedStringArray([_figure.text, _line.text])


## Takes every card away at once, e.g. when the ride ends.
func clear() -> void:
	_queue.clear()
	if _tween != null:
		_tween.kill()
		_tween = null
	hide()


func _next() -> void:
	if _queue.is_empty():
		hide()
		return
	var card: PackedStringArray = _queue.pop_front()
	_figure.text = card[0]
	_figure.visible = not card[0].is_empty()
	_line.text = card[1]
	reset_size()
	var above: float = -size.y - 8.0
	position = Vector2((get_parent_area_size().x - size.x) / 2.0, above)
	modulate.a = 0.0
	show()
	shown.emit(card[1] if card[0].is_empty() else "%s  ·  %s" % [card[0], card[1]])
	_tween = create_tween()
	_tween.set_ease(Tween.EASE_OUT).set_trans(Tween.TRANS_CUBIC)
	_tween.tween_property(self, "position:y", TOP, SLIDE_SECONDS)
	_tween.parallel().tween_property(self, "modulate:a", 1.0, SLIDE_SECONDS)
	_tween.tween_interval(SHOW_SECONDS)
	_tween.set_ease(Tween.EASE_IN)
	_tween.tween_property(self, "position:y", above, SLIDE_SECONDS)
	_tween.parallel().tween_property(self, "modulate:a", 0.0, SLIDE_SECONDS)
	_tween.finished.connect(_next)
