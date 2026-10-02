#include "../include/generation.hpp"
#include "../include/voxel_core.hpp"
#include <cmath>

namespace voxel {

TerrainGenerator::TerrainGenerator(uint64_t seed) : seed(seed) {}

// Hash function for deterministic noise
static uint64_t hash64(uint64_t x, uint64_t y, uint64_t z, uint64_t seed) {
    uint64_t h = seed;
    h ^= x * 2654435761U;
    h ^= y * 2246822519U;
    h ^= z * 3266489917U;
    h = (h ^ (h >> 33)) * 0xff51afd7ed558ccdUL;
    h = (h ^ (h >> 33));
    return h;
}

// Convert hash to float in range [0, 1)
static float hash_to_float(uint64_t h) {
    return (h & 0x7FFFFFFFU) / 2147483648.0f;
}

float TerrainGenerator::noise_3d(float x, float y, float z) const {
    int xi = (int)std::floor(x);
    int yi = (int)std::floor(y);
    int zi = (int)std::floor(z);

    float xf = x - xi;
    float yf = y - yi;
    float zf = z - zi;

    // Smooth step interpolation
    auto fade = [](float t) { return t * t * t * (t * (t * 6 - 15) + 10); };
    float u = fade(xf);
    float v = fade(yf);
    float w = fade(zf);

    // 8 corner hashes
    uint64_t h000 = hash64(xi, yi, zi, seed);
    uint64_t h001 = hash64(xi, yi, zi + 1, seed);
    uint64_t h010 = hash64(xi, yi + 1, zi, seed);
    uint64_t h011 = hash64(xi, yi + 1, zi + 1, seed);
    uint64_t h100 = hash64(xi + 1, yi, zi, seed);
    uint64_t h101 = hash64(xi + 1, yi, zi + 1, seed);
    uint64_t h110 = hash64(xi + 1, yi + 1, zi, seed);
    uint64_t h111 = hash64(xi + 1, yi + 1, zi + 1, seed);

    // Interpolate
    float n000 = hash_to_float(h000);
    float n001 = hash_to_float(h001);
    float n010 = hash_to_float(h010);
    float n011 = hash_to_float(h011);
    float n100 = hash_to_float(h100);
    float n101 = hash_to_float(h101);
    float n110 = hash_to_float(h110);
    float n111 = hash_to_float(h111);

    float nx00 = n000 * (1 - u) + n100 * u;
    float nx01 = n001 * (1 - u) + n101 * u;
    float nx10 = n010 * (1 - u) + n110 * u;
    float nx11 = n011 * (1 - u) + n111 * u;

    float nxy0 = nx00 * (1 - v) + nx10 * v;
    float nxy1 = nx01 * (1 - v) + nx11 * v;

    return nxy0 * (1 - w) + nxy1 * w;
}

float TerrainGenerator::noise_2d(float x, float y) const {
    return noise_3d(x, y, 0.0f);
}

float TerrainGenerator::get_terrain_height(float world_x, float world_z) const {
    // Multi-octave noise for interesting terrain
    float height = 0.0f;
    float amplitude = 1.0f;
    float frequency = 0.015f;  // Lowest octave spans ~65 voxels (6.5 m)
    float max_amplitude = 0.0f;

    for (int octave = 0; octave < 4; octave++) {
        height += noise_2d(world_x * frequency, world_z * frequency) * amplitude;
        max_amplitude += amplitude;
        amplitude *= 0.5f;
        frequency *= 2.0f;
    }

    height /= max_amplitude;

    // Scale and shift to reasonable terrain height
    // Range: 10 to 30 voxels high
    return 15.0f + height * 10.0f;
}

void TerrainGenerator::generate_chunk(Chunk& chunk, int chunk_x, int chunk_y, int chunk_z) {
    // Convert chunk coordinates to world coordinates
    float world_base_x = static_cast<float>(chunk_x * CHUNK_SIZE);
    float world_base_y = static_cast<float>(chunk_y * CHUNK_SIZE);
    float world_base_z = static_cast<float>(chunk_z * CHUNK_SIZE);

    // Fill chunk with terrain
    for (int lx = 0; lx < CHUNK_SIZE; lx++) {
        for (int lz = 0; lz < CHUNK_SIZE; lz++) {
            float world_x = world_base_x + lx;
            float world_z = world_base_z + lz;

            float terrain_height = get_terrain_height(world_x, world_z);

            for (int ly = 0; ly < CHUNK_SIZE; ly++) {
                float world_y = world_base_y + ly;

                MaterialType material = MaterialType::Air;

                if (world_y < terrain_height) {
                    // Underground: vary material by depth
                    float depth = terrain_height - world_y;
                    if (depth < 3.0f) {
                        material = MaterialType::Dirt;  // Top soil
                    } else if (depth < 10.0f) {
                        material = MaterialType::Stone;  // Stone layer
                    } else {
                        // Deeper stone
                        uint64_t h = hash64(lx, ly, lz, seed);
                        if (hash_to_float(h) > 0.8f) {
                            material = MaterialType::Brick;  // Occasional brick
                        } else {
                            material = MaterialType::Stone;
                        }
                    }
                } else if (world_y < terrain_height + 1.0f) {
                    // Surface grass (dirt)
                    material = MaterialType::Dirt;
                }

                chunk.set_voxel(lx, ly, lz, material);
            }
        }
    }

    // Mark chunk as clean (it's freshly generated, not player-modified)
    chunk.mark_clean();
}

}  // namespace voxel
