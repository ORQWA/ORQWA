class_name TouchScroll
extends Node
## Lets a page's controls pass pointer input to its ScrollContainer (#381), which owns the
## drag threshold, inertia and cancellation of button presses once a swipe starts.


func _ready() -> void:
	if not DisplayServer.is_touchscreen_available():
		return
	var scroll: ScrollContainer = get_parent() as ScrollContainer
	scroll.child_entered_tree.connect(_watch)
	for child: Node in scroll.get_children():
		_watch(child)


func _watch(node: Node) -> void:
	# Nested scroll areas own their gestures; popup windows are outside the page.
	if node == self or node is ScrollContainer or node is ScrollBar or node is Window:
		return
	var control: Control = node as Control
	if control != null and control.mouse_filter == Control.MOUSE_FILTER_STOP:
		control.mouse_filter = Control.MOUSE_FILTER_PASS
	var button: BaseButton = node as BaseButton
	if button != null:
		# Menus normally open on finger-down, before scrolling can cancel the tap.
		button.action_mode = BaseButton.ACTION_MODE_BUTTON_RELEASE
	if not node.child_entered_tree.is_connected(_watch):
		node.child_entered_tree.connect(_watch)
	for child: Node in node.get_children():
		_watch(child)
