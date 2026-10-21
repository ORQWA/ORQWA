extends SceneTree
## Publish reviewed capture outputs at the stable filenames used by the wiki and README.
## Run after the app, details and route batches, with REFRESH_ROOT pointing at their folder.

const SHOTS: Dictionary[String, String] = {
	"start-gallery": "app/courses.png",
	"start-gallery-focus": "app/courses-focus.png",
	"start-gallery-filters": "app/courses-filters.png",
	"course-detail": "app/course.png",
	"course-edit": "app/course-edit.png",
	"start-profile": "app/profile.png",
	"rider-settings": "app/zones.png",
	"start-devices": "app/devices.png",
	"start-devices-remote": "app/devices-remote.png",
	"start-devices-remote-buttons": "app/devices-remote-buttons.png",
	"start-analysis": "app/analysis.png",
	"start-analysis-fitness": "app/analysis-fitness.png",
	"start-analysis-weekly": "app/analysis-weekly.png",
	"start-analysis-progress": "app/analysis-progress.png",
	"account-dialog": "app/account-dialog.png",
	"account-dialog-strava": "app/account-dialog-strava.png",
	"settings-graphics": "app/settings-graphics.png",
	"settings-courses": "app/settings-courses.png",
	"settings-about": "app/settings-about.png",
	"ride-0": "app/ride.jpg",
	"ride-1": "app/ride-first-person.jpg",
	"ride-2": "app/ride-overview.jpg",
	"ride-settings": "app/ride-settings.jpg",
	"ride-profile": "app/ride-profile.png",
	"ride-devices": "issues/ride-devices.jpg",
	"hud-settings_dialog": "app/hud-editor.jpg",
	"summary": "app/summary.png",
	"summary-curve": "app/summary-curve.png",
	"upload-menu": "app/upload.png",
	"history": "app/history.png",
	"start-workouts": "workouts/tab.png",
	"start-structured": "workouts/structured-tab.png",
	"structured": "workouts/structured-ride.png",
	"start-ftp-test": "workouts/ftp-test.png",
	"start-ftp-test-20": "workouts/ftp-test-20.png",
	"workout-editor": "workouts/editor.png",
	"workout": "workouts/heart-rate-hold.png",
	"overlay": "overlay/overlay.png",
	"error-details": "app/error-details.png",
	"error-details-de": "app/error-details-de.png",
	"planned": "workouts/planned.png",
	"planned-de": "issues/planned-de.png",
	"planned-nothing": "issues/planned-nothing.png",
	"planned-offline": "issues/planned-offline.png",
	"devices-readings": "issues/devices-readings.png",
	"devices-readings-de": "issues/devices-readings-de.png",
	"devices-failed": "issues/devices-failed.png",
	"devices-failed-de": "issues/devices-failed-de.png",
}
const WORLD_SHOTS: Dictionary[String, String] = {
	"lake-chase": "ride-lake.jpg",
	"city-street": "issues/urban-street-after.jpg",
	"city-overview": "issues/urban-overview-after.jpg",
	"city-benches": "issues/urban-benches.jpg",
	"village-chase": "issues/urban-rural-after.jpg",
	"hairpin-chase": "issues/forest-gurten-after.jpg",
	"forest-floor": "issues/fern-forest.jpg",
}


func _initialize() -> void:
	var source: String = OS.get_environment("REFRESH_ROOT")
	var target: String = ProjectSettings.globalize_path("res://../docs/images")
	if source.is_empty():
		push_error("REFRESH_ROOT is required")
		quit(1)
		return
	for name: String in SHOTS:
		if not _copy(source.path_join("app/" + name + ".png"), target.path_join(SHOTS[name])):
			quit(1)
			return
	for file: String in DirAccess.get_files_at(source.path_join("views")):
		if not file.ends_with(".png"):
			continue
		var name: String = file.get_basename()
		var input: String = source.path_join("views/" + file)
		if not _copy(input, target.path_join("views/" + name + ".jpg")):
			quit(1)
			return
		if WORLD_SHOTS.has(name) and not _copy(input, target.path_join(WORLD_SHOTS[name])):
			quit(1)
			return
	if not _copy(
		source.path_join("app/video-course.png"), target.path_join("readme/video-course.jpg")
	):
		quit(1)
		return
	var overlay: Image = Image.load_from_file(source.path_join("app/overlay.png"))
	var backdrop: Image = Image.create(800, 720, false, Image.FORMAT_RGBA8)
	backdrop.fill(Color(0.12, 0.14, 0.18))
	backdrop.blend_rect(overlay, Rect2i(Vector2i.ZERO, overlay.get_size()), Vector2i(200, 45))
	if backdrop.save_jpg(target.path_join("readme/overlay.jpg"), 0.88) != OK:
		quit(1)
		return
	print("Exported app and world screenshots to ", target)
	quit(0)


func _copy(source: String, target: String) -> bool:
	var image: Image = Image.load_from_file(source)
	if image == null or image.is_empty():
		push_error("Missing screenshot: " + source)
		return false
	DirAccess.make_dir_recursive_absolute(target.get_base_dir())
	var result: Error = (
		image.save_jpg(target, 0.88) if target.ends_with(".jpg") else image.save_png(target)
	)
	if result != OK:
		push_error("Cannot export screenshot: " + target)
	return result == OK
