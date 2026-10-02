#include "../include/voxel_core.hpp"
#include "../include/physics.hpp"

extern void test_result(const char* name, bool condition);

void run_physics_tests() {
    using namespace voxel;

    std::printf("\n--- Physics Tests ---\n");

    // Test 1: Fragment creation
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        bool fragment_created = physics.active_fragments.size() == 1;
        bool fragment_in_air = !physics.active_fragments[0].is_settled;

        test_result("Physics: fragment creation",
            fragment_created && fragment_in_air);
    }

    // Test 2: Fragments fall under gravity
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        float initial_y = physics.active_fragments[0].y;
        update_fragments(physics, 1.0f);  // 1 second
        float final_y = physics.active_fragments[0].y;

        bool fell_down = final_y < initial_y;
        test_result("Physics: gravity pulls fragments down",
            fell_down);
    }

    // Test 3: Velocity accumulates over time
    {
        PhysicsState physics;
        physics.gravity_strength = -1.0f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        update_fragments(physics, 1.0f);
        float v1 = physics.active_fragments[0].vy;

        update_fragments(physics, 1.0f);
        float v2 = physics.active_fragments[0].vy;

        bool velocity_increased = v2 < v1;  // More negative = faster fall
        test_result("Physics: velocity accumulates",
            velocity_increased);
    }

    // Test 4: Fragment settling with max time
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 10;  // Short timeout

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        // Simulate 15 frames
        for (int i = 0; i < 15; i++) {
            update_fragments(physics, 1.0f);
        }

        bool fragment_settled = physics.active_fragments[0].is_settled;
        test_result("Physics: fragments settle after timeout",
            fragment_settled);
    }

    // Test 5: Multiple fragments
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5, 10, 15, 10, 15, 12, 15};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        bool correct_count = physics.active_fragments.size() == 3;
        bool all_in_air = !physics.active_fragments[0].is_settled &&
                          !physics.active_fragments[1].is_settled &&
                          !physics.active_fragments[2].is_settled;

        test_result("Physics: multiple fragments created",
            correct_count && all_in_air);
    }

    // Test 6: Ground-level fragments settle immediately
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        Chunk chunk;
        std::vector<Chunk> chunks = {chunk};

        std::vector<int> voxel_positions = {5, 0, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        settle_collided_fragments(physics, chunks, 0.0f, 0.0f, 0.0f);

        bool fragment_settled = physics.active_fragments[0].is_settled;
        test_result("Physics: ground-level fragments settle",
            fragment_settled);
    }

    // Test 7: Fragment position updates
    {
        PhysicsState physics;
        physics.gravity_strength = 0.0f;  // No gravity for predictable motion
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        physics.active_fragments[0].vx = 2.0f;
        physics.active_fragments[0].vy = -1.0f;

        float init_x = physics.active_fragments[0].x;
        float init_y = physics.active_fragments[0].y;

        update_fragments(physics, 1.0f);

        float new_x = physics.active_fragments[0].x;
        float new_y = physics.active_fragments[0].y;

        // Use tolerance for floating point comparison
        float epsilon = 0.001f;
        bool moved_right = (new_x - (init_x + 2.0f)) < epsilon && (new_x - (init_x + 2.0f)) > -epsilon;
        bool moved_down = (new_y - (init_y - 1.0f)) < epsilon && (new_y - (init_y - 1.0f)) > -epsilon;

        test_result("Physics: fragments move with velocity",
            moved_right && moved_down);
    }

    // Test 8: Settled fragments flag preserved
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        physics.active_fragments[0].is_settled = true;
        update_fragments(physics, 1.0f);

        // Settled fragments still update (is_settled is just a flag)
        // but they'll be removed by settle_collided_fragments
        test_result("Physics: settled flag is set",
            physics.active_fragments[0].is_settled);
    }

    // Test 9: Fragment material preserved
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        uint16_t material = physics.active_fragments[0].material;
        bool has_material = material != 0;

        test_result("Physics: fragment has material",
            has_material);
    }

    // Test 10: Falling time tracking
    {
        PhysicsState physics;
        physics.gravity_strength = -0.98f;
        physics.max_falling_time = 300;

        std::vector<int> voxel_positions = {5, 10, 5};
        create_fragments_from_voxels(physics, voxel_positions, 0.0f, 0.0f, 0.0f);

        float initial_time = physics.active_fragments[0].time_falling;
        update_fragments(physics, 2.5f);
        float final_time = physics.active_fragments[0].time_falling;

        bool time_increased = final_time == initial_time + 2.5f;
        test_result("Physics: falling time increases",
            time_increased);
    }
}
