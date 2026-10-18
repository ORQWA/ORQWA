class_name RemoteDialog
extends AcceptDialog
## The phone remote (#228): the QR code and address a phone in the same network opens in its
## browser, the port it listens on, and a new address that locks out the phones paired before.
## Opened from the code button beside the phone remote's row under Devices. The page on the
## phone speaks the rider's language: its texts go to the core from here.

## Pixels per module of the QR code: large enough to scan off a screen at arm's length.
const MODULE_PIXELS: int = 6
## Light modules around the code, as QR readers need.
const QUIET_ZONE: int = 4
const MIN_PORT: int = 1024
const MAX_PORT: int = 65535
# i18n-begin
## What the phone's page says, by the core's key; the actions it offers are named as for a
## shifter's buttons ([constant ButtonDialog.ACTION_LABELS]).
const PAGE_TEXTS: Dictionary[String, String] = {
	"pause": "Pause",
	"resume": "Resume",
	"title": "Torqa remote",
	"gear": "Gear",
	"paused": "Paused",
	"not_riding": "Start a ride in Torqa",
	"reconnecting": "Reconnecting to Torqa…",
	"old_link": "This address no longer works: scan the new code in Torqa.",
	"keep_awake": "Keep this screen on while you ride: turn off its auto-lock.",
}
# i18n-end

var _torqa: TorqaApp
var _code: TextureRect = TextureRect.new()
var _address: LineEdit = LineEdit.new()
var _no_network: Label = Label.new()
var _port: SpinBox = SpinBox.new()
var _renew: Button = Button.new()


## The English text of the page's `key`, "" for a key it has none for.
static func page_text(key: String) -> String:
	return PAGE_TEXTS.get(key, ButtonDialog.ACTION_LABELS.get(key, ""))


func bind(torqa: TorqaApp) -> void:
	_torqa = torqa
	_send_texts()


## Opens the dialog with the remote's code and address as they are now.
func open() -> void:
	refresh()
	# As tall as its content: the code, the address and the port.
	popup_centered_clamped(Vector2i(560, 0), 0.9)


## Shows the remote as it is now: its code and address while it is on.
func refresh() -> void:
	if _torqa == null:
		return
	var remote: Dictionary = _torqa.phone_remote()
	var on: bool = remote.get("on", false)
	var address: String = remote.get("address", "")
	var port: int = remote.get("port", 0)
	_port.set_value_no_signal(port)
	_address.text = address
	_address.visible = not address.is_empty()
	_no_network.visible = on and address.is_empty()
	_code.texture = _code_texture()
	_code.visible = _code.texture != null
	_renew.disabled = not on


func _init() -> void:
	title = tr("Phone remote")
	ok_button_text = tr("Done")
	var rows: VBoxContainer = VBoxContainer.new()
	rows.add_theme_constant_override("separation", 12)
	var note: Label = Label.new()
	note.text = (
		tr("Scan the code with a phone in the same network, or open the address in its browser.")
		+ " "
		+ tr("Keep the phone's screen on while you ride.")
	)
	note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	note.custom_minimum_size = Vector2(480, 0)
	note.add_theme_color_override("font_color", UiTheme.MUTED)
	rows.add_child(note)
	_code.stretch_mode = TextureRect.STRETCH_KEEP_CENTERED
	_code.texture_filter = CanvasItem.TEXTURE_FILTER_NEAREST
	rows.add_child(_code)
	_address.editable = false
	_address.select_all_on_focus = true
	_address.alignment = HORIZONTAL_ALIGNMENT_CENTER
	rows.add_child(_address)
	_no_network.text = tr("This computer is on no network: connect it to the phone's Wi-Fi.")
	_no_network.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_no_network.add_theme_color_override("font_color", UiTheme.WARNING)
	rows.add_child(_no_network)
	var port_row: HBoxContainer = HBoxContainer.new()
	port_row.add_theme_constant_override("separation", 12)
	var port_label: Label = Label.new()
	port_label.text = tr("Port")
	port_row.add_child(port_label)
	_port.min_value = MIN_PORT
	_port.max_value = MAX_PORT
	_port.step = 1
	_port.rounded = true
	_port.tooltip_text = tr("Change it if another program uses this port")
	_port.value_changed.connect(_on_port_changed)
	port_row.add_child(_port)
	var gap: Control = Control.new()
	gap.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	port_row.add_child(gap)
	_renew.text = tr("New address")
	_renew.tooltip_text = tr("Phones with the old address can no longer press")
	_renew.pressed.connect(_on_renew_pressed)
	port_row.add_child(_renew)
	rows.add_child(port_row)
	add_child(rows)


func _notification(what: int) -> void:
	if what == NOTIFICATION_TRANSLATION_CHANGED and is_node_ready():
		title = tr("Phone remote")
		ok_button_text = tr("Done")
		_send_texts()


## The phone's page says its texts in the interface language.
func _send_texts() -> void:
	if _torqa == null:
		return
	var words: Dictionary = {}
	for key: String in TorqaApp.phone_remote_words():
		words[key] = tr(page_text(key))
	_torqa.set_phone_remote_texts(TranslationServer.get_locale().replace("_", "-"), words)


## The remote's QR code with its quiet zone, a pixel per module: drawn without smoothing, it
## stays sharp at any size.
func _code_texture() -> ImageTexture:
	var code: Dictionary = _torqa.phone_remote_code()
	if code.is_empty():
		return null
	var size: int = code["size"]
	var data: PackedByteArray = code["modules"]
	var modules: Image = Image.create_from_data(size, size, false, Image.FORMAT_L8, data)
	var side: int = size + 2 * QUIET_ZONE
	var image: Image = Image.create_empty(side, side, false, Image.FORMAT_L8)
	image.fill(Color.WHITE)
	image.blit_rect(modules, Rect2i(0, 0, size, size), Vector2i(QUIET_ZONE, QUIET_ZONE))
	image.resize(side * MODULE_PIXELS, side * MODULE_PIXELS, Image.INTERPOLATE_NEAREST)
	return ImageTexture.create_from_image(image)


## Another port: the code and address change with it; if the port is taken, it stays.
func _on_port_changed(port: float) -> void:
	_torqa.set_phone_remote_port(roundi(port))
	refresh()


func _on_renew_pressed() -> void:
	_torqa.renew_phone_remote()
	refresh()
