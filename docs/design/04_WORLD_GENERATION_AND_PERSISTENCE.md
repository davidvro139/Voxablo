# Procedural Regions and Permanent Persistence

## Region identity
Each region has an immutable ID, world seed, region seed, generation algorithm version, biome definition, bounds and content manifest. Generation must be deterministic for the same seed/version and data assets. Keep original generated data conceptually separate from player changes.

## Generation stages
1. Determine region topology, entrances/exits and main objective path.
2. Generate terrain, biome masks, rivers/caves where applicable.
3. Place landmarks and traversal connections with accessibility constraints.
4. Instantiate destructible structures from authored modular templates.
5. Carve dungeon spaces and alternate breakable approaches.
6. Populate finite enemies and unique bosses; assign stable IDs.
7. Place loot, secrets and quest triggers; validate reachable objectives.
8. Record immutable region manifest and initial snapshot/checksum.

## Persistence schema (conceptual)
```text
WorldSave
  save_format_version
  world_id, world_seed
  character_id, character_state
  region_index[]
RegionSave
  region_id, region_seed, generation_version
  original_manifest_hash
  modified_chunks[] (coord, revision, compressed voxel snapshot)
  entities[] (stable_id, state, position, inventory)
  enemy_registry[] (stable_id, alive/dead, current state)
  boss_registry[] (stable_id, defeated, one-time reward claimed)
  quest_flags[]
  claimed_loot_ids[]
  checkpoint_sequence, checksum
```

## Save algorithm
Generate untouched chunks from seed. Persist **modified chunk snapshots** after a checkpoint, not the whole region. Keep a bounded append-only edit journal between checkpoints for crash recovery. Save region metadata and state changes transactionally; use temporary files + fsync where supported + atomic rename, and retain at least one prior valid checkpoint. Test interruption during every save stage.

## Critical permanence invariants
- A stable enemy ID can transition alive → dead only once; loading or traveling never resurrects it.
- A claimed one-time drop cannot reappear on load.
- A defeated boss never spawns again in that region.
- Rebuilding a chunk never replaces modified data with the procedural original.
- Revisiting a region loads saved state before activating encounters.
- Changing generator versions must not silently regenerate existing regions.

## Procedural validation
Generate many seeds headlessly. Validate entrance-to-objective reachability *before destruction*, at least one alternate path for prototype boss, no boss embedded in solid voxels, no inaccessible mandatory loot and safe spawn points. After player destruction, allow emergent traversal but provide recovery tools to prevent irreversible softlocks (e.g., town portal, region exit, basic excavation).

## Finite enemies and progression
Each region's population is created once and recorded in the manifest. AI may patrol, retreat or reinforce from **existing** groups only. New regions provide fresh combat; completed regions remain available for exploration. A fresh new-game world is an explicit separate save, never an implicit reset.

## Compatibility and recovery
Version all save formats, generation algorithms and structure templates. Add migrations before shipping changes that alter serialized state. Include backup/restore UI later. In development, add a save inspector and checksums for generated region manifests and modified chunks.

## Persistence acceptance test
Create region → destroy gatehouse → kill all test enemies and boss → collect boss item → save → unload → restart process → reload → verify voxel hashes, entity IDs, dead states, inventory and claimed-loot flags → leave/re-enter → verify again.
