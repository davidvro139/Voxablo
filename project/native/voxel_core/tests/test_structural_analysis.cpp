#include "../include/voxel_core.hpp"
#include "../include/structural_analysis.hpp"
#include <algorithm>
#include <cstdio>

extern void test_result(const char* name, bool condition);

void run_structural_analysis_tests() {
    using namespace voxel;

    std::printf("\n--- Structural Analysis Tests ---\n");

    // Test 1: Ground voxels are always supported
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);
        chunk.set_voxel(10, 0, 10, MaterialType::Stone);

        StructuralAnalysis result = analyze_chunk_support(chunk);
        test_result("Structural: ground voxels are supported",
            result.voxels_unsupported == 0);
    }

    // Test 2: Voxels with support below are supported
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);    // Ground level
        chunk.set_voxel(5, 1, 5, MaterialType::Stone);   // Supported by Dirt
        chunk.set_voxel(5, 2, 5, MaterialType::Wood);    // Supported by Stone

        StructuralAnalysis result = analyze_chunk_support(chunk);
        test_result("Structural: stacked voxels are supported",
            result.voxels_unsupported == 0);
    }

    // Test 3: Floating voxels are unsupported
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);    // Supported (ground)
        chunk.set_voxel(5, 20, 5, MaterialType::Stone);  // Floating voxel (nothing below)

        StructuralAnalysis result = analyze_chunk_support(chunk);
        test_result("Structural: floating voxels detected as unsupported",
            result.voxels_unsupported == 1);
    }

    // Test 4: Collapse removes unsupported voxels
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);    // Supported (ground)
        chunk.set_voxel(5, 20, 5, MaterialType::Stone);  // Floating

        collapse_unsupported_voxels(chunk);

        bool floating_removed = chunk.get_voxel(5, 20, 5) == MaterialType::Air;
        bool ground_voxel_intact = chunk.get_voxel(5, 0, 5) == MaterialType::Dirt;

        test_result("Structural: collapse removes floating voxels",
            floating_removed && ground_voxel_intact);
    }

    // Test 5: Collapsed chunk is marked dirty
    {
        Chunk chunk;
        chunk.set_voxel(5, 20, 5, MaterialType::Stone);
        chunk.mark_clean();

        collapse_unsupported_voxels(chunk);

        test_result("Structural: collapse marks chunk dirty",
            chunk.is_dirty());
    }

    // Test 6: Complex tower collapses from top down
    {
        Chunk chunk;
        // Build a tower with floating top
        for (int y = 0; y < 5; y++) {
            chunk.set_voxel(10, y, 10, MaterialType::Brick);
        }
        chunk.set_voxel(10, 20, 10, MaterialType::Stone);  // Floating block above

        collapse_unsupported_voxels(chunk);

        bool tower_intact = chunk.get_voxel(10, 0, 10) == MaterialType::Brick;
        bool top_removed = chunk.get_voxel(10, 20, 10) == MaterialType::Air;

        test_result("Structural: tower base intact after collapse",
            tower_intact && top_removed);
    }

    // Test 7: Air voxels never collapse
    {
        Chunk chunk;
        // Chunk filled with air (default)

        StructuralAnalysis result = analyze_chunk_support(chunk);
        test_result("Structural: air voxels never unsupported",
            result.voxels_unsupported == 0 && !result.has_changes);
    }

    // Test 8: Multiple floating islands collapse independently
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);     // Supported (ground)
        chunk.set_voxel(5, 20, 5, MaterialType::Stone);   // Floating island 1
        chunk.set_voxel(15, 15, 15, MaterialType::Wood);  // Floating island 2

        StructuralAnalysis result = analyze_chunk_support(chunk);
        test_result("Structural: multiple floating voxels detected",
            result.voxels_unsupported == 2);
    }

    // Test 9: Full collapse analysis result
    {
        Chunk chunk;
        chunk.set_voxel(5, 0, 5, MaterialType::Dirt);    // Supported
        chunk.set_voxel(5, 1, 5, MaterialType::Dirt);    // Supported by below
        chunk.set_voxel(10, 15, 10, MaterialType::Brick); // Floating

        StructuralAnalysis result = analyze_chunk_support(chunk);
        bool analysis_correct = result.voxels_unsupported == 1 && result.has_changes;

        test_result("Structural: analysis counts correct",
            analysis_correct);
    }

    // Test 10: Tall structure with mid-level collapse
    {
        Chunk chunk;
        // Build from ground up
        for (int y = 0; y < 10; y++) {
            chunk.set_voxel(16, y, 16, MaterialType::Stone);
        }
        // Add isolated block at top
        chunk.set_voxel(16, 20, 16, MaterialType::Brick);

        collapse_unsupported_voxels(chunk);

        bool mid_intact = chunk.get_voxel(16, 5, 16) == MaterialType::Stone;
        bool top_removed = chunk.get_voxel(16, 20, 16) == MaterialType::Air;

        test_result("Structural: isolated top removed from tall structure",
            mid_intact && top_removed);
    }

    // World-level detachment. Ground slab at y = 0; a tower at x = 30 whose top carries a bridge
    // across the chunk boundary (x = 32) to a block hanging at x = 35.
    auto build_gallows = [](VoxelWorld& world) {
        world.load_region(1);
        for (int32_t z = 0; z < 10; z++) {
            for (int32_t x = 20; x < 45; x++) {
                world.set_voxel(1, x, 0, z, MaterialType::Dirt);
            }
        }
        for (int32_t y = 1; y <= 10; y++) {
            world.set_voxel(1, 30, y, 5, MaterialType::Brick);
        }
        for (int32_t x = 31; x <= 35; x++) {
            world.set_voxel(1, x, 10, 5, MaterialType::Wood);
        }
        for (int32_t y = 6; y < 10; y++) {
            world.set_voxel(1, 35, y, 5, MaterialType::Stone);
        }
    };

    // Test 11: Intact structure has nothing detached
    {
        VoxelWorld world;
        build_gallows(world);
        auto pieces = extract_detached_pieces(world, 1, 33.0f, 10.0f, 5.0f, 2.0f, 10000);
        test_result("Structural: supported structure is not detached",
                    pieces.empty() && world.get_voxel(1, 35, 6, 5) == MaterialType::Stone);
    }

    // Test 12: Cutting the tower drops everything above the cut, across the chunk boundary
    {
        VoxelWorld world;
        build_gallows(world);
        world.set_voxel(1, 30, 5, 5, MaterialType::Air);
        auto pieces = extract_detached_pieces(world, 1, 30.0f, 5.0f, 5.0f, 0.5f, 10000);

        // Tower y 6..10 (5) + bridge (5) + hanging block (4)
        bool one_piece = pieces.size() == 1 && pieces[0].size() == 14;
        bool removed = world.get_voxel(1, 30, 6, 5) == MaterialType::Air &&
                       world.get_voxel(1, 35, 6, 5) == MaterialType::Air;
        bool base_kept = world.get_voxel(1, 30, 4, 5) == MaterialType::Brick;
        bool materials_kept = one_piece && std::any_of(pieces[0].begin(), pieces[0].end(), [](const DetachedVoxel& v) {
            return v.x == 35 && v.y == 6 && v.material == MaterialType::Stone;
        });
        test_result("Structural: cut tower detaches as one piece across chunks",
                    one_piece && removed && base_kept && materials_kept);
    }

    // Test 13: Cutting the bridge only drops the hanging side
    {
        VoxelWorld world;
        build_gallows(world);
        world.set_voxel(1, 33, 10, 5, MaterialType::Air);
        auto pieces = extract_detached_pieces(world, 1, 33.0f, 10.0f, 5.0f, 0.5f, 10000);
        bool hanging_side = pieces.size() == 1 && pieces[0].size() == 6;  // bridge x 34..35 + block
        bool tower_side_kept = world.get_voxel(1, 32, 10, 5) == MaterialType::Wood;
        test_result("Structural: cut bridge detaches only the unsupported side", hanging_side && tower_side_kept);
    }

    // Test 14: Pieces larger than the limit are treated as supported
    {
        VoxelWorld world;
        build_gallows(world);
        world.set_voxel(1, 30, 5, 5, MaterialType::Air);
        auto pieces = extract_detached_pieces(world, 1, 30.0f, 5.0f, 5.0f, 0.5f, 10);
        test_result("Structural: oversized pieces are left in place",
                    pieces.empty() && world.get_voxel(1, 35, 6, 5) == MaterialType::Stone);
    }

    // Test 15: Separate floating islands come back as separate pieces
    {
        VoxelWorld world;
        world.load_region(1);
        world.set_voxel(1, 10, 5, 10, MaterialType::Stone);
        world.set_voxel(1, 12, 5, 10, MaterialType::Wood);
        auto pieces = extract_detached_pieces(world, 1, 11.0f, 5.0f, 10.0f, 1.0f, 10000);
        test_result("Structural: separate islands are separate pieces", pieces.size() == 2);
    }
}
