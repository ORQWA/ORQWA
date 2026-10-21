class_name TreeLodMesh
extends RefCounted
## Authored foliage LODs: simplification cannot collapse disconnected cards into a canopy.
## Godot chooses the index level by projected error, including in shadow passes.

const ERRORS: Array[float] = [0.08, 0.3]


static func combine(close: ArrayMesh, levels: Array[ArrayMesh]) -> ArrayMesh:
	var result: ArrayMesh = ArrayMesh.new()
	for surface: int in close.get_surface_count():
		var tool: SurfaceTool = SurfaceTool.new()
		tool.begin(Mesh.PRIMITIVE_TRIANGLES)
		tool.append_from(close, surface, Transform3D.IDENTITY)
		var counts: Array[int] = [_index_count(close, surface)]
		var imported: Material = close.surface_get_material(surface)
		for level: ArrayMesh in levels:
			var other: int = _surface_named(level, imported.resource_name)
			if other >= 0:
				tool.append_from(level, other, Transform3D.IDENTITY)
			counts.append(_index_count(level, other) if other >= 0 else 0)
		tool.index()
		var arrays: Array = tool.commit_to_arrays()
		var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
		arrays[Mesh.ARRAY_INDEX] = indices.slice(0, counts[0])
		var lods: Dictionary = {}
		var offset: int = counts[0]
		for index: int in levels.size():
			# Tiny details such as coconuts may disappear; a collapsed triangle is a valid LOD.
			lods[ERRORS[index]] = (
				indices.slice(offset, offset + counts[index + 1])
				if counts[index + 1] > 0
				else PackedInt32Array([0, 0, 0])
			)
			offset += counts[index + 1]
		result.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, [], lods)
		result.surface_set_material(surface, imported)
	return result


static func _index_count(mesh: ArrayMesh, surface: int) -> int:
	var count: int = mesh.surface_get_array_index_len(surface)
	return count if count > 0 else mesh.surface_get_array_len(surface)


static func _surface_named(mesh: ArrayMesh, name: String) -> int:
	for surface: int in mesh.get_surface_count():
		if mesh.surface_get_material(surface).resource_name == name:
			return surface
	return -1
