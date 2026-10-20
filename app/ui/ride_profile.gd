class_name RideProfile
extends VBoxContainer
## The ride settings' Profile tab (R98): the rider's figures, drivetrain and zones, changed
## during the ride as before it. The trainer difficulty rides start with is left out; the
## Ride tab has the ride's own.

## The rider changed their profile: all of it, shaped like `TorqaApp.profile()`.
signal changed(profile: Dictionary)

var _profile: Dictionary = {}
var _form: ProfileForm = ProfileForm.new()
var _zones: ZonesEditor = ZonesEditor.new()


func _init() -> void:
	add_theme_constant_override("separation", 24)
	_form.hide_default_difficulty()
	add_child(_form)
	add_child(_zones)
	_form.changed.connect(_on_changed)
	_zones.changed.connect(_on_changed)


## Shows the rider's `profile` (`TorqaApp.profile()`) without emitting `changed`.
func edit(profile: Dictionary) -> void:
	_profile = profile.duplicate()
	_form.edit(profile)
	var power: PackedFloat64Array = profile.get("power_zones_pct", ZonesEditor.DEFAULT_POWER)
	var heart: PackedFloat64Array = profile.get("heart_rate_zones_pct", ZonesEditor.DEFAULT_HEART)
	_zones.edit(power, heart, _form.ftp(), _form.max_heart_rate())


func _on_changed() -> void:
	_zones.set_bases(_form.ftp(), _form.max_heart_rate())
	_profile.merge(_form.profile(), true)
	_profile["power_zones_pct"] = _zones.power_pct()
	_profile["heart_rate_zones_pct"] = _zones.heart_pct()
	changed.emit(_profile.duplicate())
