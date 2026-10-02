#pragma once

#include <cstdint>
#include <vector>
#include <unordered_map>
#include <span>

namespace voxel {

// Forward declarations
class Chunk;
struct ChunkMesh;
class TerrainGenerator;
struct PhysicsState;

// Configuration constants
constexpr int CHUNK_SIZE = 32;  // 32^3 voxels per chunk
constexpr float VOXEL_SIZE = 0.1f;  // 10 cm voxel resolution for prototype

// Stable identifiers
using RegionId = uint64_t;
using EntityId = uint64_t;

// Coordinate types
struct ChunkCoord {
    int32_t x, y, z;

    bool operator==(const ChunkCoord& other) const {
        return x == other.x && y == other.y && z == other.z;
    }
};

struct VoxelCoord {
    int32_t x, y, z;
};

struct WorldCoord {
    float x, y, z;
};

// Material enumeration
enum class MaterialType : uint16_t {
    Air = 0,
    Dirt = 1,
    Wood = 2,
    Brick = 3,
    Stone = 4,
    MaxMaterial = 256
};

// Voxel edit command
struct VoxelEdit {
    int32_t x, y, z;
    MaterialType material;
};

// Damage query
struct DamageSphere {
    float x, y, z;
    float radius;
    float energy;  // Damage intensity (affects material resistance)
};

// Chunk data: 32^3 voxels, stored linearly
class Chunk {
public:
    Chunk();
    ~Chunk() = default;

    MaterialType get_voxel(int lx, int ly, int lz) const;
    void set_voxel(int lx, int ly, int lz, MaterialType material);
    bool is_dirty() const { return dirty; }
    void mark_clean() { dirty = false; }

private:
    std::vector<uint16_t> voxels;  // CHUNK_SIZE^3 entries
    bool dirty;

    int linear_index(int lx, int ly, int lz) const;
};

// Mesh update payload from worker thread
struct ChunkUpdate {
    ChunkCoord coord;
    uint32_t revision;  // Used to discard stale jobs
    std::vector<float> vertices;  // Greedy mesh output
    std::vector<uint32_t> indices;
    bool has_collision_data;
};

// Forward declare damage function (implemented in damage.cpp)
void damage_chunk(Chunk& chunk, const DamageSphere& sphere,
                  float chunk_base_x, float chunk_base_y, float chunk_base_z);

// Collapse query
struct CollapseQuery {
    RegionId region_id;  // Which region to analyze
    bool apply_collapse; // If true, remove unsupported voxels; if false, just analyze
};

// Main voxel world controller
class VoxelWorld {
public:
    VoxelWorld();
    ~VoxelWorld();

    // Region lifecycle
    void load_region(RegionId id);
    void unload_region(RegionId id);

    // Fills (or overwrites) a chunk in a loaded region with procedural terrain
    void generate_chunk(RegionId id, const ChunkCoord& coord, uint64_t seed);

    // Read access; nullptr / Air when the region or chunk isn't loaded
    const Chunk* find_chunk(RegionId id, const ChunkCoord& coord) const;
    MaterialType get_voxel(RegionId id, int32_t x, int32_t y, int32_t z) const;

    // World-coordinate write; creates the chunk if needed. No-op if the region isn't loaded.
    void set_voxel(RegionId id, int32_t x, int32_t y, int32_t z, MaterialType material);

    // Voxel mutations
    void queue_edits(RegionId id, std::span<const VoxelEdit> edits);
    void queue_damage(RegionId id, const DamageSphere& damage);
    void queue_collapse(RegionId id);  // Analyze and collapse unsupported voxels

    // Physics simulation
    void update_physics(RegionId id, float delta_time);  // Update falling fragments

    // Mesh/collision update results
    std::vector<ChunkUpdate> drain_completed_updates();

    // Persistence
    void request_checkpoint(RegionId id);

    // Diagnostics
    size_t loaded_chunk_count() const;
    size_t dirty_chunk_count() const;

private:
    struct Region {
        RegionId id;
        std::unordered_map<uint64_t, Chunk> chunks;  // Keyed by packed ChunkCoord
        uint32_t generation;
        PhysicsState* physics;  // Temporary fragments falling/settling
    };

    std::unordered_map<uint64_t, Region> regions;
    uint32_t chunk_revision_counter;

    uint64_t pack_chunk_coord(const ChunkCoord& c) const;
    ChunkCoord unpack_chunk_coord(uint64_t packed) const;
};

}  // namespace voxel
