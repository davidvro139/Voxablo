# Instructions for Claude, Codex or Another Coding Agent

## Project objective
Implement the prototype described in `README.md` and `06_PROTOTYPE_ROADMAP.md`. This is **Godot 4 + native C++ GDExtension**, not Unreal and not Bevy. Preserve separate persistent procedural regions, fine-grained voxel destruction and permanent enemy/boss deaths.

## Read before coding
Read `01_GAME_DESIGN.md`, `02_TECHNICAL_ARCHITECTURE.md`, `03_VOXEL_DESTRUCTION.md`, `04_WORLD_GENERATION_AND_PERSISTENCE.md` and the current milestone in `06_PROTOTYPE_ROADMAP.md`. Treat `08_DECISIONS_AND_RISKS.md` as unresolved questions; do not silently turn assumptions into permanent design decisions.

## First implementation task
Implement **M0 only**:
1. Create the repository layout described in the architecture document.
2. Choose and document exact compatible Godot 4.x and godot-cpp versions, compiler and CMake requirements.
3. Build an independent `voxel_core` library and native unit-test executable.
4. Add a minimal GDExtension with a native class exposed to Godot and a smoke-test scene.
5. Add an isometric camera and diagnostic overlay.
6. Write Windows build/run instructions and a headless smoke-test command.
7. Run tests; report what passed, what failed and what remains unimplemented.

## Coding rules
- Native voxel core must not include Godot headers.
- Prefer explicit ownership, RAII, bounded job queues and deterministic integer-coordinate operations.
- Keep game-thread-only Godot API calls off worker threads.
- No per-voxel Godot Nodes or per-voxel physics bodies.
- Make generation deterministic; seed random number generators explicitly.
- Treat save data as versioned; never regenerate a modified chunk over player edits.
- Use stable IDs for all finite enemies, bosses and one-time loot.
- Add tests for chunk boundaries, concurrent edits, save/reload and enemy permanence as those systems are implemented.
- Record actual benchmark hardware and measured values; never report hypothetical FPS as observed performance.

## Expected agent output per milestone
- Brief design and file-change plan before editing.
- Implemented code and any new assets/configuration.
- Build/test commands and exact results.
- Known limitations, performance measurements and follow-up tasks.
- Update relevant Markdown docs if an implementation decision changes.

## Suggested milestone prompt
> Implement milestone M0 from `06_PROTOTYPE_ROADMAP.md`. Read all linked planning docs first. Keep the voxel core independent of Godot. Pin compatible dependencies, build a minimal Godot GDExtension bridge, add native tests and a headless smoke test, then report exact build/test results and any deviations from the plan. Do not start M1 until M0 is verified.

## Definition of done
A milestone is done only if it builds from a clean checkout, runs its automated tests, demonstrates its acceptance criteria and documents reproducible setup. If blocked, report the blocker rather than claiming completion.
