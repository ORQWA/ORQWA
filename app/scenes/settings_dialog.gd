class_name SettingsDialog
extends AcceptDialog
## The settings, opened from the cog at the top right of the start page (R83): this
## computer's graphics and full screen (R43, R78, #218), applied as they change, and About:
## the version, the commit and the licences.

var _tabs: TabContainer = TabContainer.new()
var _graphics: GraphicsSettings = GraphicsSettings.new()
var _about: AboutPage = AboutPage.new()


func _ready() -> void:
	theme = UiTheme.build()
	title = tr("Settings")
	ok_button_text = tr("Done")
	min_size = Vector2i(760, 480)
	_graphics.name = tr("Graphics")
	_tabs.add_child(_graphics)
	_about.name = tr("About")
	_tabs.add_child(_about)
	add_child(_tabs)


func bind(torqa: TorqaApp) -> void:
	_graphics.bind(torqa)


## Opens the settings on their first tab.
func open() -> void:
	_tabs.current_tab = 0
	UiScale.popup(self, Vector2i(960, 640))
