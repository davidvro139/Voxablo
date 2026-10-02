#include "../include/physics.hpp"
#include "../include/voxel_core.hpp"
#include <cmath>

namespace voxel {

void update_fragments(PhysicsState& physics, float delta_time) {
    for (auto& fragment : physics.active_fragments) {
        // Apply gravity
        fragment.vy += physics.gravity_strength * delta_time;

        // Update position
        fragment.x += fragment.vx * delta_time;
        fragment.y += fragment.vy * delta_time;
        fragment.z += fragment.vz * delta_time;

        // Track falling time
        fragment.time_falling += delta_time;

        // Force settle after max time
        if (fragment.time_falling > static_cast<float>(physics.max_falling_time)) {
            fragment.is_settled = true;
        }
    }
}

int settle_collided_fragments(PhysicsState& physics,
                              std::vector<Chunk>& affected_chunks,
                              float chunk_base_x, float chunk_base_y, float chunk_base_z) {
    int settled_count = 0;
    std::vector<Fragment> remaining;

    for (auto& fragment : physics.active_fragments) {
        if (fragment.is_settled) {
            settled_count++;
            continue;  // Already settled, don't add back to active list
        }

        // Check collision with voxels below
        int voxel_x = static_cast<int>(std::floor(fragment.x));
        int voxel_y = static_cast<int>(std::floor(fragment.y));
        int voxel_z = static_cast<int>(std::floor(fragment.z));

        // Calculate chunk-local coordinates
        int chunk_lx = voxel_x - static_cast<int>(chunk_base_x);
        int chunk_ly = voxel_y - static_cast<int>(chunk_base_y);
        int chunk_lz = voxel_z - static_cast<int>(chunk_base_z);

        // Check if within chunk bounds
        if (chunk_lx >= 0 && chunk_lx < CHUNK_SIZE &&
            chunk_ly >= 0 && chunk_ly < CHUNK_SIZE &&
            chunk_lz >= 0 && chunk_lz < CHUNK_SIZE) {

            // Check voxel at fragment position
            MaterialType voxel_below = affected_chunks[0].get_voxel(chunk_lx, chunk_ly, chunk_lz);

            // If solid voxel or ground, settle
            if (voxel_below != MaterialType::Air || voxel_y <= 0) {
                fragment.is_settled = true;
                settled_count++;
                continue;
            }
        } else if (voxel_y <= 0) {
            // Hit ground level (world bottom)
            fragment.is_settled = true;
            settled_count++;
            continue;
        }

        // Not settled yet, keep in active list
        remaining.push_back(fragment);
    }

    physics.active_fragments = remaining;
    return settled_count;
}

void create_fragments_from_voxels(PhysicsState& physics,
                                  const std::vector<int>& voxel_positions,
                                  [[maybe_unused]] float region_base_x,
                                  [[maybe_unused]] float region_base_y,
                                  [[maybe_unused]] float region_base_z) {
    // voxel_positions: x, y, z packed as [x0, y0, z0, x1, y1, z1, ...]
    for (size_t i = 0; i + 2 < voxel_positions.size(); i += 3) {
        Fragment frag;
        frag.x = static_cast<float>(voxel_positions[i]) + 0.5f;
        frag.y = static_cast<float>(voxel_positions[i + 1]) + 0.5f;
        frag.z = static_cast<float>(voxel_positions[i + 2]) + 0.5f;
        frag.vx = 0.0f;
        frag.vy = 0.0f;  // Start falling
        frag.vz = 0.0f;
        frag.material = static_cast<uint16_t>(MaterialType::Dirt);  // Default material
        frag.time_falling = 0.0f;
        frag.is_settled = false;

        physics.active_fragments.push_back(frag);
    }
}

}  // namespace voxel
