class_name DevicesTab
extends VBoxContainer
## Trainer, power meter, cadence, heart-rate sensor and shifter — a Shimano Di2 or a Zwift
## Click (R5, R7, R41, R71, R72, R75, R76): scanning, choosing, what the shifter's buttons do
## (#139), and the devices used last reconnecting at start. A device connects the moment it
## is chosen, and each row says how its device is doing: connecting, connected, reconnecting
## after a drop, not found, or failed.

const FAKE_TRAINER: int = -1
const NO_HEART_RATE: int = -1
const NO_POWER_METER: int = -1
## The cadence choices that are no sensor, by the core's name; sensors have their scan index.
const CADENCE_SOURCES: Dictionary[int, String] = {
	-1: "automatic",
	-2: "trainer",
	-3: "power_meter",
}
const NO_SHIFTER: int = -1
## A device used last that the scan has not shown yet (reconnecting at start, or not found):
## shown by name so the rider knows what Torqa is looking for; choosing it looks again.
const REMEMBERED: int = -9
const SCAN_SECONDS: float = 5.0
## The device roles, by the core's name for each kind of device.
const TRAINER: String = "trainer"
const POWER_METER: String = "power_meter"
const CADENCE: String = "cadence"
const HEART_RATE: String = "heart_rate"
const CONTROLLER: String = "controller"
## What a shifter's buttons can be given (#139), by the core's name for each action.
# i18n-begin
const ACTION_LABELS: Dictionary[String, String] = {
	"shift_up": "Shift up",
	"shift_down": "Shift down",
	"shift_up_two": "Shift up two gears",
	"shift_down_two": "Shift down two gears",
	"next_camera": "Next camera",
	"overlay": "Overlay on / off",
	"play_pause": "Music: play / pause",
	"next_track": "Music: next track",
	"previous_track": "Music: previous track",
	"pause": "Pause / resume",
	"difficulty_up": "Trainer difficulty +10 %",
	"difficulty_down": "Trainer difficulty −10 %",
	"intensity_up": "Workout intensity +5 %",
	"intensity_down": "Workout intensity −5 %",
	"skip_step": "Workout: skip step",
}
## The kinds of press, in the core's order.
const PRESSES: Array[String] = ["Press", "Hold", "Double press"]
## A Zwift Click's buttons (R75), in the core's numbering (1 and 2).
const CLICK_BUTTONS: Array[String] = ["Plus (+)", "Minus (−)"]
## How a row's device is doing (R76), by state.
const STATE_LABELS: Dictionary[String, String] = {
	"searching": "Searching…",
	"connecting": "Connecting…",
	"connected": "Connected",
	"reconnecting": "Reconnecting…",
	"missing": "Not found",
	"failed": "Failed",
}
# i18n-end
const CHANNELS: int = 4
## A Zwift Click tells presses and holds apart, not double presses.
const CLICK_PRESSES: int = 2
## The core's name for a Di2, whose buttons are its four D-Fly channels.
const DI2: String = "di2"
const ZWIFT_CLICK: String = "zwift_click"
## The core's name for the fake trainer, as its connection is reported.
const FAKE_TRAINER_NAME: String = "Fake trainer"

var _torqa: TorqaApp
var _scan_button: Button = Button.new()
var _scan_label: Label = Label.new()
var _trainer: OptionButton = OptionButton.new()
var _heart_rate: OptionButton = OptionButton.new()
## Power pedals or a crank power meter, whose power counts over the trainer's (R71).
var _power_meter: OptionButton = OptionButton.new()
## Where the cadence comes from (R72): a device that measures it, or a cadence sensor.
var _cadence: OptionButton = OptionButton.new()
## A shifter, and what each press of its buttons does: a row per button (a Di2's D-Fly
## channels, a Zwift Click's + and −), a column per kind of press the shifter tells apart.
var _shifter: OptionButton = OptionButton.new()
## Which shifter each scanned one is, by its index: the core's "di2" or "zwift_click".
var _shifter_models: Dictionary[int, String] = {}
## The scanned devices' names by index, as the core reports their connections.
var _names: Dictionary[int, String] = {}
## Each role's choice, and the label beside it telling how the device is doing (R76).
var _options: Dictionary[String, OptionButton] = {}
var _state_labels: Dictionary[String, Label] = {}
var _states: Dictionary[String, String] = {}
## A search button per role, looking for that kind of device alone (R76).
var _search_buttons: Dictionary[String, Button] = {}
## The role whose search is under way, "" for none or a scan for all.
var _searching: String = ""
## The name of the device each role waits for or has, to match the core's reports.
var _expected: Dictionary[String, String] = {}
var _buttons: GridContainer = GridContainer.new()
var _press_headings: Array[Label] = []
var _channel_captions: Array[Label] = []
var _assignments: Array[OptionButton] = []
var _status: Label = Label.new()
## How detailed the 3D world is drawn on this computer (R43).
var _quality: OptionButton = OptionButton.new()
var _quality_note: Label = Label.new()
## Full screen or a window (#218), as F11 switches it too.
var _fullscreen: CheckBox = CheckBox.new()


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	_torqa.devices_found.connect(_on_devices_found)
	_torqa.device_connected.connect(_on_device_connected)
	_torqa.device_disconnected.connect(_on_device_disconnected)
	_torqa.remembered_missing.connect(_on_remembered_missing)
	_torqa.failed.connect(func(_message: String) -> void: _scan_button.disabled = false)
	_select_cadence_source()
	var current: String = _torqa.graphics_quality()
	for i: int in range(_quality.item_count):
		if _quality.get_item_metadata(i) == current:
			_quality.select(i)
	_show_quality_note()
	var assigned: Array = _torqa.button_map()
	for channel: int in range(assigned.size()):
		var presses: PackedStringArray = assigned[channel]
		for press: int in range(presses.size()):
			var option: OptionButton = _assignments[channel * PRESSES.size() + press]
			for i: int in range(option.item_count):
				if option.get_item_metadata(i) == presses[press]:
					option.select(i)
	# The trainer and strap used last reconnect in the background (R41).
	if _torqa.reconnect_remembered():
		_set_searching(true)
		_scan_label.text = tr("Reconnecting your devices…")
		_show_remembered()


## Makes sure the chosen trainer, power meter, cadence, heart-rate sensor and shifter are
## connected, as choosing them already did; false if the trainer cannot be connected.
func connect_selected() -> bool:
	var trainer: bool = _choose_trainer()
	_choose_power_meter()
	_choose_cadence()
	_choose_heart_rate()
	_choose_shifter()
	return trainer


## How the role's device is doing: "" (none chosen), "connecting", "connected",
## "reconnecting", "missing" or "failed".
func state_of(role: String) -> String:
	return _states.get(role, "")


func _init() -> void:
	add_theme_constant_override("separation", 16)
	var heading: Label = Label.new()
	heading.text = tr("Devices")
	heading.add_theme_font_size_override("font_size", 22)
	add_child(heading)
	var note: Label = Label.new()
	note.text = tr(
		"The devices used last connect by themselves. Search beside a row to find one of that kind."
	)
	note.add_theme_color_override("font_color", UiTheme.MUTED)
	add_child(note)
	var grid: GridContainer = GridContainer.new()
	grid.columns = 4
	grid.add_theme_constant_override("h_separation", 24)
	grid.add_theme_constant_override("v_separation", 10)
	# i18n-begin
	for row: Array in [
		["Trainer", TRAINER, _trainer],
		["Power meter", POWER_METER, _power_meter],
		["Cadence", CADENCE, _cadence],
		["Heart rate", HEART_RATE, _heart_rate],
		["Shifter", CONTROLLER, _shifter],
	]:
		# i18n-end
		var caption: Label = Label.new()
		caption.text = row[0]
		caption.custom_minimum_size = Vector2(RideOptions.CAPTION_WIDTH, 0)
		grid.add_child(caption)
		var role: String = row[1]
		var option: OptionButton = row[2]
		# Device names are never translated; the fixed entries are, in code.
		option.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		option.custom_minimum_size = Vector2(420, 0)
		grid.add_child(option)
		_options[role] = option
		# Looks for this kind of device alone and connects the nearest one found (R76).
		var search: Button = Button.new()
		search.text = tr("Search")
		search.custom_minimum_size = Vector2(110, 0)
		search.pressed.connect(func() -> void: _on_search(role))
		grid.add_child(search)
		_search_buttons[role] = search
		var state: Label = Label.new()
		state.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		state.custom_minimum_size = Vector2(160, 0)
		state.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		grid.add_child(state)
		_state_labels[role] = state
		_states[role] = ""
	add_child(grid)
	# When neither the reconnect at start nor a row's search found a device: everything at once.
	var scan_row: HBoxContainer = HBoxContainer.new()
	scan_row.add_theme_constant_override("separation", 16)
	_scan_button.text = tr("Scan for all devices")
	_scan_button.custom_minimum_size = Vector2(220, 44)
	_scan_button.pressed.connect(_on_scan_pressed)
	scan_row.add_child(_scan_button)
	_scan_label.text = tr("Wake the device first: pedal, or put the strap on.")
	_scan_label.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	scan_row.add_child(_scan_label)
	add_child(scan_row)
	_trainer.item_selected.connect(func(_index: int) -> void: _choose_trainer())
	_power_meter.tooltip_text = tr(
		"Power pedals or crank: its power and cadence count instead of the trainer's"
	)
	_power_meter.item_selected.connect(func(_index: int) -> void: _choose_power_meter())
	_cadence.tooltip_text = tr(
		"Automatic: a cadence sensor, else the power meter, else the trainer"
	)
	_cadence.item_selected.connect(func(_index: int) -> void: _choose_cadence())
	_heart_rate.item_selected.connect(func(_index: int) -> void: _choose_heart_rate())
	_shifter.tooltip_text = tr(
		"Zwift Click, or Di2 with buttons on D-Fly channels (E-TUBE); choose what each press does"
	)
	_shifter.item_selected.connect(
		func(_index: int) -> void:
			_choose_shifter()
			_show_buttons()
	)
	_build_buttons()
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.add_theme_color_override("font_color", Color(1, 0.55, 0.45))
	add_child(_status)
	_reset_options()
	_build_graphics()


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		_trainer.set_item_text(0, tr("Fake trainer (200 W, for testing)"))
		_heart_rate.set_item_text(0, tr("None"))
		_power_meter.set_item_text(0, tr("None (the trainer measures)"))
		_label_cadence_sources()
		_shifter.set_item_text(0, tr("None (keyboard: ↑ / ↓)"))
		_label_buttons()
		for role: String in _states:
			_render_state(role)


func _on_scan_pressed() -> void:
	_set_searching(true)
	_scan_label.text = tr("Scanning…")
	_status.text = ""
	_torqa.scan(SCAN_SECONDS)


## Looks for devices of the row's kind (R76): the nearest one found is chosen and connected.
func _on_search(role: String) -> void:
	if _torqa == null:
		return
	_searching = role
	_set_state(role, "searching")
	_set_searching(true)
	_status.text = ""
	_torqa.scan(SCAN_SECONDS)


## One search at a time: the buttons wait for the scan to end.
func _set_searching(on: bool) -> void:
	_scan_button.disabled = on
	for button: Button in _search_buttons.values():
		button.disabled = on


func _on_devices_found(devices: Array) -> void:
	_set_searching(false)
	_reset_options()
	var trainers: int = 0
	var remembered: Array[int] = []
	for device: Dictionary in devices:
		var index: int = device["index"]
		if device["remembered"]:
			remembered.append(index)
		var label: String = device["name"]
		_names[index] = device["name"]
		if device["rssi"] != null:
			label += "  (%d dBm)" % device["rssi"]
		var kind: String = device["kind"]
		var option: OptionButton = _heart_rate
		if kind == "trainer":
			option = _trainer
		elif kind == "power_meter":
			option = _power_meter
		elif kind == "cadence_sensor":
			option = _cadence
		elif kind == "controller":
			option = _shifter
			_shifter_models[index] = device["model"]
		option.add_item(label)
		option.set_item_metadata(option.item_count - 1, index)
		if device["kind"] == "trainer":
			trainers += 1
	_scan_label.text = tr("Found %d device(s)") % devices.size()
	# Prefer the devices used last, else a real trainer over the fake one. A power meter only
	# when chosen once: pedals nearby may be on another bike.
	if trainers > 0:
		_trainer.select(1)
	if _heart_rate.item_count > 1:
		_heart_rate.select(1)
	for option: OptionButton in [_trainer, _power_meter, _cadence, _heart_rate, _shifter]:
		for i: int in range(option.item_count):
			var index: int = option.get_item_metadata(i)
			if remembered.has(index):
				option.select(i)
	# A row's search takes the nearest device of its kind, unless one used last was found.
	var searched: String = _searching
	_searching = ""
	var found: bool = true
	if not searched.is_empty():
		var option: OptionButton = _options[searched]
		var first: int = _first_scanned(option)
		found = first >= 0
		var chosen: int = option.get_item_metadata(option.selected)
		if found and not remembered.has(chosen):
			option.select(first)
	_show_buttons()
	# Chosen is connected (R76): the devices used last, or the ones preferred, right away.
	connect_selected()
	if not searched.is_empty() and not found:
		_set_state(searched, "missing")


## The first scanned device in a choice (the nearest: the scan lists them by signal), or −1.
static func _first_scanned(option: OptionButton) -> int:
	for i: int in range(option.item_count):
		var index: int = option.get_item_metadata(i)
		if index >= 0:
			return i
	return -1


## The core reported a device connected: the row waiting for it shows so. A row that shows
## the device without waiting for it (connected from elsewhere, e.g. the ride settings) too.
func _on_device_connected(device_name: String) -> void:
	var told: bool = false
	for role: String in _expected:
		if _expected[role] == device_name:
			_set_state(role, "connected")
			told = true
	if told:
		return
	for role: String in _options:
		if _selected_name(role) == device_name:
			_expected[role] = device_name
			_set_state(role, "connected")


## The name of the device the role's choice shows, "" for none or a fixed choice.
func _selected_name(role: String) -> String:
	var chosen: int = _chosen(role)
	if role == TRAINER and chosen == FAKE_TRAINER:
		return FAKE_TRAINER_NAME
	if chosen == REMEMBERED:
		var option: OptionButton = _options[role]
		return option.get_item_text(option.selected)
	return _names.get(chosen, "")


## A device dropped; its driver keeps looking for it.
func _on_device_disconnected(device_name: String) -> void:
	for role: String in _expected:
		if _expected[role] == device_name:
			_set_state(role, "reconnecting")


## The reconnect at start missed some devices (R41): their rows say so, by name, and
## choosing one of them looks again.
func _on_remembered_missing(names: PackedStringArray) -> void:
	for role: String in _expected:
		var expected: String = _expected[role]
		if names.has(expected):
			_set_state(role, "missing")
			_show_placeholder(role, expected)
	_status.text = (
		tr("%s not found — wake it by pedalling (or put on the strap), then scan.")
		% ", ".join(names)
	)


## While the devices used last reconnect at start, their rows show their names and that
## Torqa is looking for them, before any scan has filled the choices.
func _show_remembered() -> void:
	var roles: Dictionary[String, String] = {
		"trainer": TRAINER,
		"power_meter": POWER_METER,
		"cadence_sensor": CADENCE,
		"heart_rate": HEART_RATE,
		"controller": CONTROLLER,
	}
	for device: Dictionary in _torqa.devices():
		var role: String = roles.get(device["kind"], "")
		if role.is_empty():
			continue
		var device_name: String = device["name"]
		_expected[role] = device_name
		_set_state(role, "connected" if device["state"] == "connected" else "connecting")
		_show_placeholder(role, device_name)


## Shows `device_name` as the role's choice although the scan has not listed it.
func _show_placeholder(role: String, device_name: String) -> void:
	var option: OptionButton = _options[role]
	for i: int in range(option.item_count):
		if option.get_item_metadata(i) == REMEMBERED:
			option.remove_item(i)
			break
	option.add_item(device_name)
	option.set_item_metadata(option.item_count - 1, REMEMBERED)
	option.select(option.item_count - 1)


## What the role's choice is: a scan index, a fixed choice, or [constant REMEMBERED].
func _chosen(role: String) -> int:
	var selected: Variant = _options[role].get_selected_metadata()
	return -1 if selected == null else selected


## Connects the chosen trainer at once (R76): the fake one, or a scanned one.
func _choose_trainer() -> bool:
	if _torqa == null:
		return false
	var chosen: int = _chosen(TRAINER)
	if chosen == REMEMBERED:
		return _look_again(TRAINER)
	var expected: String = FAKE_TRAINER_NAME if chosen == FAKE_TRAINER else _names.get(chosen, "")
	if _keeps(TRAINER, expected):
		return true
	_expected[TRAINER] = expected
	var connecting: bool = (
		_torqa.connect_fake_trainer(200.0, 90.0)
		if chosen == FAKE_TRAINER
		else _torqa.connect_trainer(chosen)
	)
	_set_state(TRAINER, "connecting" if connecting else "failed")
	return connecting


func _choose_power_meter() -> void:
	_choose_device(POWER_METER, NO_POWER_METER, "connect_power_meter", "disconnect_power_meter")


func _choose_heart_rate() -> void:
	_choose_device(HEART_RATE, NO_HEART_RATE, "connect_heart_rate", "disconnect_heart_rate")


func _choose_shifter() -> void:
	_choose_device(CONTROLLER, NO_SHIFTER, "connect_controller", "disconnect_controller")


## A device's own cadence lets go of the cadence sensor; a sensor connects at once.
func _choose_cadence() -> void:
	if _torqa == null:
		return
	var chosen: int = _chosen(CADENCE)
	if chosen == REMEMBERED:
		_look_again(CADENCE)
	elif CADENCE_SOURCES.has(chosen):
		_torqa.set_cadence_source(CADENCE_SOURCES[chosen])
		_expected.erase(CADENCE)
		_set_state(CADENCE, "")
	else:
		_connect_scanned(CADENCE, chosen, "connect_cadence_sensor")


## Connects the scanned device chosen for `role` at once, or lets the role's device go when
## `none` is chosen (R76); `connect` and `disconnect` name the core's commands.
func _choose_device(role: String, none: int, connect: String, disconnect: String) -> void:
	if _torqa == null:
		return
	var chosen: int = _chosen(role)
	if chosen == REMEMBERED:
		_look_again(role)
	elif chosen == none:
		if _expected.has(role):
			_torqa.call(disconnect)
		_expected.erase(role)
		_set_state(role, "")
	else:
		_connect_scanned(role, chosen, connect)


func _connect_scanned(role: String, index: int, connect: String) -> void:
	var expected: String = _names.get(index, "")
	if _keeps(role, expected):
		return
	_expected[role] = expected
	var connecting: bool = _torqa.call(connect, index)
	_set_state(role, "connecting" if connecting else "failed")


## Whether the role already has `expected` connected or on its way: nothing to do, and the
## state stays as it is (a second connect of the same device reports nothing new).
func _keeps(role: String, expected: String) -> bool:
	return (
		_expected.get(role, "") == expected
		and _states[role] in ["connected", "connecting", "reconnecting"]
	)


## The rider chose a device used last that the scan did not show: look for it again (R69).
func _look_again(role: String) -> bool:
	_set_state(role, "connecting")
	_set_searching(true)
	_scan_label.text = tr("Reconnecting your devices…")
	return _torqa.reconnect_devices()


func _set_state(role: String, state: String) -> void:
	_states[role] = state
	_render_state(role)


## Shows the role's state in the interface language and its colour: green connected, amber
## on its way, red not found or failed.
func _render_state(role: String) -> void:
	var label: Label = _state_labels[role]
	var state: String = _states[role]
	label.text = tr(STATE_LABELS[state]) if STATE_LABELS.has(state) else ""
	var color: Color = UiTheme.MUTED
	if state == "connected":
		color = UiTheme.SUCCESS
	elif state in ["searching", "connecting", "reconnecting"]:
		color = UiTheme.WARNING
	elif state in ["missing", "failed"]:
		color = UiTheme.DANGER
	label.add_theme_color_override("font_color", color)


func _label_cadence_sources() -> void:
	# i18n-begin
	var labels: Array[String] = ["Automatic", "Trainer", "Power meter"]
	# i18n-end
	for i: int in range(labels.size()):
		_cadence.set_item_text(i, tr(labels[i]))


## Shows the cadence source chosen before; a remembered sensor is selected once found.
func _select_cadence_source() -> void:
	if _torqa == null:
		return
	var source: String = _torqa.cadence_source()
	for i: int in range(CADENCE_SOURCES.size()):
		if CADENCE_SOURCES[_cadence.get_item_metadata(i)] == source:
			_cadence.select(i)


## The buttons matter only with a shifter chosen, and are the chosen shifter's.
func _show_buttons() -> void:
	# Built before the options are filled, so there may be nothing chosen yet.
	var chosen: int = _chosen(CONTROLLER)
	_buttons.visible = chosen != NO_SHIFTER
	var model: String = _shifter_models.get(chosen, DI2)
	_layout_buttons(model)


## Shows the rows and columns of the chosen shifter's buttons and presses, captioned for it.
func _layout_buttons(model: String) -> void:
	var buttons: int = CLICK_BUTTONS.size() if model == ZWIFT_CLICK else CHANNELS
	var presses: int = CLICK_PRESSES if model == ZWIFT_CLICK else PRESSES.size()
	_buttons.columns = 1 + presses
	for press: int in range(PRESSES.size()):
		_press_headings[press].visible = press < presses
	for channel: int in range(CHANNELS):
		var caption: Label = _channel_captions[channel]
		caption.visible = channel < buttons
		if model == ZWIFT_CLICK and channel < buttons:
			caption.text = tr(CLICK_BUTTONS[channel])
		else:
			caption.text = tr("D-Fly channel %d") % (channel + 1)
		for press: int in range(PRESSES.size()):
			var option: OptionButton = _assignments[channel * PRESSES.size() + press]
			option.visible = channel < buttons and press < presses


func _build_buttons() -> void:
	_buttons.columns = 1 + PRESSES.size()
	_buttons.add_theme_constant_override("h_separation", 16)
	_buttons.add_theme_constant_override("v_separation", 10)
	var corner: Control = Control.new()
	corner.custom_minimum_size = Vector2(RideOptions.CAPTION_WIDTH, 0)
	_buttons.add_child(corner)
	for heading: String in PRESSES:
		var label: Label = Label.new()
		label.text = heading
		_press_headings.append(label)
		_buttons.add_child(label)
	for channel: int in range(1, CHANNELS + 1):
		var caption: Label = Label.new()
		caption.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		_channel_captions.append(caption)
		_buttons.add_child(caption)
		for press: int in range(PRESSES.size()):
			var option: OptionButton = OptionButton.new()
			option.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
			option.custom_minimum_size = Vector2(240, 0)
			option.add_item("")
			option.set_item_metadata(0, "")
			for action: String in TorqaApp.button_actions():
				option.add_item("")
				option.set_item_metadata(option.item_count - 1, action)
			option.item_selected.connect(
				func(index: int) -> void:
					var action: String = option.get_item_metadata(index)
					_torqa.assign_button(channel, press, action)
			)
			_assignments.append(option)
			_buttons.add_child(option)
	add_child(_buttons)
	_label_buttons()


## Names the buttons and actions in the interface language.
func _label_buttons() -> void:
	_show_buttons()
	for option: OptionButton in _assignments:
		for i: int in range(option.item_count):
			var action: String = option.get_item_metadata(i)
			var label: String = ACTION_LABELS.get(action, action)
			option.set_item_text(i, tr(label) if action else tr("Nothing"))


func _build_graphics() -> void:
	var heading: Label = Label.new()
	heading.text = tr("Graphics")
	heading.add_theme_font_size_override("font_size", 22)
	add_child(heading)
	var row: HBoxContainer = HBoxContainer.new()
	row.add_theme_constant_override("separation", 16)
	var caption: Label = Label.new()
	caption.text = tr("Quality")
	caption.custom_minimum_size = Vector2(180, 0)
	row.add_child(caption)
	# i18n-begin
	for preset: Array in [
		["low", "Low"], ["medium", "Medium"], ["high", "High"], ["ultra", "Ultra"]
	]:
		# i18n-end
		var key: String = preset[0]
		var label: String = preset[1]
		_quality.add_item(tr(label))
		_quality.set_item_metadata(_quality.item_count - 1, key)
	_quality.custom_minimum_size = Vector2(240, 0)
	_quality.item_selected.connect(_on_quality_selected)
	row.add_child(_quality)
	add_child(row)
	_quality_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_quality_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_quality_note)
	var screen_row: HBoxContainer = HBoxContainer.new()
	screen_row.add_theme_constant_override("separation", 16)
	var screen_caption: Label = Label.new()
	screen_caption.text = tr("Screen")
	screen_caption.custom_minimum_size = Vector2(180, 0)
	screen_row.add_child(screen_caption)
	_fullscreen.text = tr("Full screen (F11)")
	_fullscreen.toggled.connect(func(on: bool) -> void: FullScreen.set_on(get_window(), _torqa, on))
	screen_row.add_child(_fullscreen)
	add_child(screen_row)


func _ready() -> void:
	# F11 or the system may switch it while this page is open.
	get_window().size_changed.connect(
		func() -> void: _fullscreen.set_pressed_no_signal(FullScreen.is_on(get_window()))
	)
	_fullscreen.set_pressed_no_signal(FullScreen.is_on(get_window()))


func _on_quality_selected(index: int) -> void:
	var key: String = _quality.get_item_metadata(index)
	_torqa.set_graphics_quality(key)
	_show_quality_note()


func _show_quality_note() -> void:
	# i18n-begin
	var notes: Dictionary[String, String] = {
		"low": "For weaker computers: shorter view, simpler shadows and sky.",
		"medium": "A balance of visual quality and performance.",
		"high": "For stronger GPUs: softer shadows, bounced light, light fog, more distance.",
		"ultra": "For powerful GPUs: all effects, including global illumination.",
	}
	# i18n-end
	var key: String = _torqa.graphics_quality() if _torqa != null else "medium"
	var note: String = notes.get(key, notes["medium"])
	_quality_note.text = tr(note) + " " + tr("Applies from the next ride.")


func _reset_options() -> void:
	_trainer.clear()
	_trainer.add_item(tr("Fake trainer (200 W, for testing)"))
	_trainer.set_item_metadata(0, FAKE_TRAINER)
	_heart_rate.clear()
	_heart_rate.add_item(tr("None"))
	_heart_rate.set_item_metadata(0, NO_HEART_RATE)
	_power_meter.clear()
	_power_meter.add_item(tr("None (the trainer measures)"))
	_power_meter.set_item_metadata(0, NO_POWER_METER)
	_cadence.clear()
	for key: int in CADENCE_SOURCES:
		_cadence.add_item("")
		_cadence.set_item_metadata(_cadence.item_count - 1, key)
	_label_cadence_sources()
	_select_cadence_source()
	_shifter.clear()
	_shifter.add_item(tr("None (keyboard: ↑ / ↓)"))
	_shifter.set_item_metadata(0, NO_SHIFTER)
	_shifter_models.clear()
	_names.clear()
	_show_buttons()
