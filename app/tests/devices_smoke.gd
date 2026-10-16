extends SceneTree
## Headless checks of the Devices tab: the shifter's buttons (#139, R75), how each row tells
## what its device is doing (R76), and what stands beside it: the device's reading, or why
## connecting failed (R80).
## Run: godot --headless --path app -s res://tests/devices_smoke.gd

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_shifter_buttons()
	_device_states()
	if not _failed:
		print("DEVICES SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## Every action the core offers for a shifter's buttons can be chosen by its label (#139),
## and a Zwift Click (R75) shows its two buttons with a press and a hold, no double press.
func _shifter_buttons() -> void:
	var actions: PackedStringArray = TorqaApp.button_actions()
	for action: String in actions:
		_check(DevicesTab.ACTION_LABELS.has(action), "a label for %s" % action)
	_check(actions.size() == DevicesTab.ACTION_LABELS.size(), "no label for a gone action")
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	var choices: Array[Node] = tab.find_children("*", "OptionButton", true, false)
	# Trainer, power meter, cadence, heart rate and shifter, a press, a hold and a double press
	# of four channels, and the graphics quality.
	_check(
		choices.size() == 5 + 4 * 3 + 1, "a choice per press of each channel: %d" % choices.size()
	)
	var press: OptionButton = choices[5]
	_check(press.item_count == 1 + actions.size(), "nothing or any action")
	var grid: GridContainer = tab.find_children("*", "GridContainer", true, false)[1]
	_check(grid.columns == 4, "a Di2 offers a press, a hold and a double press")
	tab._on_devices_found([_click()])
	_check(grid.visible, "the Click found is chosen")
	_check(grid.columns == 3, "a Click offers a press and a hold")
	var captions: PackedStringArray = PackedStringArray()
	# Hidden cells stay children: a row is a caption and a choice per kind of press.
	for i: int in range(0, grid.get_child_count(), 1 + DevicesTab.PRESSES.size()):
		var caption: Label = grid.get_child(i) as Label
		if caption != null and caption.visible:
			captions.append(caption.text)
	_check(captions == PackedStringArray(["Plus (+)", "Minus (−)"]), "its buttons: %s" % captions)
	var shown: int = 0
	for choice: OptionButton in grid.find_children("*", "OptionButton", false, false):
		if choice.visible:
			shown += 1
	_check(shown == 2 * 2, "a choice per press of each of its buttons: %d" % shown)
	tab.free()


## Each row says how its device is doing (R76), from the core's reports by name: connected,
## reconnecting after a drop, or not found at start, when it is shown by name to look again.
func _device_states() -> void:
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	var choices: Array[Node] = tab.find_children("*", "OptionButton", true, false)
	var shifter: OptionButton = choices[4]
	var states: Array[Node] = tab.find_children("*", "Label", true, false)
	_check(tab.state_of("controller") == "", "nothing chosen, nothing said")
	# A search per row looks for that kind alone: nothing of the kind found says so, and the
	# nearest one found is chosen.
	var searches: int = 0
	for button: Button in tab.find_children("*", "Button", true, false):
		if button.text == "Search":
			searches += 1
	_check(searches == 5, "a search per row: %d" % searches)
	tab._searching = "trainer"
	tab._on_devices_found([_click()])
	_check(tab.state_of("trainer") == "missing", "no trainer found by the trainer's search")
	tab._searching = "controller"
	tab._on_devices_found([_click()])
	_check(
		shifter.get_item_text(shifter.selected).begins_with("Zwift Click"),
		"the Click found by the shifter's search is chosen"
	)
	tab._on_devices_found([_click()])
	# Without the core, the Click is expected and waited for all the same.
	tab._expected["controller"] = "Zwift Click"
	tab._on_device_connected("Zwift Click")
	_check(tab.state_of("controller") == "connected", "connected once the core says so")
	_check(_shown(states, "Connected"), "the row says connected")
	tab._on_device_connected("KICKR CORE")
	_check(
		tab.state_of("controller") == "connected", "another device's report is not the shifter's"
	)
	_check(tab.state_of("trainer") != "connected", "another device's report is not the trainer's")
	tab._on_device_disconnected("Zwift Click")
	_check(tab.state_of("controller") == "reconnecting", "a drop is told while the driver looks")
	_check(_shown(states, "Reconnecting…"), "the row says reconnecting")
	# A failed attempt says why, until the device connects (R80).
	tab._on_device_failed("Zwift Click", "connecting timed out")
	_check(tab.state_of("controller") == "failed", "a failed attempt is told")
	_check(_shown(states, "Failed: connecting timed out"), "the row says why")
	tab._on_device_connected("Zwift Click")
	_check(_shown(states, "Connected"), "the reason is gone once connected")
	_check(not _shown(states, "Failed: connecting timed out"), "and not shown any more")
	# A connected device's reading stands beside it, in the units of its kind (R80).
	var trainer_state: Label = tab._state_labels["trainer"]
	tab._expected["trainer"] = "KICKR CORE"
	tab._set_state("trainer", "connected")
	tab._show_reading("trainer", {"power": 200.0, "cadence": 90.0, "heart_rate": null})
	_check(
		trainer_state.text == "Connected  200 W · 90 rpm",
		"power and cadence: %s" % trainer_state.text
	)
	tab._set_state("heart_rate", "connected")
	tab._show_reading("heart_rate", {"power": null, "cadence": null, "heart_rate": 142.0})
	var strap_state: Label = tab._state_labels["heart_rate"]
	_check(strap_state.text == "Connected  142 bpm", "the heart rate: %s" % strap_state.text)
	tab._show_reading("trainer", {})
	_check(trainer_state.text == "Connected", "nothing beside a quiet trainer")
	tab._on_device_disconnected("KICKR CORE")
	tab._show_reading("trainer", {"power": 200.0, "cadence": 90.0, "heart_rate": null})
	_check(trainer_state.text == "Reconnecting…", "no reading beside a device that dropped")
	tab._on_remembered_missing(PackedStringArray(["Zwift Click"]))
	_check(tab.state_of("controller") == "missing", "not found at start")
	_check(_shown(states, "Not found"), "the row says not found")
	_check(
		shifter.get_item_text(shifter.selected) == "Zwift Click",
		"the device looked for is shown by name: %s" % shifter.get_item_text(shifter.selected)
	)
	tab.free()


## A scanned Zwift Click, remembered from last time.
static func _click() -> Dictionary:
	return {
		"index": 0,
		"name": "Zwift Click",
		"kind": "controller",
		"model": "zwift_click",
		"rssi": -50,
		"remembered": true,
	}


static func _shown(labels: Array[Node], text: String) -> bool:
	for label: Label in labels:
		if label.visible and label.text == text:
			return true
	return false


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("DEVICES SMOKE TEST FAILED: " + what)
		_failed = true
