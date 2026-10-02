# Technical Architecture — Godot 4 + C++

## Architectural rule
Godot owns presentation and high-level gameplay; an independent native C++ library owns deterministic voxel data, generation, damage, structural analysis and persistence primitives. A thin GDExtension adapter converts native results into Godot-compatible meshes, collision and events. Avoid putting one Godot Node per voxel.

## Components
| Layer | Responsibilities |
|---|---|
| Godot game | Scenes, UI, input, camera, audio, animation, combat presentation, inventory |
| GDExtension adapter | Native API bridge, async job results, mesh/collision upload on permitted threads |
| C++ voxel core | Sparse chunks, material IDs, damage edits, meshing, connected-component structural analysis |
| C++ generation | Seeded terrain, landmarks, structures, enemy spawn manifests |
| Save subsystem | Region metadata, modified chunk snapshots, enemy/quest/loot state, atomic checkpoints |
| Navigation adapter | Local navmesh/graph invalidation and rebuild after edits |

## Suggested repository
```text
project/
  README.md
  docs/
  godot/                  # Godot project and GDScript/C#-free gameplay prototype
    project.godot
    scenes/
    scripts/
    assets/
  native/
    CMakeLists.txt
    voxel_core/           # No Godot headers or engine dependencies
      include/
      src/
      tests/
    godot_extension/      # godot-cpp bindings and .gdextension manifest
  tools/
    region_inspector/
  benchmarks/
  tests/
```

## Threading and ownership
- Main thread: Godot scene changes, gameplay event application, rendering resource creation/upload according to Godot API constraints.
- Worker threads: region generation, voxel edits on owned chunk data, meshing, structural graph calculation, compression and disk preparation.
- Transfer immutable mesh/collision update payloads through a bounded queue; discard stale jobs using chunk revision numbers.
- Assign explicit chunk ownership and lock ordering or use message-passing to prevent concurrent edit/save races.
- Never call arbitrary scene-tree methods from native worker threads.

## Key APIs (conceptual)
```cpp
struct RegionId { uint64_t value; };
struct ChunkCoord { int32_t x, y, z; };
struct VoxelEdit { int32_t x, y, z; uint16_t material; };
struct DamageSphere { float x, y, z, radius, energy; };

class VoxelWorld {
public:
  void load_region(RegionId id);
  void queue_damage(RegionId id, const DamageSphere& damage);
  void queue_edits(RegionId id, std::span<const VoxelEdit> edits);
  std::vector<ChunkUpdate> drain_completed_updates();
  void request_checkpoint(RegionId id);
};
```
These are planning signatures, not drop-in compilable code; adapt types and ownership during implementation.

## Rendering approach
Begin with surface extraction / greedy meshing of exposed voxel faces and per-material atlasing. Hide interior faces, merge compatible coplanar faces, perform frustum and distance culling, and update only dirty chunks. Keep conventional skinned meshes for characters. Add LOD, indirect draws or more advanced voxel ray tracing only after profiling.

## Physics
Godot handles characters and simple rigid bodies. The voxel core detects disconnected structural islands. Large falling pieces may become temporary rigid bodies with simplified collision; after settling, rasterize or bake their final state into persistent static geometry. Cap active fragments and simulation distance.

## Navigation
Treat terrain changes as invalidation events. Rebuild only affected nav regions asynchronously; maintain a coarse fallback grid for temporary movement and ensure enemies do not walk through recently destroyed/blocked geometry. Prioritize correctness before sophisticated pathfinding.

## Toolchain
Pin an exact Godot 4.x version, matching godot-cpp binding version, compiler version and GDExtension ABI in the repository once development begins. Use CMake for native code, unit tests and benchmarks; keep the native library buildable headlessly. CI should compile the native tests and launch Godot headless smoke tests.

## Instrumentation
Expose CPU frame time, GPU frame time, loaded/dirty chunks, meshing queue depth, mesh upload time, active rigid bodies, memory, save duration and nav rebuild latency. Performance tuning without these measurements is guesswork.
