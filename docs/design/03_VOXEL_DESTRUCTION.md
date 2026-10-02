# Voxel Terrain and Destruction Design

## Resolution and representation
Prototype with **10 cm voxels**; benchmark **5 cm** voxels in a constrained test. A 5 cm grid contains eight times as many voxels per unit volume as a 10 cm grid. Keep voxel resolution configurable per test build, but do not change resolution in existing saved regions without a migration strategy.

Use sparse 3D chunks (initial experiment: 32³ voxels), material palette IDs and compressed uniform/empty chunk representations. Use signed 64-bit world coordinates or region-local integer coordinates with safe conversions for large regions. Chunk dimensions and voxel size are hypotheses to benchmark, not locked constants.

## Material model
Material properties: density, damage resistance, fracture threshold, flammability flag, support behavior, sound/VFX identifiers and optional loot yield. Initial materials: air, dirt, stone, brick, wood, metal and bedrock. Bedrock is reserved for explicit region boundaries or protected narrative geometry; avoid arbitrary invulnerable walls elsewhere.

## Destruction pipeline
1. Receive attack volume, impact location, damage type and energy.
2. Identify intersecting chunks and candidate voxels.
3. Apply material-specific damage and replace removed voxels with air.
4. Mark edited chunks and boundary neighbors dirty.
5. Schedule mesh and collision updates; publish revisioned results.
6. Invalidate nearby navigation.
7. Schedule bounded structural connectivity analysis for affected structures.
8. Record modifications for durable save; emit loot/debris/VFX events.

## Structural collapse
Represent structures as connected occupied voxel components with material-dependent support rules and anchored foundations. After damage, run connectivity tests only within affected structures or a bounded neighborhood. Unsupported components above a size threshold become simplified rigid-body fragments; tiny components become particles or settled rubble decals/voxels. Enforce per-frame work budgets and active rigid-body limits.

This is a stylized structural model, not an engineering-grade finite-element simulation. A fully realistic fracture solver is out of MVP scope.

## Terrain versus structures
Terrain supports digging and craters; buildings have explicit foundation/anchor metadata and destructible material volumes. Both share voxel storage and mesh generation but may use different collapse heuristics. Large terrain cave-ins are a later feature, because global terrain connectivity can be expensive.

## Mesh strategy
- Extract visible surfaces, combine coplanar compatible faces, and batch by material.
- Rebuild only dirty chunks; include neighbor boundary visibility.
- Debounce repeated hits in the same chunk; prioritize chunks near the camera.
- Maintain separate render mesh and simplified collision representation.
- Use revision counters so old asynchronous results cannot overwrite newer edits.

## Debris lifecycle
Damage creates a limited number of visual particles and physical chunks. Physical fragments have simplified shapes and lifetimes; once motion stops, bake meaningful debris into voxel/static rubble and discard transient physics objects. Persist the final state, not every physics timestep.

## Tests
- Damage across chunk boundaries creates no cracks or missing faces.
- Save/reload reproduces crater and destroyed building exactly.
- Multiple overlapping explosions are deterministic under a fixed ordered event log.
- Structural collapse does not leave large unsupported islands after jobs settle.
- Rapid edits never apply stale meshes or stale collision.
- Stress test repeated destruction while tracking peak RAM and frame-time spikes.
