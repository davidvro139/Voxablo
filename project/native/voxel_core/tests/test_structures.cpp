#include "../include/voxel_core.hpp"
#include "../include/structures.hpp"
#include <cstdio>

extern void test_result(const char* name, bool condition);

void run_structure_tests() {
    using namespace voxel;
    using namespace voxel::house;

    std::printf("\n--- Structure Tests ---\n");

    VoxelWorld world;
    world.load_region(1);

    // Flat dirt ground, top surface at y = 10.
    for (int32_t z = 0; z < 100; z++) {
        for (int32_t x = 0; x < 110; x++) {
            for (int32_t y = 0; y < 10; y++) {
                world.set_voxel(1, x, y, z, MaterialType::Dirt);
            }
        }
    }

    HouseSpec spec;
    spec.x = 10;
    spec.z = 10;
    const int32_t x0 = spec.x, z0 = spec.z;
    const int32_t z1 = spec.z + spec.depth;
    HouseLayout layout = build_house(world, 1, spec);
    const int32_t fy = layout.floor_y;
    auto at = [&](int32_t x, int32_t y, int32_t z) { return world.get_voxel(1, x, y, z); };

    test_result("Structures: surface_height finds top of ground", surface_height(world, 1, 0, 0, 128) == 10);
    test_result("Structures: floor sits one voxel above highest ground", fy == 11);
    test_result("Structures: stone foundation under floor", at(40, fy - 1, 40) == MaterialType::Stone);
    test_result("Structures: brick outer wall", at(x0, fy + 5, 40) == MaterialType::Brick);
    test_result("Structures: interior is empty", at(40, fy + 10, 40) == MaterialType::Air);
    test_result("Structures: front door is open",
                at(layout.door_x, fy, z1 - 1) == MaterialType::Air &&
                at(layout.door_x, fy + DOOR_HEIGHT - 1, z1 - 1) == MaterialType::Air);
    test_result("Structures: wall above door", at(layout.door_x, fy + DOOR_HEIGHT, z1 - 1) == MaterialType::Brick);
    test_result("Structures: wooden upper floor", at(80, fy + STOREY - 1, 40) == MaterialType::Wood);
    test_result("Structures: wooden roof with brick parapet",
                at(40, fy + 2 * STOREY - 1, 40) == MaterialType::Wood &&
                at(x0, fy + 2 * STOREY + PARAPET - 1, 40) == MaterialType::Brick);

    const int32_t stair_z = z0 + WALL;
    test_result("Structures: first stair step is one rise tall",
                at(layout.stair_x, fy + STAIR_RISE - 1, stair_z) == MaterialType::Wood &&
                at(layout.stair_x, fy + STAIR_RISE, stair_z) == MaterialType::Air);
    const int32_t last_step_x = layout.stair_x + (STOREY / STAIR_RISE - 1) * STAIR_TREAD;
    test_result("Structures: top stair step meets upper floor",
                at(last_step_x, fy + STOREY - 1, stair_z) == MaterialType::Wood &&
                at(last_step_x, fy + STOREY, stair_z) == MaterialType::Air);
    test_result("Structures: stairwell opening in upper floor",
                at(layout.stair_x + 10, fy + STOREY - 1, stair_z) == MaterialType::Air);

    // First window on the -X wall is centred 15 voxels along Z, one per storey.
    const int32_t window_z = z0 + WINDOW_SPACING * 3 / 4;
    test_result("Structures: windows on both storeys",
                at(x0, fy + WINDOW_SILL, window_z) == MaterialType::Air &&
                at(x0, fy + STOREY + WINDOW_SILL, window_z) == MaterialType::Air &&
                at(x0, fy + WINDOW_SILL - 1, window_z) == MaterialType::Brick);

    test_result("Structures: step outside the door is flush with the floor",
                at(layout.door_x, fy - 1, z1) == MaterialType::Stone &&
                at(layout.door_x, fy, z1) == MaterialType::Air);
}
