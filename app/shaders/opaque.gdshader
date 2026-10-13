// Makes the window opaque wherever it is drawn, and leaves its colours as they are (#220).
// Godot's sky writes alpha 0 (the alpha channel carries subsurface scattering in Forward+), and
// with per-pixel transparency allowed (for the overlay) the swapchain is made with alpha: on
// Windows other windows then show through the sky and the panels over it.
shader_type canvas_item;
// Added: the colour gains nothing, alpha 1 plus what is there clamps to 1.
render_mode blend_add, unshaded;

void fragment() {
	COLOR = vec4(0.0, 0.0, 0.0, 1.0);
}
