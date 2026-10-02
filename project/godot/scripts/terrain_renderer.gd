extends Node3D

# Simple terrain renderer showing procedural voxel terrain
# For MVP; full GDExtension integration in M2

class_name TerrainRenderer

const CHUNK_SIZE = 32
const VOXEL_SIZE = 0.1

var test_mesh: Mesh
var material: Material

func _ready() -> void:
	print("TerrainRenderer: Starting terrain generation")

	# Create simple test material
	material = StandardMaterial3D.new()
	material.albedo_color = Color.GRAY
	material.roughness = 0.8

	# Generate test terrain mesh
	test_mesh = generate_test_terrain()

	if test_mesh:
		var mesh_instance = MeshInstance3D.new()
		mesh_instance.mesh = test_mesh
		mesh_instance.material_override = material
		add_child(mesh_instance)
		print("TerrainRenderer: Terrain mesh created with %d vertices" % test_mesh.get_surface_count())
	else:
		print("TerrainRenderer: Failed to generate terrain mesh")

func generate_test_terrain() -> Mesh:
	# Create a simple procedural terrain mesh using ArrayMesh
	var mesh = ArrayMesh.new()

	var vertices = PackedVector3Array()
	var indices = PackedInt32Array()
	var vertex_index = 0

	# Generate a small flat platform with some height variation
	var height_map = {}

	for x in range(CHUNK_SIZE):
		for z in range(CHUNK_SIZE):
			# Simple height function
			var height = 1.0 + 0.3 * sin(x * 0.3) * cos(z * 0.3)
			height_map[Vector2i(x, z)] = height

	# Build mesh from height map
	for x in range(CHUNK_SIZE - 1):
		for z in range(CHUNK_SIZE - 1):
			var h00 = height_map[Vector2i(x, z)]
			var h10 = height_map[Vector2i(x + 1, z)]
			var h01 = height_map[Vector2i(x, z + 1)]
			var h11 = height_map[Vector2i(x + 1, z + 1)]

			# Two triangles per quad
			var v00 = Vector3(x, h00, z) * VOXEL_SIZE
			var v10 = Vector3(x + 1, h10, z) * VOXEL_SIZE
			var v01 = Vector3(x, h01, z + 1) * VOXEL_SIZE
			var v11 = Vector3(x + 1, h11, z + 1) * VOXEL_SIZE

			# Triangle 1
			vertices.append(v00)
			vertices.append(v10)
			vertices.append(v11)
			indices.append(vertex_index)
			indices.append(vertex_index + 1)
			indices.append(vertex_index + 2)
			vertex_index += 3

			# Triangle 2
			vertices.append(v00)
			vertices.append(v11)
			vertices.append(v01)
			indices.append(vertex_index)
			indices.append(vertex_index + 1)
			indices.append(vertex_index + 2)
			vertex_index += 3

	# Create mesh surface
	var arrays = Array()
	arrays.resize(Mesh.ARRAY_MAX)
	arrays[Mesh.ARRAY_VERTEX] = vertices
	arrays[Mesh.ARRAY_INDEX] = indices

	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
	return mesh

func _process(delta: float) -> void:
	# Placeholder for camera-based chunk streaming
	pass
