class_name PowerRecordBadges
extends HFlowContainer
## The ride's all-time power records (R62), shared by its quick summary and full details.


func _init() -> void:
	add_theme_constant_override("h_separation", 24)
	add_theme_constant_override("v_separation", 8)


## Shows only the record durations supplied by the core, with their power from the ride.
func show_ride(ride: Dictionary) -> void:
	for child: Node in get_children():
		remove_child(child)
		child.queue_free()
	var records: PackedFloat64Array = ride.get("power_records", PackedFloat64Array())
	var curve: PackedVector2Array = ride.get("power_curve", PackedVector2Array())
	visible = not records.is_empty()
	for seconds: float in records:
		var watts: float = 0.0
		for point: Vector2 in curve:
			if is_equal_approx(point.x, seconds):
				watts = point.y
		var badge: Label = Label.new()
		badge.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		badge.text = (
			tr("★ Best %s: %d W") % [PowerCurveChart.duration_label(seconds), roundi(watts)]
		)
		badge.add_theme_color_override("font_color", UiTheme.CLIMB_COLORS["Cat 3"])
		add_child(badge)
