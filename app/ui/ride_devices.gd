class_name RideDevices
extends VBoxContainer
## The trainer, sensors and shifter during a ride, each with whether it is connected (and why
## not, when connecting failed, R80), and a button to reconnect those that are not (R69): a
## strap that dropped while the rider was off the bike, or one not found at start. Below them
## the drivetrain the shifter shifts, changed for the ride at once and kept in the profile (R98).

var _torqa: TorqaApp
var _rows: GridContainer = GridContainer.new()
var _reconnect: Button = Button.new()
var _note: Label = Label.new()
## A reconnect is under way.
var _searching: bool = false
var _gears: GridContainer = GridContainer.new()
var _drivetrain: OptionButton = OptionButton.new()
var _chainring: SpinBox = _teeth(20.0, 60.0)
var _cog: SpinBox = _teeth(9.0, 36.0)
var _teeth_rows: Array[Control] = []


func _init() -> void:
	add_theme_constant_override("separation", 16)
	_rows.columns = 3
	_rows.add_theme_constant_override("h_separation", 24)
	_rows.add_theme_constant_override("v_separation", 8)
	add_child(_rows)
	_reconnect.text = tr("Reconnect devices")
	_reconnect.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_reconnect.pressed.connect(_on_reconnect_pressed)
	add_child(_reconnect)
	_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_note.add_theme_color_override("font_color", UiTheme.MUTED)
	add_child(_note)
	add_child(HSeparator.new())
	_build_gears()


func _build_gears() -> void:
	_gears.columns = 2
	_gears.add_theme_constant_override("h_separation", 24)
	_gears.add_theme_constant_override("v_separation", 8)
	# In the order of ProfileDialog.DRIVETRAINS.
	_drivetrain.add_item(tr("Cassette: shift on the bike"))
	_drivetrain.add_item(tr("Single cog: virtual gears"))
	_drivetrain.tooltip_text = tr(
		"On a single cog such as the Zwift Cog, Torqa shifts 24 virtual gears: ↑ and ↓ while riding"
	)
	_drivetrain.item_selected.connect(func(_index: int) -> void: _on_gears_changed())
	for spin: SpinBox in [_chainring, _cog]:
		spin.value_changed.connect(func(_value: float) -> void: _on_gears_changed())
	# i18n-begin
	for row: Array in [["Drivetrain", _drivetrain], ["Chainring", _chainring], ["Cog", _cog]]:
		# i18n-end
		var caption: Label = Label.new()
		caption.text = row[0]
		caption.custom_minimum_size.x = RideOptions.CAPTION_WIDTH
		var field: Control = row[1]
		field.custom_minimum_size.x = RideOptions.FIELD_WIDTH
		field.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
		_gears.add_child(caption)
		_gears.add_child(field)
		if field != _drivetrain:
			_teeth_rows.append_array([caption, field])
	add_child(_gears)


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	var changed: Callable = func(_device_name: String) -> void: refresh()
	_torqa.device_connected.connect(changed)
	_torqa.device_disconnected.connect(changed)
	_torqa.device_failed.connect(func(_device_name: String, _reason: String) -> void: refresh())
	_torqa.devices_found.connect(func(_devices: Array) -> void: _on_scanned())
	_torqa.remembered_missing.connect(_on_remembered_missing)
	_torqa.failed.connect(func(_message: String) -> void: _on_failed())


## Shows how the devices are doing now, and the rider's drivetrain.
func refresh() -> void:
	_show_drivetrain()
	for child: Node in _rows.get_children():
		child.queue_free()
	var devices: Array = _torqa.devices() if _torqa != null else []
	# i18n-begin
	var kinds: Dictionary[String, String] = {
		"trainer": "Trainer",
		"heart_rate": "Heart rate",
		"power_meter": "Power meter",
		"cadence_sensor": "Cadence",
		"controller": "Shifter",
	}
	# i18n-end
	var all_connected: bool = true
	for device: Dictionary in devices:
		var kind: String = device["kind"]
		var state_name: String = device["state"]
		var connected: bool = state_name == "connected"
		all_connected = all_connected and connected
		var caption: Label = Label.new()
		var kind_name: String = kinds.get(kind, kind)
		caption.text = tr(kind_name)
		caption.custom_minimum_size.x = RideOptions.CAPTION_WIDTH
		_rows.add_child(caption)
		var device_name: Label = Label.new()
		device_name.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
		device_name.text = device["name"]
		device_name.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		_rows.add_child(device_name)
		var state: Label = Label.new()
		# i18n-begin
		var labels: Dictionary[String, String] = {
			"connected": "Connected",
			"connecting": "Connecting…",
			"missing": "Not found",
		}
		# i18n-end
		var text: String = labels.get(state_name, "Not connected")
		state.text = tr(text)
		# While the driver keeps trying, the rider sees why the last attempt failed (R80).
		var failure: String = device.get("failure", "")
		if not connected and not failure.is_empty():
			state.text = tr("Failed") + ": " + tr(failure)
		var color: Color = UiTheme.DANGER
		if connected:
			color = UiTheme.SUCCESS
		elif state_name == "connecting" and failure.is_empty():
			color = UiTheme.WARNING
		state.add_theme_color_override("font_color", color)
		_rows.add_child(state)
	if devices.is_empty():
		_note.text = tr("No devices connected yet: connect them under Devices on the start page.")
	elif all_connected:
		_note.text = ""
	_reconnect.visible = not devices.is_empty()
	_reconnect.disabled = _searching or all_connected


func _on_reconnect_pressed() -> void:
	if _torqa == null or not _torqa.reconnect_devices():
		return
	_searching = true
	_reconnect.disabled = true
	_note.text = tr("Looking for your devices… wake them by pedalling or putting on the strap.")


func _on_scanned() -> void:
	_searching = false
	_note.text = ""
	refresh()


## A failed scan (e.g. Bluetooth off) ends the reconnect too; the ride shows the message.
func _on_failed() -> void:
	if _searching:
		_searching = false
		_note.text = ""
		refresh()


func _on_remembered_missing(names: PackedStringArray) -> void:
	_note.text = tr("%s not found — wake it, then reconnect again.") % ", ".join(names)


func _show_drivetrain() -> void:
	var profile: Dictionary = _torqa.profile() if _torqa != null else {}
	var drivetrain: String = profile.get("drivetrain", "single_cog")
	_drivetrain.select(maxi(ProfileDialog.DRIVETRAINS.find(drivetrain), 0))
	var chainring: float = profile.get("chainring", 50)
	var cog: float = profile.get("cog", 14)
	_chainring.set_value_no_signal(chainring)
	_cog.set_value_no_signal(cog)
	_show_teeth()


func _on_gears_changed() -> void:
	_show_teeth()
	if _torqa != null:
		_torqa.set_drivetrain(
			ProfileDialog.DRIVETRAINS[_drivetrain.selected],
			roundi(_chainring.value),
			roundi(_cog.value)
		)


func _show_teeth() -> void:
	for control: Control in _teeth_rows:
		control.visible = ProfileDialog.DRIVETRAINS[_drivetrain.selected] == "single_cog"


static func _teeth(low: float, high: float) -> SpinBox:
	var spin: SpinBox = SpinBox.new()
	spin.min_value = low
	spin.max_value = high
	spin.step = 1.0
	spin.suffix = " T"
	return spin
