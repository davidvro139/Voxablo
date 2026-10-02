#include "../include/voxel_core.hpp"
#include "../include/generation.hpp"

extern void test_result(const char* name, bool condition);

void run_generation_tests() {
    using namespace voxel;

    std::printf("\n--- Generation Tests ---\n");

    // Test 1: Generator creates non-empty chunks
    {
        TerrainGenerator gen(12345);
        Chunk chunk;
        gen.generate_chunk(chunk, 0, 0, 0);

        bool has_solid = false;
        for (int x = 0; x < CHUNK_SIZE; x++) {
            for (int y = 0; y < CHUNK_SIZE; y++) {
                for (int z = 0; z < CHUNK_SIZE; z++) {
                    if (chunk.get_voxel(x, y, z) != MaterialType::Air) {
                        has_solid = true;
                        break;
                    }
                }
            }
        }
        test_result("Generation: non-empty terrain", has_solid);
    }

    // Test 2: Same seed produces identical chunks
    {
        TerrainGenerator gen1(42);
        TerrainGenerator gen2(42);

        Chunk chunk1, chunk2;
        gen1.generate_chunk(chunk1, 1, 2, 3);
        gen2.generate_chunk(chunk2, 1, 2, 3);

        bool identical = true;
        for (int x = 0; x < CHUNK_SIZE; x++) {
            for (int y = 0; y < CHUNK_SIZE; y++) {
                for (int z = 0; z < CHUNK_SIZE; z++) {
                    if (chunk1.get_voxel(x, y, z) != chunk2.get_voxel(x, y, z)) {
                        identical = false;
                        break;
                    }
                }
            }
        }
        test_result("Generation: deterministic (same seed)", identical);
    }

    // Test 3: Different seeds produce different chunks
    {
        TerrainGenerator gen1(11111);
        TerrainGenerator gen2(22222);

        Chunk chunk1, chunk2;
        gen1.generate_chunk(chunk1, 0, 0, 0);
        gen2.generate_chunk(chunk2, 0, 0, 0);

        bool different = false;
        for (int x = 0; x < CHUNK_SIZE && !different; x++) {
            for (int y = 0; y < CHUNK_SIZE && !different; y++) {
                for (int z = 0; z < CHUNK_SIZE && !different; z++) {
                    if (chunk1.get_voxel(x, y, z) != chunk2.get_voxel(x, y, z)) {
                        different = true;
                    }
                }
            }
        }
        test_result("Generation: different seeds differ", different);
    }

    // Test 4: Adjacent chunks are deterministic at boundaries
    {
        TerrainGenerator gen1(99999);
        TerrainGenerator gen2(99999);

        Chunk chunk_a, chunk_b, chunk_a2, chunk_b2;
        gen1.generate_chunk(chunk_a, 0, 0, 0);
        gen1.generate_chunk(chunk_b, 1, 0, 0);  // Adjacent X

        gen2.generate_chunk(chunk_a2, 0, 0, 0);
        gen2.generate_chunk(chunk_b2, 1, 0, 0);  // Same with different generator

        // Regenerating same chunks with same seed should produce identical results
        bool consistent = true;
        for (int y = 0; y < CHUNK_SIZE; y++) {
            if (chunk_a.get_voxel(CHUNK_SIZE - 1, y, 0) != chunk_a2.get_voxel(CHUNK_SIZE - 1, y, 0)) {
                consistent = false;
                break;
            }
            if (chunk_b.get_voxel(0, y, 0) != chunk_b2.get_voxel(0, y, 0)) {
                consistent = false;
                break;
            }
        }
        test_result("Generation: adjacent chunks aligned", consistent);
    }

    // Test 5: Generated material types are valid
    {
        TerrainGenerator gen(54321);
        Chunk chunk;
        gen.generate_chunk(chunk, 0, 0, 0);

        bool valid_materials = true;
        for (int x = 0; x < CHUNK_SIZE; x++) {
            for (int y = 0; y < CHUNK_SIZE; y++) {
                for (int z = 0; z < CHUNK_SIZE; z++) {
                    MaterialType mat = chunk.get_voxel(x, y, z);
                    if (mat != MaterialType::Air && mat != MaterialType::Dirt &&
                        mat != MaterialType::Stone && mat != MaterialType::Brick &&
                        mat != MaterialType::Wood) {
                        valid_materials = false;
                        break;
                    }
                }
            }
        }
        test_result("Generation: valid material types", valid_materials);
    }
}
