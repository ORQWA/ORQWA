class_name Spinner
extends Control
## An arc turning round a faint ring: Torqa is working something out and cannot say how far
## it has got, e.g. a course's route being prepared. It only turns while it is on screen.

## Seconds for one turn.
const TURN_S: float = 0.9
## The share of the ring the arc covers.
const ARC: float = 0.28
const WIDTH: float = 3.0

var _angle: float = 0.0


func _init(diameter: float = 20.0) -> void:
	custom_minimum_size = Vector2(diameter, diameter)
	mouse_filter = Control.MOUSE_FILTER_IGNORE


func _ready() -> void:
	set_process(is_visible_in_tree())


func _notification(what: int) -> void:
	if what == NOTIFICATION_VISIBILITY_CHANGED:
		set_process(is_visible_in_tree())


func _process(delta: float) -> void:
	_angle = fmod(_angle + TAU * delta / TURN_S, TAU)
	queue_redraw()


func _draw() -> void:
	var center: Vector2 = size / 2.0
	var radius: float = minf(size.x, size.y) / 2.0 - WIDTH / 2.0
	draw_arc(center, radius, 0.0, TAU, 48, UiTheme.SURFACE, WIDTH, true)
	draw_arc(center, radius, _angle, _angle + ARC * TAU, 16, UiTheme.ACCENT, WIDTH, true)
