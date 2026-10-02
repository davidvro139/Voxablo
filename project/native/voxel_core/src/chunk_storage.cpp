#include "../include/voxel_core.hpp"

namespace voxel {

Chunk::Chunk() : dirty(false) {
    voxels.resize(CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE, 0);
}

int Chunk::linear_index(int lx, int ly, int lz) const {
    return lx + ly * CHUNK_SIZE + lz * CHUNK_SIZE * CHUNK_SIZE;
}

MaterialType Chunk::get_voxel(int lx, int ly, int lz) const {
    if (lx < 0 || lx >= CHUNK_SIZE || ly < 0 || ly >= CHUNK_SIZE || lz < 0 || lz >= CHUNK_SIZE) {
        return MaterialType::Air;
    }
    return static_cast<MaterialType>(voxels[linear_index(lx, ly, lz)]);
}

void Chunk::set_voxel(int lx, int ly, int lz, MaterialType material) {
    if (lx < 0 || lx >= CHUNK_SIZE || ly < 0 || ly >= CHUNK_SIZE || lz < 0 || lz >= CHUNK_SIZE) {
        return;
    }
    voxels[linear_index(lx, ly, lz)] = static_cast<uint16_t>(material);
    dirty = true;
}

}  // namespace voxel
