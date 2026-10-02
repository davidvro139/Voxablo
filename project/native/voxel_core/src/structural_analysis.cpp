#include "../include/structural_analysis.hpp"
#include "../include/voxel_core.hpp"
#include <cmath>
#include <unordered_map>

namespace voxel {

namespace {
uint64_t pack_voxel(int32_t x, int32_t y, int32_t z) {
    constexpr uint64_t MASK = (1u << 21) - 1;
    return (static_cast<uint64_t>(static_cast<uint32_t>(x)) & MASK) |
           ((static_cast<uint64_t>(static_cast<uint32_t>(y)) & MASK) << 21) |
           ((static_cast<uint64_t>(static_cast<uint32_t>(z)) & MASK) << 42);
}

// Down is listed last so it is popped first: supported voxels reach the ground quickly.
constexpr int32_t NEIGHBOURS[6][3] = {{0, 1, 0}, {1, 0, 0}, {-1, 0, 0}, {0, 0, 1}, {0, 0, -1}, {0, -1, 0}};
}  // namespace

std::vector<DetachedPiece> extract_detached_pieces(VoxelWorld& world, RegionId id,
                                                   float x, float y, float z, float radius,
                                                   size_t max_piece_voxels) {
    auto solid = [&](int32_t vx, int32_t vy, int32_t vz) {
        return world.get_voxel(id, vx, vy, vz) != MaterialType::Air;
    };

    std::unordered_map<uint64_t, size_t> component_of;
    std::vector<bool> component_anchored;
    std::vector<DetachedPiece> pieces;

    const float search = radius + 1.5f;
    const int32_t reach = static_cast<int32_t>(std::ceil(search));
    const int32_t cx = static_cast<int32_t>(std::floor(x));
    const int32_t cy = static_cast<int32_t>(std::floor(y));
    const int32_t cz = static_cast<int32_t>(std::floor(z));

    for (int32_t dy = -reach; dy <= reach; dy++) {
        for (int32_t dz = -reach; dz <= reach; dz++) {
            for (int32_t dx = -reach; dx <= reach; dx++) {
                const int32_t sx = cx + dx, sy = cy + dy, sz = cz + dz;
                const float ox = static_cast<float>(sx) - x;
                const float oy = static_cast<float>(sy) - y;
                const float oz = static_cast<float>(sz) - z;
                if (ox * ox + oy * oy + oz * oz > search * search || !solid(sx, sy, sz) ||
                    component_of.count(pack_voxel(sx, sy, sz))) {
                    continue;
                }

                const size_t component = component_anchored.size();
                component_anchored.push_back(false);
                component_of[pack_voxel(sx, sy, sz)] = component;

                std::vector<VoxelCoord> members;
                std::vector<VoxelCoord> stack{{sx, sy, sz}};
                bool anchored = false;

                while (!stack.empty() && !anchored) {
                    const VoxelCoord c = stack.back();
                    stack.pop_back();
                    members.push_back(c);
                    if (c.y <= ANCHOR_Y || members.size() > max_piece_voxels) {
                        anchored = true;
                        break;
                    }

                    for (const auto& n : NEIGHBOURS) {
                        const int32_t nx = c.x + n[0], ny = c.y + n[1], nz = c.z + n[2];
                        if (!solid(nx, ny, nz)) {
                            continue;
                        }
                        const uint64_t key = pack_voxel(nx, ny, nz);
                        auto it = component_of.find(key);
                        if (it == component_of.end()) {
                            component_of.emplace(key, component);
                            stack.push_back({nx, ny, nz});
                        } else if (it->second != component && component_anchored[it->second]) {
                            anchored = true;
                            break;
                        }
                    }
                }

                component_anchored[component] = anchored;
                if (anchored) {
                    continue;
                }

                DetachedPiece piece;
                piece.reserve(members.size());
                for (const VoxelCoord& c : members) {
                    piece.push_back({c.x, c.y, c.z, world.get_voxel(id, c.x, c.y, c.z)});
                }
                for (const VoxelCoord& c : members) {
                    world.set_voxel(id, c.x, c.y, c.z, MaterialType::Air);
                }
                pieces.push_back(std::move(piece));
            }
        }
    }

    return pieces;
}

StructuralAnalysis analyze_chunk_support(Chunk& chunk) {
    StructuralAnalysis result = {0, 0, false};

    for (int y = 0; y < CHUNK_SIZE; y++) {
        for (int x = 0; x < CHUNK_SIZE; x++) {
            for (int z = 0; z < CHUNK_SIZE; z++) {
                MaterialType material = chunk.get_voxel(x, y, z);

                if (material == MaterialType::Air) {
                    continue;  // Air voxels don't need support
                }

                bool is_supported = false;

                if (y == 0) {
                    // Ground level voxels are always supported
                    is_supported = true;
                } else {
                    // Check if voxel below has solid material
                    MaterialType below = chunk.get_voxel(x, y - 1, z);
                    if (below != MaterialType::Air) {
                        is_supported = true;
                    }
                }

                if (!is_supported) {
                    result.voxels_unsupported++;
                    result.has_changes = true;
                }
            }
        }
    }

    return result;
}

void collapse_unsupported_voxels(Chunk& chunk) {
    for (int y = 0; y < CHUNK_SIZE; y++) {
        for (int x = 0; x < CHUNK_SIZE; x++) {
            for (int z = 0; z < CHUNK_SIZE; z++) {
                MaterialType material = chunk.get_voxel(x, y, z);

                if (material == MaterialType::Air) {
                    continue;
                }

                bool is_supported = false;

                if (y == 0) {
                    is_supported = true;
                } else {
                    MaterialType below = chunk.get_voxel(x, y - 1, z);
                    if (below != MaterialType::Air) {
                        is_supported = true;
                    }
                }

                if (!is_supported) {
                    chunk.set_voxel(x, y, z, MaterialType::Air);
                    // Note: chunk.set_voxel already marks chunk as dirty
                }
            }
        }
    }
}

}  // namespace voxel
