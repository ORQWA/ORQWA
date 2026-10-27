extends SceneTree
## Season selection, compatibility with earlier ride options, and independent conditions (R154).

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var options: RideOptions = RideOptions.new()
	root.add_child(options)
	options.set_options({})
	var selected: String = options.options()["season"]
	_check(selected == "Summer", "old options default to Summer")
	for season: String in OrqwaApp.seasons():
		options.set_options({"season": season, "weather": "Rain", "time": "Evening"})
		selected = options.options()["season"]
		var weather: String = options.options()["weather"]
		var time: String = options.options()["time"]
		_check(selected == season, "each available season can be selected")
		_check(weather == "Rain", "season preserves weather")
		_check(time == "Evening", "season preserves time of day")
		options.show_option_groups(false, true)
		var control: OptionButton = options.get("_season")
		_check(not control.visible, "season is hidden for video rides")
		selected = options.options()["season"]
		_check(selected == season, "hidden season retains selection")
	options.free()
	if not _failed:
		print("SEASONS SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _check(ok: bool, message: String) -> void:
	if not ok:
		_failed = true
		push_error(message)
