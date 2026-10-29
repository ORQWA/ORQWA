class_name GroupRidersPanel
extends PanelContainer
## Current group members beside the map, with locally interpolated distance gaps.

var _rows: VBoxContainer = VBoxContainer.new()
var _labels: Dictionary[int, Label] = {}
var _key: String = ""


func _ready() -> void:
	var content: VBoxContainer = VBoxContainer.new()
	content.add_theme_constant_override("separation", 12)
	content.add_child(UiTheme.caption(tr("Group Rides")))
	_rows.add_theme_constant_override("separation", 8)
	content.add_child(_rows)
	add_child(content)
	hide()


func show_riders(peers: Array, imperial: bool) -> void:
	visible = not peers.is_empty()
	var identities: Array[String] = []
	for peer: Dictionary in peers:
		identities.append(str(peer["id"]) + ":" + str(peer["generation"]) + ":" + str(peer["name"]))
	var key: String = str(identities)
	if key != _key:
		_key = key
		_labels.clear()
		for child: Node in _rows.get_children():
			_rows.remove_child(child)
			child.queue_free()
		for peer: Dictionary in peers:
			var row: HBoxContainer = HBoxContainer.new()
			row.add_theme_constant_override("separation", 12)
			var id: int = peer["id"]
			var rider_name: String = peer["name"]
			var label: Label = Label.new()
			label.text = rider_name
			label.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
			label.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
			label.tooltip_text = rider_name
			label.custom_minimum_size.x = 70
			label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
			label.add_theme_color_override("font_color", UiTheme.group_color(id))
			row.add_child(label)
			var gap: Label = Label.new()
			gap.add_theme_font_size_override("font_size", 13)
			row.add_child(gap)
			_labels[id] = gap
			_rows.add_child(row)
	for peer: Dictionary in peers:
		var id: int = peer["id"]
		var gap: float = peer["gap_m"]
		var distance: String = (
			tr("%s ft") % roundi(absf(gap) / HudPanel.METERS_PER_FOOT)
			if imperial
			else tr("%s m") % roundi(absf(gap))
		)
		_labels[id].text = tr("%s ahead") % distance if gap >= 0 else tr("%s behind") % distance
