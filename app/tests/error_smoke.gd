extends SceneTree
## #261: errors translate before formatting; diagnostics survive notification lifetimes.

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	TranslationServer.set_locale("en")
	var course: CourseDetail = CourseDetail.new()
	var settings: RideSettingsDialog = RideSettingsDialog.new()
	# Built before the rider's profile picks the language, as the start page builds it.
	var zones: ZonesEditor = ZonesEditor.new()
	root.add_child(course)
	root.add_child(settings)
	root.add_child(zones)
	await process_frame
	TranslationServer.set_locale("de")
	await process_frame
	var ride_button: Button = course.get("_ride_button")
	var tabs: TabContainer = settings.get("_tabs")
	var ride_options: RideOptions = settings.get("_options")
	_check(ride_button.text == "Fahren", "ride action refreshes after language switch")
	_check(tabs.get_tab_title(ride_options.get_index()) == "Fahrt", "ride tab refreshes")
	_check(TranslationServer.translate("Ride", "ride_action") == "Fahren", "ride action")
	_check(TranslationServer.translate("Ride", "ride_tab") == "Fahrt", "ride tab")
	var heading: Label = zones.get_child(0)
	_check(
		heading.atr(heading.text) == "Leistungszonen: jeweilige Obergrenze in % der FTP",
		"captions built in English follow the switch to German: %s" % heading.atr(heading.text)
	)
	var zone: Label = (zones.get_child(1) as GridContainer).get_child(0)
	_check(zone.text == "Z1 Erholung", "zone names follow the language: %s" % zone.text)
	var path: String = "C:/Räder/100%/%s.fit"
	var failure: Dictionary = {
		"message": "Could not open %s.",
		"args": PackedStringArray([path]),
		"details": "Permission denied: " + path,
	}
	_check(
		FailureDetails.summary(failure) == path + " konnte nicht geöffnet werden.",
		"filenames stay literal after German translation"
	)
	var history: FailureDetails = FailureDetails.new()
	root.add_child(history)
	history.report(failure)
	await process_frame
	_check(history.visible, "details remain available independently of a toast")
	var panel: PanelContainer = history.get("_panel")
	var toggle: Button = history.get("_toggle")
	_check(not panel.visible and toggle.text == "Technische Details", "details start collapsed")
	toggle.button_pressed = true
	_check(panel.visible, "details expand")
	var rows: VBoxContainer = history.get("_rows")
	var original_details: String = failure["details"]
	_check((rows.get_child(1) as Label).text == original_details, "diagnostic stays exact")
	history.clear()
	_check(not history.visible and not panel.visible, "dismissed details hide until the next")
	history.report(failure)
	_check(history.visible and rows.get_child_count() == 2, "dismissing forgets earlier failures")
	TranslationServer.set_locale("en")
	await process_frame
	_check(ride_button.text == "Ride", "ride action switches back to English")
	_check(tabs.get_tab_title(ride_options.get_index()) == "Ride", "ride tab switches back")
	_check(FailureDetails.summary(failure) == "Could not open " + path + ".", "English summary")
	var torqa: TorqaApp = TorqaApp.new()
	root.add_child(torqa)
	await process_frame
	var summaries: Array[String] = []
	torqa.failed.connect(func(message: String) -> void: summaries.append(message))
	TranslationServer.set_locale("de")
	torqa.ride_detail("/missing-261.fit", 10)
	var last_summary: String = summaries.back()
	_check(last_summary == "Die Aktivität konnte nicht geladen werden.", "core error translates")
	var payload: Dictionary = torqa.last_failure()
	var message_key: String = payload["message"]
	var diagnostic: String = payload["details"]
	_check(message_key == "Could not load the activity.", "binding preserves the key")
	_check("/missing-261.fit" in diagnostic, "binding preserves the original cause")
	torqa.free()
	history.free()
	zones.free()
	settings.free()
	course.free()
	if not _failed:
		print("ERROR SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _check(condition: bool, what: String) -> void:
	if not condition:
		_failed = true
		push_error(what)
