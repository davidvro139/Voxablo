# Progress

See [Engine development](ENGINE_DEVELOPMENT.md) for the isometric destruction roadmap.
Latest native foundation validation: 71 tests passed, 0 failed; Release wrapper rebuilt.

## Building and running

1. Build the C++ library: `project/native/voxel_wrapper.vcxproj`, Release|x64 (MSBuild, VS 2022).
   `project/native/voxel_core_tests.vcxproj` builds the C++ unit tests (46 passing).
2. For fast iteration, run `cargo run -p voxel-viewer`. Use
   `cargo run --release -p voxel-viewer` when you want the smoother optimized build.

`voxel-core-ffi/build.rs` watches `voxel_wrapper.lib`, so rebuilding the C++ side triggers a relink.
`cargo run --release --example test_mesh` checks the FFI: mesh generation, triangle winding, damage.

## Viewer (Bevy 0.13)

Units are metres; voxels are 10 cm (Teardown scale).

- 16×16 chunks (51.2 m × 51.2 m) of procedural terrain, colored by material; 3 chunk layers
  (9.6 m) of height are rendered
- A two-storey test house (8 m × 6 m) east of spawn, from `voxel_core/src/structures.cpp`:
  20 cm brick walls, wooden floors/roof/stairs, windows, interior partition, stone foundation
  and front steps. It is meant as the test bed for Teardown-style structural physics.
- A bridge test bed sits just north of spawn: a carved ravine, stone abutments, brick piers,
  wooden deck, and rails. Blast the deck or piers to test unsupported-piece extraction,
  falling-piece collision, and direct spell hits on moving rubble.
- Fixed isometric orthographic camera following the player; wheel zooms
- Walls, floors and hills between the camera and the player are dithered away within a 2.5 m
  radius (`src/occluder_fade.rs` + `.wgsl`); tune with `FADE_RADIUS` / `FADE_STRENGTH`
- Player: 1.8 m voxel character, collides with the voxel grid, steps up and snaps down ledges
  ≤ 30 cm, drawn with smoothed height so small steps don't jolt
  (`src/player.rs`, `src/player_model.rs`, `src/raycast.rs`; `cargo test --release -p voxel-viewer`)
- Left click an enemy: walk into range and cut the part under the cursor. Left click ground: walk
  (gold ring marks it; gives up if blocked). The foe's stance and your health are on screen.
- Player and NPC visuals are authored at 5 cm voxels while world voxels/collision remain 10 cm.
  The player and the demons plant each step: the foot stays put while the body moves
  over it, the swing knee bends so the foot clears, and at a run both feet leave the
  ground between steps. Arms swing opposite the legs. A one-handed
  sword, or a cleaver in each remaining hand, swings through windup, a strike that connects
  as the blade comes forward, then recover. Spells still raise both arms.
  Demons spawn in a lit ring
  around the player start plus landmark locations, and come back when the world is reset.
- Horned voxel demons spawn near the bridge and house. They are not a health bar. Each limb is a
  weapon: both arms swing cleavers, one arm lunges with the cleaver it has left, no arms means a horn charge or a tail whip,
  one leg hops, no legs crawl and swipe, and a lone tail only thrashes if you stand on it.
  They stop when the head or the torso core is destroyed, or when nothing left can reach you.
  A windup locks their facing, so stepping out of that arc makes the swing miss. Wrecks stay put.
- Three more bodies use that same part rule. Skeletons carry a sword and a shield; a frontal cut
  aimed at the head or core hits the shield until that arm comes off, then they bash, and with both
  arms gone they kick. Zombies are slow and a connecting hit rots you; with both arms gone they bite.
  Corrupt rogues loose an arrow while both arms can draw; one arm left and they rush in with a blade,
  and with neither arm they kick. Nothing here spawns another enemy. R still brings the same roster back.
- Right click / hold casts the selected spell (1-5), all in `src/spells.rs`:
  - Arcane Bolt: 28 m/s projectile, 0.4 m crater on impact
  - Disintegrate: channelled 25 m beam that drills ~0.25 m per tick, 20 ticks/s
  - Fire Orb: lobbed arc to the cursor, bounces, explodes after 1.4 s (1.4 m radius)
  - Meteor: targeted high-damage blast for buildings, bridges, and NPC clusters
  - Frost Nova: close-range radial blast around the player for nearby NPCs and props
  - Spells choose the nearer target between static voxels and live falling pieces; moving
    rubble can now be shot, beamed, and detonated by orbs. A bolt or beam hits the body part
    it meets. A blast takes the closest part and splashes the others.
  - Destroyed voxels fly out as debris cubes in their material colour; HDR bloom, flashes,
    point lights and camera shake
- Structural support (`src/falling.rs`, `extract_detached_pieces` in `structural_analysis.cpp`):
  after every blast, voxel groups no longer 6-connected to the ground (y = 0) are removed from
  the world and fall as one piece. On landing they are written back into the world; hard
  landings shatter the impact side into debris, and anything that shatter cut loose falls again.
  Pieces rotate and receive blast impulses, collide with the static voxel grid, and settle
  back into it (`src/rigid.rs`). Groups over 150k voxels count as supported.
  Moving pieces also collide with each other using sampled voxel surface contacts, exchanging
  linear/angular impulses and friction in shared substeps. Large-piece sampling and very fast
  impacts remain approximate. Later spell blasts and direct spell hits now shove and spin
  live falling pieces too. Active-body budgets and a spatial broad phase are still needed.
- Shift sprint, Space jump, Left Ctrl dodge toward the cursor (0.25 s invulnerable, 0.75 s cooldown),
  R regenerate terrain and respawn
- Group combat (`src/feel.rs`, tokens in `main.rs`): at most two melee bodies and one archer attack
  at once; the rest circle a wider ring. Each body has poise; breaking it or taking a part
  cancels the windup and staggers. Windups draw a filling ground arc (melee) or lane (arrow).
  Severs, staggers, kills and hits on the player trigger a short hit-stop.
- Senses (`src/senses.rs`): enemies start unaware. They see 22 m ahead with a voxel line-of-sight
  test, sense the player within 3 m regardless, and hear blasts, collapses, death cries and sword
  hits. A body that spots you growls and calls its pack within 12 m. Alert bodies hunt your last
  known position for 8 s. Fallen within 5 m of a death shriek and scatter for 3.5 s unless cornered;
  some armless bodies limp away. The HUD shows unaware / searching / panicking.
- Navigation (`src/nav.rs`, `update_nav` in `main.rs`): a Dijkstra map toward the player over
  standable floors (0.5 m cells, up to four storeys per column, one-way drops). Enemies follow it
  when the straight line is blocked or they get stuck; fleeing bodies use Brogue's flee map.
  Blasts rebuild the cells under them, so a hole in a wall is a new route on the next refill.
- Roles and environment: archers back off before drawing when you close in and shoot point-blank
  only when cornered; waiting shielded skeletons stand between you and an archer. Alert bodies run
  from a fire orb about to burst. Fast rubble damages the part it hits. Two "Volatile" elites glow
  orange and, once downed, show a filling ring and burst 0.9 s later (crater, body damage, chains).
- Decisions and pacing (`src/utility.rs`, `src/director.rs`): engaged bodies choose to press, hold,
  flank (circling behind you when you're watching them, with a growl), or fall back to an ally
  when wrecked and alone. A pacing director wakes the nearest unaware group after a quiet spell and
  allows only one melee attacker at a time while you are below 35 health.
  `RUST_LOG=voxel_viewer=debug` logs director phase changes and nav rebuild timings.
  F3 shows the director overlay: phase, intensity, mercy, next wave, body states, tokens, intents.
- Routing beyond the player: cached goal maps for searching and falling back, a shelter map that
  sends maimed bodies under the nearest roof, and archer firing positions with a clear shot.
  Fast rubble now hurts the player as well.
- Elite traits: Volatile (orange; bursts after going down), Wallbreaker (blue; smashes through walls
  it's stuck on), Frenzied (red; a nearby death speeds its attacks for 8 s). Two roster slots each.

## Known issues in voxel_core

- `VoxelWorld::update_physics` casts `std::vector<Chunk*>` to `std::vector<Chunk>&`, which is
  undefined behaviour and will likely crash. The viewer does not call it.
- Fixed: destruction retains the dirty flag set by voxel edits for downstream remeshing.
- The old per-chunk `collapse_unsupported_voxels` / `VoxelWorld::queue_collapse` is superseded
  by `extract_detached_pieces` and no longer used by the viewer.
- Fixed: `queue_damage` skips unloaded chunks; empty-space blasts do not allocate world storage.

## Next

- M4: save/load of modified chunks
- M5: more bodies on the same part rule (archer, hammer, sword and shield), then the player losing limbs too
