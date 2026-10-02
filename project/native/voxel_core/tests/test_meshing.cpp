#include "../include/voxel_core.hpp"
#include "../include/meshing.hpp"

extern void test_result(const char* name, bool condition);

void run_meshing_tests() {
    using namespace voxel;

    std::printf("\n--- Meshing Tests ---\n");

    // Test 1: Empty chunk produces empty mesh
    {
        Chunk chunk;
        ChunkMesh mesh = mesh_chunk(chunk);
        test_result("Meshing: empty chunk", mesh.empty());
    }

    // Test 2: Single voxel produces triangles
    {
        Chunk chunk;
        chunk.set_voxel(0, 0, 0, MaterialType::Dirt);
        ChunkMesh mesh = mesh_chunk(chunk);
        test_result("Meshing: single voxel", mesh.triangle_count() == 12);  // Cube = 6 faces * 2 triangles
    }

    // Test 3: Mesh has valid indices
    {
        Chunk chunk;
        chunk.set_voxel(0, 0, 0, MaterialType::Stone);
        chunk.set_voxel(1, 1, 1, MaterialType::Wood);
        ChunkMesh mesh = mesh_chunk(chunk);

        bool valid = true;
        for (uint32_t idx : mesh.indices) {
            if (idx >= mesh.vertex_count()) {
                valid = false;
                break;
            }
        }
        test_result("Meshing: indices are valid", valid);
    }

    // Test 4: Greedy merging reduces vertex count
    {
        // 2x1x1 flat cube face (should merge)
        Chunk chunk;
        chunk.set_voxel(0, 0, 0, MaterialType::Dirt);
        chunk.set_voxel(1, 0, 0, MaterialType::Dirt);
        ChunkMesh mesh = mesh_chunk(chunk);

        // Two adjacent voxels with same material should have some merged faces
        test_result("Meshing: greedy merging", mesh.vertex_count() < 48);  // 48 = 2 cubes unmerged
    }

    // Test 5: Material is preserved in vertices
    {
        Chunk chunk;
        chunk.set_voxel(0, 0, 0, MaterialType::Brick);
        ChunkMesh mesh = mesh_chunk(chunk);

        bool all_brick = true;
        uint16_t brick_mat = static_cast<uint16_t>(MaterialType::Brick);
        for (const auto& v : mesh.vertices) {
            if (v.material != brick_mat) {
                all_brick = false;
                break;
            }
        }
        test_result("Meshing: material preserved", all_brick);
    }
}
