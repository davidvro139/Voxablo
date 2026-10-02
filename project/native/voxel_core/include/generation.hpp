#pragma once

#include <cstdint>

namespace voxel {

class Chunk;

// Simple Perlin-like noise generator for terrain
class TerrainGenerator {
public:
    explicit TerrainGenerator(uint64_t seed);

    // Generate a chunk at given coordinate
    // Fills chunk with terrain: air above surface, solid below
    void generate_chunk(Chunk& chunk, int chunk_x, int chunk_y, int chunk_z);

private:
    uint64_t seed;

    // Pseudo-random noise function
    float noise_2d(float x, float y) const;
    float noise_3d(float x, float y, float z) const;

    // Get terrain height at world coordinates
    float get_terrain_height(float world_x, float world_z) const;
};

}  // namespace voxel
