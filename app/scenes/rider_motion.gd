class_name RiderMotion
extends RefCounted
## How rider and bike move on the road besides pedalling (R46, R91): the bike pitches with the
## road and leans into bends as speed and curve ask, without jolts when the grade or the curve
## steps; the rider stands up on the pedals on steep climbs with effort and in sprints, and sits
## down again only once that has passed for a moment; standing, the bike rocks under the rider
## with each stroke; seated, the upper body sways more the harder they push.
## Every motion is a critically damped spring (it settles as fast as it can without
## overshooting), stepped with the frame time; `RiderAvatar` and `RideWorld` read the results.

## How quickly pitch, lean and standing follow what the road and the rider ask (rad/s): about
## the time a real bike takes to settle into a change of grade, a bend, standing up.
const PITCH_RESPONSE: float = 3.0
const LEAN_RESPONSE: float = 2.5
const STAND_RESPONSE: float = 4.0
## Standing on the pedals: on climbs this steep (percent) with this much effort (W/kg), and in
## sprints at this effort on any grade.
const STAND_GRADE: float = 7.0
const STAND_CLIMB_EFFORT: float = 3.2
const STAND_SPRINT_EFFORT: float = 6.5
## How long the reason to stand up, or to sit down, has to last first (seconds).
const STAND_AFTER: float = 0.8
const SIT_AFTER: float = 1.5
## Standing, the bike rocks under the rider with each stroke (radians, 7° at 60 rpm, less as
## the cadence rises); seated it rocks a little with effort (0.6° at 4 W/kg).
const ROCK_STANDING: float = 0.12
const ROCK_SEATED: float = 0.01
## The upper body's sway about the hips with each stroke, seated: at rest and added at 4 W/kg
## (radians, 0.7° and 1.4°).
const SWAY_AT_REST: float = 0.012
const SWAY_WITH_EFFORT: float = 0.024
## The longest step one spring update takes; longer frames are split (stability).
const MAX_STEP: float = 1.0 / 60.0

## The bike's pitch on the road (radians, nose up positive) and its lean into the bend
## (radians, to the right positive), both smoothed.
var pitch: float = 0.0
var lean: float = 0.0
## 0 seated, 1 standing on the pedals, smoothly in between.
var standing: float = 0.0
## The bike's rock under the rider at this moment (radians, to the right positive).
var rock: float = 0.0
## How far the upper body sways with each stroke (radians).
var sway: float = SWAY_AT_REST

var _pitch_rate: float = 0.0
var _lean_rate: float = 0.0
var _standing_rate: float = 0.0
var _wants_to_stand: bool = false
## How long the current reason to stand up or sit down has lasted.
var _reason_for: float = 0.0
var _placed: bool = false


## Advances by `delta` seconds: `grade` in percent, `lean_target` as the bend asks for it
## (radians), `effort` in W/kg, `cadence_rpm`, and `crank_angle` (radians) for the rock.
func step(
	delta: float,
	grade: float,
	lean_target: float,
	effort: float,
	cadence_rpm: float,
	crank_angle: float
) -> void:
	var pitch_target: float = atan(grade / 100.0)
	if not _placed:
		# The first step after a jump starts at rest where the road is.
		pitch = pitch_target
		lean = lean_target
		_placed = true
	_decide_standing(delta, grade, effort, cadence_rpm)
	var left: float = delta
	while left > 0.0:
		var dt: float = minf(left, MAX_STEP)
		var p: Vector2 = spring(pitch, _pitch_rate, pitch_target, PITCH_RESPONSE, dt)
		pitch = p.x
		_pitch_rate = p.y
		var l: Vector2 = spring(lean, _lean_rate, lean_target, LEAN_RESPONSE, dt)
		lean = l.x
		_lean_rate = l.y
		var s: Vector2 = spring(
			standing, _standing_rate, 1.0 if _wants_to_stand else 0.0, STAND_RESPONSE, dt
		)
		standing = clampf(s.x, 0.0, 1.0)
		_standing_rate = s.y
		left -= dt
	var push: float = clampf(effort / 4.0, 0.0, 1.5)
	# Standing, the rock is widest at low cadence: each stroke has time to throw the bike over.
	var rock_size: float = lerpf(
		ROCK_SEATED * push,
		ROCK_STANDING * clampf(60.0 / maxf(cadence_rpm, 30.0), 0.5, 1.2),
		standing
	)
	rock = rock_size * sin(crank_angle) if cadence_rpm > 1.0 else 0.0
	sway = (SWAY_AT_REST + SWAY_WITH_EFFORT * push) * (1.0 - standing)


## Places the motion at rest where the road is with the next step, e.g. after a jump.
func reset() -> void:
	_placed = false
	_pitch_rate = 0.0
	_lean_rate = 0.0
	_standing_rate = 0.0
	standing = 0.0
	_wants_to_stand = false
	_reason_for = 0.0


## One step of a critically damped spring from `value` (moving at `rate`) towards `target`,
## `response` rad/s: returns the new value and rate.
static func spring(value: float, rate: float, target: float, response: float, dt: float) -> Vector2:
	var accel: float = response * response * (target - value) - 2.0 * response * rate
	var new_rate: float = rate + accel * dt
	return Vector2(value + new_rate * dt, new_rate)


func _decide_standing(delta: float, grade: float, effort: float, cadence_rpm: float) -> void:
	var pedalling: bool = cadence_rpm > 20.0
	var climb: bool = grade >= STAND_GRADE and effort >= STAND_CLIMB_EFFORT
	var sprint: bool = effort >= STAND_SPRINT_EFFORT
	# Sitting down takes a clear drop below the reason to stand, so it does not flicker.
	var stay: bool = (
		grade >= STAND_GRADE * 0.8 and effort >= STAND_CLIMB_EFFORT * 0.85
		or effort >= STAND_SPRINT_EFFORT * 0.8
	)
	var reason: bool = pedalling and (climb or sprint or (_wants_to_stand and stay))
	if reason == _wants_to_stand:
		_reason_for = 0.0
		return
	_reason_for += delta
	if _reason_for >= (STAND_AFTER if reason else SIT_AFTER):
		_wants_to_stand = reason
		_reason_for = 0.0
