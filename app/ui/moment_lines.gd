class_name MomentLines
extends RefCounted
## What the app says of a moment of the ride (R124), in the rider's voice: the warm, cheering
## casual voice picks one of its lines at random, the plain one states the facts. All lines
## live here so they are easy to refresh.

const KM_PER_MILE: float = 1.609344
const METERS_PER_FOOT: float = 0.3048

## Per moment kind (as the core's `moment` signal names it): the casual lines, then the plain
## one. `{…}` are filled in by `line()`.
# i18n-begin
const LINES: Dictionary[String, Array] = {
	"climb_topped":
	[
		[
			"Climb {n} is done and it averaged {grade}. Well climbed!",
			"You're over the top of climb {n}, which averaged {grade}. Get your breath back!",
		],
		"Climb {n} topped, {grade} average.",
	],
	"climb_record":
	[
		[
			"{time} on climb {n} is your best time up there yet!",
			"You've never been up climb {n} faster than that, {gain} quicker than before!",
		],
		"Climb {n} in {time}, a personal record by {gain}.",
	],
	"halfway":
	[
		[
			"You're halfway there and going really well!",
			"Half the ride is done already, keep it rolling!",
		],
		"Halfway.",
	],
	"one_to_go":
	[
		[
			"Just {left} to go, give it everything you have left!",
			"Only {left} left, so bring it home strong!",
		],
		"{left} to go.",
	],
	"distance":
	[
		[
			"{distance} done and you're still going strong!",
			"That's {distance} on the clock already, great going!",
		],
		"{distance} ridden.",
	],
	"power_peak":
	[
		[
			"{power} for {duration} is a new best for you!",
			"That was your best {duration} ever, at {power}!",
		],
		"Best {duration} power: {power}.",
	],
	"first_ride":
	[
		[
			"Your first ride on {course}, so enjoy the new roads!",
			"You've never ridden {course} before. Have fun exploring it!",
		],
		"Your first ride on {course}.",
	],
}
# i18n-end


## The line for `moment` (`{kind, …}` as the core sends it) in `voice` ("casual", "plain";
## "off" speaks plainly too, for the summary's highlights); empty for an unknown kind.
static func line(moment: Dictionary, voice: String, imperial: bool) -> String:
	var kind: String = moment.get("kind", "")
	if not LINES.has(kind):
		return ""
	var lines: Array = LINES[kind]
	var text: String
	if voice == "casual":
		var casual: Array = lines[0]
		text = casual.pick_random()
	else:
		text = lines[1]
	return TranslationServer.translate(text).format(_values(moment, imperial))


## The figure a card shows large for `moment`, e.g. "14:02" or "25 km"; empty for none.
static func figure(moment: Dictionary, imperial: bool) -> String:
	match moment.get("kind", ""):
		"climb_topped", "climb_record":
			var elapsed: float = moment["elapsed_s"]
			return UiTheme.duration(elapsed)
		"halfway", "distance":
			var meters: float = moment["distance_m"]
			return distance(meters, imperial)
		"one_to_go":
			return _one(imperial)
		"power_peak":
			var power: float = moment["power_w"]
			return "%d W" % roundi(power)
	return ""


## `meters` in the rider's units: "32.1 km", whole ones without a decimal ("25 km").
static func distance(meters: float, imperial: bool) -> String:
	var value: float = meters / 1000.0 / (KM_PER_MILE if imperial else 1.0)
	var unit: String = "mi" if imperial else "km"
	var rounded: float = snappedf(value, 0.1)
	if is_equal_approx(rounded, roundf(rounded)):
		return "%d %s" % [roundi(rounded), unit]
	return "%.1f %s" % [rounded, unit]


## A power record's duration: "5 s", "20 min", "1 h".
static func span(seconds: float) -> String:
	var whole: int = roundi(seconds)
	if whole < 60:
		return "%d s" % whole
	if whole < 3600:
		return "%d min" % (whole / 60)
	return "%d h" % (whole / 3600)


## `meters` of climbing in the rider's units.
static func climbing(meters: float, imperial: bool) -> String:
	if imperial:
		return "%d ft" % roundi(meters / METERS_PER_FOOT)
	return "%d m" % roundi(meters)


static func _one(imperial: bool) -> String:
	return "1 mi" if imperial else "1 km"


static func _values(moment: Dictionary, imperial: bool) -> Dictionary:
	var values: Dictionary = {"left": _one(imperial)}
	if moment.has("index"):
		var index: int = moment["index"]
		values["n"] = index + 1
	if moment.has("elapsed_s"):
		var elapsed: float = moment["elapsed_s"]
		values["time"] = UiTheme.duration(elapsed)
	if moment.has("gain_s"):
		var gain: float = moment["gain_s"]
		values["gain"] = UiTheme.duration(gain)
	if moment.has("grade_pct"):
		var grade: float = moment["grade_pct"]
		values["grade"] = "%.1f %%" % grade
	if moment.has("distance_m"):
		var meters: float = moment["distance_m"]
		values["distance"] = distance(meters, imperial)
	if moment.has("power_w"):
		var power: float = moment["power_w"]
		values["power"] = "%d W" % roundi(power)
	if moment.has("duration_s"):
		var seconds: float = moment["duration_s"]
		values["duration"] = span(seconds)
	if moment.has("course"):
		values["course"] = moment["course"]
	return values
