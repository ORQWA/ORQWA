class_name RegionPicker
extends ConfirmationDialog
## The regions Geofabrik offers for map detail (ADR 0033), to choose one to download: under
## their continent and country, searchable, each with its download's size once chosen.

## The rider chose to download region `id`.
signal chosen(id: String)

var _orqwa: OrqwaApp
var _search: LineEdit = LineEdit.new()
var _tree: Tree = Tree.new()
var _note: Label = Label.new()
## The offers as `OrqwaApp`'s `region_offers` gives them, by id.
var _offers: Dictionary[String, Dictionary] = {}
## The ids of each region's parts, by the region's id; continents under "".
var _parts: Dictionary[String, Array] = {}
## The bytes of the downloads asked for, by region id; -1 if the server did not tell.
var _sizes: Dictionary[String, int] = {}


func _init() -> void:
	min_size = Vector2i(480, 0)
	var box: VBoxContainer = VBoxContainer.new()
	box.add_theme_constant_override("separation", 8)
	_search.clear_button_enabled = true
	_search.right_icon = UiIcons.texture("search", 16, UiTheme.MUTED)
	_search.text_changed.connect(func(_text: String) -> void: _fill())
	box.add_child(_search)
	_tree.hide_root = true
	_tree.custom_minimum_size = Vector2(440, 360)
	_tree.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_tree.item_selected.connect(_on_selected)
	_tree.item_activated.connect(_on_activated)
	box.add_child(_tree)
	_note.add_theme_color_override("font_color", UiTheme.MUTED)
	_note.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_note.custom_minimum_size = Vector2(440, 0)
	box.add_child(_note)
	add_child(box)
	confirmed.connect(_on_confirmed)


func _ready() -> void:
	theme = UiTheme.build()
	title = tr("Add a region")
	ok_button_text = tr("Download")
	_search.placeholder_text = tr("Search")


func bind(orqwa: OrqwaApp) -> void:
	_orqwa = orqwa
	_orqwa.region_offers.connect(show_offers)
	_orqwa.region_size.connect(_on_size)
	_orqwa.map_detail_failed.connect(func(message: String) -> void: _note.text = message)


## Shows itself and asks Geofabrik for its regions.
func open() -> void:
	_search.text = ""
	_note.text = tr("Loading the regions…")
	get_ok_button().disabled = true
	show_offers([])
	if _orqwa != null:
		_orqwa.load_region_offers()
	UiScale.popup(self, Vector2i(520, 560))


## Lists `offers` as `OrqwaApp`'s `region_offers` gives them.
func show_offers(offers: Array) -> void:
	_offers.clear()
	_parts.clear()
	for offer: Dictionary in offers:
		var id: String = offer.get("id", "")
		var parent: String = offer.get("parent", "")
		_offers[id] = offer
		if not _parts.has(parent):
			_parts[parent] = []
		_parts[parent].append(id)
	if not offers.is_empty():
		_note.text = tr("Choose a country, or a part of a large one: it downloads faster.")
	_fill()


## The tree of regions, with only those whose name or whose parts' names hold the search.
func _fill() -> void:
	_tree.clear()
	var root: TreeItem = _tree.create_item()
	var wanted: String = _search.text.strip_edges().to_lower()
	_add_parts(root, "", wanted)


func _add_parts(under: TreeItem, parent: String, wanted: String) -> void:
	var ids: Array = _parts.get(parent, [])
	ids.sort_custom(func(a: String, b: String) -> bool: return _name(a) < _name(b))
	for id: String in ids:
		if not _matches(id, wanted):
			continue
		var item: TreeItem = _tree.create_item(under)
		item.set_text(0, _name(id))
		item.set_metadata(0, id)
		# Continents are offered by their parts: too large to download whole.
		item.set_selectable(0, parent != "")
		if parent == "":
			item.set_custom_color(0, UiTheme.MUTED)
		item.collapsed = wanted.is_empty()
		_add_parts(item, id, wanted)


func _matches(id: String, wanted: String) -> bool:
	if wanted.is_empty() or _name(id).to_lower().contains(wanted):
		return true
	for part: String in _parts.get(id, []):
		if _matches(part, wanted):
			return true
	return false


## The name Geofabrik gives region `id`; `id` until it listed it.
func name_of(id: String) -> String:
	return _name(id)


func _name(id: String) -> String:
	var offer: Dictionary = _offers.get(id, {})
	return offer.get("name", id)


func _chosen() -> String:
	var item: TreeItem = _tree.get_selected()
	if item == null or not item.is_selectable(0):
		return ""
	return item.get_metadata(0)


func _on_selected() -> void:
	var id: String = _chosen()
	get_ok_button().disabled = id.is_empty()
	if id.is_empty():
		return
	if _sizes.has(id):
		_show_size(id)
		return
	_note.text = tr("Asking for the size of %s…") % _name(id)
	if _orqwa != null:
		_orqwa.load_region_size(id)


func _on_size(id: String, size: int) -> void:
	_sizes[id] = size
	if id == _chosen():
		_show_size(id)


func _show_size(id: String) -> void:
	var size: int = _sizes.get(id, -1)
	if size < 0:
		_note.text = tr("%s: its size is unknown.") % _name(id)
	else:
		_note.text = tr("%s: %s to download.") % [_name(id), UiTheme.bytes(size)]


func _on_activated() -> void:
	if not _chosen().is_empty():
		_on_confirmed()
		hide()


func _on_confirmed() -> void:
	var id: String = _chosen()
	if not id.is_empty():
		chosen.emit(id)
