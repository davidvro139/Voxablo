# Combat & AI to-do

Goal: fights that read as a sequence of deliberate duels, hits with weight, and enemies that
perceive, route, panic, and use a world that can be blown apart. The limb rule in
[05_COMBAT_ENEMIES_LOOT](design/05_COMBAT_ENEMIES_LOOT.md) stays the core; this list builds around it.
Constraint carried from the design: nothing respawns or spawns extra enemies.

## Tier 1 — feel and fairness
- [x] **Attack tokens.** Shared pool (2 melee, 1 ranged). An enemy needs a token to start a windup and
      releases it after recovering. Tokenless enemies hold a wider ring and circle instead of piling in.
      *Ref: Doom (2016) AI.*
- [x] **Hit-stop.** Brief freeze on a sever, a stagger, a kill, and a hit on the player. Not on every
      beam tick; overlapping stops merge instead of stacking.
- [x] **Poise and stagger.** Each body has poise. Damage drains it; at zero, or when a part comes off,
      the current attack is cancelled and the body staggers. Short guard after a stagger prevents
      stunlock; poise refills when left alone. *Ref: Sekiro posture, Souls poise.*
- [x] **Windup telegraph.** A ground arc (melee) or lane (arrow) in the locked direction that fills as the
      windup runs, so stepping out of it is a readable decision. *Ref: Arkham counter cues.*
- [x] **Player dodge.** Short dash toward the cursor with invulnerability frames and a cooldown.
      Cancels the current swing.

## Tier 2 — perception and personality
- [x] **Line of sight.** Replace the 28 m aggro radius with a voxel raycast to the player.
      Idle bodies see 22 m ahead of them only; within 3 m they sense the player through walls.
      Alert bodies remember for 8 s and walk to the last known spot when they lose sight.
- [x] **Noise.** Spells, blasts and collapses alert enemies in a loudness-scaled radius; melee is quiet.
- [x] **Pack alert.** An alerted enemy wakes its nearby group (12 m call that reveals the player).
- [ ] **Persist alert state** with the region once M4 saves exist.
- [x] **Fallen cowardice.** Fallen near a death scatter for a few seconds; cornered ones keep fighting.
      *Ref: Diablo II retreat mode.*
- [x] **Maimed retreat (first pass).** Some armless bodies (not zombies) limp away from the player for 6 s.
- [ ] **Maimed retreat toward structures.** Needs the tier 3 navigation.
- [x] **Exaggerated reads.** Growl on spotting, shriek on panic, HUD shows unaware / searching /
      panicking. One bark per group. *Ref: Halo, F.E.A.R.*
- [ ] **Flank bark.** Waits for flanking behaviour (tier 4).

## Tier 3 — navigation in a destructible world
- [x] **Dijkstra map to the player** on a 0.5 m grid with up to four floors per column (storeys, bridge
      deck over ravine), one-way links probed voxel by voxel (drops allowed, climbs one 30 cm step at a
      time). Edited chunks rebuild their cells (~1 ms); the map refills when the player changes cell
      (≤ 8/s, ~2 ms). Blown-open walls become routes automatically. Full build ~130 ms at start and on R.
- [x] **Flee map** (chase map × −1.2, rescanned) for retreating without running into dead ends.
- [x] **Stuck recovery.** 0.5 s of no progress switches to the map for 1 s, or slides sideways if
      there is no route; a search trail that can't be routed is dropped.
- [ ] **Routes to arbitrary goals** (searching a spot far from the player, retreating to buildings).
      Needs per-goal fills or a few cached landmark maps.
- [ ] **Body width.** Probes are one voxel wide, so a route may use gaps narrower than a body.

## Tier 4 — roles and environment
- [x] **Archer kiting.** Inside ~3.6 m an archer won't draw; it scrambles back at 1.6× speed and only
      shoots point-blank once cornered. Losing sight already sends it to the last known spot.
- [ ] **Archer firing positions.** Pick a cell with a clear lane instead of walking toward the last sighting.
- [x] **Skeleton front line.** A shielded skeleton waiting its turn stands 2.2 m from the player on the
      line to the nearest archer, shield facing the player.
- [x] **Fuse dodging.** Alert bodies within an orb's reach (+0.6 m) in its last second drop the swing and run.
- [x] **Rubble hurts.** Falling pieces faster than 3 m/s damage the part they meet, scaled by speed and
      size, and shove the body.
- [ ] **Rubble hurts the player** and environmental kills earn credit once progression exists.
- [x] **Volatile elite.** Two roster slots (a fallen, a zombie) smoulder orange. Downed, they show a
      filling burst ring for 0.9 s, then crater 1.6 m, hurt nearby bodies, and can chain.
      *Ref: Diablo champion affixes.*
- [ ] **More affixes.** "Charges through brick" and others that use destruction.

## Tier 5 — orchestration
- [x] **Utility layer** (`src/utility.rs`). Engaged bodies score Press / Hold / Flank / Regroup from
      token, distance, parts left, allies near, and where the player is looking (IAUS-style curves,
      re-scored every 0.35 s with a stickiness bonus). Flankers circle behind a player who is watching
      them and growl as they go; a wrecked, isolated body falls back to an ally. Zombies never plan.
- [x] **Pacing director** (`src/director.rs`). Build-up → peak → relax from damage taken and bodies
      engaged. In a quiet build-up it has the nearest unaware body call its pack in, at most every 12 s;
      never adds enemies. Below 35 health only one melee body may swing at a time.
      *Ref: Left 4 Dead AI Director.*
- [ ] **Director in the HUD / debug overlay** so pacing can be tuned by eye.
- [ ] **Tune by playtest**: token pool, poise, flank distance, director thresholds.
