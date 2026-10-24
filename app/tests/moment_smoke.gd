extends SceneTree
## Headless checks of the ride's moments (R124): every moment has its line in both voices, in
## the rider's units, and the card shows them one after the other.
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
]

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	_lines()
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
	_check(MomentLines.figure(MOMENTS[1], false) == "14:02", "a time figure")
	_check(_plain(MOMENTS[6], false) == "Best 5 min power: 312 W.", "a power record")
	_check(MomentLines.figure(MOMENTS[6], false) == "312 W", "a power figure")
	_check(MomentLines.span(5.0) == "5 s" and MomentLines.span(3600.0) == "1 h", "spans")
	_check(MomentLines.line({"kind": "unknown"}, "casual", false).is_empty(), "unknown kinds")


func _plain(moment: Dictionary, imperial: bool) -> String:
	return MomentLines.line(moment, "plain", imperial)


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
