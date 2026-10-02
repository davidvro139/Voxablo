# Decisions, Open Questions and Risk Register

## Confirmed decisions
- Godot 4 + C++ GDExtension, with standalone native voxel core.
- Isometric dark-fantasy Diablo-style ARPG with detailed Teardown-like destructible voxels.
- Separate procedurally generated persistent regions.
- Terrain/building destruction persists permanently.
- Enemies and bosses stay dead permanently in their region.
- Single-player Windows prototype first (working assumption, not a final shipping commitment).

## Provisional technical choices
| Topic | Initial experiment | Revisit when |
|---|---|---|
| Voxel size | 10 cm; benchmark 5 cm | M2 profiling |
| Chunk dimensions | 32³ | M1 memory/meshing tests |
| Rendering | Greedy-meshed exposed surfaces | M2 GPU profiling |
| Physics | Local support checks, capped fragments | M3 collapse tests |
| Saves | Modified chunk snapshots + bounded journal | M4 crash recovery tests |
| AI | Local navigation rebuilds | M3 pathfinding tests |

## Open design questions
1. Can players repair/rebuild destroyed structures, and how are modifications attributed?
2. Is there a fixed number of regions per world, or an effectively endless frontier?
3. How does progression scale across newly generated regions?
4. What happens if a player destroys a quest-critical structure before accepting its quest?
5. Can the player permanently trap themselves, and what recovery tools exist?
6. Is cooperative multiplayer a future requirement? If yes, networking constraints should influence native state design early.
7. What level of physical realism is desirable versus cinematic, readable destruction?
8. How are loot scarcity and finite enemy populations balanced across unlucky runs?

## Major risks and mitigations
| Risk | Consequence | Mitigation |
|---|---|---|
| Tiny voxel resolution | Large RAM, mesh and save costs | Start 10 cm; profile 5 cm; sparse chunks and LOD |
| Collapse cascades | CPU spikes and too many rigid bodies | Local connectivity, per-frame budgets, capped debris |
| Rapid mesh updates | GPU upload stalls | Async meshing, revision IDs, throttled uploads |
| Dynamic navigation | Stuck enemies, expensive rebuilds | Dirty-region updates and fallback traversal |
| Save corruption | Permanent loss of progress | Atomic checkpoints, journals, checksums, recovery tests |
| Generator changes | Old regions change unexpectedly | Pin generation versions and migrate deliberately |
| Finite enemy population | Loot starvation, empty late game | Guaranteed progression, crafting later, new frontier regions |
| Scope creep | No playable build | Strict M0–M5 vertical slice, defer advanced features |

## Project decision log
Record every significant decision with date, rationale, alternatives and consequences. Do not silently replace confirmed player-facing permanence rules for implementation convenience.

### 2026-09-30 — Creature combat is part capabilities, not a health pool
Creatures are disabled by removing the parts that let them fight. A hit damages the part it meets. The stance is whatever capabilities remain, and the fight ends when a vital is gone or nothing left can reach the player. This is Clone Drone's rule adapted to isometric aiming: the cursor picks the part, and a committed swing can be stepped out of. The alternative was a health bar with cosmetic limb loss, which the viewer already had and which does not make enemies change how they fight. Buildings stay on the separate material model. The first body is the claw brute in the Bevy viewer. Archer, hammer, and sword-and-shield are the same rule with different maps and are not built yet. The player does not lose limbs yet. Wrecks are not persisted yet.
