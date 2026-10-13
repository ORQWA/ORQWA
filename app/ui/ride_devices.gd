class_name RideDevices
extends VBoxContainer
## The trainer, heart-rate strap and shifter during a ride, each with whether it is connected,
## and a button to reconnect those that are not (R69): a strap that dropped while the rider
## was off the bike, or one not found at start.

var _torqa: TorqaApp
var _rows: GridContainer = GridContainer.new()
var _reconnect: Button = Button.new()
var _note: Label = Label.new()
## A reconnect is under way.
var _searching: bool = false


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


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	var changed: Callable = func(_device_name: String) -> void: refresh()
	_torqa.device_connected.connect(changed)
	_torqa.device_disconnected.connect(changed)
	_torqa.devices_found.connect(func(_devices: Array) -> void: _on_scanned())
	_torqa.remembered_missing.connect(_on_remembered_missing)
	_torqa.failed.connect(func(_message: String) -> void: _on_failed())


## Shows how the devices are doing now.
func refresh() -> void:
	for child: Node in _rows.get_children():
		child.queue_free()
	var devices: Array = _torqa.devices() if _torqa != null else []
	# i18n-begin
	var kinds: Dictionary[String, String] = {
		"trainer": "Trainer",
		"heart_rate": "Heart rate",
		"controller": "Shifter",
	}
	# i18n-end
	var all_connected: bool = true
	for device: Dictionary in devices:
		var kind: String = device["kind"]
		var connected: bool = device["connected"]
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
		state.text = tr("Connected") if connected else tr("Not connected")
		state.add_theme_color_override(
			"font_color", UiTheme.ACCENT if connected else UiTheme.DANGER
		)
		_rows.add_child(state)
	if devices.is_empty():
		_note.text = tr("No devices connected yet: connect them in Devices & Settings.")
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
