class_name RemoteNavigation
extends RefCounted
## The remote's pad (#228) works Torqa as the keyboard does: its arrows move the focus, OK
## presses what has it, back closes a dialog or goes back a screen (Escape's `ui_cancel`). A
## dialog open over the window takes the steps first. The first arrow on a screen gives the
## focus to its main control, one in the group [constant FIRST], else to its first.

## Controls an arrow focuses first on their screen, e.g. a course's Ride button.
const FIRST: StringName = &"remote_first"
## The input actions each step of the pad stands for.
const ACTIONS: Dictionary[String, StringName] = {
	"up": &"ui_up",
	"down": &"ui_down",
	"left": &"ui_left",
	"right": &"ui_right",
	"ok": &"ui_accept",
	"back": &"ui_cancel",
}


## Carries out `step` ("up", "down", "left", "right", "ok" or "back") in `window`.
static func step(window: Window, step: String) -> void:
	if not ACTIONS.has(step):
		return
	var target: Viewport = _topmost(window)
	if target.gui_get_focus_owner() == null and step in ["up", "down", "left", "right"]:
		var first: Control = _first(target)
		if first != null:
			first.grab_focus()
			return
	for pressed: bool in [true, false]:
		var event: InputEventAction = InputEventAction.new()
		event.action = ACTIONS[step]
		event.pressed = pressed
		Input.parse_input_event(event)


## The dialog open over `window`, the last opened, or the window itself.
static func _topmost(window: Window) -> Viewport:
	var topmost: Viewport = window
	for dialog: Window in window.get_embedded_subwindows():
		if dialog.visible:
			topmost = dialog
	return topmost


## The control the first arrow focuses in `viewport`: its screen's main one, else its first.
static func _first(viewport: Viewport) -> Control:
	for node: Node in viewport.get_tree().get_nodes_in_group(FIRST):
		var control: Control = node as Control
		if control != null and control.get_viewport() == viewport and _focusable(control):
			return control
	return _first_below(viewport)


static func _first_below(node: Node) -> Control:
	for child: Node in node.get_children():
		# A dialog is a viewport of its own: open, it would have taken the step.
		if child is Window:
			continue
		var control: Control = child as Control
		if control != null and not control.is_visible_in_tree():
			continue
		if control != null and _focusable(control):
			return control
		var found: Control = _first_below(child)
		if found != null:
			return found
	return null


static func _focusable(control: Control) -> bool:
	var button: BaseButton = control as BaseButton
	return (
		control.is_visible_in_tree()
		and control.focus_mode == Control.FOCUS_ALL
		and not (button != null and button.disabled)
	)
