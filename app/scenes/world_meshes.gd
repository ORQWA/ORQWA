class_name WorldMeshes
extends RefCounted
## Converts the core's shared mesh-array contract into a Godot mesh for world surfaces.


static func from_arrays(arrays: Dictionary) -> ArrayMesh:
	var surface: Array = []
	surface.resize(Mesh.ARRAY_MAX)
	var vertices: PackedVector3Array = arrays["vertices"]
	var normals: PackedVector3Array = arrays["normals"]
	var uvs: PackedVector2Array = arrays["uvs"]
	var colors: PackedColorArray = arrays["colors"]
	var indices: PackedInt32Array = arrays["indices"]
	surface[Mesh.ARRAY_VERTEX] = vertices
	surface[Mesh.ARRAY_NORMAL] = normals
	surface[Mesh.ARRAY_TEX_UV] = uvs
	if not colors.is_empty():
		surface[Mesh.ARRAY_COLOR] = colors
	surface[Mesh.ARRAY_INDEX] = indices
	var mesh: ArrayMesh = ArrayMesh.new()
	if not vertices.is_empty():
		mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, surface)
	return mesh
