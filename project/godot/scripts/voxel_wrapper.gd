extends Node

class_name VoxelWrapper

# Load native DLL
var native_lib: Object

var world_handle: int = 0

func _init():
	# Load the voxel_wrapper.dll
	native_lib = load("res://bin/voxel_wrapper.dll")
	if native_lib:
		print("✓ Voxel wrapper DLL loaded")
		_create_world()
	else:
		print("✗ Failed to load voxel_wrapper.dll")

func _create_world():
	if not native_lib:
		return
	# Call voxel_create_world()
	world_handle = native_lib.call("voxel_create_world")
	print("World created: ", world_handle)

func load_region(region_id: int) -> void:
	if not native_lib or world_handle == 0:
		return
	native_lib.call("voxel_load_region", world_handle, region_id)

func apply_damage(x: float, y: float, z: float, radius: float, energy: float) -> void:
	if not native_lib or world_handle == 0:
		return
	native_lib.call("voxel_apply_damage", world_handle, x, y, z, radius, energy)

func apply_collapse(region_id: int) -> void:
	if not native_lib or world_handle == 0:
		return
	native_lib.call("voxel_apply_collapse", world_handle, region_id)

func update_physics(region_id: int, delta: float) -> void:
	if not native_lib or world_handle == 0:
		return
	native_lib.call("voxel_update_physics", world_handle, region_id, delta)

func get_loaded_chunk_count() -> int:
	if not native_lib or world_handle == 0:
		return 0
	return native_lib.call("voxel_loaded_chunk_count", world_handle)

func get_dirty_chunk_count() -> int:
	if not native_lib or world_handle == 0:
		return 0
	return native_lib.call("voxel_dirty_chunk_count", world_handle)

func _notification(what):
	if what == NOTIFICATION_PREDELETE:
		if native_lib and world_handle != 0:
			native_lib.call("voxel_destroy_world", world_handle)
