extends SceneTree
## Deterministic documentation fixtures for planned workouts, readings and failures. They
## exercise the real UI without connecting to hardware or a training-platform account.

var _main: Control
var _out: String


func _initialize() -> void:
	_out = OS.get_environment("SCREENSHOT_DIR")
	_run.call_deferred()


func _run() -> void:
	for locale: String in ["en", "de"]:
		if _main != null:
			_main.free()
		TranslationServer.set_locale(locale)
		_main = (load("res://scenes/main.tscn") as PackedScene).instantiate()
		root.add_child(_main)
		await process_frame
		TranslationServer.set_locale(locale)
		var suffix: String = "-de" if locale == "de" else ""
		var orqwa: OrqwaApp = _main.get_node("Orqwa")
		var start: StartPage = _main.get_node("StartPage")
		var tabs: TabContainer = start.get("_tabs")
		tabs.current_tab = StartPage.Tab.DEVICES
		var devices: DevicesTab = start.find_children("*", "DevicesTab", true, false)[0]
		devices.set_process(false)
		for pair: Array in [
			["trainer", "KICKR CORE"],
			["power_meter", "Assioma"],
			["heart_rate", "Polar H9"],
			["controller", "Zwift Click v2"],
		]:
			var role: String = pair[0]
			var device: String = pair[1]
			var option: OptionButton = devices._options[role]
			option.clear()
			option.add_item(device)
			option.set_item_metadata(0, DevicesTab.REMEMBERED)
			option.select(0)
			devices._expected[role] = device
			devices._set_state(role, "connected")
		devices._show_reading("trainer", {"power": 218.0, "cadence": 88.0})
		devices._show_reading("power_meter", {"power": 221.0, "cadence": 89.0, "battery": 76})
		devices._show_reading("heart_rate", {"heart_rate": 142.0, "battery": 83})
		devices._show_reading("controller", {"battery_left": 80, "battery_right": 77})
		await _save("devices-readings" + suffix)
		devices._on_device_failed("Zwift Click v2", "connecting timed out")
		await _save("devices-failed" + suffix)
		tabs.current_tab = StartPage.Tab.WORKOUTS
		var workouts: WorkoutsTab = start.find_children("*", "WorkoutsTab", true, false)[0]
		var plan: Dictionary = {}
		for candidate: Dictionary in orqwa.workouts():
			if candidate["id"] == "builtin:threshold-2x15":
				plan = candidate.duplicate(true)
				break
		if plan.is_empty():
			push_error("Missing documentation workout fixture")
			quit(1)
			return
		plan["id"] = "planned:intervals:documentation"
		plan["planned"] = "intervals.icu"
		plan["builtin"] = false
		workouts._options.set_plans([plan])
		workouts._show_planned([plan])
		workouts._options.set_workout({"kind": "plan", "id": plan["id"]})
		workouts._show_plan()
		await _save("planned" + suffix)
		workouts._show_planned([])
		await _save("planned-nothing" + suffix)
		workouts._show_planned([plan])
		workouts._on_planned_workouts_failed("intervals.icu", "Network unreachable")
		await _save("planned-offline" + suffix)
		orqwa.ride_detail("/missing-documentation-ride.fit", 10)
		var details: FailureDetails = _main.get("_failures")
		var toggle: Button = details.get("_toggle")
		toggle.button_pressed = true
		await _save("error-details" + suffix)
	TranslationServer.set_locale("en")
	quit(0)


func _save(name: String) -> void:
	await create_timer(0.3).timeout
	var result: Error = root.get_texture().get_image().save_png(_out.path_join(name + ".png"))
	if result != OK:
		push_error("Cannot save screenshot: " + name)
		quit(1)
	print("saved ", name)
