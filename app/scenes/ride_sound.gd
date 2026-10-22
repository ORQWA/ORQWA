class_name RideSound
extends AudioStreamPlayer
## Plays the core's live ambient PCM. Stopping drops queued frames, so resuming never plays
## old chain movement, and a finished ride cannot leak sound into the next screen (R114).

var _playback: AudioStreamGeneratorPlayback


func _init() -> void:
	var generator: AudioStreamGenerator = AudioStreamGenerator.new()
	generator.mix_rate = OrqwaApp.ambient_sound_rate()
	generator.buffer_length = 0.12
	stream = generator


## Feed only while the ride screen and core both allow sound, including in the overlay.
func update(orqwa: OrqwaApp, screen_active: bool) -> void:
	if not screen_active or not orqwa.ambient_sound_active():
		stop()
		_playback = null
		return
	if _playback == null:
		orqwa.restart_ambient_sound()
		play()
		_playback = get_stream_playback()
	var frames: int = _playback.get_frames_available()
	if frames > 0:
		_playback.push_buffer(orqwa.ambient_sound(frames))
