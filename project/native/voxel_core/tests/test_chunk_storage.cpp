#include "../include/voxel_core.hpp"

extern void test_result(const char* name, bool condition);

void run_chunk_storage_tests() {
    using namespace voxel;

    std::printf("\n--- Chunk Storage Tests ---\n");

    // Test 1: Create and read/write voxels
    {
        Chunk chunk;
        chunk.set_voxel(0, 0, 0, MaterialType::Dirt);
        chunk.set_voxel(1, 2, 3, MaterialType::Stone);

        test_result("Chunk: set/get voxel",
            chunk.get_voxel(0, 0, 0) == MaterialType::Dirt &&
            chunk.get_voxel(1, 2, 3) == MaterialType::Stone &&
            chunk.get_voxel(5, 5, 5) == MaterialType::Air);
    }

    // Test 2: Chunk dirty flag
    {
        Chunk chunk;
        test_result("Chunk: clean on creation", !chunk.is_dirty());

        chunk.set_voxel(0, 0, 0, MaterialType::Wood);
        test_result("Chunk: marks dirty on edit", chunk.is_dirty());

        chunk.mark_clean();
        test_result("Chunk: can mark clean", !chunk.is_dirty());
    }

    // Test 3: Out-of-bounds access
    {
        Chunk chunk;
        chunk.set_voxel(-1, 0, 0, MaterialType::Dirt);  // Should be safe
        chunk.set_voxel(32, 0, 0, MaterialType::Dirt);  // Should be safe
        chunk.set_voxel(0, 0, 32, MaterialType::Dirt);  // Should be safe

        test_result("Chunk: out-of-bounds writes are safe",
            chunk.get_voxel(0, 0, 0) == MaterialType::Air);
    }

    // Test 4: Chunk size constant
    {
        test_result("Chunk size constant", voxel::CHUNK_SIZE == 32);
    }
}
