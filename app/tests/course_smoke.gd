extends SceneTree
## Headless checks of a course's page (R40): its name with a pencil and a bin, as a ride in
## the history has (#190), and the dialog the pencil opens to rename the course and add, align
## or remove its video (R17).
## Also the gallery's cards and toolbar (R67).
## Run: godot --headless --path app -s res://tests/course_smoke.gd

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_course_page()
	_edit_dialog()
	_estimates()
	_card_countries()
	await _course_gallery()
	await _card_widths()
	if not _failed:
		print("COURSE SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## The page's name has a pencil that asks for the dialog, and a bin beside it; the video's
## controls are in the dialog, not on the page.
func _course_page() -> void:
	var title: EditableTitle = EditableTitle.new("Edit it", false)
	root.add_child(title)
	var requests: Array[bool] = []
	title.edit_requested.connect(func() -> void: requests.append(true))
	var pencil: Button = title.get("_button")
	pencil.pressed.emit()
	var field: LineEdit = title.get("_edit")
	_check(requests == [true] and not field.editable, "the pencil asks for an editor")
	title.free()
	var page: CourseDetail = CourseDetail.new()
	root.add_child(page)
	var bin: Button = page.get("_delete_button")
	var page_title: EditableTitle = page.get("_title")
	_check(
		bin.get_parent() == page_title and bin.icon != null and bin.tooltip_text != "",
		"a bin beside the pencil, with a tooltip"
	)
	var box: StyleBoxFlat = bin.get_theme_stylebox("normal") as StyleBoxFlat
	_check(
		box != null and Color(box.bg_color, 1.0).is_equal_approx(UiTheme.DANGER), "the bin is red"
	)
	_check(_button_with_text(page, "Delete course") == null, "no Delete course button")
	var dialog: CourseEditDialog = page.get("_edit_dialog")
	var add_on_page: Button = _button_with_text(page, "Add video…")
	_check(dialog.is_ancestor_of(add_on_page), "videos are added in the dialog, not on the page")
	page.free()


## The pencil's dialog: nothing changes until Save, then the changes come in the order they
## are applied.
func _edit_dialog() -> void:
	var dialog: CourseEditDialog = CourseEditDialog.new()
	root.add_child(dialog)
	var events: Array = []
	dialog.course_renamed.connect(
		func(course_name: String) -> void: events.append(["renamed", course_name])
	)
	dialog.video_removed.connect(func() -> void: events.append(["removed"]))
	dialog.video_added.connect(
		func(path: String, marks: PackedVector2Array) -> void: events.append(["added", path, marks])
	)
	dialog.video_aligned.connect(
		func(marks: PackedVector2Array) -> void: events.append(["aligned", marks])
	)
	var name_edit: LineEdit = dialog.get("_name_edit")
	var add: Button = dialog.get("_add_button")
	var align: Button = dialog.get("_align_button")
	var remove: Button = dialog.get("_remove_button")
	var align_dialog: VideoAlignDialog = dialog.get("_align_dialog")
	var marks: PackedVector2Array = [Vector2(0, 0), Vector2(2000, 60)]
	var other_marks: PackedVector2Array = [Vector2(0, 5), Vector2(2000, 88)]
	var video: Dictionary = {
		"path": "/videos/gurten.mp4",
		"duration_s": 60.0,
		"aligned_by_hand": true,
		"located": true,
		"marks": marks,
	}
	dialog.edit(null, "Gurten", video)
	_check(name_edit.text == "Gurten", "the dialog shows the course's name")
	_check(not add.visible and align.visible and remove.visible, "a video placed by hand")
	name_edit.text = "Gurten climb"
	remove.pressed.emit()
	dialog.canceled.emit()
	_check(events.is_empty(), "Cancel changes nothing: %s" % [events])

	dialog.edit(null, "Gurten", video)
	dialog.confirmed.emit()
	name_edit.text = "   "
	dialog.confirmed.emit()
	_check(events.is_empty(), "the same name or a blank one is not saved: %s" % [events])
	name_edit.text = " Gurten climb "
	dialog.confirmed.emit()
	_check(events == [["renamed", "Gurten climb"]], "renamed on Save: %s" % [events])

	events.clear()
	dialog.edit(null, "Gurten", video)
	align_dialog.aligned.emit(other_marks)
	dialog.confirmed.emit()
	_check(events == [["aligned", other_marks]], "aligned again on Save: %s" % [events])

	events.clear()
	dialog.edit(null, "Gurten", video)
	remove.pressed.emit()
	_check(add.visible and not align.visible and not remove.visible, "removed: one can be added")
	dialog.confirmed.emit()
	_check(events == [["removed"]], "removed on Save: %s" % [events])

	# Another video in its place: the old one goes first.
	events.clear()
	dialog.edit(null, "Gurten", video)
	remove.pressed.emit()
	dialog.set("_choosing", "/videos/new.mp4")
	dialog.set("_choosing_duration_s", 90.0)
	align_dialog.aligned.emit(other_marks)
	_check(align.visible and remove.visible, "the new video shows")
	dialog.confirmed.emit()
	_check(
		events == [["removed"], ["added", "/videos/new.mp4", other_marks]],
		"replaced on Save: %s" % [events]
	)

	dialog.edit(null, "Gurten", {})
	_check(add.visible and not align.visible and not remove.visible, "no video: add one")
	var rlv: Dictionary = video.duplicate()
	rlv["aligned_by_hand"] = false
	rlv["located"] = false
	dialog.edit(null, "Tour", rlv)
	_check(not add.visible and not align.visible and not remove.visible, "an RLV keeps its video")
	# While the route loads, its video is not known yet: only the name changes.
	events.clear()
	dialog.edit(null, "Gurten", {}, false)
	var section: Control = dialog.get("_video_section")
	_check(not section.visible, "no video before the route is loaded")
	name_edit.text = "Gurten climb"
	dialog.confirmed.emit()
	_check(events == [["renamed", "Gurten climb"]], "renamed while loading: %s" % [events])
	dialog.free()


## A card names the countries its course passes (#230): codes, their names on hover.
func _card_countries() -> void:
	var course: Dictionary = {
		"path": "/tmp/x.tqc",
		"name": "Dreiländer",
		"length_m": 54000.0,
		"elevation_gain_m": 420.0,
		"max_grade": 8.0,
		"track": PackedVector2Array([Vector2(0, 0), Vector2(500, 800)]),
		"profile": PackedVector2Array([Vector2(0, 260), Vector2(54000, 300)]),
	}
	course["countries"] = [
		{"code": "CH", "name": "Switzerland"},
		{"code": "FR", "name": "France"},
	]
	var countries_card: CourseCard = CourseCard.new(course, false)
	var countries: Label = _label_with_text(countries_card, "CH · FR")
	_check(
		countries != null and countries.tooltip_text == "Switzerland, France",
		"countries by code, named on hover"
	)
	_check(_label_with_text(countries_card, "Countries") != null, "under Countries")
	countries_card.free()


## Estimated times show to the nearest 5 minutes, on the course page and the cards (R66, R67).
func _estimates() -> void:
	var shown: PackedStringArray = PackedStringArray()
	for seconds: float in [60.0, 1000.0, 3540.0, 6300.0, 7650.0]:
		shown.append(CourseCard.estimate(seconds))
	_check(
		shown == PackedStringArray(["5 min", "15 min", "1 h 00", "1 h 45", "2 h 10"]),
		"estimates to 5 minutes: %s" % shown
	)


## The gallery's toolbar and chips (R67) ask the core for what the rider chose; clearing
## the filters keeps the order and groups.
func _course_gallery() -> void:
	var torqa: TorqaApp = TorqaApp.new()
	root.add_child(torqa)
	torqa.set_courses_view("name", false, "none")
	var tab: CoursesTab = CoursesTab.new()
	root.add_child(tab)
	tab.theme = UiTheme.build()
	tab.size = Vector2(1600, 900)
	tab.bind(torqa)
	var asked: Dictionary = tab.query()
	var asked_sort: String = asked["sort"]
	var asked_descending: bool = asked["descending"]
	var asked_group: String = asked["group"]
	_check(
		asked_sort == "name" and not asked_descending and asked_group == "none",
		"the view as remembered: %s" % asked
	)
	var chips: HFlowContainer = tab.get("_chips")
	var texts: PackedStringArray = PackedStringArray()
	var hilly: Button = null
	for child: Node in chips.get_children():
		var chip: Button = child as Button
		if chip != null and chip.toggle_mode:
			texts.append(chip.text)
			if chip.text == "Hilly":
				hilly = chip
	_check(
		"3D" in texts and "< 25 km" in texts and "25–50 km" in texts and "≥ 100 km" in texts,
		"chips for type, terrain and length: %s" % texts
	)
	# A chip chosen shows Clear filters beside them; nothing in the row grows or moves. The
	# chips show whatever the library holds.
	chips.show()
	await process_frame
	await process_frame
	var chip_size: Vector2 = hilly.size
	var row_height: float = chips.size.y
	tab.call("_show_clear", true)
	await process_frame
	await process_frame
	_check(
		hilly.size == chip_size and is_equal_approx(chips.size.y, row_height),
		(
			"chips keep their size: %s → %s, row %s → %s"
			% [chip_size, hilly.size, row_height, chips.size.y]
		)
	)
	hilly.button_pressed = true
	var terrains: PackedStringArray = tab.query()["terrains"]
	_check(terrains == PackedStringArray(["hilly"]), "a chip narrows down: %s" % terrains)
	# Another order starts in its own direction: the latest courses first.
	var sort: OptionButton = tab.get("_sort")
	for i: int in sort.item_count:
		if sort.get_item_metadata(i) == "added":
			sort.select(i)
			sort.item_selected.emit(i)
	var latest_first: bool = tab.query()["descending"]
	_check(latest_first, "the latest first")
	var direction: Button = tab.get("_direction")
	direction.pressed.emit()
	var turned: bool = tab.query()["descending"]
	_check(not turned, "the arrow turns the order round")
	var remembered: Dictionary = torqa.courses_view()
	var remembered_sort: String = remembered["sort"]
	_check(remembered_sort == "added", "the order is remembered: %s" % remembered)
	var search: LineEdit = tab.get("_search")
	search.text = "gurten"
	search.text_changed.emit("gurten")
	_check(tab.filtered(), "a search filters")
	tab.clear_filters()
	var cleared: Dictionary = tab.query()
	var cleared_terrains: PackedStringArray = cleared["terrains"]
	var cleared_search: String = cleared["search"]
	var cleared_sort: String = cleared["sort"]
	_check(
		cleared_search.is_empty() and cleared_terrains.is_empty() and cleared_sort == "added",
		"filters cleared, the order kept: %s" % cleared
	)
	_check(not hilly.button_pressed, "the chip is off again")
	torqa.set_courses_view("name", false, "none")
	tab.free()
	torqa.free()


## The gallery's cards fill its whole width, however wide the window (R39): as many side by
## side as fit at their narrowest, sharing the row; the route grows with the card.
func _card_widths() -> void:
	var gap: float = CoursesTab.CARD_GAP
	for available: float in [300.0, 640.0, 1280.0, 1918.0, 3840.0]:
		var width: float = CoursesTab.card_width(available)
		var columns: int = floori((available + gap) / (width + gap))
		var row: float = columns * width + (columns - 1) * gap
		_check(
			width >= CourseCard.WIDTH and row <= available and available - row < columns,
			"%d cards of %s fill a row of %s" % [columns, width, available]
		)
	_check(is_equal_approx(CoursesTab.card_width(200.0), CourseCard.WIDTH), "never narrower")
	var course: Dictionary = {
		"name": "Gurten", "length_m": 2400.0, "elevation_gain_m": 149.4, "max_grade": 17.6
	}
	var card: CourseCard = CourseCard.new(course, false)
	card.theme = UiTheme.build()
	root.add_child(card)
	card.custom_minimum_size.x = 600.0
	await process_frame
	await process_frame
	var route: PathCard = card.get("_path_card")
	_check(
		absf(route.size.y - route.size.x * CourseCard.MAP_ASPECT) <= 1.0 and route.size.x > 500.0,
		"the route grows with the card: %s" % route.size
	)
	card.free()


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("COURSE SMOKE TEST FAILED: " + what)
		_failed = true


## The first button under `node` showing `text`.
func _button_with_text(node: Node, text: String) -> Button:
	for button: Node in node.find_children("*", "Button", true, false):
		if (button as Button).text == text:
			return button
	return null


func _label_with_text(node: Node, text: String) -> Label:
	for child: Node in node.get_children():
		var label: Label = child as Label
		if label != null and label.text == text:
			return label
		var found: Label = _label_with_text(child, text)
		if found != null:
			return found
	return null
