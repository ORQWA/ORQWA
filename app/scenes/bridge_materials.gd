class_name BridgeMaterials
extends RefCounted
## Shared bridge materials also cover exposed undersides of road ribbons in kept worlds.


static func configure(material: ShaderMaterial) -> void:
	for channel: String in ["albedo", "normal", "rough"]:
		material.set_shader_parameter(
			"bridge_concrete_" + channel,
			load("res://assets/textures/bridge_concrete_%s.webp" % channel)
		)
	material.set_shader_parameter("concrete_color", Palette.color("structure.concrete"))
	material.set_shader_parameter("steel_color", Palette.color("structure.steel"))
	var steel: Color = Palette.color("structure.steel")
	material.set_shader_parameter("steel_srgb", Vector3(steel.r, steel.g, steel.b))


static func structure() -> ShaderMaterial:
	var material: ShaderMaterial = ShaderMaterial.new()
	material.shader = preload("res://shaders/structure.gdshader")
	configure(material)
	for channel: String in ["albedo", "normal"]:
		material.set_shader_parameter(
			"masonry_" + channel, load("res://assets/textures/masonry_%s.webp" % channel)
		)
	return material
