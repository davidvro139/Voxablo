#pragma once

#include <cstdint>

// C interface over voxel_core, consumed by the Rust FFI crate

extern "C" {
    typedef void* VoxelWorldHandle;

    VoxelWorldHandle voxel_create_world();
    void voxel_destroy_world(VoxelWorldHandle world);

    void voxel_load_region(VoxelWorldHandle world, uint64_t region_id);
    void voxel_unload_region(VoxelWorldHandle world, uint64_t region_id);

    // Generates terrain into the world's chunk, seeded by region_id
    void voxel_generate_terrain(VoxelWorldHandle world, uint64_t region_id, int chunk_x, int chunk_y, int chunk_z);

    // Builds a house with its footprint's minimum corner at (x, z); returns the ground-floor Y.
    int32_t voxel_build_house(VoxelWorldHandle world, uint64_t region_id, int x, int z,
                              int width, int depth, int storeys);

    // Material id at a world voxel coordinate (0 = air)
    uint16_t voxel_get_voxel(VoxelWorldHandle world, uint64_t region_id, int x, int y, int z);

    // Positions are chunk-local. Arrays are parallel, vertex_count entries each
    // (positions/normals hold 3 floats per vertex). Free with voxel_free_mesh_data.
    typedef struct {
        float* positions;
        float* normals;
        uint16_t* materials;
        uint32_t* indices;
        uint32_t vertex_count;
        uint32_t index_count;
    } MeshData;

    MeshData voxel_get_chunk_mesh(VoxelWorldHandle world, uint64_t region_id, int chunk_x, int chunk_y, int chunk_z);
    void voxel_free_mesh_data(MeshData mesh);

    typedef struct {
        int32_t x, y, z;
        uint16_t material;
    } PieceVoxel;

    // All detached pieces concatenated; piece i is voxels[piece_starts[i] .. piece_starts[i + 1]].
    typedef struct {
        PieceVoxel* voxels;
        uint32_t* piece_starts;  // piece_count + 1 entries
        uint32_t piece_count;
        uint32_t voxel_count;
    } DetachedPieces;

    // Removes groups near the sphere that no longer connect to the ground and returns them.
    DetachedPieces voxel_extract_detached(VoxelWorldHandle world, uint64_t region_id,
                                          float x, float y, float z, float radius, uint32_t max_piece_voxels);
    void voxel_free_detached(DetachedPieces pieces);

    void voxel_set_voxels(VoxelWorldHandle world, uint64_t region_id, const PieceVoxel* voxels, uint32_t count);

    void voxel_apply_damage(VoxelWorldHandle world, float x, float y, float z, float radius, float energy);
    void voxel_apply_collapse(VoxelWorldHandle world, uint64_t region_id);
    void voxel_update_physics(VoxelWorldHandle world, uint64_t region_id, float delta_time);

    uint64_t voxel_loaded_chunk_count(VoxelWorldHandle world);
    uint64_t voxel_dirty_chunk_count(VoxelWorldHandle world);
}
