#include "../include/voxel_core.hpp"
#include "../include/damage.hpp"
#include "../include/structural_analysis.hpp"
#include "../include/physics.hpp"
#include "../include/generation.hpp"
#include <cmath>

namespace voxel {

// Forward declaration of damage function (defined in damage.cpp)
void damage_chunk(Chunk& chunk, const DamageSphere& sphere,
                  float chunk_base_x, float chunk_base_y, float chunk_base_z);

VoxelWorld::VoxelWorld() : chunk_revision_counter(0) {}

VoxelWorld::~VoxelWorld() {}

void VoxelWorld::load_region(RegionId id) {
    uint64_t packed_id = id;
    if (regions.find(packed_id) == regions.end()) {
        auto physics_state = new PhysicsState();
        physics_state->gravity_strength = -0.98f;  // ~10 cm/s^2 in voxel units
        physics_state->max_falling_time = 300;     // 5 seconds at 60 FPS
        regions[packed_id] = {id, {}, 0, physics_state};
    }
}

void VoxelWorld::unload_region(RegionId id) {
    auto it = regions.find(id);
    if (it != regions.end()) {
        delete it->second.physics;
        regions.erase(it);
    }
}

void VoxelWorld::generate_chunk(RegionId id, const ChunkCoord& coord, uint64_t seed) {
    auto it = regions.find(id);
    if (it == regions.end()) {
        return;
    }

    Chunk& chunk = it->second.chunks[pack_chunk_coord(coord)];
    TerrainGenerator(seed).generate_chunk(chunk, coord.x, coord.y, coord.z);
}

const Chunk* VoxelWorld::find_chunk(RegionId id, const ChunkCoord& coord) const {
    auto region_it = regions.find(id);
    if (region_it == regions.end()) {
        return nullptr;
    }

    auto chunk_it = region_it->second.chunks.find(pack_chunk_coord(coord));
    return chunk_it == region_it->second.chunks.end() ? nullptr : &chunk_it->second;
}

static ChunkCoord chunk_of(int32_t x, int32_t y, int32_t z) {
    auto floor_div = [](int32_t v) {
        return v >= 0 ? v / CHUNK_SIZE : (v - CHUNK_SIZE + 1) / CHUNK_SIZE;
    };
    return {floor_div(x), floor_div(y), floor_div(z)};
}

MaterialType VoxelWorld::get_voxel(RegionId id, int32_t x, int32_t y, int32_t z) const {
    ChunkCoord coord = chunk_of(x, y, z);
    const Chunk* chunk = find_chunk(id, coord);
    if (!chunk) {
        return MaterialType::Air;
    }

    return chunk->get_voxel(x - coord.x * CHUNK_SIZE,
                            y - coord.y * CHUNK_SIZE,
                            z - coord.z * CHUNK_SIZE);
}

void VoxelWorld::set_voxel(RegionId id, int32_t x, int32_t y, int32_t z, MaterialType material) {
    auto it = regions.find(id);
    if (it == regions.end()) {
        return;
    }

    ChunkCoord coord = chunk_of(x, y, z);
    it->second.chunks[pack_chunk_coord(coord)].set_voxel(
        x - coord.x * CHUNK_SIZE, y - coord.y * CHUNK_SIZE, z - coord.z * CHUNK_SIZE, material);
}

void VoxelWorld::queue_edits(RegionId id, std::span<const VoxelEdit> edits) {
    for (const auto& edit : edits) {
        set_voxel(id, edit.x, edit.y, edit.z, edit.material);
    }
}

void VoxelWorld::queue_damage(RegionId id, const DamageSphere& damage) {
    if (!(damage.radius > 0.0f) || !(damage.energy > 0.0f) ||
        !std::isfinite(damage.radius) || !std::isfinite(damage.energy) ||
        !std::isfinite(damage.x) || !std::isfinite(damage.y) || !std::isfinite(damage.z)) {
        return;
    }
    auto it = regions.find(id);
    if (it == regions.end()) {
        return;  // Region not loaded
    }

    Region& region = it->second;

    // Find all chunks affected by damage sphere
    int min_chunk_x = static_cast<int>(std::floor((damage.x - damage.radius) / CHUNK_SIZE));
    int max_chunk_x = static_cast<int>(std::floor((damage.x + damage.radius) / CHUNK_SIZE));
    int min_chunk_y = static_cast<int>(std::floor((damage.y - damage.radius) / CHUNK_SIZE));
    int max_chunk_y = static_cast<int>(std::floor((damage.y + damage.radius) / CHUNK_SIZE));
    int min_chunk_z = static_cast<int>(std::floor((damage.z - damage.radius) / CHUNK_SIZE));
    int max_chunk_z = static_cast<int>(std::floor((damage.z + damage.radius) / CHUNK_SIZE));

    // Apply damage to all affected chunks
    for (int cx = min_chunk_x; cx <= max_chunk_x; cx++) {
        for (int cy = min_chunk_y; cy <= max_chunk_y; cy++) {
            for (int cz = min_chunk_z; cz <= max_chunk_z; cz++) {
                uint64_t chunk_key = pack_chunk_coord({cx, cy, cz});

                if (region.chunks.find(chunk_key) == region.chunks.end()) {
                    // Unloaded space contains no voxels to destroy.
                    continue;
                }

                Chunk& chunk = region.chunks[chunk_key];
                float chunk_base_x = static_cast<float>(cx * CHUNK_SIZE);
                float chunk_base_y = static_cast<float>(cy * CHUNK_SIZE);
                float chunk_base_z = static_cast<float>(cz * CHUNK_SIZE);

                damage_chunk(chunk, damage, chunk_base_x, chunk_base_y, chunk_base_z);
            }
        }
    }
}

std::vector<ChunkUpdate> VoxelWorld::drain_completed_updates() {
    std::vector<ChunkUpdate> updates;
    // Placeholder for mesh job draining (M1)
    return updates;
}

void VoxelWorld::request_checkpoint([[maybe_unused]] RegionId id) {
    // Placeholder for save implementation (M4)
}

size_t VoxelWorld::loaded_chunk_count() const {
    size_t count = 0;
    for (const auto& region : regions) {
        count += region.second.chunks.size();
    }
    return count;
}

size_t VoxelWorld::dirty_chunk_count() const {
    size_t count = 0;
    for (const auto& region : regions) {
        for (const auto& chunk_pair : region.second.chunks) {
            if (chunk_pair.second.is_dirty()) {
                count++;
            }
        }
    }
    return count;
}

uint64_t VoxelWorld::pack_chunk_coord(const ChunkCoord& c) const {
    // Pack 3 int32 into uint64 (simplified; assumes reasonable ranges)
    return ((uint64_t)c.x & 0xFFFFFF) | (((uint64_t)c.y & 0xFFFFFF) << 20) | (((uint64_t)c.z & 0xFFFFFF) << 40);
}

ChunkCoord VoxelWorld::unpack_chunk_coord(uint64_t packed) const {
    ChunkCoord c;
    c.x = (int32_t)(packed & 0xFFFFFF);
    c.y = (int32_t)((packed >> 20) & 0xFFFFFF);
    c.z = (int32_t)((packed >> 40) & 0xFFFFFF);
    return c;
}

void VoxelWorld::queue_collapse(RegionId id) {
    auto it = regions.find(id);
    if (it == regions.end()) {
        return;  // Region not loaded
    }

    Region& region = it->second;

    // Analyze and collapse all chunks in region
    for (auto& chunk_pair : region.chunks) {
        Chunk& chunk = chunk_pair.second;
        collapse_unsupported_voxels(chunk);
    }
}

void VoxelWorld::update_physics(RegionId id, float delta_time) {
    auto it = regions.find(id);
    if (it == regions.end()) {
        return;  // Region not loaded
    }

    Region& region = it->second;
    if (!region.physics) {
        return;
    }

    // Update fragment positions under gravity
    update_fragments(*region.physics, delta_time);

    // Settle fragments that collided
    // For now, use first chunk as reference (simplified single-chunk physics)
    if (!region.chunks.empty()) {
        auto chunk_it = region.chunks.begin();
        Chunk& first_chunk = chunk_it->second;
        std::vector<Chunk*> chunks_vec;
        chunks_vec.push_back(&first_chunk);

        settle_collided_fragments(*region.physics,
                                 reinterpret_cast<std::vector<Chunk>&>(chunks_vec),
                                 0.0f, 0.0f, 0.0f);
    }
}

}  // namespace voxel
