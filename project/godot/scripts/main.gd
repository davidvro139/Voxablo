extends Node3D

func _ready() -> void:
	set_process_input(true)
	var camera = get_node("Camera3D")
	if camera:
		camera.make_current()

func _process(delta: float) -> void:
	pass

func _input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed:
		if event.keycode == KEY_ESCAPE:
			get_tree().quit()
