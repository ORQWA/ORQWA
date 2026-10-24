class_name RideRecap
extends Label
## One line in a ride's summaries (R124) for what its figures and record badges do not say:
## that it was the rider's first ride on the course, or how far into their week it is. Told
## in the rider's voice, looking back on the ride, the same line each time the summary opens.


func _init() -> void:
	auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_theme_font_size_override("font_size", 18)
	hide()


## Shows the line for the ride saved at `path`, as `OrqwaApp.ride_recap()` has it.
func show_ride(orqwa: OrqwaApp, path: String) -> void:
	var profile: Dictionary = orqwa.profile()
	var bias_minutes: int = Time.get_time_zone_from_system().get("bias", 0)
	var voice: String = profile.get("voice", "casual")
	var imperial: bool = profile.get("units", "metric") == "imperial"
	show_recap(orqwa.ride_recap(path, bias_minutes), voice, imperial, path.hash())


## Shows `recap` (a moment, empty for none) in `voice`, the casual line chosen by `seed`.
func show_recap(recap: Dictionary, voice: String, imperial: bool, seed: int) -> void:
	var moment: Dictionary = recap.duplicate()
	# The ride is over: its first ride on the course is told looking back.
	if moment.get("kind", "") == "first_ride":
		moment["kind"] = "first_ride_done"
	text = MomentLines.line(moment, voice, imperial, absi(seed))
	visible = not text.is_empty()
