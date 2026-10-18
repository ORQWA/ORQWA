extends SceneTree
## Headless check of the riders on their bikes (R46): over a full crank turn each rider's legs
## reach the pedals without stretching, the knees bend forward within a natural range (almost
## straight at the bottom, not cramped at the top), and cranks and pedals turn forward together.
## Standing on the pedals, the hips rise and move forward while the hands stay on the hoods and
## the legs still reach; coasting, the cranks come level. `RiderMotion` stands up on steep
## climbs with effort and sits down again, pitches with the road without overshooting, and the
## bike rocks only while pedalling.
## Run: godot --headless --path app -s res://tests/rider_smoke.gd

const STEPS: int = 36
## Bike fit: the knee's bend at the bottom of the stroke and at most, in degrees.
const BEND_AT_BOTTOM: Vector2 = Vector2(20.0, 40.0)
const BEND_AT_MOST: float = 125.0
## How far a joint may be from where the leg part before it ends, in metres.
const JOINT_TOLERANCE: float = 0.005


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var fits: JSON = load(RiderAvatar.DIRECTORY + "riders.json")
	for rider: String in RiderAvatar.RIDERS:
		var avatar: RiderAvatar = RiderAvatar.new()
		avatar.rider = rider
		root.add_child(avatar)
		var fit: Dictionary = fits.data[rider]
		var thigh: float = fit["thigh"]
		var shin: float = fit["shin"]
		var crank: float = fit["crank"]
		var bottom_bracket: Vector3 = _node(avatar, "crankset").position
		var least: float = 180.0
		var most: float = 0.0
		for step: int in range(STEPS):
			# At 60 rpm a turn takes a second: each step turns the cranks by 10°.
			avatar.animate(1.0 / STEPS, 60.0, 30.0)
			var pedal: Vector3 = _node(avatar, "pedal_r").position
			if step == 0:
				_check(
					pedal.y < bottom_bracket.y and pedal.z < bottom_bracket.z,
					"%s: the right pedal goes forward and down from the front: %s" % [rider, pedal]
				)
			var cranks: Node3D = _node(avatar, "crankset")
			var crank_end: Vector3 = cranks.basis * Vector3(0.0, 0.0, -crank)
			var to_pedal: Vector3 = pedal - bottom_bracket
			_check(
				crank_end.distance_to(Vector3(0.0, to_pedal.y, to_pedal.z)) < JOINT_TOLERANCE,
				"%s: the right crank points at the right pedal at step %d" % [rider, step]
			)
			for side: String in ["l", "r"]:
				var hip: Node3D = _node(avatar, "thigh_" + side)
				var knee: Node3D = _node(avatar, "shin_" + side)
				var ankle: Node3D = _node(avatar, "foot_" + side)
				var where: String = "%s, %s leg, step %d" % [rider, side, step]
				_check(
					(
						(hip.position - hip.basis.y * thigh).distance_to(knee.position)
						< JOINT_TOLERANCE
					),
					"%s: the thigh ends at the knee" % where
				)
				_check(
					(
						(knee.position - knee.basis.y * shin).distance_to(ankle.position)
						< JOINT_TOLERANCE
					),
					"%s: the shin reaches the ankle over the pedal" % where
				)
				var line: Vector3 = (ankle.position - hip.position).normalized()
				var out: Vector3 = knee.position - hip.position
				out -= line * out.dot(line)
				_check(out.z < 0.0, "%s: the knee bends forward" % where)
				var bend: float = rad_to_deg(
					(knee.position - hip.position).angle_to(ankle.position - knee.position)
				)
				least = minf(least, bend)
				most = maxf(most, bend)
		_check(
			least > BEND_AT_BOTTOM.x and least < BEND_AT_BOTTOM.y,
			"%s: the knee is almost straight at the bottom: %.0f°" % [rider, least]
		)
		_check(most < BEND_AT_MOST, "%s: the knee is not cramped at the top: %.0f°" % [rider, most])
		_standing(avatar, rider, thigh + shin)
		_coasting(avatar, rider)
		avatar.show_rider(false)
		_check(
			not _node(avatar, "body").visible and _node(avatar, "frame").visible,
			"%s: hiding the rider keeps the bike" % rider
		)
		avatar.free()
	_motion()
	print("RIDER SMOKE TEST PASSED")
	quit(0)


## Standing: hips higher and further forward than seated, the legs reaching all round the stroke.
func _standing(avatar: RiderAvatar, rider: String, leg: float) -> void:
	var seated: RiderMotion = RiderMotion.new()
	avatar.animate(0.0, 60.0, 30.0, seated)
	var seated_hip: Vector3 = _node(avatar, "thigh_r").position
	var standing: RiderMotion = RiderMotion.new()
	standing.standing = 1.0
	var lowest: float = INF
	for step: int in range(STEPS):
		avatar.animate(1.0 / STEPS, 60.0, 30.0, standing)
		var hip: Vector3 = _node(avatar, "thigh_r").position
		lowest = minf(lowest, hip.y)
		for side: String in ["l", "r"]:
			var reach: float = _node(avatar, "thigh_" + side).position.distance_to(
				_node(avatar, "foot_" + side).position
			)
			_check(
				reach <= leg + 0.001, "%s standing: the %s leg reaches its pedal" % [rider, side]
			)
		_check(hip.z < seated_hip.z - 0.05, "%s standing: the hips move forward" % rider)
	_check(lowest > seated_hip.y + 0.02, "%s standing: the hips rise: %.3f" % [rider, lowest])


## Coasting, the cranks come level.
func _coasting(avatar: RiderAvatar, rider: String) -> void:
	for _step: int in range(180):
		avatar.animate(1.0 / 60.0, 0.0, 30.0, RiderMotion.new())
	var angle: float = avatar.crank_angle()
	_check(
		absf(sin(angle)) < 0.05,
		"%s coasting: the cranks are level: %.0f°" % [rider, rad_to_deg(angle)]
	)


## The motion over a ride: up a steep climb at effort and over, seated on the flat.
func _motion() -> void:
	var motion: RiderMotion = RiderMotion.new()
	var target: float = atan(0.1)
	var most: float = 0.0
	for _step: int in range(240):
		motion.step(1.0 / 60.0, 10.0, 0.0, 4.0, 70.0, 0.0)
		most = maxf(most, motion.pitch)
	_check(motion.standing > 0.95, "standing up on a steep climb at effort")
	_check(most <= target + 0.0001, "the pitch settles without overshooting: %.4f" % most)
	_check(absf(motion.pitch - target) < 0.005, "and reaches the grade")
	for _step: int in range(240):
		motion.step(1.0 / 60.0, 0.0, 0.0, 2.0, 90.0, 0.0)
	_check(motion.standing < 0.05, "seated again on the flat")
	motion.step(1.0 / 60.0, 0.0, 0.0, 0.0, 0.0, 1.0)
	_check(motion.rock == 0.0, "no rock while coasting")
	var brief: RiderMotion = RiderMotion.new()
	for _step: int in range(30):
		brief.step(1.0 / 60.0, 10.0, 0.0, 4.0, 70.0, 0.0)
	_check(brief.standing == 0.0, "a moment's effort does not stand the rider up")


static func _node(avatar: RiderAvatar, part: String) -> Node3D:
	return avatar.find_child(part, true, false)


func _check(condition: bool, what: String) -> void:
	if not condition:
		push_error("RIDER SMOKE TEST FAILED: " + what)
		quit(1)
