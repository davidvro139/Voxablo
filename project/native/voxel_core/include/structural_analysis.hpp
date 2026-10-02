#pragma once

#include "voxel_core.hpp"
#include <cstdint>
#include <vector>

namespace voxel {

// Voxels at or below this Y rest on bedrock and anchor everything connected to them.
constexpr int32_t ANCHOR_Y = 0;

struct DetachedVoxel {
    int32_t x, y, z;
    MaterialType material;
};

using DetachedPiece = std::vector<DetachedVoxel>;

// Finds solid groups touching the sphere (x, y, z, radius + 1.5) that are no longer
// 6-connected to the anchor layer, removes them from the world and returns them.
// Groups larger than max_piece_voxels are assumed to be supported (keeps the search bounded).
std::vector<DetachedPiece> extract_detached_pieces(VoxelWorld& world, RegionId id,
                                                   float x, float y, float z, float radius,
                                                   size_t max_piece_voxels);

// Structural properties for a voxel
struct VoxelSupport {
    bool is_supported;    // True if voxel has structural support
    bool is_ground;       // True if voxel is on ground level (y=0)
};

// Result of structural analysis
struct StructuralAnalysis {
    int voxels_unsupported;
    int voxels_collapsed;
    bool has_changes;
};

// Analyze structural support in a chunk
// Returns true if any voxels lost support and should collapse
StructuralAnalysis analyze_chunk_support(Chunk& chunk);

// Collapse unsupported voxels in a chunk (removes them to Air)
void collapse_unsupported_voxels(Chunk& chunk);

}  // namespace voxel
