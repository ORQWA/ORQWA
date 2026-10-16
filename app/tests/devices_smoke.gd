extends SceneTree
## Headless checks of the Devices tab: the shifter's buttons (#139, R75) and how each row
## tells what its device is doing (R76).
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
## in the dialog the pen beside the shifter's row opens (#271): a Zwift Click (R75) shows its
## two buttons with a press and a hold, no double press, a Click v2 (R77) its ten, + and −
## first, and a Di2 its four channels with a double press too.
func _shifter_buttons() -> void:
	var actions: PackedStringArray = TorqaApp.button_actions()
	for action: String in actions:
		_check(ButtonDialog.ACTION_LABELS.has(action), "a label for %s" % action)
	_check(actions.size() == ButtonDialog.ACTION_LABELS.size(), "no label for a gone action")
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	var dialog: ButtonDialog = tab.find_children("*", "ButtonDialog", true, false)[0]
	var grid: GridContainer = dialog.find_children("*", "GridContainer", true, false)[0]
	var choices: Array[Node] = grid.find_children("*", "OptionButton", false, false)
	# A press, a hold and a double press of ten buttons, the most a shifter has.
	_check(choices.size() == 10 * 3, "a choice per press of each button: %d" % choices.size())
	var press: OptionButton = choices[0]
	_check(press.item_count == 1 + actions.size(), "nothing or any action")
	# The tab itself stays one screen: one pen, on the shifter's row, disabled without one.
	var pens: Array[Button] = []
	for button: Button in tab.find_children("*", "Button", true, false):
		if button.text == "✎":
			pens.append(button)
	_check(pens.size() == 1, "one pen, beside the shifter: %d" % pens.size())
	var pen: Button = pens[0]
	_check(pen.disabled, "nothing to set up without a shifter")
	tab._on_devices_found([_click()])
	_check(not pen.disabled, "the Click found can be set up")
	pen.pressed.emit()
	_check(dialog.visible, "the pen opens the dialog")
	_check(grid.columns == 3, "a Click offers a press and a hold")
	var captions: PackedStringArray = _captions(grid)
	_check(captions == PackedStringArray(["Plus (+)", "Minus (−)"]), "its buttons: %s" % captions)
	_check(_shown_choices(grid) == 2 * 2, "a choice per press of each of its buttons")
	dialog.hide()
	tab._on_devices_found([_click_v2()])
	pen.pressed.emit()
	_check(grid.columns == 3, "a Click v2 offers a press and a hold")
	captions = _captions(grid)
	_check(captions.size() == 10, "a Click v2's ten buttons: %s" % captions)
	_check(
		captions[0] == "Plus (+)" and captions[1] == "Minus (−)",
		"+ and − first, so the default map shifts: %s" % captions
	)
	_check(
		captions[2] == "Y (right pod)" and captions[9] == "Right (left pod)",
		"then the right pod's letters and the left pod's arrows: %s" % captions
	)
	_check(_shown_choices(grid) == 10 * 2, "a choice per press of each of its buttons")
	dialog.hide()
	dialog.open(ButtonDialog.DI2, "Di2")
	_check(grid.columns == 4, "a Di2 offers a press, a hold and a double press")
	captions = _captions(grid)
	_check(captions.size() == 4 and captions[0] == "D-Fly channel 1", "its channels: %s" % captions)
	_check(_shown_choices(grid) == 4 * 3, "a choice per press of each channel")
	tab.free()


## The visible button captions of the assignments grid, in order. Hidden cells stay
## children: a row is a caption and a choice per kind of press.
static func _captions(grid: GridContainer) -> PackedStringArray:
	var captions: PackedStringArray = PackedStringArray()
	for i: int in range(0, grid.get_child_count(), 1 + ButtonDialog.PRESSES.size()):
		var caption: Label = grid.get_child(i) as Label
		if caption != null and caption.visible:
			captions.append(caption.text)
	return captions


static func _shown_choices(grid: GridContainer) -> int:
	var shown: int = 0
	for choice: OptionButton in grid.find_children("*", "OptionButton", false, false):
		if choice.visible:
			shown += 1
	return shown


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


## A scanned Zwift Click v2, its pods as one device, remembered from last time.
static func _click_v2() -> Dictionary:
	return {
		"index": 0,
		"name": "Zwift Click v2",
		"kind": "controller",
		"model": "zwift_click_v2",
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
