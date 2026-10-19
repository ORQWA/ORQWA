extends SceneTree
## Headless checks of the Workouts tab's planned workouts (R81): the day's workouts from the
## platform holding the rider's plan are listed with a button that chooses each, are edited
## only as copies, the section is hidden without such an account, and says so when nothing is
## planned. Choosing another course clears the build log of the one before (R93).
## Run: godot --headless --path app -s res://tests/workouts_smoke.gd

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_planned()
	_course_choice()
	if not _failed:
		print("WORKOUTS SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _planned() -> void:
	var tab: WorkoutsTab = WorkoutsTab.new()
	root.add_child(tab)
	var panel: PanelContainer = tab._planned_panel
	_check(not panel.visible, "no plan section without an account")
	var plans: Array = [
		_plan("planned:intervals:1", "Sweet spot", "intervals.icu"),
		_plan("builtin:recovery-30", "Recovery 30", ""),
	]
	tab._options.set_plans(plans)
	tab._show_planned(plans)
	_check(panel.visible, "the section shows with a plan")
	var rows: VBoxContainer = tab._planned_rows
	var buttons: Array[Node] = rows.find_children("*", "Button", true, false)
	_check(
		buttons.size() == 1,
		"a button per planned workout, not per library one: %d" % buttons.size()
	)
	var labels: Array[Node] = rows.find_children("*", "Label", true, false)
	var text: String = (labels[0] as Label).text if not labels.is_empty() else ""
	_check(text.begins_with("Sweet spot") and text.ends_with("intervals.icu"), "named: %s" % text)
	_check(not tab._planned_note.visible, "no note while something is planned")
	(buttons[0] as Button).pressed.emit()
	var chosen: Dictionary = tab.workout()
	var chosen_plan: Dictionary = chosen["plan"]
	var chosen_id: String = chosen_plan.get("id", "")
	_check(chosen_id == "planned:intervals:1", "chosen on a click: %s" % chosen)
	# The plan stays the platform's: edited as a copy in the library, never deleted.
	var edit: Button = tab._edit_button
	_check(edit.text == "Edit a copy…", "a planned workout is edited as a copy: %s" % edit.text)
	tab._show_planned([plans[1]])
	_check(rows.find_children("*", "Button", true, false).is_empty(), "nothing listed")
	_check(
		tab._planned_note.visible and tab._planned_note.text == "Nothing planned for today.",
		"an empty day says so: %s" % tab._planned_note.text
	)
	tab.free()


## A `TorqaApp.workouts()` entry, planned on `platform` or from the library when empty.
static func _plan(id: String, workout_name: String, platform: String) -> Dictionary:
	return {
		"id": id,
		"name": workout_name,
		"description": "",
		"duration_s": 3600.0,
		"builtin": id.begins_with("builtin:"),
		"planned": platform,
		"steps": [],
	}


## The log of the world built for one course goes once another is chosen; the log of a world
## still being built stays.
func _course_choice() -> void:
	var tab: WorkoutsTab = WorkoutsTab.new()
	root.add_child(tab)
	var where: OptionButton = tab._where
	where.add_item("On its own (HUD only)")
	where.add_item("Alps")
	where.add_item("Coast")
	var log: PreparationLog = tab._log
	log.start()
	log.step("Building 3D world", "chunks", 40, 40)
	tab._building = true
	where.item_selected.emit(2)
	_check(log.visible and not log.steps().is_empty(), "a build under way keeps its log")
	tab._building = false
	log.finish()
	where.item_selected.emit(2)
	_check(not log.visible and log.steps().is_empty(), "another course clears the built one's log")
	tab.free()


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("WORKOUTS SMOKE TEST FAILED: " + what)
		_failed = true
