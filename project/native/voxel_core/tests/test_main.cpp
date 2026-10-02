#include <cstdio>

// Global test counter
int tests_passed = 0;
int tests_failed = 0;

void test_result(const char* name, bool condition) {
    if (condition) {
        std::printf("[PASS] %s\n", name);
        tests_passed++;
    } else {
        std::printf("[FAIL] %s\n", name);
        tests_failed++;
    }
}

int main() {
    extern void run_chunk_storage_tests();
    extern void run_voxel_world_tests();
    extern void run_meshing_tests();
    extern void run_generation_tests();
    extern void run_damage_tests();
    extern void run_structural_analysis_tests();
    extern void run_physics_tests();
    extern void run_structure_tests();

    std::printf("=== Voxel Core Unit Tests ===\n\n");

    run_chunk_storage_tests();
    run_voxel_world_tests();
    run_meshing_tests();
    run_generation_tests();
    run_damage_tests();
    run_structural_analysis_tests();
    run_physics_tests();
    run_structure_tests();

    std::printf("\n=== Test Summary ===\n");
    std::printf("Passed: %d\n", tests_passed);
    std::printf("Failed: %d\n", tests_failed);

    return tests_failed == 0 ? 0 : 1;
}
