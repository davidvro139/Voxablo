#include <raylib.h>
#include <cstdio>
#include <vector>

#include "../voxel_core/include/voxel_core.hpp"
#include "../voxel_core/include/meshing.hpp"
#include "../voxel_core/include/generation.hpp"

using namespace voxel;

// Convert voxel mesh to Raylib mesh
Mesh create_raylib_mesh(const ChunkMesh& chunk_mesh) {
    Mesh mesh = { 0 };

    if (chunk_mesh.vertices.empty() || chunk_mesh.indices.empty()) {
        return mesh;
    }

    mesh.vertexCount = static_cast<int>(chunk_mesh.vertices.size() / 3);
    mesh.triangleCount = static_cast<int>(chunk_mesh.indices.size() / 3);

    // Allocate vertex positions
    mesh.vertices = static_cast<float*>(MemAlloc(mesh.vertexCount * 3 * sizeof(float)));
    for (size_t i = 0; i < chunk_mesh.vertices.size(); i++) {
        mesh.vertices[i] = chunk_mesh.vertices[i];
    }

    // Allocate indices
    mesh.indices = static_cast<unsigned short*>(MemAlloc(chunk_mesh.indices.size() * sizeof(unsigned short)));
    for (size_t i = 0; i < chunk_mesh.indices.size(); i++) {
        mesh.indices[i] = static_cast<unsigned short>(chunk_mesh.indices[i]);
    }

    // Default normals and colors
    mesh.normals = static_cast<float*>(MemAlloc(mesh.vertexCount * 3 * sizeof(float)));
    for (int i = 0; i < mesh.vertexCount * 3; i++) {
        mesh.normals[i] = 0.5f;
    }

    mesh.colors = static_cast<unsigned char*>(MemAlloc(mesh.vertexCount * 4 * sizeof(unsigned char)));
    for (int i = 0; i < mesh.vertexCount * 4; i++) {
        mesh.colors[i] = 200;
    }

    return mesh;
}

int main() {
    const int screenWidth = 1920;
    const int screenHeight = 1080;

    InitWindow(screenWidth, screenHeight, "Voxablo - Raylib Viewer");
    SetTargetFPS(60);

    // Setup camera
    Camera3D camera = { 0 };
    camera.position = { 50.0f, 50.0f, 50.0f };
    camera.target = { 0.0f, 0.0f, 0.0f };
    camera.up = { 0.0f, 1.0f, 0.0f };
    camera.fovy = 45.0f;
    camera.projection = CAMERA_PERSPECTIVE;

    // Create voxel world
    VoxelWorld world;
    TerrainGenerator terrain_gen;

    // Load region and generate terrain
    world.load_region(1);

    Chunk chunk;
    terrain_gen.generate_chunk(chunk, 0, 0, 0);

    // Mesh the chunk
    ChunkMesh chunk_mesh = mesh_chunk(chunk);
    Mesh raylib_mesh = create_raylib_mesh(chunk_mesh);
    Model model = { 0 };
    model.meshCount = 1;
    model.meshes = static_cast<Mesh*>(MemAlloc(sizeof(Mesh)));
    model.meshes[0] = raylib_mesh;

    // Material
    Material material = LoadMaterialDefault();
    model.materials = static_cast<Material*>(MemAlloc(sizeof(Material)));
    model.materials[0] = material;
    model.meshMaterial = static_cast<int*>(MemAlloc(sizeof(int)));
    model.meshMaterial[0] = 0;

    Model3D modelTransform = MatrixIdentity();
    Vector3 damage_pos = { 16.0f, 16.0f, 16.0f };
    float damage_radius = 3.0f;

    printf("Raylib Voxel Viewer Started\n");
    printf("Controls:\n");
    printf("  WASD - Move camera\n");
    printf("  Mouse - Look around\n");
    printf("  SPACE - Damage at center\n");
    printf("  C - Collapse unsupported voxels\n");
    printf("  ESC - Quit\n");

    while (!WindowShouldClose()) {
        // Update camera
        UpdateCamera(&camera, CAMERA_FIRST_PERSON);

        // Input: Damage
        if (IsKeyPressed(KEY_SPACE)) {
            DamageSphere damage;
            damage.x = damage_pos.x;
            damage.y = damage_pos.y;
            damage.z = damage_pos.z;
            damage.radius = damage_radius;
            damage.energy = 2.0f;

            world.queue_damage(1, damage);
            printf("Damage applied at (%.1f, %.1f, %.1f)\n", damage.x, damage.y, damage.z);

            // Remesh the chunk
            chunk = Chunk();
            terrain_gen.generate_chunk(chunk, 0, 0, 0);
            chunk_mesh = mesh_chunk(chunk);

            // Update mesh
            UnloadMesh(raylib_mesh);
            raylib_mesh = create_raylib_mesh(chunk_mesh);
            model.meshes[0] = raylib_mesh;
        }

        // Input: Collapse
        if (IsKeyPressed(KEY_C)) {
            world.queue_collapse(1);
            printf("Collapse applied\n");

            // Remesh the chunk
            chunk = Chunk();
            terrain_gen.generate_chunk(chunk, 0, 0, 0);
            chunk_mesh = mesh_chunk(chunk);

            UnloadMesh(raylib_mesh);
            raylib_mesh = create_raylib_mesh(chunk_mesh);
            model.meshes[0] = raylib_mesh;
        }

        // Rendering
        BeginDrawing();
        ClearBackground(RAYWHITE);

        BeginMode3D(camera);

        // Draw terrain
        DrawModel(model, { 0.0f, 0.0f, 0.0f }, 1.0f, GRAY);

        // Draw damage sphere indicator
        DrawSphere(damage_pos, damage_radius, { 255, 0, 0, 100 });
        DrawSphereWires(damage_pos, damage_radius, 8, 8, RED);

        EndMode3D();

        // UI
        DrawText("Voxablo - Raylib", 10, 10, 20, BLACK);
        DrawText(TextFormat("FPS: %d", GetFPS()), 10, 40, 20, BLACK);
        DrawText("SPACE: Damage | C: Collapse | WASD: Move | ESC: Quit", 10, 70, 16, DARKGRAY);

        EndDrawing();
    }

    UnloadModel(model);
    CloseWindow();

    return 0;
}
