#include "../include/meshing.hpp"
#include "../include/voxel_core.hpp"
#include <cstring>

namespace voxel {

// Encode normal from direction (6 directions)
void encode_normal(int face_dir, uint8_t& nx, uint8_t& ny, uint8_t& nz) {
    switch (face_dir) {
        case 0: nx = 255; ny = 128; nz = 128; break;  // +X
        case 1: nx = 0;   ny = 128; nz = 128; break;  // -X
        case 2: nx = 128; ny = 255; nz = 128; break;  // +Y
        case 3: nx = 128; ny = 0;   nz = 128; break;  // -Y
        case 4: nx = 128; ny = 128; nz = 255; break;  // +Z
        case 5: nx = 128; ny = 128; nz = 0;   break;  // -Z
    }
}

struct QuadFace {
    int x, y, z;        // Origin voxel
    int width, height;  // Quad dimensions
    MaterialType material;
    int face_dir;
};

// Extract quads for a single face direction
std::vector<QuadFace> extract_quads(const Chunk& chunk, int face_dir) {
    std::vector<QuadFace> quads;
    bool mask[CHUNK_SIZE][CHUNK_SIZE];

    // Determine iteration order based on face direction
    auto should_draw = [&](int x, int y, int z, int fd) -> bool {
        MaterialType voxel = chunk.get_voxel(x, y, z);
        if (voxel == MaterialType::Air) return false;

        MaterialType neighbor;
        switch (fd) {
            case 0: neighbor = chunk.get_voxel(x + 1, y, z); break;  // +X
            case 1: neighbor = chunk.get_voxel(x - 1, y, z); break;  // -X
            case 2: neighbor = chunk.get_voxel(x, y + 1, z); break;  // +Y
            case 3: neighbor = chunk.get_voxel(x, y - 1, z); break;  // -Y
            case 4: neighbor = chunk.get_voxel(x, y, z + 1); break;  // +Z
            case 5: neighbor = chunk.get_voxel(x, y, z - 1); break;  // -Z
            default: return false;
        }
        return neighbor == MaterialType::Air;  // Draw if neighbor is air
    };

    // Process each layer (u, v plane)
    for (int w = 0; w < CHUNK_SIZE; w++) {
        std::memset(mask, 0, sizeof(mask));

        // Determine which faces are visible
        for (int v = 0; v < CHUNK_SIZE; v++) {
            for (int u = 0; u < CHUNK_SIZE; u++) {
                int x = 0, y = 0, z = 0;
                switch (face_dir) {
                    case 0: x = w; y = v; z = u; break;  // +X
                    case 1: x = w; y = v; z = u; break;  // -X
                    case 2: x = u; y = w; z = v; break;  // +Y
                    case 3: x = u; y = w; z = v; break;  // -Y
                    case 4: x = u; y = v; z = w; break;  // +Z
                    case 5: x = u; y = v; z = w; break;  // -Z
                }
                mask[v][u] = should_draw(x, y, z, face_dir);
            }
        }

        // Greedy merge: create quads from mask
        for (int v = 0; v < CHUNK_SIZE; v++) {
            for (int u = 0; u < CHUNK_SIZE; u++) {
                if (!mask[v][u]) continue;

                // Find width of this quad
                int width = 1;
                while (u + width < CHUNK_SIZE && mask[v][u + width]) {
                    width++;
                }

                // Find height of this quad (all rows with same width)
                int height = 1;
                bool can_extend = true;
                while (v + height < CHUNK_SIZE && can_extend) {
                    for (int uu = 0; uu < width; uu++) {
                        if (!mask[v + height][u + uu]) {
                            can_extend = false;
                            break;
                        }
                    }
                    if (can_extend) height++;
                }

                // Record quad
                int x = 0, y = 0, z = 0;
                switch (face_dir) {
                    case 0: x = w; y = v; z = u; break;
                    case 1: x = w; y = v; z = u; break;
                    case 2: x = u; y = w; z = v; break;
                    case 3: x = u; y = w; z = v; break;
                    case 4: x = u; y = v; z = w; break;
                    case 5: x = u; y = v; z = w; break;
                }
                MaterialType material = chunk.get_voxel(x, y, z);
                quads.push_back({x, y, z, width, height, material, face_dir});

                // Mark as processed
                for (int hh = 0; hh < height; hh++) {
                    for (int ww = 0; ww < width; ww++) {
                        mask[v + hh][u + ww] = false;
                    }
                }
            }
        }
    }

    return quads;
}

// Convert quad to 2 triangles (6 vertices, 6 indices)
void quad_to_triangles(const QuadFace& quad, ChunkMesh& mesh) {
    uint8_t nx, ny, nz;
    encode_normal(quad.face_dir, nx, ny, nz);

    float x = static_cast<float>(quad.x);
    float y = static_cast<float>(quad.y);
    float z = static_cast<float>(quad.z);
    float w = static_cast<float>(quad.width);
    float h = static_cast<float>(quad.height);

    uint32_t base_idx = static_cast<uint32_t>(mesh.vertices.size());
    uint16_t mat = static_cast<uint16_t>(quad.material);

    // Create 4 corner vertices
    Vertex corners[4];
    switch (quad.face_dir) {
        case 0:  // +X face
            corners[0] = {x + 1, y,     z,     mat, nx, ny, nz};
            corners[1] = {x + 1, y + h, z,     mat, nx, ny, nz};
            corners[2] = {x + 1, y + h, z + w, mat, nx, ny, nz};
            corners[3] = {x + 1, y,     z + w, mat, nx, ny, nz};
            break;
        case 1:  // -X face
            corners[0] = {x,     y,     z + w, mat, nx, ny, nz};
            corners[1] = {x,     y + h, z + w, mat, nx, ny, nz};
            corners[2] = {x,     y + h, z,     mat, nx, ny, nz};
            corners[3] = {x,     y,     z,     mat, nx, ny, nz};
            break;
        case 2:  // +Y face
            corners[0] = {x,     y + 1, z,     mat, nx, ny, nz};
            corners[1] = {x + w, y + 1, z,     mat, nx, ny, nz};
            corners[2] = {x + w, y + 1, z + h, mat, nx, ny, nz};
            corners[3] = {x,     y + 1, z + h, mat, nx, ny, nz};
            break;
        case 3:  // -Y face
            corners[0] = {x + w, y,     z,     mat, nx, ny, nz};
            corners[1] = {x,     y,     z,     mat, nx, ny, nz};
            corners[2] = {x,     y,     z + h, mat, nx, ny, nz};
            corners[3] = {x + w, y,     z + h, mat, nx, ny, nz};
            break;
        case 4:  // +Z face
            corners[0] = {x,     y, z + 1, mat, nx, ny, nz};
            corners[1] = {x,     y + h, z + 1, mat, nx, ny, nz};
            corners[2] = {x + w, y + h, z + 1, mat, nx, ny, nz};
            corners[3] = {x + w, y, z + 1, mat, nx, ny, nz};
            break;
        case 5:  // -Z face
            corners[0] = {x + w, y, z, mat, nx, ny, nz};
            corners[1] = {x + w, y + h, z, mat, nx, ny, nz};
            corners[2] = {x, y + h, z, mat, nx, ny, nz};
            corners[3] = {x, y, z, mat, nx, ny, nz};
            break;
    }

    // Add vertices
    for (int i = 0; i < 4; i++) {
        mesh.vertices.push_back(corners[i]);
    }

    // Triangles must be counter-clockwise seen from outside. The corner order
    // above yields that for +/-X but the reverse for +/-Y and +/-Z.
    const bool flip = quad.face_dir >= 2;
    const uint32_t a = base_idx + (flip ? 2 : 1);
    const uint32_t b = base_idx + (flip ? 1 : 2);
    const uint32_t c = base_idx + (flip ? 3 : 2);
    const uint32_t d = base_idx + (flip ? 2 : 3);

    mesh.indices.push_back(base_idx);
    mesh.indices.push_back(a);
    mesh.indices.push_back(b);

    mesh.indices.push_back(base_idx);
    mesh.indices.push_back(c);
    mesh.indices.push_back(d);
}

ChunkMesh mesh_chunk(const Chunk& chunk) {
    ChunkMesh mesh;

    // Process all 6 face directions
    for (int face_dir = 0; face_dir < 6; face_dir++) {
        auto quads = extract_quads(chunk, face_dir);
        for (const auto& quad : quads) {
            quad_to_triangles(quad, mesh);
        }
    }

    return mesh;
}

}  // namespace voxel

