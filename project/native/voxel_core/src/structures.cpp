#include "../include/structures.hpp"
#include <algorithm>
#include <climits>

namespace voxel {

namespace {
constexpr int32_t MAX_SCAN_Y = 4 * CHUNK_SIZE;
constexpr int32_t MAX_FRONT_STEPS = 40;

void fill(VoxelWorld& world, RegionId id, int32_t x0, int32_t y0, int32_t z0,
          int32_t x1, int32_t y1, int32_t z1, MaterialType material) {
    for (int32_t y = y0; y < y1; y++) {
        for (int32_t z = z0; z < z1; z++) {
            for (int32_t x = x0; x < x1; x++) {
                world.set_voxel(id, x, y, z, material);
            }
        }
    }
}
}  // namespace

int32_t surface_height(const VoxelWorld& world, RegionId id, int32_t x, int32_t z, int32_t max_y) {
    for (int32_t y = max_y - 1; y >= 0; y--) {
        if (world.get_voxel(id, x, y, z) != MaterialType::Air) {
            return y + 1;
        }
    }
    return 0;
}

HouseLayout build_house(VoxelWorld& world, RegionId id, const HouseSpec& spec) {
    using namespace house;

    const int32_t x0 = spec.x, x1 = spec.x + spec.width;
    const int32_t z0 = spec.z, z1 = spec.z + spec.depth;

    int32_t ground_min = INT32_MAX, ground_max = 0;
    for (int32_t z = z0; z < z1; z++) {
        for (int32_t x = x0; x < x1; x++) {
            int32_t h = surface_height(world, id, x, z, MAX_SCAN_Y);
            ground_min = std::min(ground_min, h);
            ground_max = std::max(ground_max, h);
        }
    }

    const int32_t floor_y = ground_max + 1;
    const int32_t roof_top = floor_y + spec.storeys * STOREY;
    const int32_t door_x = x0 + spec.width / 2;
    const int32_t stair_x = x0 + WALL + STAIR_INSET;
    const int32_t stair_steps = STOREY / STAIR_RISE;
    const int32_t stair_z0 = z0 + WALL, stair_z1 = z0 + WALL + STAIR_WIDTH;

    // Foundation, then clear the volume above it.
    fill(world, id, x0, std::max(0, ground_min - 2), z0, x1, floor_y, z1, MaterialType::Stone);
    fill(world, id, x0, floor_y, z0, x1, roof_top + PARAPET, z1, MaterialType::Air);

    // Outer walls.
    for (int32_t y = floor_y; y < roof_top + PARAPET; y++) {
        for (int32_t z = z0; z < z1; z++) {
            for (int32_t x = x0; x < x1; x++) {
                bool on_perimeter = x < x0 + WALL || x >= x1 - WALL || z < z0 + WALL || z >= z1 - WALL;
                if (on_perimeter) {
                    world.set_voxel(id, x, y, z, MaterialType::Brick);
                }
            }
        }
    }

    // Upper floors and roof, with a stairwell opening in the first upper floor.
    for (int32_t s = 1; s <= spec.storeys; s++) {
        int32_t top = floor_y + s * STOREY;
        fill(world, id, x0 + WALL, top - SLAB, z0 + WALL, x1 - WALL, top, z1 - WALL, MaterialType::Wood);
    }
    if (spec.storeys > 1) {
        int32_t top = floor_y + STOREY;
        fill(world, id, stair_x, top - SLAB, stair_z0, stair_x + stair_steps * STAIR_TREAD, top, stair_z1,
             MaterialType::Air);
    }

    // Staircase from the ground floor to the first upper floor.
    if (spec.storeys > 1) {
        for (int32_t i = 0; i < stair_steps; i++) {
            int32_t sx = stair_x + i * STAIR_TREAD;
            fill(world, id, sx, floor_y, stair_z0, sx + STAIR_TREAD, floor_y + (i + 1) * STAIR_RISE, stair_z1,
                 MaterialType::Wood);
        }
    }

    // Ground-floor partition with a doorway, beyond the end of the staircase.
    const int32_t partition_x = x0 + spec.width * 7 / 10;
    fill(world, id, partition_x, floor_y, z0 + WALL, partition_x + WALL, floor_y + STOREY - SLAB, z1 - WALL,
         MaterialType::Brick);
    const int32_t inner_door_z = z0 + spec.depth / 2 - DOOR_WIDTH / 2;
    fill(world, id, partition_x, floor_y, inner_door_z, partition_x + WALL, floor_y + DOOR_HEIGHT,
         inner_door_z + DOOR_WIDTH, MaterialType::Air);

    // Windows on every storey. Openings run through the full wall thickness.
    for (int32_t s = 0; s < spec.storeys; s++) {
        int32_t wy0 = floor_y + s * STOREY + WINDOW_SILL, wy1 = wy0 + WINDOW_HEIGHT;

        for (int32_t c = x0 + WINDOW_SPACING * 3 / 4; c + WINDOW_WIDTH / 2 <= x1 - WALL * 4; c += WINDOW_SPACING) {
            int32_t wx0 = c - WINDOW_WIDTH / 2, wx1 = c + WINDOW_WIDTH / 2;
            bool clashes_with_door = s == 0 && wx1 > door_x - DOOR_WIDTH && wx0 < door_x + DOOR_WIDTH;
            fill(world, id, wx0, wy0, z0, wx1, wy1, z0 + WALL, MaterialType::Air);
            if (!clashes_with_door) {
                fill(world, id, wx0, wy0, z1 - WALL, wx1, wy1, z1, MaterialType::Air);
            }
        }
        for (int32_t c = z0 + WINDOW_SPACING * 3 / 4; c + WINDOW_WIDTH / 2 <= z1 - WALL * 4; c += WINDOW_SPACING) {
            int32_t wz0 = c - WINDOW_WIDTH / 2, wz1 = c + WINDOW_WIDTH / 2;
            fill(world, id, x0, wy0, wz0, x0 + WALL, wy1, wz1, MaterialType::Air);
            fill(world, id, x1 - WALL, wy0, wz0, x1, wy1, wz1, MaterialType::Air);
        }
    }

    // Front door.
    fill(world, id, door_x - DOOR_WIDTH / 2, floor_y, z1 - WALL, door_x + DOOR_WIDTH / 2, floor_y + DOOR_HEIGHT, z1,
         MaterialType::Air);

    // Stone steps from the door down to the terrain: 1 voxel lower every 2 voxels outward.
    const int32_t steps_x0 = door_x - DOOR_WIDTH / 2 - 2, steps_x1 = door_x + DOOR_WIDTH / 2 + 2;
    for (int32_t r = 0; r < MAX_FRONT_STEPS; r++) {
        int32_t z = z1 + r;
        int32_t top = floor_y - (r + 1) / 2;
        bool reached_ground = true;
        for (int32_t x = steps_x0; x < steps_x1; x++) {
            int32_t ground = surface_height(world, id, x, z, MAX_SCAN_Y);
            if (ground < top) {
                reached_ground = false;
                fill(world, id, x, ground, z, x + 1, top, z + 1, MaterialType::Stone);
            }
            // Keep head room over the steps if a hill rises in front of the door.
            fill(world, id, x, top, z, x + 1, top + DOOR_HEIGHT, z + 1, MaterialType::Air);
        }
        if (reached_ground) {
            break;
        }
    }

    return {floor_y, door_x, stair_x};
}

}  // namespace voxel
