#include "../include/damage.hpp"
#include "../include/voxel_core.hpp"
#include <cmath>

namespace voxel {

// Material hardness and resistance values
MaterialProperties get_material_properties(uint16_t material) {
    MaterialProperties props;
    props.material_id = material;

    switch (static_cast<MaterialType>(material)) {
        case MaterialType::Air:
            props.hardness = 0.0f;
            props.resistance = 0.0f;
            break;
        case MaterialType::Dirt:
            props.hardness = 0.3f;  // Soft, easy to dig
            props.resistance = 0.1f;
            break;
        case MaterialType::Wood:
            props.hardness = 0.4f;  // Slightly harder than dirt
            props.resistance = 0.15f;
            break;
        case MaterialType::Brick:
            props.hardness = 0.7f;  // Hard, slow to break
            props.resistance = 0.3f;
            break;
        case MaterialType::Stone:
            props.hardness = 0.9f;  // Very hard
            props.resistance = 0.5f;
            break;
        default:
            props.hardness = 0.5f;
            props.resistance = 0.25f;
    }

    return props;
}

// Calculate if a voxel is destroyed by damage energy
// Returns remaining energy after this voxel's resistance
float apply_damage_to_voxel(float energy, uint16_t material) {
    if (energy <= 0.0f) {
        return 0.0f;
    }

    MaterialProperties props = get_material_properties(material);

    // Air doesn't resist damage
    if (props.hardness <= 0.0f) {
        return energy;
    }

    // Damage is reduced by material hardness and resistance
    // Hardness acts as multiplicative resistance
    float damage_reduction = props.hardness * (1.0f + props.resistance);
    float remaining_energy = energy - damage_reduction;

    return remaining_energy;
}

// Apply damage sphere to a chunk
// This is called from VoxelWorld::queue_damage
void damage_chunk(Chunk& chunk, const DamageSphere& sphere,
                  float chunk_base_x, float chunk_base_y, float chunk_base_z) {
    if (!(sphere.radius > 0.0f) || !(sphere.energy > 0.0f) ||
        !std::isfinite(sphere.radius) || !std::isfinite(sphere.energy) ||
        !std::isfinite(sphere.x) || !std::isfinite(sphere.y) || !std::isfinite(sphere.z)) {
        return;
    }

    for (int lx = 0; lx < CHUNK_SIZE; lx++) {
        for (int ly = 0; ly < CHUNK_SIZE; ly++) {
            for (int lz = 0; lz < CHUNK_SIZE; lz++) {
                float world_x = chunk_base_x + lx;
                float world_y = chunk_base_y + ly;
                float world_z = chunk_base_z + lz;

                // Distance from damage sphere center
                float dx = world_x - sphere.x;
                float dy = world_y - sphere.y;
                float dz = world_z - sphere.z;
                float distance = std::sqrt(dx * dx + dy * dy + dz * dz);

                // Only affect voxels within sphere radius
                if (distance > sphere.radius) {
                    continue;
                }

                MaterialType voxel = chunk.get_voxel(lx, ly, lz);
                if (voxel == MaterialType::Air) {
                    continue;  // Already empty
                }

                // Falloff: damage decreases with distance from center
                // Linear falloff: energy = initial_energy * (1 - distance/radius)
                float falloff = 1.0f - (distance / sphere.radius);
                float voxel_damage_energy = sphere.energy * falloff;

                // Check if voxel is destroyed
                float remaining_energy = apply_damage_to_voxel(voxel_damage_energy,
                                                               static_cast<uint16_t>(voxel));

                if (remaining_energy > 0.0f || voxel_damage_energy >= 1.0f) {
                    // Voxel is destroyed
                    chunk.set_voxel(lx, ly, lz, MaterialType::Air);
                }
            }
        }
    }

    // set_voxel keeps edited chunks dirty until their mesh has been rebuilt.
}

}  // namespace voxel
