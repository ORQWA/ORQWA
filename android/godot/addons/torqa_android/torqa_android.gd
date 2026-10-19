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

	const VERIFIER_VERSION: String = "res://addons/torqa_android/verifier_version.txt"
	const VERIFIER_REPOSITORY: String = (
		"https://github.com/rustls/rustls-platform-verifier/raw/maven-archive/"
		+ "android-release-support/maven/"
	)

	func _get_name() -> String:
		return "TorqaAndroid"

	func _supports_platform(platform: EditorExportPlatform) -> bool:
		return platform is EditorExportPlatformAndroid

	func _get_android_libraries(_platform: EditorExportPlatform, _debug: bool) -> PackedStringArray:
		return PackedStringArray(["torqa_android/bin/torqa-android-release.aar"])

	# HTTPS's certificate checks through Android: rustls-platform-verifier's Java part, in the
	# version Cargo.lock names (written beside this script by the build), from its own archive.
	func _get_android_dependencies(
		_platform: EditorExportPlatform, _debug: bool
	) -> PackedStringArray:
		var version: String = FileAccess.get_file_as_string(VERIFIER_VERSION).strip_edges()
		return PackedStringArray(["org.rustls:rustls-platform-verifier:%s" % version])

	func _get_android_dependencies_maven_repos(
		_platform: EditorExportPlatform, _debug: bool
	) -> PackedStringArray:
		return PackedStringArray([VERIFIER_REPOSITORY])
