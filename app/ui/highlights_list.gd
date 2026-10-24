class_name HighlightsList
extends VBoxContainer
## A ride's highlights (R124) in the rider's voice, for its quick summary and full details: its
## finish, the records it set (starred, like the history's) and how it fits the week. Power
## records are left to `PowerRecordBadges` beside it.

const RECORD_COLOR: Color = UiTheme.CLIMB_COLORS["Cat 3"]


func _init() -> void:
	add_theme_constant_override("separation", 6)
	hide()


## Shows the highlights of the ride saved at `path`, as `OrqwaApp.highlights()` has them.
func show_ride(orqwa: OrqwaApp, path: String) -> void:
	var profile: Dictionary = orqwa.profile()
	var voice: String = profile.get("voice", "casual")
	var imperial: bool = profile.get("units", "metric") == "imperial"
	var bias_minutes: int = Time.get_time_zone_from_system().get("bias", 0)
	show_highlights(orqwa.highlights(path, bias_minutes), voice, imperial, path.hash())


## Shows `highlights` in `voice`, the casual lines chosen by `seed`.
func show_highlights(highlights: Array, voice: String, imperial: bool, seed: int) -> void:
	for child: Node in get_children():
		remove_child(child)
		child.queue_free()
	var shown: int = 0
	for item: Variant in highlights:
		var moment: Dictionary = item
		if moment.get("kind", "") == "power_peak":
			continue
		var text: String = MomentLines.line(moment, voice, imperial, absi(seed + shown))
		if text.is_empty():
			continue
		var label: Label = Label.new()
		label.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		label.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
		if MomentLines.is_record(moment):
			label.text = "★ " + text
			label.add_theme_color_override("font_color", RECORD_COLOR)
		else:
			label.text = text
		if shown == 0:
			label.add_theme_font_size_override("font_size", 18)
		add_child(label)
		shown += 1
	visible = shown > 0


## The lines shown, for tests.
func lines() -> PackedStringArray:
	var texts: PackedStringArray = []
	for child: Node in get_children():
		texts.append((child as Label).text)
	return texts
