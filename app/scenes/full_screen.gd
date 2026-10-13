class_name FullScreen
extends RefCounted
## Full screen on and off (#218, R25): for a TV or a second screen by the trainer, without a
## title bar or taskbar. The rider's choice is kept for the next start.


## Whether `window` fills its screen.
static func is_on(window: Window) -> bool:
	return window.mode in [Window.MODE_FULLSCREEN, Window.MODE_EXCLUSIVE_FULLSCREEN]


## Fills the screen with `window`, or puts it back in a window, and remembers the choice.
static func set_on(window: Window, torqa: TorqaApp, on: bool) -> void:
	window.mode = Window.MODE_FULLSCREEN if on else Window.MODE_WINDOWED
	torqa.set_fullscreen(on)
