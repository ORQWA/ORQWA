class_name PickedFile
extends RefCounted
## A file the rider picked, as Torqa's core can open it. On Android the file picker hands over a
## content address (`content://…`) that only Android's own file access reads (ADR 0024): the
## file is copied into Torqa's folder first, under its own name. Elsewhere the path stays as it is.

const COPIES: String = "user://picked"


## The path to give the core for `picked`; `picked` itself if an Android file cannot be copied,
## so the core reports it as unreadable as before.
static func local_path(picked: String) -> String:
	if not picked.begins_with("content://"):
		return picked
	var data: PackedByteArray = FileAccess.get_file_as_bytes(picked)
	if data.is_empty():
		return picked
	# The address ends in the file's own path, encoded: …%3ADownload%2FRide.gpx.
	var file_name: String = picked.uri_decode().get_file()
	DirAccess.make_dir_recursive_absolute(COPIES)
	var copy: String = COPIES.path_join(file_name if not file_name.is_empty() else "picked")
	var file: FileAccess = FileAccess.open(copy, FileAccess.WRITE)
	if file == null:
		return picked
	file.store_buffer(data)
	file.close()
	return ProjectSettings.globalize_path(copy)
