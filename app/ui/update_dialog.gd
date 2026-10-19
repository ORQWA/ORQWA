class_name UpdateDialog
extends ConfirmationDialog
## Offers a newer Torqa (R99): on Windows it downloads the installer, closes Torqa and installs
## it, Torqa starting again when done; elsewhere it opens the release page. Shown only outside
## rides; the rider's rides and settings stay as they are.

var _torqa: TorqaApp
var _version: String = ""
var _page: String = ""
var _installable: bool = false
## The installer is being downloaded.
var _downloading: bool = false


func _init() -> void:
	# Not exclusive: the settings may be open when the rider asked for the check there.
	exclusive = false
	dialog_hide_on_ok = false
	dialog_autowrap = true
	min_size = Vector2i(520, 0)
	confirmed.connect(_on_confirmed)
	custom_action.connect(_on_custom_action)
	add_button(tr("What's new"), false, "notes")


func _ready() -> void:
	theme = UiTheme.build()
	title = tr("Update available")


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	_torqa.update_downloaded.connect(_on_downloaded)
	_torqa.update_failed.connect(_on_failed)


## Shows the newer `version`, its release `page`, and whether Torqa can install it itself.
func offer(version: String, page: String, installable: bool) -> void:
	_version = version
	_page = page
	_installable = installable
	_downloading = false
	_show_offer()
	UiScale.popup(self, Vector2i(560, 220))


func _show_offer() -> void:
	var text: String = tr("Torqa %s is available; you have %s.") % [_version, TorqaCore.version()]
	if _installable:
		text += " " + tr("Torqa downloads it, closes and installs it, then starts again.")
		text += " " + tr("Your rides and settings stay as they are.")
		ok_button_text = tr("Update now")
	else:
		text += " " + tr("Download it from its release page.")
		ok_button_text = tr("Open the release page")
	dialog_text = text
	cancel_button_text = tr("Later")
	get_ok_button().disabled = false


func _on_confirmed() -> void:
	if not _installable:
		OS.shell_open(_page)
		hide()
		return
	if _torqa == null or not _torqa.download_update():
		return
	_downloading = true
	get_ok_button().disabled = true
	dialog_text = tr("Downloading Torqa %s…") % _version


func _on_custom_action(action: StringName) -> void:
	if action == &"notes":
		OS.shell_open(_page)


func _on_downloaded() -> void:
	if not _downloading:
		return
	dialog_text = tr("Installing Torqa %s…") % _version
	# The installer replaces Torqa's files only once it has closed.
	if _torqa.install_update():
		get_tree().quit()
		return
	_downloading = false
	_show_offer()


func _on_failed(message: String) -> void:
	if not _downloading:
		return
	_downloading = false
	_show_offer()
	dialog_text = message + "\n\n" + dialog_text
