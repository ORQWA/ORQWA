extends SceneTree
## Group setup and activity preview behavior (R169).

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	await _group_activity()
	if not _failed:
		print("GROUP UI SMOKE TEST PASSED")
	quit(1 if _failed else 0)


## Group choices preview before sharing, survive a refresh, and audio devices stay usable.
func _group_activity() -> void:
	var app: OrqwaApp = OrqwaApp.new()
	root.add_child(app)
	var group: GroupRidesTab = GroupRidesTab.new()
	group.bind(app)
	root.add_child(group)
	await process_frame
	group.set_process(false)
	var picker: GroupActivityPicker = group._picker
	_check(picker._course.item_count > 1, "group picker offers the bundled course")
	_check(picker._workout.item_count > 1, "group picker offers structured workouts")
	picker._course.select(1)
	picker._workout.select(1)
	picker._preview()
	_check(picker._map.visible and picker._elevation.visible, "course map and elevation preview")
	_check(picker._chart.visible and picker._duration.visible, "workout power and duration preview")
	var course: String = picker.course()
	var workout: String = picker.workout()
	picker.refresh()
	_check(picker.course() == course and picker.workout() == workout, "refresh preserves choices")
	_check(not picker._apply.disabled, "a preview is not shared until confirmed")
	picker.show_selection(true, course, workout)
	_check(picker._apply.disabled, "shared activity has an acknowledgement")
	picker.show_selection(false, course, workout)
	_check(
		picker._course.disabled and not picker._apply.visible, "joiners preview the shared choice"
	)
	var voice: GroupVoiceControls = group._voice_controls
	voice.update_status({"voice": true, "microphone": false, "devices": []})
	_check(
		not voice._inputs.disabled and not voice._outputs.disabled,
		"devices can change during voice"
	)
	_check(not voice._mic.button_pressed, "listening does not imply microphone consent")
	voice.update_status({"voice_pending": true})
	_check(
		not voice._inputs.disabled and not voice._outputs.disabled,
		"devices can change during startup"
	)
	voice.update_status(
		{
			"voice": true,
			"input_device": "headset",
			"output_device": "headset",
			"voice_volume": 0.4,
			"devices": [{"id": "headset", "name": "Headset", "input": true, "output": true}]
		}
	)
	var input: String = voice._inputs.get_selected_metadata()
	var output: String = voice._outputs.get_selected_metadata()
	_check(input == "headset", "selected microphone follows the session")
	_check(output == "headset", "selected speakers follow the session")
	_check(is_equal_approx(voice._volume.value, 40.0), "volume follows the session")
	voice.update_status({"voice": true, "deafened": true, "microphone": false})
	_check(voice._mic.disabled, "mute all sound prevents microphone activation")
	group.free()
	app.free()


func _check(condition: bool, message: String) -> void:
	if not condition:
		_failed = true
		push_error(message)
