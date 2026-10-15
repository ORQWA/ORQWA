class_name DevicesTab
extends VBoxContainer
## Trainer, power meter, cadence, heart-rate sensor and Di2 shifter (R5, R7, R41, R71, R72):
## scanning, choosing, what the shifter's buttons do (#139), and the devices used last
## reconnecting at start.

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
const SCAN_SECONDS: float = 5.0
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
# i18n-end
const CHANNELS: int = 4

var _torqa: TorqaApp
var _scan_button: Button = Button.new()
var _scan_label: Label = Label.new()
var _trainer: OptionButton = OptionButton.new()
var _heart_rate: OptionButton = OptionButton.new()
## Power pedals or a crank power meter, whose power counts over the trainer's (R71).
var _power_meter: OptionButton = OptionButton.new()
## Where the cadence comes from (R72): a device that measures it, or a cadence sensor.
var _cadence: OptionButton = OptionButton.new()
## A Shimano Di2 shifter, and what each press of its D-Fly channels does: a row per channel,
## a column per kind of press.
var _shifter: OptionButton = OptionButton.new()
var _buttons: GridContainer = GridContainer.new()
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
	_torqa.device_connected.connect(
		func(device_name: String) -> void: _scan_label.text = tr("%s connected") % device_name
	)
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
		_scan_button.disabled = true
		_scan_label.text = tr("Reconnecting your devices…")


## Connects the chosen trainer, power meter, heart-rate sensor and shifter (devices connected
## already stay connected); false if the trainer cannot be connected.
func connect_selected() -> bool:
	var trainer: int = _trainer.get_selected_metadata()
	var connected: bool = (
		_torqa.connect_fake_trainer(200.0, 90.0)
		if trainer == FAKE_TRAINER
		else _torqa.connect_trainer(trainer)
	)
	if not connected:
		return false
	var power_meter: int = _power_meter.get_selected_metadata()
	if power_meter != NO_POWER_METER:
		_torqa.connect_power_meter(power_meter)
	var cadence: int = _cadence.get_selected_metadata()
	if not CADENCE_SOURCES.has(cadence):
		_torqa.connect_cadence_sensor(cadence)
	var heart_rate: int = _heart_rate.get_selected_metadata()
	if heart_rate != NO_HEART_RATE:
		_torqa.connect_heart_rate(heart_rate)
	var shifter: int = _shifter.get_selected_metadata()
	if shifter != NO_SHIFTER:
		_torqa.connect_controller(shifter)
	return true


func _init() -> void:
	add_theme_constant_override("separation", 16)
	var heading: Label = Label.new()
	heading.text = tr("Devices")
	heading.add_theme_font_size_override("font_size", 22)
	add_child(heading)
	var scan_row: HBoxContainer = HBoxContainer.new()
	scan_row.add_theme_constant_override("separation", 16)
	_scan_button.text = tr("Scan for devices")
	_scan_button.custom_minimum_size = Vector2(220, 44)
	_scan_button.pressed.connect(_on_scan_pressed)
	scan_row.add_child(_scan_button)
	_scan_label.text = tr("Wake your trainer by pedalling, then scan.")
	_scan_label.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	scan_row.add_child(_scan_label)
	add_child(scan_row)
	var grid: GridContainer = GridContainer.new()
	grid.columns = 2
	grid.add_theme_constant_override("h_separation", 24)
	grid.add_theme_constant_override("v_separation", 10)
	# i18n-begin
	for row: Array in [
		["Trainer", _trainer],
		["Power meter", _power_meter],
		["Cadence", _cadence],
		["Heart rate", _heart_rate],
		["Shifter (Di2)", _shifter],
	]:
		# i18n-end
		var caption: Label = Label.new()
		caption.text = row[0]
		caption.custom_minimum_size = Vector2(RideOptions.CAPTION_WIDTH, 0)
		grid.add_child(caption)
		var option: OptionButton = row[1]
		# Device names are never translated; the fixed entries are, in code.
		option.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		option.custom_minimum_size = Vector2(420, 0)
		grid.add_child(option)
	add_child(grid)
	_power_meter.tooltip_text = tr(
		"Power pedals or crank: its power and cadence count instead of the trainer's"
	)
	_cadence.tooltip_text = tr(
		"Automatic: a cadence sensor, else the power meter, else the trainer"
	)
	_cadence.item_selected.connect(_on_cadence_selected)
	_shifter.tooltip_text = tr(
		"Assign buttons to D-Fly channels in E-TUBE, then choose here what each press does"
	)
	_shifter.item_selected.connect(func(_index: int) -> void: _show_channels())
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


func _on_scan_pressed() -> void:
	_scan_button.disabled = true
	_scan_label.text = tr("Scanning…")
	_status.text = ""
	_torqa.scan(SCAN_SECONDS)


func _on_devices_found(devices: Array) -> void:
	_scan_button.disabled = false
	_reset_options()
	var trainers: int = 0
	var remembered: Array[int] = []
	for device: Dictionary in devices:
		var index: int = device["index"]
		if device["remembered"]:
			remembered.append(index)
		var label: String = device["name"]
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
	_show_channels()


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


## A device's own cadence lets go of the cadence sensor; a sensor connects with the others.
func _on_cadence_selected(index: int) -> void:
	var key: int = _cadence.get_item_metadata(index)
	if CADENCE_SOURCES.has(key):
		_torqa.set_cadence_source(CADENCE_SOURCES[key])


## The buttons matter only with a shifter chosen.
func _show_channels() -> void:
	_buttons.visible = _shifter.get_selected_metadata() != NO_SHIFTER


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


## Names the channels and actions in the interface language.
func _label_buttons() -> void:
	for i: int in range(_channel_captions.size()):
		_channel_captions[i].text = tr("D-Fly channel %d") % (i + 1)
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


## The reconnect at start missed some devices (R41): ask the rider to wake them.
func _on_remembered_missing(names: PackedStringArray) -> void:
	_status.text = (
		tr("%s not found — wake it by pedalling (or put on the strap), then scan.")
		% ", ".join(names)
	)


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
	_show_channels()
