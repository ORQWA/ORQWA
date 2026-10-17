class_name AboutPage
extends VBoxContainer
## What Torqa is, on the settings' About tab (R83): its version and the commit it is built
## from, its licence and where its source code is, and the licence notices of everything built
## into the app, each with its full text.

const LOGO: Texture2D = preload("res://assets/brand/torqa-logo-dark.svg")
const WORDMARK: Texture2D = preload("res://assets/brand/torqa-wordmark-dark.svg")

var _version: Label = Label.new()
var _copy: Button = Button.new()
var _engine: Label = Label.new()
var _free: Label = Label.new()
## The parts of the app with a notice, and the selected part's notice.
var _parts: ItemList = ItemList.new()
var _notice: TextEdit = TextEdit.new()
## `TorqaCore.notices()`: `[{part, text}]`.
var _notices: Array = TorqaCore.notices()


func _init() -> void:
	add_theme_constant_override("separation", 12)
	var brand: HBoxContainer = HBoxContainer.new()
	brand.add_theme_constant_override("separation", 12)
	for image: Array in [[LOGO, Vector2(40, 40)], [WORDMARK, Vector2(84, 32)]]:
		var texture: TextureRect = TextureRect.new()
		texture.texture = image[0]
		texture.custom_minimum_size = image[1]
		texture.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		texture.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		brand.add_child(texture)
	add_child(brand)
	var version_row: HBoxContainer = HBoxContainer.new()
	version_row.add_theme_constant_override("separation", 16)
	version_row.add_child(_version)
	_copy.focus_mode = Control.FOCUS_NONE
	_copy.pressed.connect(func() -> void: DisplayServer.clipboard_set(_version_report()))
	version_row.add_child(_copy)
	add_child(version_row)
	_engine.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	_engine.text = "Godot Engine %s" % Engine.get_version_info()["string"]
	_engine.add_theme_color_override("font_color", UiTheme.MUTED)
	add_child(_engine)
	# The link goes below the sentence where a language makes the two too wide for one line.
	var source: HFlowContainer = HFlowContainer.new()
	source.add_theme_constant_override("h_separation", 8)
	source.add_child(_free)
	var link: LinkButton = LinkButton.new()
	link.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	link.text = TorqaCore.source_url()
	link.uri = link.text
	link.focus_mode = Control.FOCUS_NONE
	link.add_theme_color_override("font_color", UiTheme.ACCENT)
	source.add_child(link)
	add_child(source)
	add_child(UiTheme.caption(tr("Licences")))
	var licences: HBoxContainer = HBoxContainer.new()
	licences.add_theme_constant_override("separation", 12)
	licences.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_parts.custom_minimum_size = Vector2(200, 0)
	_parts.focus_mode = Control.FOCUS_NONE
	_parts.item_selected.connect(_show_notice)
	licences.add_child(_parts)
	# Legal texts in their own words: not looked up for a translation, but selectable to copy.
	_notice.auto_translate_mode = Node.AUTO_TRANSLATE_MODE_DISABLED
	_notice.editable = false
	_notice.wrap_mode = TextEdit.LINE_WRAPPING_BOUNDARY
	_notice.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_notice.add_theme_font_size_override("font_size", 13)
	licences.add_child(_notice)
	add_child(licences)
	_translate()
	_show_notice(0)


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		_translate()


func _translate() -> void:
	var commit: String = TorqaCore.commit()
	if commit.is_empty():
		_version.text = tr("Version %s") % TorqaCore.version()
	else:
		_version.text = tr("Version %s, commit %s") % [TorqaCore.version(), commit]
	_copy.text = tr("Copy")
	_copy.tooltip_text = tr("Copy the version, e.g. for a bug report")
	_free.text = tr("Torqa is free software under the GNU General Public License, version 3:")
	var selected: PackedInt32Array = _parts.get_selected_items()
	_parts.clear()
	for notice: Dictionary in _notices:
		var part: String = notice["part"]
		_parts.add_item(_title(part))
	_parts.select(selected[0] if not selected.is_empty() else 0)


## The parts' titles, by the core's names for them; names stay as they are.
func _title(part: String) -> String:
	match part:
		"torqa":
			return "Torqa"
		"godot":
			return "Godot Engine"
		"ffmpeg":
			return "FFmpeg"
		"crates":
			return tr("Rust libraries")
		"assets":
			return tr("Models and data")
	return part


func _show_notice(index: int) -> void:
	var notice: Dictionary = _notices[index]
	var text: String = notice["text"]
	_notice.text = text
	_notice.scroll_vertical = 0


## What the Copy button puts on the clipboard: Torqa's version and commit, Godot's and the
## system's, as a bug report asks for them.
func _version_report() -> String:
	var commit: String = TorqaCore.commit()
	var torqa: String = TorqaCore.version() + ("" if commit.is_empty() else " (%s)" % commit)
	return (
		"Torqa %s, Godot %s, %s %s"
		% [torqa, Engine.get_version_info()["string"], OS.get_name(), OS.get_version()]
	)
