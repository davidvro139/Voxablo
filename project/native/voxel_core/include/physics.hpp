#pragma once

#include <cstdint>
#include <vector>

namespace voxel {

// Forward declarations
class Chunk;

// A single voxel falling under gravity
struct Fragment {
    float x, y, z;           // World position (voxel center)
    float vx, vy, vz;        // Velocity (units/frame)
    uint16_t material;       // Material type
    float time_falling;      // How long it's been falling
    bool is_settled;         // True if collision detected
};

// Physics simulation state for a region
struct PhysicsState {
    std::vector<Fragment> active_fragments;
    float gravity_strength;  // Units/frame^2, typically -0.1 for 10cm voxels
    int max_falling_time;    // Frames before forced settle (e.g., 300 = 5 sec @ 60fps)
};

// Apply gravity and update fragment positions
void update_fragments(PhysicsState& physics, float delta_time);

// Detect collisions and settle fragments that hit voxels
// Returns count of newly settled fragments
int settle_collided_fragments(PhysicsState& physics,
                              std::vector<Chunk>& affected_chunks,
                              float chunk_base_x, float chunk_base_y, float chunk_base_z);

// Create fragments from destroyed voxels (for visual/physics representation)
// Caller provides list of voxel positions; fragments are added to physics state
void create_fragments_from_voxels(PhysicsState& physics,
                                  const std::vector<int>& voxel_positions,
                                  float region_base_x, float region_base_y, float region_base_z);

}  // namespace voxel
