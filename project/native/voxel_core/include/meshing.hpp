#pragma once

#include <vector>
#include <cstdint>

namespace voxel {

class Chunk;

// Single triangle vertex
struct Vertex {
    float x, y, z;      // Position
    uint16_t material;  // Material ID (for texturing)
    uint8_t normal_x;   // Encoded normal (0-255 maps to -1..1)
    uint8_t normal_y;
    uint8_t normal_z;
};

// Mesh output from greedy meshing
struct ChunkMesh {
    std::vector<Vertex> vertices;
    std::vector<uint32_t> indices;
    size_t vertex_count() const { return vertices.size(); }
    size_t triangle_count() const { return indices.size() / 3; }
    bool empty() const { return vertices.empty(); }
};

// Greedy mesh a single chunk
// Returns vertices and indices for GPU rendering
ChunkMesh mesh_chunk(const Chunk& chunk);

}  // namespace voxel
