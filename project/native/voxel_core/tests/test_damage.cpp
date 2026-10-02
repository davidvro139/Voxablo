#include "../include/voxel_core.hpp"
#include "../include/damage.hpp"

extern void test_result(const char* name, bool condition);

void run_damage_tests() {
    using namespace voxel;

    std::printf("\n--- Damage Tests ---\n");

    // Test 1: Material properties are correct
    {
        auto dirt_props = get_material_properties(static_cast<uint16_t>(MaterialType::Dirt));
        auto stone_props = get_material_properties(static_cast<uint16_t>(MaterialType::Stone));

        test_result("Damage: material hardness",
            dirt_props.hardness < stone_props.hardness);
    }

    // Test 2: Damage reduces energy based on hardness
    {
        float dirt_damage = apply_damage_to_voxel(1.0f, static_cast<uint16_t>(MaterialType::Dirt));
        float stone_damage = apply_damage_to_voxel(1.0f, static_cast<uint16_t>(MaterialType::Stone));

        test_result("Damage: stone resists more than dirt",
            stone_damage < dirt_damage);
    }

    // Test 3: Air has no resistance
    {
        float air_damage = apply_damage_to_voxel(0.5f, static_cast<uint16_t>(MaterialType::Air));
        test_result("Damage: air has no resistance",
            air_damage == 0.5f);
    }

    // Test 4: Damage sphere carves chunk
    {
        Chunk chunk;
        // Fill chunk with dirt
        for (int x = 0; x < 10; x++) {
            for (int y = 0; y < 10; y++) {
                for (int z = 0; z < 10; z++) {
                    chunk.set_voxel(x, y, z, MaterialType::Dirt);
                }
            }
        }

        // Apply damage sphere
        DamageSphere sphere;
        sphere.x = 5.0f;
        sphere.y = 5.0f;
        sphere.z = 5.0f;
        sphere.radius = 3.0f;
        sphere.energy = 2.0f;

        chunk.mark_clean();
        damage_chunk(chunk, sphere, 0.0f, 0.0f, 0.0f);
        test_result("Damage: destroyed chunk stays dirty for remeshing", chunk.is_dirty());

        // Check that center was destroyed
        bool center_destroyed = chunk.get_voxel(5, 5, 5) == MaterialType::Air;
        test_result("Damage: sphere destroys voxels", center_destroyed);
    }

    // Test 5: Damage falloff with distance
    {
        Chunk chunk;
        // Fill with stone (hard material)
        for (int x = 0; x < 32; x++) {
            for (int y = 0; y < 32; y++) {
                for (int z = 0; z < 32; z++) {
                    chunk.set_voxel(x, y, z, MaterialType::Stone);
                }
            }
        }

        // Small damage sphere at edge
        DamageSphere sphere;
        sphere.x = 16.0f;
        sphere.y = 16.0f;
        sphere.z = 16.0f;
        sphere.radius = 2.0f;
        sphere.energy = 0.5f;  // Low energy for hard material

        damage_chunk(chunk, sphere, 0.0f, 0.0f, 0.0f);

        // Voxel at edge should not be destroyed (too far from damage sphere)
        bool edge_intact = chunk.get_voxel(0, 0, 0) == MaterialType::Stone;

        test_result("Damage: falloff preserves far voxels", edge_intact);
    }
}
