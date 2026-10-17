class_name DevicesTab
extends VBoxContainer
## Trainer, power meter, cadence, heart-rate sensor and shifter — a Shimano Di2 or a Zwift
## Click (R5, R7, R41, R71, R72, R75, R76): scanning, choosing, what the shifter's buttons do
## (#139, in a dialog behind the pen beside its row), and the devices used last reconnecting
## at start. A device connects the moment it
## is chosen, and each row says how its device is doing: connecting, connected, reconnecting
## after a drop, not found, or failed. Beside a connected device stands what it reports, and
## beside a failed one why it failed (R80). All of it is the active rider's (R82): switching
## riders brings their own devices.

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
## How often a connected device's reading beside its row is refreshed (R80).
const READING_SECONDS: float = 0.5
## The device roles, by the core's name for each kind of device.
const TRAINER: String = "trainer"
const POWER_METER: String = "power_meter"
const CADENCE: String = "cadence"
const HEART_RATE: String = "heart_rate"
const CONTROLLER: String = "controller"
## The core's kind of device each role's reading is asked for (R80); a shifter reports none.
const READING_KINDS: Dictionary[String, String] = {
	TRAINER: "trainer",
	POWER_METER: "power_meter",
	CADENCE: "cadence_sensor",
	HEART_RATE: "heart_rate",
}
# i18n-begin
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
## The core's name for the fake trainer, as its connection is reported.
const FAKE_TRAINER_NAME: String = "Fake trainer"

var _torqa: TorqaApp
## The rider whose devices the rows show (R82), by id.
var _rider: String = ""
var _scan_button: Button = Button.new()
var _scan_label: Label = Label.new()
var _trainer: OptionButton = OptionButton.new()
var _heart_rate: OptionButton = OptionButton.new()
## Power pedals or a crank power meter, whose power counts over the trainer's (R71).
var _power_meter: OptionButton = OptionButton.new()
## Where the cadence comes from (R72): a device that measures it, or a cadence sensor.
var _cadence: OptionButton = OptionButton.new()
## A shifter; what each press of its buttons does is set in [member _button_dialog], opened
## by the pen beside its row (#271), so the tab stays one screen whatever the shifter.
var _shifter: OptionButton = OptionButton.new()
var _button_dialog: ButtonDialog = ButtonDialog.new()
var _edit_button: Button = Button.new()
## Which shifter each scanned one is, by its index: the core's "di2", "zwift_click" or
## "zwift_click_v2".
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
## Why a role's connection failed, shown beside Failed, and the reading shown beside
## Connected, as last rendered (R80).
var _reasons: Dictionary[String, String] = {}
var _readings: Dictionary[String, String] = {}
var _since_reading: float = 0.0
var _status: Label = Label.new()


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	_torqa.devices_found.connect(_on_devices_found)
	_torqa.device_connected.connect(_on_device_connected)
	_torqa.device_disconnected.connect(_on_device_disconnected)
	_torqa.device_failed.connect(_on_device_failed)
	_torqa.remembered_missing.connect(_on_remembered_missing)
	_torqa.failed.connect(func(_message: String) -> void: _scan_button.disabled = false)
	_torqa.profile_changed.connect(_on_profile_changed)
	_rider = _torqa.profile().get("id", "")
	_button_dialog.bind(torqa)
	_select_cadence_source()
	_reconnect_remembered()


## Makes sure the chosen trainer, power meter, cadence, heart-rate sensor and shifter are
## connected, as choosing them already did; false if the trainer cannot be connected.
func connect_selected() -> bool:
	var connected: bool = true
	for role: String in _options:
		connected = _choose(role) and connected
	return connected


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
	grid.columns = 5
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
		# A reason that wraps makes the row taller; the controls keep their height.
		option.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		grid.add_child(option)
		_options[role] = option
		# A pen opens what there is to set up on the device: a shifter's buttons (#271). The
		# other rows keep the column, so Search and the states line up.
		if role == CONTROLLER:
			_edit_button.icon = UiIcons.texture("pencil")
			_edit_button.icon_alignment = HORIZONTAL_ALIGNMENT_CENTER
			_edit_button.tooltip_text = tr("What the buttons do")
			_edit_button.custom_minimum_size = Vector2(44, 0)
			_edit_button.size_flags_vertical = Control.SIZE_SHRINK_CENTER
			_edit_button.disabled = true
			_edit_button.pressed.connect(_on_edit_pressed)
			grid.add_child(_edit_button)
		else:
			var gap: Control = Control.new()
			gap.custom_minimum_size = Vector2(44, 0)
			grid.add_child(gap)
		# Looks for this kind of device alone and connects the nearest one found (R76).
		var search: Button = Button.new()
		search.text = tr("Search")
		search.custom_minimum_size = Vector2(110, 0)
		search.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		search.pressed.connect(func() -> void: _on_search(role))
		grid.add_child(search)
		_search_buttons[role] = search
		var state: Label = Label.new()
		state.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		# Room for a reading, or a reason: the rest of the row's width, then it wraps rather
		# than widens the page (a German reason or a Bluetooth stack's message runs long).
		state.custom_minimum_size = Vector2(260, 0)
		state.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		state.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
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
	_scan_label.text = tr("Wake the devices first.")
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
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_status.add_theme_color_override("font_color", Color(1, 0.55, 0.45))
	add_child(_status)
	_reset_options()
	add_child(_button_dialog)


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		_trainer.set_item_text(0, tr("Fake trainer (200 W, for testing)"))
		_heart_rate.set_item_text(0, tr("None"))
		_power_meter.set_item_text(0, tr("None (the trainer measures)"))
		_label_cadence_sources()
		_shifter.set_item_text(0, tr("None (keyboard: ↑ / ↓)"))
		_edit_button.tooltip_text = tr("What the buttons do")
		for role: String in _states:
			_render_state(role)


## The devices used last reconnect in the background (R41), shown by name until found.
func _reconnect_remembered() -> void:
	if _torqa.reconnect_remembered():
		_set_searching(true)
		_scan_label.text = tr("Reconnecting your devices…")
	_show_remembered()


## Another rider rides (R82): the core let go of the devices that are not theirs. The rows
## start afresh with what is left and the rider's cadence source, and the rider's own devices
## reconnect as at start. Saving the same rider changes nothing here.
func _on_profile_changed() -> void:
	var rider: String = _torqa.profile().get("id", "")
	if rider == _rider:
		return
	_rider = rider
	_searching = ""
	_expected.clear()
	for role: String in _states:
		_set_state(role, "")
	_status.text = ""
	_scan_label.text = tr("Wake the devices first.")
	_reset_options()
	_reconnect_remembered()


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
	# Prefer the devices used last, else a real trainer over the fake one. A power meter or a
	# strap only when used last: pedals nearby may be on another bike, a strap on another
	# rider (R82).
	if trainers > 0:
		_trainer.select(1)
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
	# Chosen is connected (R76): the devices used last, or the ones preferred, right away. A
	# device a row has or waits for that the scan did not list (asleep, out of reach) stays
	# its choice by name: missing it is not the rider choosing none, which would forget it.
	for role: String in _options:
		var kept: String = _expected.get(role, "")
		if (
			(role == searched and found)
			or kept.is_empty()
			or kept == FAKE_TRAINER_NAME
			or _lists(role, kept)
		):
			_choose(role)
		else:
			_show_placeholder(role, kept)
	_show_buttons()
	if not searched.is_empty() and not found:
		_set_state(searched, "missing")


## Whether the scan listed `device_name` among the role's choices.
func _lists(role: String, device_name: String) -> bool:
	var option: OptionButton = _options[role]
	for i: int in range(option.item_count):
		var index: int = option.get_item_metadata(i)
		if _names.get(index, "") == device_name:
			return true
	return false


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


## An attempt to connect failed (R80): the row waiting for the device says why, while the
## driver tries again; the next report of a connection clears it.
func _on_device_failed(device_name: String, reason: String) -> void:
	for role: String in _expected:
		if _expected[role] == device_name:
			_reasons[role] = reason
			_set_state(role, "failed")


## Twice a second, a connected device's reading is shown beside it (R80).
func _process(delta: float) -> void:
	if _torqa == null:
		return
	_since_reading += delta
	if _since_reading < READING_SECONDS:
		return
	_since_reading = 0.0
	for role: String in READING_KINDS:
		if _states[role] == "connected":
			_show_reading(role, _torqa.device_reading(READING_KINDS[role]))


## Shows what the role's device reports: the trainer's and the power meter's power and
## cadence, a cadence sensor's cadence, a strap's heart rate; nothing while it is quiet.
func _show_reading(role: String, reading: Dictionary) -> void:
	var parts: PackedStringArray = PackedStringArray()
	if role in [TRAINER, POWER_METER] and reading.get("power") != null:
		var power: float = reading["power"]
		parts.append(tr("%d W") % roundi(power))
	if role in [TRAINER, POWER_METER, CADENCE] and reading.get("cadence") != null:
		var cadence: float = reading["cadence"]
		parts.append(tr("%d rpm") % roundi(cadence))
	if role == HEART_RATE and reading.get("heart_rate") != null:
		var heart_rate: float = reading["heart_rate"]
		parts.append(tr("%d bpm") % roundi(heart_rate))
	var text: String = " · ".join(parts)
	if _readings.get(role, "") == text:
		return
	if text.is_empty():
		_readings.erase(role)
	else:
		_readings[role] = text
	_render_state(role)


## The reconnect at start missed some devices (R41): their rows say so, by name, and
## choosing one of them looks again.
func _on_remembered_missing(names: PackedStringArray) -> void:
	for role: String in _expected:
		var expected: String = _expected[role]
		if names.has(expected):
			_set_state(role, "missing")
			_show_placeholder(role, expected)
	_status.text = tr("Not found: %s. Wake the devices, then scan again.") % ", ".join(names)


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
		# The fake trainer, kept from the rider before, has its own choice.
		if role == TRAINER and device_name == FAKE_TRAINER_NAME:
			_trainer.select(0)
		else:
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


## Connects the role's choice at once (R76); false only for a trainer that cannot be.
func _choose(role: String) -> bool:
	match role:
		TRAINER:
			return _choose_trainer()
		POWER_METER:
			_choose_power_meter()
		CADENCE:
			_choose_cadence()
		HEART_RATE:
			_choose_heart_rate()
		CONTROLLER:
			_choose_shifter()
	return true


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
	if state != "failed":
		_reasons.erase(role)
	if state != "connected":
		_readings.erase(role)
	_render_state(role)


## Shows the role's state in the interface language and its colour: green connected, amber
## on its way, red not found or failed; beside it the device's reading or why it failed.
func _render_state(role: String) -> void:
	var label: Label = _state_labels[role]
	var state: String = _states[role]
	var text: String = tr(STATE_LABELS[state]) if STATE_LABELS.has(state) else ""
	if state == "failed" and _reasons.has(role):
		# The core's reasons are English; the known ones are translated, a stack's own passes.
		text += ": " + tr(_reasons[role])
	elif state == "connected" and _readings.has(role):
		text += "  " + _readings[role]
	label.text = text
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


## The buttons matter only with a shifter chosen: the pen opens the chosen shifter's.
func _show_buttons() -> void:
	# Built before the options are filled, so there may be nothing chosen yet.
	_edit_button.disabled = _chosen(CONTROLLER) == NO_SHIFTER


## Opens what the chosen shifter's buttons do (#139), captioned for the shifter it is.
func _on_edit_pressed() -> void:
	var chosen: int = _chosen(CONTROLLER)
	var model: String = _shifter_models.get(chosen, ButtonDialog.DI2)
	_button_dialog.open(model, _shifter.get_item_text(_shifter.selected))


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
