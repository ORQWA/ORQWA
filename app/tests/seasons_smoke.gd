extends SceneTree
## Season selection, compatibility with earlier ride options, and independent conditions (R154).

var _failed: bool = false


func _initialize() -> void:
	_run.call_deferred()


func _run() -> void:
	var options: RideOptions = RideOptions.new()
	root.add_child(options)
	options.set_options({})
	var selected: String = options.options()["season"]
	_check(selected == "Summer", "old options default to Summer")
	for season: String in OrqwaApp.seasons():
		options.set_options({"season": season, "weather": "Rain", "time": "Evening"})
		selected = options.options()["season"]
		var weather: String = options.options()["weather"]
		var time: String = options.options()["time"]
		_check(selected == season, "each available season can be selected")
		_check(weather == "Rain", "season preserves weather")
		_check(time == "Evening", "season preserves time of day")
		options.show_option_groups(false, true)
		var control: OptionButton = options.get("_season")
		_check(not control.visible, "season is hidden for video rides")
		selected = options.options()["season"]
		_check(selected == season, "hidden season retains selection")
	options.free()
	_small_plants()


	_winter()
	if not _failed:
		print("SEASONS SMOKE TEST PASSED")
	quit(1 if _failed else 0)


func _check(ok: bool, message: String) -> void:
	if not ok:
		_failed = true
		push_error(message)


func _small_plants() -> void:
	var original: PackedFloat32Array = PackedFloat32Array()
	for index: int in range(100):
		original.append_array(PackedFloat32Array([1, 0, 0, float(index), 0, 1, 0, 700, 0, 0, 1, 5]))
	for season: String in OrqwaApp.seasons():
		var plants: Dictionary = OrqwaApp.small_plants_for_season(
			PackedFloat32Array(), original, season
		)
		var grass: PackedFloat32Array = plants["grass"]
		var flowers: PackedFloat32Array = plants["flowers"]
		_check(
			grass.size() + flowers.size() == original.size(), "season preserves all plant positions"
		)
		if season == "Summer":
			_check(grass.size() > original.size() * 0.75, "summer is predominantly grass")
		elif season == "Autumn":
			_check(flowers.is_empty(), "autumn has no meadow flowers")
		elif season == "Spring":
			_check(flowers == original, "spring restores all meadow flowers")


func _winter() -> void:
	var world: RideWorld = (load("res://scenes/world.tscn") as PackedScene).instantiate()
	root.add_child(world)
	world.apply_options({"season": "Winter", "weather": "Clear", "time": "Midday", "camera": 0})
	var needles: ShaderMaterial = VegetationModels.material("needles")
	var strength: float = needles.get_shader_parameter("winter_strength")
	_check(strength == 1.0, "winter reaches existing evergreen materials")
	var broadleaf: bool = needles.get_shader_parameter("winter_broadleaf")
	_check(not broadleaf, "winter keeps evergreen needles dense")
	var road: ShaderMaterial = world.get("_road_material")
	strength = road.get_shader_parameter("winter_strength")
	_check(strength == 1.0, "winter reaches street snow and roadside grass")
	var banks: WinterSnowbanks = world.get_node("WinterSnowbanks") as WinterSnowbanks
	_check(banks.visible, "winter enables the disposable snowbank layer")
	var terrain: ShaderMaterial = world.get("_terrain_material")
	strength = terrain.get_shader_parameter("winter_strength")
	_check(strength == 1.0, "winter enables the raised natural-ground blanket")
	world.apply_options({"season": "Summer", "weather": "Rain", "time": "Evening", "camera": 0})
	strength = needles.get_shader_parameter("winter_strength")
	_check(strength == 0.0, "summer clears accumulated snow")
	_check(not banks.visible, "summer hides roadside snow banks")
	strength = road.get_shader_parameter("winter_strength")
	_check(strength == 0.0, "summer restores road verges too")
	strength = terrain.get_shader_parameter("winter_strength")
	_check(strength == 0.0, "summer restores the original ground height")
	var snow_line: float = terrain.get_shader_parameter("snow_line")
	_check(snow_line == 2400.0, "summer restores the high snow line")
	world.free()
