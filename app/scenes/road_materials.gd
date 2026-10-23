class_name RoadMaterials
extends RefCounted
## Shared bindings for the world's road shader and the road-profile review scene.


static func configure(material: ShaderMaterial, shoulders: bool) -> void:
	material.shader = preload("res://shaders/road.gdshader")
	BridgeMaterials.configure(material)
	for texture_name: String in ["asphalt", "gravel", "grass", "dirt", "concrete", "paving"]:
		for channel: String in ["albedo", "normal"]:
			material.set_shader_parameter(
				texture_name + "_" + channel,
				load("res://assets/textures/%s_%s.webp" % [texture_name, channel])
			)
	material.set_shader_parameter("asphalt_rough", load("res://assets/textures/asphalt_rough.webp"))
	material.set_shader_parameter("marking_color", Palette.color("road.marking"))
	material.set_shader_parameter("shoulders", shoulders)
