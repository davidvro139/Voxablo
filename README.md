# Voxablo

An isometric dark-fantasy action RPG prototype that combines Diablo II-style combat and
itemization with Teardown-style destructible voxel environments. Every region is
procedurally generated and persists: destroyed terrain stays destroyed, and killed enemies
stay dead.

**Status:** early prototype. **Target:** Windows PC, single-player.

## What's here

| Path | Contents |
|------|----------|
| [`project/native/`](project/native/) | C++20 voxel core: chunk storage, generation, greedy meshing, damage, structural analysis. No engine dependencies. Includes unit tests. |
| [`voxel-core-ffi/`](voxel-core-ffi/) | Rust bindings to the C++ core (links `voxel_wrapper.lib`). |
| [`voxel-viewer/`](voxel-viewer/) | Bevy 0.13 playable prototype: isometric camera, player, spells, destructible structures, rigid falling pieces, limb-based enemies. |
| [`project/godot/`](project/godot/) | Original Godot 4 front-end scaffold (dormant; the shipping front-end is still undecided). |
| [`docs/`](docs/) | [Progress](docs/PROGRESS.md), [engine roadmap](docs/ENGINE_DEVELOPMENT.md), [design documents](docs/design/), and [historical notes](docs/history/). |

## Building

Requirements: Windows, Visual Studio 2022 (MSVC v143, C++20), and a Rust toolchain.

```powershell
# 1. Build and test the C++ core (from a VS Developer PowerShell)
MSBuild project/native/voxel_core_tests.vcxproj /p:Configuration=Release /p:Platform=x64
./project/native/bin/Release/voxel_core_tests.exe

# 2. Build the C wrapper the Rust crate links against
MSBuild project/native/voxel_wrapper.vcxproj /p:Configuration=Release /p:Platform=x64

# 3. Run the viewer
cargo run --release -p voxel-viewer
```

`voxel-core-ffi/build.rs` watches `voxel_wrapper.lib`, so rebuilding the C++ side triggers
a relink. `cargo test --release -p voxel-viewer` runs the Rust tests.

## Controls

| Input | Action |
|-------|--------|
| Left click | Walk to the ground, or attack the enemy part under the cursor |
| Right click / hold | Cast the selected spell |
| 1–5 | Select spell: Arcane Bolt, Disintegrate, Fire Orb, Meteor, Frost Nova |
| Shift / Space | Sprint / jump |
| Left Ctrl | Dodge toward the cursor (brief invulnerability) |
| Mouse wheel | Zoom |
| R | Regenerate terrain and respawn |

## Design documents

1. [Game design](docs/design/01_GAME_DESIGN.md)
2. [Technical architecture](docs/design/02_TECHNICAL_ARCHITECTURE.md)
3. [Voxel destruction](docs/design/03_VOXEL_DESTRUCTION.md)
4. [Generation and persistence](docs/design/04_WORLD_GENERATION_AND_PERSISTENCE.md)
5. [Combat, enemies and loot](docs/design/05_COMBAT_ENEMIES_LOOT.md)
6. [Prototype roadmap](docs/design/06_PROTOTYPE_ROADMAP.md)
7. [AI coding agent guide](docs/design/07_AGENT_IMPLEMENTATION_GUIDE.md)
8. [Decisions and risks](docs/design/08_DECISIONS_AND_RISKS.md)

The design documents predate the Bevy viewer and describe a Godot + GDExtension stack; the
voxel core is kept engine-independent so either front-end can use it.

## Design constraints

- Separate persistent regions that remain accessible after completion.
- Fine destructible voxels: 10 cm resolution (5 cm only if benchmarks justify it).
- All world modifications persist across travel and restart.
- Enemies and bosses never respawn in the same region.
- Combat must remain playable after substantial environmental changes.
