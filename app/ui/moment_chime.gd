class_name MomentChime
extends AudioStreamPlayer
## The chime of a moment of the ride (R125): two soft bell notes a fifth apart, made when the
## app starts rather than shipped as a recording.

const RATE: int = 44100
const SECONDS: float = 0.9
## A5 and E6, the second a little later.
const NOTES: Array[Array] = [[880.0, 0.0], [1318.5, 0.11]]
const DECAY: float = 5.0
## Bells ring with a quieter overtone above each note.
const OVERTONE: float = 2.76
const OVERTONE_LEVEL: float = 0.18
const LEVEL: float = 0.22


func _init() -> void:
	stream = make()
	volume_db = -6.0


## The chime as 16-bit mono PCM.
static func make() -> AudioStreamWAV:
	var count: int = roundi(RATE * SECONDS)
	var data: PackedByteArray = PackedByteArray()
	data.resize(count * 2)
	for i: int in range(count):
		var t: float = float(i) / RATE
		var value: float = 0.0
		for note: Array in NOTES:
			var pitch: float = note[0]
			var start: float = note[1]
			if t < start:
				continue
			var since: float = t - start
			# A few milliseconds of attack keep the start from clicking.
			var envelope: float = minf(since / 0.004, 1.0) * exp(-DECAY * since)
			value += (
				envelope
				* (sin(TAU * pitch * since) + OVERTONE_LEVEL * sin(TAU * pitch * OVERTONE * since))
			)
		data.encode_s16(i * 2, clampi(roundi(value * LEVEL * 32767.0), -32768, 32767))
	var wav: AudioStreamWAV = AudioStreamWAV.new()
	wav.format = AudioStreamWAV.FORMAT_16_BITS
	wav.mix_rate = RATE
	wav.stereo = false
	wav.data = data
	return wav
