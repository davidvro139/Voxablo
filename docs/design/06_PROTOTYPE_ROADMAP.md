# Prototype Roadmap and Acceptance Criteria

These are sequential milestones, not calendar promises. Each milestone must have a runnable demo, tests and performance measurements before moving on.

## M0 — Repository and instrumentation
- Pin Godot/godot-cpp/compiler versions; create native CMake project, GDExtension and headless tests.
- Display empty 3D scene with controllable isometric camera and frame-time debug overlay.
- Native library builds and tests without Godot.
**Done:** clean Windows build, CI smoke test, profiling counters visible.

## M1 — Voxel terrain
- Implement chunk storage, material palette, procedural terrain from fixed seed and greedy meshing.
- Stream a small outdoor area; display textured terrain in Godot.
- Add chunk boundary, coordinate and deterministic generation tests.
**Done:** repeatable terrain across runs; no visible chunk seams; baseline performance recorded.

## M2 — Destruction
- Implement sphere damage, dirt/wood/brick/stone resistance and dirty chunk rebuild.
- Add a small voxel house and gatehouse; update render and collision after damage.
- Add impact VFX and temporary bounded debris.
**Done:** repeated attacks open a traversable hole; 60 FPS at 1080p on target hardware is an aspiration, measured frame times documented.

## M3 — Collapse and navigation
- Implement support anchors and local connectivity analysis for building collapse.
- Add temporary rigid fragments and baked settled rubble.
- Rebuild local navigation after destruction.
**Done:** removing supports collapses a test roof; enemies can route through newly opened passage; no long frame stalls in representative test.

## M4 — Procedural region and permanence
- Generate village, cellar, gatehouse, entrances and finite enemy manifest.
- Implement stable IDs, modified-chunk snapshots, enemy death registry and atomic checkpoint saves.
- Add travel out/back and process-restart reload test.
**Done:** destruction, loot and deaths persist after restarting the game.

## M5 — Playable combat slice
- Implement player movement, three attacks, three enemy archetypes and one unique boss.
- Add basic inventory, randomized loot and one-time boss reward.
- Complete the region and unlock a second generated region stub.
**Done:** 20–30 minute vertical slice; completed region stays cleared; fresh region supplies new encounters.

## M6 — Optimization and polish
- Profile representative combat + destruction workload; optimize highest measured bottleneck.
- Add mesh job prioritization, bounded physics, streaming and save telemetry.
- Improve lighting, materials, audio and readability.
**Done:** publish reproducible benchmark scene and hardware/spec results; no save corruption in interruption tests.

## Benchmark scene
At minimum: 10 cm voxels, several multiroom buildings, 30 active enemies initially, repeated overlapping impacts and region save/reload. Expand toward 5 cm and 100 enemies only after baseline results justify the cost. Track average FPS, 1% lows, peak RAM, meshing milliseconds, upload milliseconds, nav rebuild latency and save times.

## MVP definition
One generated region with at least two destructible structures, an alternate route created by destruction, a finite enemy population, one permanent boss kill, loot and complete persistence after process restart.
