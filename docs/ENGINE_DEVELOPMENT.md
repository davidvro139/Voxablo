# Destructible engine development

Direction: Teardown-inspired material destruction and physical structures, viewed through
the existing isometric camera. Keep 10 cm voxels and permanent world changes as core goals.
This is an incremental engine roadmap, not a claim of feature parity with Teardown.

## Current implementation

The runnable prototype uses Bevy 0.13 with the independent C++ voxel core through Rust FFI.
The original planning documents describe Godot; choosing a shipping frontend remains a
separate decision. Native destruction improvements benefit either adapter.

The viewer already has orthographic player following, zoom, occluder fading, material-colored
terrain, a two-storey house, three destructive spells, and detached rigid voxel pieces.
Pieces rotate, receive blast impulses, collide with static voxels, and bake back into the
world after resting. Hard impacts can shatter them. Moving pieces now exchange collision
impulses, including friction and angular response, and later blasts can shove already
falling pieces. Projectiles, fire orbs, and the disintegrate beam can hit moving pieces.
Interaction between rubble and the player remains to be implemented.

## Development sequence

1. **Reliable destruction foundation.** Preserve dirty state after edits; avoid allocating
   chunks when blasting empty space; test boundary hits. Next audit signed chunk addressing,
   remove the unsafe legacy physics cast, and bound destruction work per frame.
2. **Physical destruction sandbox.** Add active-body budgets and safe handling of pieces
   that exceed the simulation budget. Demonstrate cutting a bridge and collapsing a house
   without pieces passing through one another or baking into the player. Track frame time
   and active body counts.
3. **Materials and fracture.** Make wood, masonry, and metal respond distinctly to tools;
   introduce controlled fracture for large unsupported components. Currently components
   above 150,000 voxels are treated as supported, which can leave floating structures.
4. **Permanent aftermath.** Implement versioned modified-chunk saves and atomic checkpoints.
   Define how moving bodies are captured before saving. Verify identical rubble and holes
   after restart and safe recovery from an interrupted save.
5. **Isometric interaction and presentation.** Aim accurately through faded occluders,
   distinguish walk targets from damage targets, and keep the player readable in rubble.
   Build lighting, dust, impact sounds, and material feedback around a demolition test scene.

## Foundation pass validation

Native Release x64 tests: 71 passed, 0 failed. Native viewer wrapper rebuilt successfully.
New regression checks cover dirty-state retention, destruction across a chunk boundary,
empty-space blasts, stable loaded-chunk counts, and nonpositive radius/energy.
Damage entry points also reject nonfinite sphere inputs.
No interactive visual or performance benchmark was run in this pass.

## Moving-piece collision pass

Validation: `cargo test --release -p voxel-viewer --offline` passed all 26 tests, including
four new cases for momentum conservation, off-center angular response, hollow interiors,
and separating contacts. No interactive visual validation or performance measurement yet.

`rigid::collide_pair` rejects distant pairs with bounding spheres, then checks sampled
surface spheres against the other body's local voxel occupancy in both directions.
This preserves holes instead of treating an entire structure's bounding box as solid.
Contacts exchange equal and opposite impulses with friction, restitution, rotational
response, mass-weighted penetration correction, and impact-speed tracking for shattering.

Falling pieces advance in shared steps no longer than 1/240 second before pair resolution;
settling and rendering occur after those contacts. Impacts wake disturbed pieces. Quiet
pieces supported by static terrain can settle, becoming support for the remaining pieces.

Limits: contacts approximate voxel surfaces with inscribed spheres and at most 512 surface
samples per direction. Small features on large bodies and high-speed impacts can still be
missed. Pair enumeration is quadratic in active piece count; spatial partitioning and body
budgets remain necessary before dense demolition scenes. This is discrete collision, not
continuous collision detection. The existing 12-second forced-settlement fallback remains.
Player collision with moving pieces and performance profiling remain open work. The
isometric camera is unchanged.

## Live blast impulse pass

Validation: `cargo test --release -p voxel-viewer --offline` passed all 28 tests, including
new cases for applying blast impulses to live falling pieces and ignoring distant blasts.
`cargo build --release -p voxel-viewer --offline` also succeeded.

Blasts are read by the falling-piece update as well as the support detector. A live piece
finds its nearest sampled exposed point to the blast centre and receives a falloff-scaled
point impulse away from the blast, so a second explosion can kick, spin, or re-wake rubble
that is already moving. This still uses the same approximate surface sampling as dynamic
piece collision.

## Direct moving-piece spell hits

Validation: `cargo test --release -p voxel-viewer --offline` passed all 30 tests, including
new segment-hit checks for sampled rigid-body surfaces. `cargo run --release -p voxel-viewer
--offline` launched successfully; the expected `dxil.dll` shader fallback warning may appear.

Rigid shapes can now test a spell segment against their sampled exposed points. Spell systems
choose the nearer target between static voxels and live falling pieces. Arcane bolts and fire
orbs detonate on moving rubble, while the disintegrate beam stops on moving rubble and pushes
it during beam ticks. This is still surface-sample based, so very small or very fast targets
can be missed until a broader collision budget and continuous-hit strategy exist.

Build with Visual Studio 2022 MSBuild:

```powershell
MSBuild project/native/voxel_core_tests.vcxproj /p:Configuration=Release /p:Platform=x64
& project/native/bin/Release/voxel_core_tests.exe
MSBuild project/native/voxel_wrapper.vcxproj /p:Configuration=Release /p:Platform=x64
cargo run --release -p voxel-viewer
```
