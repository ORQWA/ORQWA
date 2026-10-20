extends SceneTree
## Headless checks of the settings (R83): the cog at the top right of the start page opens
## them, with this computer's graphics, no longer among the devices, its courses' kept worlds
## (R101), and About: the version, the commit and the licences of everything in the app.
## Run: godot --headless --path app -s res://tests/settings_smoke.gd

const MAIN_SCENE: String = "res://scenes/main.tscn"

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	TranslationServer.set_locale("en")
	var main: Control = (load(MAIN_SCENE) as PackedScene).instantiate()
	root.add_child(main)
	await process_frame
	var start: StartPage = main.get_node("StartPage")
	await _cog(start)
	var dialog: SettingsDialog = start.get("_settings")
	var orqwa: OrqwaApp = main.get_node("Orqwa")
	await _graphics(orqwa, dialog)
	_courses(orqwa, dialog)
	_about(dialog)
	_updates(orqwa, dialog, main)
	await _in_german(dialog)
	dialog.hide()
	main.free()
	if not _failed:
		print("SETTINGS SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## The cog closes the header's row, right of the tabs, and opens the settings on Graphics; the
## Devices tab keeps the devices alone.
func _cog(start: StartPage) -> void:
	var header: HBoxContainer = start.get_node("%Header")
	var cog: Button = start.get("_settings_button")
	var tab_bar: TabBar = start.get("_tab_bar")
	_check(cog.get_parent() == header, "the cog is in the header")
	_check(cog.get_index() == header.get_child_count() - 1, "at its end")
	_check(
		absf(cog.get_global_rect().end.x - header.get_global_rect().end.x) < 1.0,
		"at the top right: %s of %s" % [cog.get_global_rect(), header.get_global_rect()]
	)
	_check(cog.get_global_rect().position.x >= tab_bar.get_global_rect().end.x, "beside the tabs")
	_check(cog.tooltip_text == "Settings", "it says what it opens")
	_check(tab_bar.get_tab_title(StartPage.Tab.DEVICES) == "Devices", "the tab is Devices")
	var devices: DevicesTab = start.get("_devices")
	for text: String in ["Graphics", "Quality", "Full screen (F11)"]:
		_check(not _shows(devices, text), "no %s among the devices" % text)
	var dialog: SettingsDialog = start.get("_settings")
	var tabs: TabContainer = dialog.get("_tabs")
	tabs.current_tab = 1
	# Headless, the window is tiny unless sized like the app's.
	root.size = Vector2i(1600, 900)
	await process_frame
	cog.pressed.emit()
	await process_frame
	_check(dialog.visible, "the cog opens the settings")
	var window: Rect2i = Rect2i(Vector2i.ZERO, root.size)
	_check(
		window.encloses(Rect2i(dialog.position, dialog.size)),
		"within the window: %s in %s" % [Rect2i(dialog.position, dialog.size), window]
	)
	_check(tabs.get_tab_count() == 3, "three tabs")
	var titles: Array = range(tabs.get_tab_count()).map(tabs.get_tab_title)
	_check(titles == ["Graphics", "Courses", "About"], "titled: %s" % [titles])
	_check(tabs.current_tab == 0, "opening on Graphics")


## The quality is kept for this computer (R43, R78); full screen fills the app's window, not
## the settings' own (#218).
func _graphics(orqwa: OrqwaApp, dialog: SettingsDialog) -> void:
	var graphics: GraphicsSettings = dialog.get("_graphics")
	var quality: OptionButton = graphics.get("_quality")
	var before: String = orqwa.graphics_quality()
	for i: int in range(quality.item_count):
		quality.select(i)
		quality.item_selected.emit(i)
		var key: String = quality.get_item_metadata(i)
		_check(orqwa.graphics_quality() == key, "choosing %s keeps it" % key)
	orqwa.set_graphics_quality(before)
	var box: CheckBox = graphics.get("_fullscreen")
	# Headless, the app's window has no mode of its own: it is checked on a real display.
	var headless: bool = DisplayServer.get_name() == "headless"
	orqwa.set_fullscreen(false)
	box.button_pressed = true
	await process_frame
	_check(orqwa.fullscreen(), "full screen, also next time")
	_check(headless or FullScreen.is_on(root), "the app's window fills the screen")
	_check(not FullScreen.is_on(dialog), "not the settings' window")
	box.button_pressed = false
	_check(not orqwa.fullscreen(), "and back in a window")
	_details(orqwa, graphics)


## The details set apart from the preset (R90): one choice per option the core knows, Auto
## first; a choice is kept for this computer, Auto and the reset follow the preset again.
func _details(orqwa: OrqwaApp, graphics: GraphicsSettings) -> void:
	var details: Dictionary = graphics.get("_details")
	var options: Dictionary = OrqwaApp.graphics_option_values()
	_check(details.size() == options.size(), "a choice per option: %d" % details.size())
	var shadows: OptionButton = details["shadows"]
	var first: String = shadows.get_item_metadata(0)
	_check(first == "auto", "Auto first")
	_check(_shadows(orqwa) == "auto", "following the preset at first")
	var last: int = shadows.item_count - 1
	shadows.select(last)
	shadows.item_selected.emit(last)
	_check(_shadows(orqwa) == "ultra", "the choice is kept")
	_check(not orqwa.set_graphics_option("shadows", "extreme"), "unknown values refused")
	graphics.call("_reset_details")
	_check(_shadows(orqwa) == "auto", "the reset follows the preset again")
	_check(shadows.selected == 0, "and shows it")


func _shadows(orqwa: OrqwaApp) -> String:
	var options: Dictionary = orqwa.graphics_options()
	return options["shadows"]


## About: the version and the commit, then each part's licence notice with its full text.
func _about(dialog: SettingsDialog) -> void:
	var about: AboutPage = dialog.get("_about")
	var version: Label = about.get("_version")
	var commit: String = OrqwaCore.commit()
	_check(version.text.contains(OrqwaCore.version()), "the version: %s" % version.text)
	_check(commit.is_empty() or version.text.contains(commit), "and the commit: %s" % commit)
	var report: String = about.call("_version_report")
	_check(report.begins_with("ORQWA " + OrqwaCore.version()), "copied for a bug report")
	var godot: String = Engine.get_version_info()["string"]
	_check(report.contains(godot), "with Godot's version")
	var parts: ItemList = about.get("_parts")
	var titles: PackedStringArray = []
	for i: int in range(parts.item_count):
		titles.append(parts.get_item_text(i))
	var listed: PackedStringArray = [
		"ORQWA", "Godot Engine", "FFmpeg", "Rust libraries", "Models and data"
	]
	_check(titles == listed, "ORQWA, then what it is built from: %s" % titles)
	var notice: TextEdit = about.get("_notice")
	_check(notice.text.contains("GNU GENERAL PUBLIC LICENSE"), "ORQWA's licence shows first")
	_check(notice.text.contains(OrqwaCore.source_url()), "with where its source code is")
	var expected: Array = [
		[1, Engine.get_license_text().left(60)],
		[1, "FreeType"],
		[2, "GNU LESSER GENERAL PUBLIC LICENSE"],
		[3, "Permission is hereby granted, free of charge"],
		[4, "Human Base Meshes"],
	]
	for part: Array in expected:
		var index: int = part[0]
		var text: String = part[1]
		parts.select(index)
		parts.item_selected.emit(index)
		_check(notice.text.contains(text), "%s: %s" % [titles[index], text])


## Keeping the courses' worlds (R101): off until this computer turns it on, which tells the
## room they take; off again, they are deleted.
func _courses(orqwa: OrqwaApp, dialog: SettingsDialog) -> void:
	var courses: CourseSettings = dialog.get("_courses")
	var keep: CheckBox = courses.get("_keep")
	var note: Label = courses.get("_note")
	orqwa.set_keeps_worlds(false)
	courses.refresh()
	_check(not keep.button_pressed, "worlds are not kept until asked")
	_check(keep.text == "Keep courses ready to ride", "the switch: %s" % keep.text)
	_check(note.text.contains("10 MB per kilometre"), "telling the room they take: %s" % note.text)
	keep.button_pressed = true
	_check(orqwa.keeps_worlds(), "kept on this computer")
	_check(note.text.contains("The kept worlds take"), "and how much they take now: %s" % note.text)
	keep.button_pressed = false
	_check(not orqwa.keeps_worlds(), "and off again")
	_check(not note.text.contains("The kept worlds take"), "they are deleted: %s" % note.text)


## Looking for updates (R99): on at start-up until this computer turns it off; a newer
## version is offered on the start page, or once a ride is over, never during it.
func _updates(orqwa: OrqwaApp, dialog: SettingsDialog, main: Control) -> void:
	var about: AboutPage = dialog.get("_about")
	var at_start: CheckBox = about.get("_check_at_start")
	_check(at_start.button_pressed and orqwa.checks_for_updates(), "looks for updates at start")
	at_start.button_pressed = false
	_check(not orqwa.checks_for_updates(), "turned off for this computer")
	at_start.button_pressed = true
	_check(orqwa.checks_for_updates(), "and on again")
	var offer: UpdateDialog = main.get("_update")
	orqwa.update_available.emit("99.0.0", "https://example.org/v99.0.0", false)
	var status: Label = about.get("_update_status")
	_check(status.text.contains("99.0.0"), "the About tab tells: %s" % status.text)
	_check(offer.visible and offer.dialog_text.contains("99.0.0"), "offered on the start page")
	_check(offer.dialog_text.contains(OrqwaCore.version()), "beside the running version")
	_check(offer.ok_button_text == "Open the release page", "elsewhere, its page")
	offer.hide()
	offer.offer("99.0.0", "https://example.org/v99.0.0", true)
	_check(offer.ok_button_text == "Update now", "on Windows, installed by ORQWA")
	offer.hide()
	var start: StartPage = main.get_node("StartPage")
	start.hide()
	orqwa.update_available.emit("99.0.0", "https://example.org/v99.0.0", false)
	_check(not offer.visible, "not during a ride")
	main.call("_back_home")
	_check(offer.visible, "once the ride is over")
	offer.hide()


## Built in English before the rider's language is known, as the start page builds it.
func _in_german(dialog: SettingsDialog) -> void:
	TranslationServer.set_locale("de")
	await process_frame
	var tabs: TabContainer = dialog.get("_tabs")
	# The tab bar translates its titles as it draws them.
	var graphics: String = tabs.get_tab_bar().atr(tabs.get_tab_title(0))
	var courses_title: String = tabs.get_tab_bar().atr(tabs.get_tab_title(1))
	var about_title: String = tabs.get_tab_bar().atr(tabs.get_tab_title(2))
	_check(graphics == "Grafik", "Graphics in German: %s" % graphics)
	_check(courses_title == "Routen", "Courses in German: %s" % courses_title)
	_check(about_title == "Über ORQWA", "About in German: %s" % about_title)
	_check(dialog.atr(dialog.title) == "Einstellungen", "titled: %s" % dialog.atr(dialog.title))
	var about: AboutPage = dialog.get("_about")
	var parts: ItemList = about.get("_parts")
	_check(parts.get_item_text(3) == "Rust-Bibliotheken", "parts: %s" % parts.get_item_text(3))
	var version: Label = about.get("_version")
	_check(version.text.begins_with("Version"), "the version: %s" % version.text)
	TranslationServer.set_locale("en")
	await process_frame


## Whether a label or button under `node` shows `text`.
func _shows(node: Node, text: String) -> bool:
	for control: Node in node.find_children("*", "", true, false):
		if (control is Label or control is Button) and control.get("text") == text:
			return true
	return false


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("SETTINGS SMOKE TEST FAILED: " + what)
		_failed = true
