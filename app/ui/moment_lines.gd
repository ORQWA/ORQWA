class_name MomentLines
extends RefCounted
## What the app says of a moment of the ride (R124), in the rider's voice: the warm, cheering
## casual voice picks one of its lines at random, the plain one states the facts. All lines
## live here so they are easy to refresh.

const KM_PER_MILE: float = 1.609344
const METERS_PER_FOOT: float = 0.3048

## Records among the moments: the summaries mark them with a star, like the history (R27).
const RECORDS: Array[String] = ["route_record", "climb_record", "power_peak", "ftp_raised"]
## A ride with less climbing than this is told as a flat one.
const FLAT_M: float = 10.0

## Per moment kind (as the core's `moment` signal names it, or a variant `_kind()` picks): the
## casual lines, then the plain one. `{…}` are filled in by `line()`.
# i18n-begin
const LINES: Dictionary[String, Array] = {
	"finished":
	[
		[
			"What a ride! {distance} and {climbing} of climbing are done.",
			"All {distance} ridden with {climbing} of climbing. Well ridden!",
		],
		"Ride finished: {distance}, {climbing} of climbing.",
	],
	"finished_flat":
	[
		[
			"What a ride! You rode {distance} in {time}.",
			"That's {time} on the bike and {distance} ridden. Well done!",
		],
		"Ride finished: {distance} in {time}.",
	],
	"route_record":
	[
		[
			"That's your fastest ride on {course} ever, {gain} quicker than before!",
			"New best on {course}! You beat your old time by {gain}.",
		],
		"New personal record on {course}, {gain} faster.",
	],
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
	"step_held":
	[
		[
			"You held that step right on target, nicely done!",
			"That step was spot on the whole way through!",
		],
		"Step completed on target.",
	],
	"ftp_raised":
	[
		[
			"Your FTP is up to {power}, all that training is paying off!",
			"A new FTP of {power}! You're getting stronger every week.",
		],
		"New FTP: {power}.",
	],
	"streak":
	[
		[
			"That's {rides} rides this week, keep it up!",
			"{rides} rides this week already, you're on a roll!",
		],
		"Ride {rides} this week.",
	],
	"week_plan":
	[
		[
			"That's ride {rides} of the {planned} you planned this week, nice work!",
			"{rides} of your {planned} rides this week are done, keep going!",
		],
		"Ride {rides} of {planned} planned this week.",
	],
	"week_plan_done":
	[
		[
			"That's all {planned} rides you planned this week. Well done!",
			"Every ride you planned this week is done, all {planned} of them!",
		],
		"All {planned} planned rides this week done.",
	],
	"week_plan_beyond":
	[
		[
			"That's {rides} rides this week, even more than the {planned} you planned!",
			"Ride {rides} this week puts you beyond your plan of {planned}. Brilliant!",
		],
		"Ride {rides} this week, {planned} planned.",
	],
	"week_load":
	[
		[
			"Your rides this week add up to {stress} TSS. Solid work!",
			"That's {stress} TSS this week so far, a strong week of training!",
		],
		"This week's load: {stress} TSS.",
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
## "off" speaks plainly too, for the summary's highlights); empty for an unknown kind. The
## casual voice picks one of its lines at random, or by `seed` when one is given, so a ride's
## summary says the same each time it opens.
static func line(moment: Dictionary, voice: String, imperial: bool, seed: int = -1) -> String:
	var kind: String = _kind(moment)
	if not LINES.has(kind):
		return ""
	var lines: Array = LINES[kind]
	var text: String
	if voice == "casual":
		var casual: Array = lines[0]
		text = casual.pick_random() if seed < 0 else casual[seed % casual.size()]
	else:
		text = lines[1]
	return TranslationServer.translate(text).format(_values(moment, imperial))


## Whether `moment` is a record, starred in the summaries.
static func is_record(moment: Dictionary) -> bool:
	return moment.get("kind", "") in RECORDS


## The lines for `moment`: some kinds are told differently by their figures.
static func _kind(moment: Dictionary) -> String:
	var kind: String = moment.get("kind", "")
	match kind:
		"finished":
			var climbing: float = moment.get("climbing_m", 0.0)
			return "finished_flat" if climbing < FLAT_M else kind
		"week_plan":
			var rides: int = moment.get("rides", 0)
			var planned: int = moment.get("planned", 0)
			if rides == planned:
				return "week_plan_done"
			return "week_plan_beyond" if rides > planned else kind
	return kind


## The figure a card shows large for `moment`, e.g. "14:02" or "25 km"; empty for none.
static func figure(moment: Dictionary, imperial: bool) -> String:
	match moment.get("kind", ""):
		"climb_topped", "climb_record", "route_record":
			var elapsed: float = moment["elapsed_s"]
			return UiTheme.duration(elapsed)
		"halfway", "distance", "finished":
			var meters: float = moment["distance_m"]
			return distance(meters, imperial)
		"one_to_go":
			return _one(imperial)
		"power_peak", "step_held", "ftp_raised":
			var power: float = moment["power_w"]
			return "%d W" % roundi(power)
	return ""


## `meters` in the rider's units: "32.1 km", whole ones without a decimal ("25 km"), short
## ones in metres or feet ("400 m").
static func distance(meters: float, imperial: bool) -> String:
	if meters < 1000.0:
		return climbing(meters, imperial)
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
	if moment.has("climbing_m"):
		var climbed: float = moment["climbing_m"]
		values["climbing"] = climbing(climbed, imperial)
	if moment.has("rides"):
		values["rides"] = moment["rides"]
	if moment.has("planned"):
		values["planned"] = moment["planned"]
	if moment.has("stress"):
		var stress: float = moment["stress"]
		values["stress"] = roundi(stress)
	if moment.has("course"):
		values["course"] = moment["course"]
	return values
