class_name PickedFile
extends RefCounted
## A file the rider picked, as Torqa's core can open or write it. On Android the file pickers
## hand over a content address (`content://…`) that only Android's own file access reads and
## writes (ADR 0024): a picked file is copied into Torqa's folder first, and one to save is
## written there by the core, then copied to the address. Elsewhere paths stay as they are.

const COPIES: String = "user://picked"


## The path to give the core for `picked`; `picked` itself if an Android file cannot be copied,
## so the core reports it as unreadable as before.
static func local_path(picked: String) -> String:
	if not picked.begins_with("content://"):
		return picked
	var data: PackedByteArray = FileAccess.get_file_as_bytes(picked)
	if data.is_empty():
		return picked
	DirAccess.make_dir_recursive_absolute(COPIES)
	var copy: String = COPIES.path_join(name_of(picked))
	var file: FileAccess = FileAccess.open(copy, FileAccess.WRITE)
	if file == null:
		return picked
	file.store_buffer(data)
	file.close()
	return ProjectSettings.globalize_path(copy)


## Where the core should write a file the rider wants saved at `picked`; then [method save] it.
static func save_path(picked: String) -> String:
	if not picked.begins_with("content://"):
		return picked
	DirAccess.make_dir_recursive_absolute(COPIES)
	return ProjectSettings.globalize_path(COPIES.path_join(name_of(picked)))


## Copies what the core wrote at `written` (from [method save_path]) to `picked`; whether that
## worked. Nothing to copy outside Android.
static func save(written: String, picked: String) -> bool:
	if written == picked:
		return true
	var data: PackedByteArray = FileAccess.get_file_as_bytes(written)
	var file: FileAccess = FileAccess.open(picked, FileAccess.WRITE)
	if data.is_empty() or file == null:
		return false
	file.store_buffer(data)
	file.close()
	DirAccess.remove_absolute(written)
	return true


## How to name `picked` to the rider: the file's own name rather than a content address, which
## ends in the file's path, encoded (…%3ADownload%2FRide.gpx).
static func name_of(picked: String) -> String:
	if not picked.begins_with("content://"):
		return picked
	var file_name: String = picked.uri_decode().get_file()
	return file_name if not file_name.is_empty() else "picked"
