#pragma once

#include "voxel_core.hpp"

namespace voxel {

// House dimensions in voxels (10 cm each).
namespace house {
constexpr int32_t WALL = 2;            // Wall thickness
constexpr int32_t STOREY = 30;         // Floor-to-floor height, including the floor slab
constexpr int32_t SLAB = 2;            // Floor / roof thickness
constexpr int32_t PARAPET = 5;         // Wall height above the roof
constexpr int32_t DOOR_WIDTH = 10;
constexpr int32_t DOOR_HEIGHT = 21;
constexpr int32_t WINDOW_WIDTH = 10;
constexpr int32_t WINDOW_HEIGHT = 12;
constexpr int32_t WINDOW_SILL = 9;
constexpr int32_t WINDOW_SPACING = 20;
constexpr int32_t STAIR_WIDTH = 10;
constexpr int32_t STAIR_RISE = 2;      // Per step; must stay within the player's step height
constexpr int32_t STAIR_TREAD = 3;
constexpr int32_t STAIR_INSET = 2;     // Gap between the side wall and the first step
}  // namespace house

struct HouseSpec {
    int32_t x = 0;         // Minimum corner of the footprint
    int32_t z = 0;
    int32_t width = 80;    // Along X
    int32_t depth = 60;    // Along Z; the front door is on the +Z wall
    int32_t storeys = 2;
};

struct HouseLayout {
    int32_t floor_y;       // Ground-floor walking surface (top of the foundation)
    int32_t door_x;        // Centre of the front door along X
    int32_t stair_x;       // First step of the staircase (runs toward +X along the back wall)
};

// Brick walls, wooden floors/roof/stairs, stone foundation levelled above the highest terrain
// under the footprint, and stone steps from the door down to the ground.
HouseLayout build_house(VoxelWorld& world, RegionId id, const HouseSpec& spec);

// Y of the first empty voxel above the highest solid voxel in the column (0 if none below max_y).
int32_t surface_height(const VoxelWorld& world, RegionId id, int32_t x, int32_t z, int32_t max_y);

}  // namespace voxel
