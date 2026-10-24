extends SceneTree
## Headless checks of the ride's moments (R124): every moment has its line in both voices, in
## the rider's units, the card shows them one after the other and the summaries' line looks
## back on the ride.
## Run: godot --headless --path app -s res://tests/moment_smoke.gd

## One moment of each kind, as the core's `moment` signal sends them.
const MOMENTS: Array[Dictionary] = [
	{"kind": "climb_topped", "index": 1, "elapsed_s": 842.0, "grade_pct": 8.04},
	{"kind": "climb_record", "index": 0, "elapsed_s": 842.0, "gain_s": 12.0},
	{"kind": "halfway", "distance_m": 16050.0},
	{"kind": "one_to_go"},
	{"kind": "distance", "distance_m": 25000.0},
	{"kind": "first_ride", "course": "Oberalp"},
	{"kind": "power_peak", "duration_s": 300.0, "power_w": 311.6},
	{"kind": "step_held", "index": 3, "power_w": 262.0},
	{"kind": "streak", "rides": 3},
	{"kind": "week_plan", "rides": 3, "planned": 4},
]

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_lines()
	_recap()
	_chime()
	await _card()
	if not _failed:
		print("MOMENT SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _lines() -> void:
	for moment: Dictionary in MOMENTS:
		var kind: String = moment["kind"]
		for voice: String in ["casual", "plain", "off"]:
			for _i: int in range(6):
				var line: String = MomentLines.line(moment, voice, false)
				_check(not line.is_empty(), "%s has a %s line" % [kind, voice])
				_check(not "{" in line, "%s in %s fills every slot: %s" % [kind, voice, line])
	_check(_plain(MOMENTS[0], false) == "Climb 2 topped, 8.0 % average.", "climbs from 1")
	_check(_plain(MOMENTS[2], false) == "Halfway.", "plain facts")
	_check(_plain(MOMENTS[3], true) == "1 mi to go.", "in miles")
	_check(_plain(MOMENTS[4], false) == "25 km ridden.", "whole distances")
	_check(MomentLines.figure(MOMENTS[2], false) == "16.1 km", "a distance figure")
	_check(MomentLines.distance(420.0, false) == "420 m", "a short distance")
	_check(MomentLines.figure(MOMENTS[1], false) == "14:02", "a time figure")
	_check(_plain(MOMENTS[6], false) == "Best 5 min power: 312 W.", "a power record")
	_check(MomentLines.figure(MOMENTS[6], false) == "312 W", "a power figure")
	_check(MomentLines.span(5.0) == "5 s" and MomentLines.span(3600.0) == "1 h", "spans")
	_check(_plain(MOMENTS[9], false) == "Ride 3 of 4 planned this week.", "the plan")
	var done: Dictionary = MOMENTS[9].duplicate()
	done["rides"] = 4
	_check(_plain(done, false) == "All 4 planned rides this week done.", "the plan done")
	done["rides"] = 5
	_check(_plain(done, false) == "Ride 5 this week, 4 planned.", "beyond the plan")
	var seeded: String = MomentLines.line(MOMENTS[8], "casual", false, 7)
	for _i: int in range(4):
		_check(MomentLines.line(MOMENTS[8], "casual", false, 7) == seeded, "a seed keeps the line")
	_check(MomentLines.line({"kind": "unknown"}, "casual", false).is_empty(), "unknown kinds")


func _plain(moment: Dictionary, imperial: bool) -> String:
	return MomentLines.line(moment, "plain", imperial)


## The summaries' line looks back on the ride: no "enjoy the new roads" once it is over.
func _recap() -> void:
	var recap: RideRecap = RideRecap.new()
	root.add_child(recap)
	recap.show_recap(MOMENTS[5], "plain", false, 3)
	_check(recap.visible and recap.text == "First ride on Oberalp.", "first ride: %s" % recap.text)
	for seed: int in range(4):
		recap.show_recap(MOMENTS[5], "casual", false, seed)
		_check(not recap.text.contains("enjoy"), "told looking back: %s" % recap.text)
		_check(recap.text.contains("Oberalp"), "naming the course: %s" % recap.text)
	recap.show_recap(MOMENTS[8], "off", false, 0)
	_check(recap.text == "Ride 3 this week.", "plain when the voice is off: %s" % recap.text)
	recap.show_recap({}, "casual", false, 0)
	_check(not recap.visible, "nothing to say, no line")
	recap.queue_free()


func _chime() -> void:
	var wav: AudioStreamWAV = MomentChime.make()
	var loudest: int = 0
	for i: int in range(0, wav.data.size(), 2):
		loudest = maxi(loudest, absi(wav.data.decode_s16(i)))
	_check(is_equal_approx(wav.get_length(), MomentChime.SECONDS), "the chime's length")
	_check(loudest > 3000 and loudest < 32767, "a chime heard but not clipped: %d" % loudest)
	_check(absi(wav.data.decode_s16(0)) < 100, "the chime starts without a click")


func _card() -> void:
	var card: MomentCard = MomentCard.new()
	root.add_child(card)
	var told: Array[String] = []
	card.shown.connect(func(text: String) -> void: told.append(text))
	card.tell("14:02", "First line.")
	card.tell("", "Second line.")
	card.tell("", "")
	_check(card.visible and card.is_showing(), "a card shows")
	_check(card.current() == PackedStringArray(["14:02", "First line."]), "the first first")
	_check(told == ["14:02  ·  First line."], "told once: %s" % [told])
	await create_timer(MomentCard.SHOW_SECONDS + MomentCard.SLIDE_SECONDS * 2.0 + 0.2).timeout
	_check(card.current() == PackedStringArray(["", "Second line."]), "then the next")
	_check(told.size() == 2, "an empty line is not shown: %s" % [told])
	card.clear()
	_check(not card.visible and not card.is_showing(), "cleared")
	card.queue_free()


func _check(condition: bool, what: String) -> void:
	if not condition:
		_failed = true
		push_error("FAILED: " + what)
