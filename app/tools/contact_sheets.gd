extends SceneTree
## Contact sheets for reviewing every capture without loading hundreds of full-size images.
## INPUT_DIR is scanned recursively; sheets and a filename index go to CONTACT_DIR.

const TILE: Vector2i = Vector2i(320, 200)
const COLUMNS: int = 4
const PER_SHEET: int = 12


func _initialize() -> void:
	var source: String = OS.get_environment("INPUT_DIR")
	var output: String = OS.get_environment("CONTACT_DIR")
	if source.is_empty() or output.is_empty():
		push_error("INPUT_DIR and CONTACT_DIR are required")
		quit(1)
		return
	DirAccess.make_dir_recursive_absolute(output)
	var files: Array[String] = []
	_collect(source, files)
	files.sort()
	var index: Dictionary = {}
	var page: int = 0
	for first: int in range(0, files.size(), PER_SHEET):
		page += 1
		var sheet: Image = Image.create(TILE.x * COLUMNS, TILE.y * 3, false, Image.FORMAT_RGBA8)
		sheet.fill(Color(0.22, 0.24, 0.27))
		var names: Array[String] = []
		for offset: int in range(mini(PER_SHEET, files.size() - first)):
			var file: String = files[first + offset]
			var image: Image = Image.load_from_file(file)
			if image == null or image.is_empty():
				push_error("Unreadable capture: " + file)
				quit(1)
				return
			var factor: float = minf(
				float(TILE.x) / image.get_width(), float(TILE.y - 8) / image.get_height()
			)
			image.resize(
				maxi(1, roundi(image.get_width() * factor)),
				maxi(1, roundi(image.get_height() * factor)),
				Image.INTERPOLATE_LANCZOS
			)
			image.convert(Image.FORMAT_RGBA8)
			var at: Vector2i = Vector2i((offset % COLUMNS) * TILE.x, (offset / COLUMNS) * TILE.y)
			sheet.blend_rect(image, Rect2i(Vector2i.ZERO, image.get_size()), at)
			names.append(file.trim_prefix(source + "/"))
		var name: String = "sheet-%02d.png" % page
		sheet.save_png(output.path_join(name))
		index[name] = names
		print(name, ": ", ", ".join(names))
	var file: FileAccess = FileAccess.open(output.path_join("index.json"), FileAccess.WRITE)
	file.store_string(JSON.stringify(index, "\t"))
	print("Capture count: ", files.size())
	quit(0)


func _collect(directory: String, files: Array[String]) -> void:
	for name: String in DirAccess.get_files_at(directory):
		if name.get_extension().to_lower() in ["png", "jpg", "webp"]:
			files.append(directory.path_join(name))
	for name: String in DirAccess.get_directories_at(directory):
		_collect(directory.path_join(name), files)
