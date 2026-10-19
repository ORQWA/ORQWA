class_name ReflowGrid
extends GridContainer
## As many of its children side by side as fit, up to `max_columns`, the rest in the rows below
## (R94): e.g. a map beside its figures in a large window and above them in a small one. Unlike
## in a flow container, the columns line up and a child that expands fills the room.

## The most children side by side.
var max_columns: int = 2
## The width a column with a child that expands needs beyond that child's minimum to be worth
## having, e.g. so a map is not squeezed to a strip beside its figures.
var expand_room: float = 0.0


func _notification(what: int) -> void:
	if what in [NOTIFICATION_RESIZED, NOTIFICATION_SORT_CHILDREN]:
		_reflow()


## The width the children need in `count` columns.
func width_in(count: int) -> float:
	var widths: PackedFloat32Array = PackedFloat32Array()
	widths.resize(count)
	widths.fill(0.0)
	var placed: int = 0
	for child: Node in get_children():
		var control: Control = child as Control
		if control == null or not control.visible or control.top_level:
			continue
		var width: float = control.get_combined_minimum_size().x
		if control.size_flags_horizontal & Control.SIZE_EXPAND:
			width += expand_room
		widths[placed % count] = maxf(widths[placed % count], width)
		placed += 1
	var used: int = mini(placed, count)
	var total: float = 0.0
	for column: int in range(used):
		total += widths[column]
	return total + maxi(used - 1, 0) * get_theme_constant("h_separation")


func _reflow() -> void:
	# Too wide for its parent, the grid is as wide as its columns, never narrower; so an exact
	# fit counts as too narrow, and the grid settles on the most columns that fit.
	var count: int = max_columns
	while count > 1 and width_in(count) >= size.x:
		count -= 1
	if count != columns:
		columns = count
