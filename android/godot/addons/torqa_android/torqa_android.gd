@tool
extends EditorPlugin
## Puts Torqa's Android plugin (android/plugin, built by scripts/build-android.sh) into Android
## exports (ADR 0024). Copied into the Android build's copy of the project only.

var _export: TorqaAndroidExport


func _enter_tree() -> void:
	_export = TorqaAndroidExport.new()
	add_export_plugin(_export)


func _exit_tree() -> void:
	remove_export_plugin(_export)
	_export = null


class TorqaAndroidExport:
	extends EditorExportPlugin

	func _get_name() -> String:
		return "TorqaAndroid"

	func _supports_platform(platform: EditorExportPlatform) -> bool:
		return platform is EditorExportPlatformAndroid

	func _get_android_libraries(_platform: EditorExportPlatform, _debug: bool) -> PackedStringArray:
		return PackedStringArray(["torqa_android/bin/torqa-android-release.aar"])
