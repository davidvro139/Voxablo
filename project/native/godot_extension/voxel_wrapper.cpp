#include "voxel_wrapper.hpp"
#include "../voxel_core/include/voxel_core.hpp"
#include "../voxel_core/include/meshing.hpp"
#include "../voxel_core/include/structures.hpp"
#include "../voxel_core/include/structural_analysis.hpp"
#include <vector>
#include <cstring>

using namespace voxel;

namespace {
VoxelWorld* as_world(VoxelWorldHandle world) {
    return static_cast<VoxelWorld*>(world);
}

float decode_normal(uint8_t n) {
    return n == 255 ? 1.0f : (n == 0 ? -1.0f : 0.0f);
}
}  // namespace

extern "C" {

VoxelWorldHandle voxel_create_world() {
    return new VoxelWorld();
}

void voxel_destroy_world(VoxelWorldHandle world) {
    delete as_world(world);
}

void voxel_load_region(VoxelWorldHandle world, uint64_t region_id) {
    as_world(world)->load_region(region_id);
}

void voxel_unload_region(VoxelWorldHandle world, uint64_t region_id) {
    as_world(world)->unload_region(region_id);
}

void voxel_generate_terrain(VoxelWorldHandle world, uint64_t region_id, int chunk_x, int chunk_y, int chunk_z) {
    as_world(world)->generate_chunk(region_id, ChunkCoord{chunk_x, chunk_y, chunk_z}, region_id);
}

int32_t voxel_build_house(VoxelWorldHandle world, uint64_t region_id, int x, int z,
                          int width, int depth, int storeys) {
    HouseSpec spec;
    spec.x = x;
    spec.z = z;
    spec.width = width;
    spec.depth = depth;
    spec.storeys = storeys;
    return build_house(*as_world(world), region_id, spec).floor_y;
}

uint16_t voxel_get_voxel(VoxelWorldHandle world, uint64_t region_id, int x, int y, int z) {
    return static_cast<uint16_t>(as_world(world)->get_voxel(region_id, x, y, z));
}

MeshData voxel_get_chunk_mesh(VoxelWorldHandle world, uint64_t region_id, int chunk_x, int chunk_y, int chunk_z) {
    MeshData result = {nullptr, nullptr, nullptr, nullptr, 0, 0};

    const Chunk* chunk = as_world(world)->find_chunk(region_id, ChunkCoord{chunk_x, chunk_y, chunk_z});
    if (!chunk) {
        return result;
    }

    ChunkMesh mesh = mesh_chunk(*chunk);
    if (mesh.vertices.empty() || mesh.indices.empty()) {
        return result;
    }

    const size_t n = mesh.vertices.size();
    result.vertex_count = static_cast<uint32_t>(n);
    result.positions = new float[n * 3];
    result.normals = new float[n * 3];
    result.materials = new uint16_t[n];

    for (size_t i = 0; i < n; ++i) {
        const Vertex& v = mesh.vertices[i];
        result.positions[i * 3 + 0] = v.x;
        result.positions[i * 3 + 1] = v.y;
        result.positions[i * 3 + 2] = v.z;
        result.normals[i * 3 + 0] = decode_normal(v.normal_x);
        result.normals[i * 3 + 1] = decode_normal(v.normal_y);
        result.normals[i * 3 + 2] = decode_normal(v.normal_z);
        result.materials[i] = v.material;
    }

    result.index_count = static_cast<uint32_t>(mesh.indices.size());
    result.indices = new uint32_t[mesh.indices.size()];
    std::memcpy(result.indices, mesh.indices.data(), mesh.indices.size() * sizeof(uint32_t));

    return result;
}

void voxel_free_mesh_data(MeshData mesh) {
    delete[] mesh.positions;
    delete[] mesh.normals;
    delete[] mesh.materials;
    delete[] mesh.indices;
}

DetachedPieces voxel_extract_detached(VoxelWorldHandle world, uint64_t region_id,
                                      float x, float y, float z, float radius, uint32_t max_piece_voxels) {
    DetachedPieces result = {nullptr, nullptr, 0, 0};
    std::vector<DetachedPiece> pieces =
        extract_detached_pieces(*as_world(world), region_id, x, y, z, radius, max_piece_voxels);
    if (pieces.empty()) {
        return result;
    }

    size_t total = 0;
    for (const auto& piece : pieces) {
        total += piece.size();
    }

    result.piece_count = static_cast<uint32_t>(pieces.size());
    result.voxel_count = static_cast<uint32_t>(total);
    result.voxels = new PieceVoxel[total];
    result.piece_starts = new uint32_t[pieces.size() + 1];

    uint32_t next = 0;
    for (size_t i = 0; i < pieces.size(); i++) {
        result.piece_starts[i] = next;
        for (const DetachedVoxel& v : pieces[i]) {
            result.voxels[next++] = {v.x, v.y, v.z, static_cast<uint16_t>(v.material)};
        }
    }
    result.piece_starts[pieces.size()] = next;
    return result;
}

void voxel_free_detached(DetachedPieces pieces) {
    delete[] pieces.voxels;
    delete[] pieces.piece_starts;
}

void voxel_set_voxels(VoxelWorldHandle world, uint64_t region_id, const PieceVoxel* voxels, uint32_t count) {
    for (uint32_t i = 0; i < count; i++) {
        const PieceVoxel& v = voxels[i];
        as_world(world)->set_voxel(region_id, v.x, v.y, v.z, static_cast<MaterialType>(v.material));
    }
}

void voxel_apply_damage(VoxelWorldHandle world, float x, float y, float z, float radius, float energy) {
    DamageSphere damage;
    damage.x = x;
    damage.y = y;
    damage.z = z;
    damage.radius = radius;
    damage.energy = energy;

    as_world(world)->queue_damage(1, damage);
}

void voxel_apply_collapse(VoxelWorldHandle world, uint64_t region_id) {
    as_world(world)->queue_collapse(region_id);
}

void voxel_update_physics(VoxelWorldHandle world, uint64_t region_id, float delta_time) {
    as_world(world)->update_physics(region_id, delta_time);
}

uint64_t voxel_loaded_chunk_count(VoxelWorldHandle world) {
    return as_world(world)->loaded_chunk_count();
}

uint64_t voxel_dirty_chunk_count(VoxelWorldHandle world) {
    return as_world(world)->dirty_chunk_count();
}

}  // extern "C"
