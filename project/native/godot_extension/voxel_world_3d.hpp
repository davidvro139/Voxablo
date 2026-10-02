#pragma once

// Minimal GDExtension header for voxel_world binding
// Full godot-cpp integration deferred to when godot-cpp is vendored

namespace voxel {

// Forward declaration of native voxel core
class VoxelWorld;
class Chunk;
class TerrainGenerator;

// This will be the Godot-facing wrapper class
// For now, just a stub showing the interface
class VoxelWorld3D {
public:
    VoxelWorld3D();
    ~VoxelWorld3D();

    // Generate a chunk at world coordinates
    void generate_chunk(int chunk_x, int chunk_y, int chunk_z, uint64_t seed);

    // Get mesh for a chunk
    // Returns serialized mesh data (vertices, indices)
    void get_chunk_mesh(int chunk_x, int chunk_y, int chunk_z);

    // Get diagnostics
    int get_loaded_chunk_count() const;
    int get_dirty_chunk_count() const;

private:
    VoxelWorld* world;
    TerrainGenerator* generator;
};

}  // namespace voxel
