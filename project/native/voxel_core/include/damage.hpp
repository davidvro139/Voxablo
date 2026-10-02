#pragma once

#include <cstdint>

namespace voxel {

// Forward declarations
class Chunk;
struct DamageSphere;

// Material properties for damage simulation
struct MaterialProperties {
    uint16_t material_id;
    float hardness;      // 0.0 (soft) to 1.0 (hard) - resistance to damage
    float resistance;    // Additional damage reduction factor
};

// Result of applying damage to a chunk
struct DamageResult {
    int voxels_removed;
    int chunks_affected;
    float max_depth;     // Deepest excavation
};

// Get material properties (hardness, resistance)
MaterialProperties get_material_properties(uint16_t material);

// Apply damage sphere to chunk at given local coordinates
// Returns depth of damage penetration
float apply_damage_to_voxel(float energy, uint16_t material);

// Apply damage sphere to a chunk
// Forward declared Chunk class
class Chunk;
void damage_chunk(Chunk& chunk, const DamageSphere& sphere,
                  float chunk_base_x, float chunk_base_y, float chunk_base_z);

}  // namespace voxel
