class_name GroupRidesTab
extends VBoxContainer
## Private group lobby controls; the Rust core owns admission, downloads and readiness.

signal devices_requested

var compact: bool = false
var _orqwa: OrqwaApp
var _setup: VBoxContainer = VBoxContainer.new()
var _lobby: VBoxContainer = VBoxContainer.new()
var _admin: VBoxContainer = VBoxContainer.new()
var _stage: Label = Label.new()
var _activity: Label = Label.new()
var _error: Label = Label.new()
var _paste: LineEdit = LineEdit.new()
var _reply: LineEdit = LineEdit.new()
var _lan: CheckBox = CheckBox.new()
var _stun: CheckBox = CheckBox.new()
var _course: OptionButton = OptionButton.new()
var _workout: OptionButton = OptionButton.new()
var _code: LineEdit = LineEdit.new()
var _codes: VBoxContainer = VBoxContainer.new()
var _roster: VBoxContainer = VBoxContainer.new()
var _ready_button: Button = Button.new()
var _start: Button = Button.new()
var _retry: Button = Button.new()
var _listen: CheckBox = CheckBox.new()
var _mic: CheckBox = CheckBox.new()
var _deafen: CheckBox = CheckBox.new()
var _volume: HSlider = HSlider.new()
var _inputs: OptionButton = OptionButton.new()
var _outputs: OptionButton = OptionButton.new()
var _device_key: String = ""
var _progress: ProgressBar = ProgressBar.new()
var _bytes: Label = Label.new()
var _status: Dictionary = {}
var _roster_key: String = ""
var _elapsed: float = 0.0


func bind(orqwa: OrqwaApp) -> void:
	_orqwa = orqwa
	if is_node_ready():
		refresh()


func _ready() -> void:
	add_theme_constant_override("separation", 12)
	var title: Label = Label.new()
	title.text = tr("Group rides")
	add_child(title)
	_stage.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	add_child(_stage)
	add_child(_setup)
	_lan.text = tr("Allow local network peers")
	_setup.add_child(_lan)
	_stun.text = tr("Use public address discovery (STUN)")
	_stun.button_pressed = true
	_setup.add_child(_stun)
	_paste.placeholder_text = tr("Paste an invite code to join")
	_paste.max_length = 2048
	_setup.add_child(_paste)
	var actions: HFlowContainer = HFlowContainer.new()
	_button(actions, tr("Create lobby"), func() -> void: _open(""))
	_button(actions, tr("Join lobby"), _join)
	_setup.add_child(actions)
	var privacy: Label = Label.new()
	privacy.text = tr(
		"Exchange codes privately. Codes reveal connection addresses; replies contain admission secrets."
	)
	privacy.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_setup.add_child(privacy)
	add_child(_lobby)
	_lobby.add_child(_codes)
	_code.editable = false
	_code.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	_codes.add_child(_code)
	_button(
		_codes, tr("Copy private code"), func() -> void: DisplayServer.clipboard_set(_code.text)
	)
	_reply.placeholder_text = tr("Host: paste a joiner's reply code")
	_reply.max_length = 2048
	_admin.add_child(_reply)
	_button(_admin, tr("Admit rider"), _admit)
	_course.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_workout.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	for picker: OptionButton in [_course, _workout, _inputs, _outputs]:
		picker.clip_text = true
		picker.fit_to_longest_item = false
		picker.custom_minimum_size.x = 240
	_admin.add_child(_course)
	_admin.add_child(_workout)
	_button(_admin, tr("Select activity"), _select)
	_lobby.add_child(_admin)
	_activity.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_activity.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	_lobby.add_child(_activity)
	_lobby.add_child(_progress)
	_lobby.add_child(_bytes)
	_lobby.add_child(_roster)
	var ride_actions: HFlowContainer = HFlowContainer.new()
	_ready_button.text = tr("Ready")
	_ready_button.toggle_mode = true
	_ready_button.toggled.connect(func(on: bool) -> void: _orqwa.group_ready(on))
	ride_actions.add_child(_ready_button)
	_start.text = tr("Start group ride")
	_start.pressed.connect(func() -> void: _orqwa.group_start())
	ride_actions.add_child(_start)
	_retry.text = tr("Retry download")
	_retry.pressed.connect(func() -> void: _orqwa.group_retry())
	ride_actions.add_child(_retry)
	if not compact:
		_button(ride_actions, tr("Devices"), func() -> void: devices_requested.emit())
	_button(ride_actions, tr("Leave group"), func() -> void: _orqwa.group_leave())
	_lobby.add_child(ride_actions)
	var audio: HFlowContainer = HFlowContainer.new()
	_listen.text = tr("Voice chat (headphones)")
	_mic.text = tr("Microphone")
	_deafen.text = tr("Deafen")
	for control: CheckBox in [_listen, _mic, _deafen]:
		audio.add_child(control)
	_listen.toggled.connect(_on_voice_listen)
	_mic.toggled.connect(func(_on: bool) -> void: _voice())
	_deafen.toggled.connect(_on_voice_listen)
	_volume.min_value = 0
	_volume.max_value = 100
	_volume.step = 5
	_volume.value = 60
	_volume.custom_minimum_size.x = 120
	_volume.tooltip_text = tr("Voice volume")
	_volume.value_changed.connect(
		func(value: float) -> void: _orqwa.group_voice_volume(value / 100.0)
	)
	audio.add_child(_volume)
	_lobby.add_child(audio)
	var devices: HFlowContainer = HFlowContainer.new()
	_button(devices, tr("Refresh audio devices"), func() -> void: _orqwa.group_voice_devices())
	_inputs.add_item(tr("Default microphone"))
	_inputs.set_item_metadata(0, "")
	_outputs.add_item(tr("Default speakers"))
	_outputs.set_item_metadata(0, "")
	_inputs.item_selected.connect(func(_index: int) -> void: _choose_devices())
	_outputs.item_selected.connect(func(_index: int) -> void: _choose_devices())
	devices.add_child(_inputs)
	devices.add_child(_outputs)
	_lobby.add_child(devices)
	_error.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_error.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	add_child(_error)
	visibility_changed.connect(refresh)
	refresh()


func _button(parent: Node, text: String, action: Callable) -> void:
	var button: Button = Button.new()
	button.text = text
	button.pressed.connect(action)
	parent.add_child(button)


func refresh() -> void:
	if _orqwa == null or not is_node_ready():
		return
	_course.clear()
	_course.add_item(tr("No course — workout only"))
	_course.set_item_metadata(0, "")
	for course: Dictionary in _orqwa.courses():
		if course.get("video", false):
			continue
		var course_name: String = course["name"]
		_course.add_item(course_name)
		_course.set_item_metadata(_course.item_count - 1, course["path"])
	_workout.clear()
	_workout.add_item(tr("No workout — course ride"))
	_workout.set_item_metadata(0, "")
	for workout: Dictionary in _orqwa.workouts():
		var workout_name: String = workout["name"]
		_workout.add_item(workout_name)
		_workout.set_item_metadata(_workout.item_count - 1, workout["id"])
	_update()


func _process(delta: float) -> void:
	_elapsed += delta
	if _elapsed >= 0.2 and visible and _orqwa != null:
		_elapsed = 0.0
		_update()


func _update() -> void:
	_status = _orqwa.group_status()
	var phase: String = _status.get("phase", "idle")
	var countdown: float = _status.get("countdown_s", 0.0)
	var received: float = _status.get("received", 0)
	var total: float = _status.get("total", 0)
	var ready: bool = _status.get("ready", false)
	var voice: bool = _status.get("voice", false)
	var microphone: bool = _status.get("microphone", false)
	var deafened: bool = _status.get("deafened", false)
	var details: String = _status.get("error", "")
	var captions: Dictionary = {
		"idle": tr("Create a lobby or join friends with a private code."),
		"connecting": tr("Connecting — send your reply to the host within one minute."),
		"waiting": tr("Waiting for the host to select an activity."),
		"downloading": tr("Downloading or checking the selected activity…"),
		"preparing": tr("Preparing the course on this computer…"),
		"ready": tr("Activity prepared. Connect your trainer, then mark yourself ready."),
		"countdown": tr("Starting in %s seconds…") % ceili(countdown),
		"riding": tr("Group ride in progress"),
		"ended": tr("Group connection ended. Your local ride continues."),
		"failed": tr("Could not connect. Leave the lobby and exchange fresh codes to retry."),
	}
	_stage.text = captions.get(phase, "")
	if phase == "waiting" and _status.get("host", false):
		_stage.text = tr("Choose a course or workout to share with the group.")
	_setup.visible = phase == "idle" and not compact
	_lobby.visible = phase != "idle"
	var code: String = _status.get("invite", "")
	if code.is_empty():
		code = _status.get("reply", "")
	_codes.visible = not code.is_empty() and not compact
	_code.text = code
	_admin.visible = (
		_status.get("host", false) and not compact and phase not in ["countdown", "riding", "ended"]
	)
	_activity.text = _status.get("name", "")
	_progress.visible = phase == "downloading" and _status.get("total", 0) > 0
	_bytes.visible = _progress.visible
	_progress.max_value = maxf(1.0, total)
	_progress.value = _status.get("received", 0)
	_bytes.text = (tr("Current file: %.1f / %.1f MiB") % [received / 1048576.0, total / 1048576.0])
	_ready_button.disabled = (
		not _status.get("prepared", false) or phase in ["countdown", "riding", "ended"]
	)
	_ready_button.set_pressed_no_signal(ready)
	_start.visible = _status.get("host", false) and not compact
	_start.disabled = not _status.get("can_start", false)
	_retry.visible = (
		not _status.get("host", false) and not details.is_empty() and phase == "waiting"
	)
	var voice_pending: bool = _status.get("voice_pending", false)
	_listen.set_pressed_no_signal(voice or voice_pending)
	_mic.disabled = not _status.get("voice", false)
	_deafen.disabled = _mic.disabled
	_mic.set_pressed_no_signal(microphone)
	_deafen.set_pressed_no_signal(deafened)
	_inputs.disabled = _status.get("voice", false)
	_outputs.disabled = _inputs.disabled
	var device_key: String = str(_status.get("devices", []))
	if device_key != _device_key:
		_device_key = device_key
		_inputs.clear()
		_outputs.clear()
		_inputs.add_item(tr("Default microphone"))
		_inputs.set_item_metadata(0, "")
		_outputs.add_item(tr("Default speakers"))
		_outputs.set_item_metadata(0, "")
		for device: Dictionary in _status.get("devices", []):
			var device_name: String = device["name"]
			if device["input"]:
				_inputs.add_item(device_name)
				_inputs.set_item_metadata(_inputs.item_count - 1, device["id"])
			if device["output"]:
				_outputs.add_item(device_name)
				_outputs.set_item_metadata(_outputs.item_count - 1, device["id"])
	_error.text = "" if details.is_empty() else tr("Group ride action failed:") + "\n" + details
	var key: String = (
		str(_status.get("roster", []))
		+ str(_status.get("voice", false))
		+ TranslationServer.get_locale()
	)
	if key != _roster_key:
		_roster_key = key
		for child: Node in _roster.get_children():
			_roster.remove_child(child)
			child.queue_free()
		for rider: Dictionary in _status.get("roster", []):
			var row: HBoxContainer = HBoxContainer.new()
			var label: Label = Label.new()
			label.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
			label.text = (
				rider["name"] + " · " + (tr("Ready") if rider["ready"] else tr("Not ready"))
			)
			if phase == "riding" and rider.get("present", false):
				var imperial: bool = _orqwa.profile().get("units", "metric") == "imperial"
				var gap: float = rider["gap_m"]
				label.text = (
					rider["name"]
					+ " · "
					+ (
						tr("%s ft") % roundi(gap / HudPanel.METERS_PER_FOOT)
						if imperial
						else tr("%s m") % roundi(gap)
					)
				)
			row.add_child(label)
			if not rider["me"]:
				var speaker: int = rider["id"]
				var mute: CheckBox = CheckBox.new()
				mute.text = tr("Mute locally")
				mute.disabled = not _status.get("voice", false)
				var muted: bool = rider.get("muted", false)
				mute.set_pressed_no_signal(muted)
				mute.toggled.connect(func(on: bool) -> void: _orqwa.group_voice_mute(speaker, on))
				row.add_child(mute)
			if (
				_status.get("host", false)
				and not rider["me"]
				and phase not in ["countdown", "riding"]
			):
				var id: int = rider["id"]
				_button(row, tr("Remove"), func() -> void: _orqwa.group_remove(id))
			_roster.add_child(row)


func _open(invite: String) -> void:
	_orqwa.group_open(invite, _lan.button_pressed, _stun.button_pressed)
	_paste.clear()


func _join() -> void:
	if not _paste.text.strip_edges().is_empty():
		_open(_paste.text.strip_edges())


func _admit() -> void:
	_orqwa.group_admit(_reply.text.strip_edges())
	_reply.clear()


func _select() -> void:
	var course: String = _course.get_selected_metadata()
	var workout: String = _workout.get_selected_metadata()
	_orqwa.group_select(course, workout)


func _choose_devices() -> void:
	var input: String = _inputs.get_selected_metadata()
	var output: String = _outputs.get_selected_metadata()
	_orqwa.group_voice_choose(input, output)


func _voice() -> void:
	if _mic.button_pressed and OS.has_feature("android"):
		if not OS.request_permission("android.permission.RECORD_AUDIO"):
			_mic.set_pressed_no_signal(false)
	_orqwa.group_voice(
		_listen.button_pressed, _mic.button_pressed, _deafen.button_pressed, _volume.value / 100.0
	)


func _on_voice_listen(_on: bool) -> void:
	_mic.set_pressed_no_signal(false)
	_voice()
