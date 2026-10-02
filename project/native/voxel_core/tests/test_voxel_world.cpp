#include "../include/voxel_core.hpp"

extern void test_result(const char* name, bool condition);

void run_voxel_world_tests() {
    using namespace voxel;

    std::printf("\n--- VoxelWorld Tests ---\n");

    {
        VoxelWorld world;
        world.load_region(1);
        world.queue_damage(1, {31.5f, 8.0f, 8.0f, 3.0f, 10.0f});
        test_result("Damage: empty world blast allocates no chunks", world.loaded_chunk_count() == 0);
        world.set_voxel(1, 31, 8, 8, MaterialType::Brick);
        world.set_voxel(1, 32, 8, 8, MaterialType::Brick);
        world.queue_damage(1, {31.5f, 8.0f, 8.0f, 3.0f, 10.0f});
        test_result("Damage: blast crosses chunk boundary",
            world.get_voxel(1, 31, 8, 8) == MaterialType::Air &&
            world.get_voxel(1, 32, 8, 8) == MaterialType::Air);
        test_result("Damage: boundary blast preserves loaded chunk count", world.loaded_chunk_count() == 2);
        test_result("Damage: both affected chunks remain dirty", world.dirty_chunk_count() == 2);
        world.set_voxel(1, 31, 8, 8, MaterialType::Brick);
        world.queue_damage(1, {31.0f, 8.0f, 8.0f, 0.0f, 10.0f});
        world.queue_damage(1, {31.0f, 8.0f, 8.0f, -3.0f, 10.0f});
        world.queue_damage(1, {31.0f, 8.0f, 8.0f, 3.0f, 0.0f});
        test_result("Damage: invalid radius and zero energy leave voxels intact",
            world.get_voxel(1, 31, 8, 8) == MaterialType::Brick);
    }

    // Test 1: Load/unload regions
    {
        VoxelWorld world;
        RegionId region_id = 12345;

        world.load_region(region_id);
        test_result("VoxelWorld: load region", world.loaded_chunk_count() == 0);

        world.unload_region(region_id);
        test_result("VoxelWorld: unload region", world.loaded_chunk_count() == 0);
    }

    // Test 2: Queue edits
    {
        VoxelWorld world;
        RegionId region_id = 12345;
        world.load_region(region_id);

        VoxelEdit edits[] = {
            {0, 0, 0, MaterialType::Dirt},
            {1, 0, 0, MaterialType::Stone},
            {0, 1, 0, MaterialType::Wood},
        };

        world.queue_edits(region_id, std::span(edits));
        test_result("VoxelWorld: queue edits", world.loaded_chunk_count() > 0);
    }

    // Test 3: Loaded chunk count
    {
        VoxelWorld world;
        RegionId region_id = 12345;
        world.load_region(region_id);

        VoxelEdit edits[] = {
            {0, 0, 0, MaterialType::Dirt},
            {32, 32, 32, MaterialType::Stone},  // Different chunk
        };

        world.queue_edits(region_id, std::span(edits));
        test_result("VoxelWorld: multiple chunks", world.loaded_chunk_count() == 2);
    }

    // Test 4: Dirty chunk tracking
    {
        VoxelWorld world;
        RegionId region_id = 12345;
        world.load_region(region_id);

        VoxelEdit edits[] = {
            {0, 0, 0, MaterialType::Dirt},
        };

        world.queue_edits(region_id, std::span(edits));
        test_result("VoxelWorld: dirty chunks tracked", world.dirty_chunk_count() > 0);
    }
}
