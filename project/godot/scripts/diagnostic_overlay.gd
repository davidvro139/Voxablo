extends Label

var frame_times: PackedFloat32Array = []
var max_frame_history: int = 60

func _process(delta: float) -> void:
	frame_times.append(delta * 1000.0)
	if frame_times.size() > max_frame_history:
		frame_times.remove_at(0)

	var avg_frame_time = 0.0
	for ft in frame_times:
		avg_frame_time += ft
	avg_frame_time /= frame_times.size()

	var fps = Engine.get_frames_per_second()
	var chunks_loaded = 0
	var chunks_dirty = 0

	text = "FPS: %d\n" % fps
	text += "Chunks Loaded: %d\n" % chunks_loaded
	text += "Chunks Dirty: %d\n" % chunks_dirty
	text += "Frame Time: %.2f ms" % avg_frame_time
