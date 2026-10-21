extends SceneTree
## Headless checks of the Devices tab: the shifter's buttons (#139, R75), how each row tells
## what its device is doing (R76), what stands beside it: the device's reading, or why
## connecting failed (R80), that the devices are each rider's own (R82), what a scan found
## (R107), and the phone remote (#228).
## Run: godot --headless --path app -s res://tests/devices_smoke.gd

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_shifter_buttons()
	_device_states()
	_missed_devices()
	_found_devices()
	await _rider_devices()
	await _phone_remote()
	if not _failed:
		print("DEVICES SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## Every action the core offers for a shifter's buttons can be chosen by its label (#139),
## in the dialog the pen beside the shifter's row opens (#271): a Zwift Click (R75) shows its
## two buttons with a press and a hold, no double press, a Click v2 (R77) its ten, + and −
## first, and a Di2 its four channels with a double press too.
func _shifter_buttons() -> void:
	var actions: PackedStringArray = OrqwaApp.button_actions()
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
	# The tab itself stays one screen: a pen on the shifter's row, disabled without one, and
	# one on the phone remote's (#228).
	var pens: Array[Button] = []
	for button: Button in tab.find_children("*", "Button", true, false):
		if button.tooltip_text == "What the buttons do":
			pens.append(button)
	_check(pens.size() == 2, "a pen beside the shifter and the phone: %d" % pens.size())
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
	dialog.hide()
	var phone_pen: Button = pens[1]
	_check(not phone_pen.disabled, "the phone's buttons are set up with the remote off too")
	phone_pen.pressed.emit()
	_check(dialog.visible, "the phone's pen opens the dialog")
	_check(grid.columns == 2, "a phone has presses only")
	captions = _captions(grid)
	_check(
		captions.size() == 10 and captions[0] == "Big button 1" and captions[9] == "Row 4, right",
		"the two big buttons, then the grid row by row: %s" % captions
	)
	_check(_shown_choices(grid) == 10, "a choice per button")
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


## A scan that misses a device used last (asleep, out of reach) keeps it chosen, by name:
## not finding it is not choosing none, which would forget it. A strap found nearby is only
## chosen if the rider used it last: it may be another rider's (R82).
func _missed_devices() -> void:
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	var choices: Array[Node] = tab.find_children("*", "OptionButton", true, false)
	var heart_rate: OptionButton = choices[3]
	tab._expected["heart_rate"] = "Polar H9"
	tab._set_state("heart_rate", "connecting")
	var others_strap: Dictionary = {
		"index": 0,
		"name": "HRM-Pro",
		"kind": "heart_rate",
		"model": null,
		"rssi": -40,
		"remembered": false,
	}
	tab._on_devices_found([others_strap])
	_check(
		heart_rate.get_item_text(heart_rate.selected) == "Polar H9",
		"the strap missed stays chosen: %s" % heart_rate.get_item_text(heart_rate.selected)
	)
	tab._on_remembered_missing(PackedStringArray(["Polar H9"]))
	_check(tab.state_of("heart_rate") == "missing", "and is told not found")
	tab._expected.clear()
	tab._set_state("heart_rate", "")
	tab._on_devices_found([others_strap])
	_check(heart_rate.selected == 0, "a strap nearby is not chosen for the rider")
	tab.free()


## Hovering the scan's result lists what it found (R107): each device under its row's
## caption with its signal, in the rows' order whatever order the scan heard them in.
func _found_devices() -> void:
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	var result: Label = tab._scan_label
	_check(result.mouse_filter != Control.MOUSE_FILTER_IGNORE, "the result sees the mouse")
	var strap: Dictionary = {
		"index": 1,
		"name": "Polar H9",
		"kind": "heart_rate",
		"model": null,
		"rssi": -62,
		"remembered": false,
	}
	var trainer: Dictionary = {
		"index": 2,
		"name": "KICKR CORE",
		"kind": "trainer",
		"model": null,
		"rssi": null,
		"remembered": false,
	}
	tab._on_devices_found([_click(), strap, trainer])
	_check(result.text == "Found 3 device(s)", "how many: %s" % result.text)
	_check(
		(
			result.tooltip_text
			== "Trainer: KICKR CORE\nHeart rate: Polar H9  (-62 dBm)\nShifter: Zwift Click  (-50 dBm)"
		),
		"what was found: %s" % result.tooltip_text
	)
	tab._on_devices_found([])
	_check(result.tooltip_text.is_empty(), "nothing to show when nothing was found")
	tab.free()


## Each rider has their own devices (R82): switching riders starts the rows afresh with the
## new rider's cadence source; saving the same rider leaves them as they are.
func _rider_devices() -> void:
	var orqwa: OrqwaApp = OrqwaApp.new()
	root.add_child(orqwa)
	await process_frame
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	tab.bind(orqwa)
	var cadence: OptionButton = tab.find_children("*", "OptionButton", true, false)[2]
	var first: Dictionary = orqwa.profile()
	var first_id: String = first["id"]
	var before: String = orqwa.cadence_source()
	var other: String = "power_meter" if before != "power_meter" else "trainer"
	tab._set_state("controller", "connected")
	orqwa.save_profile(first_id, first)
	_check(tab.state_of("controller") == "connected", "saving the rider keeps the rows")
	var ben: Dictionary = first.duplicate()
	ben["name"] = "Ben (devices smoke test)"
	var ben_id: String = orqwa.save_profile("", ben)
	_check(tab.state_of("controller") == "", "another rider's rows start afresh")
	orqwa.set_cadence_source(other)
	orqwa.select_profile(first_id)
	_check(
		cadence.get_item_text(cadence.selected) == _cadence_label(before),
		"the first rider's cadence: %s" % cadence.get_item_text(cadence.selected)
	)
	orqwa.select_profile(ben_id)
	_check(
		cadence.get_item_text(cadence.selected) == _cadence_label(other),
		"Ben's cadence: %s" % cadence.get_item_text(cadence.selected)
	)
	orqwa.delete_profile(ben_id)
	var active: String = orqwa.profile()["id"]
	_check(active == first_id, "back to the first rider")
	tab.free()
	orqwa.free()


## The phone remote (#228): its page has a text for everything it says, and its row turns it
## on, showing the code to scan at once, and off again.
func _phone_remote() -> void:
	for key: String in OrqwaApp.phone_remote_words():
		_check(not RemoteDialog.page_text(key).is_empty(), "a text for the page's %s" % key)
	var orqwa: OrqwaApp = OrqwaApp.new()
	root.add_child(orqwa)
	await process_frame
	var tab: DevicesTab = DevicesTab.new()
	root.add_child(tab)
	tab.bind(orqwa)
	var toggle: CheckButton = tab.find_children("*", "CheckButton", true, false)[0]
	var dialog: RemoteDialog = tab.find_children("*", "RemoteDialog", true, false)[0]
	_check(not toggle.button_pressed, "off until the rider turns it on")
	toggle.button_pressed = true
	var remote: Dictionary = orqwa.phone_remote()
	var on: bool = remote["on"]
	_check(on, "on")
	_check(dialog.visible, "its code shows at once")
	var address: String = remote["address"]
	var code: TextureRect = dialog.find_children("*", "TextureRect", true, false)[0]
	_check(address.is_empty() or code.texture != null, "a code to scan for %s" % address)
	dialog.hide()
	toggle.button_pressed = false
	var off: bool = not orqwa.phone_remote()["on"]
	_check(off, "off again, for the next time too")
	tab.free()
	orqwa.free()


static func _cadence_label(source: String) -> String:
	var labels: Dictionary[String, String] = {
		"automatic": "Automatic", "trainer": "Trainer", "power_meter": "Power meter"
	}
	return labels[source]


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
