# Game Design Document

## Elevator pitch
An isometric dark-fantasy action RPG in which procedurally generated regions can be permanently reshaped. Demolish fortifications, tunnel through dungeons, collapse enemy strongholds and kill unique bosses. The world remembers every change; expansion into new regions supplies fresh challenges.

## Design pillars
1. **Permanent consequences:** terrain, structures, loot claims, quests and enemy deaths persist.
2. **Destruction is gameplay:** spells and weapons change paths, cover, hazards and access—not merely visuals.
3. **Action RPG depth:** responsive combat, distinct builds, randomized loot and meaningful skill choices.
4. **Discoverable procedural regions:** generated layouts with authored encounter grammar, landmarks and secrets.
5. **Performance by design:** limited active simulation, optimized voxel meshes and robust saves.

## Core loop
Discover region → explore landmarks and enemy territory → fight, loot and modify terrain → defeat finite enemy groups and unique bosses → complete regional objectives → revisit persistent aftermath or open a new region.

## World model
A world contains an expanding set of independent regions. Each region has a seed, generation version, biome, landmark graph, population registry, quest state and saved voxel modifications. Travel between regions occurs through explicit exits or a map. Region completion does **not** erase unexplored content or force travel.

### Example prototype region: The Ruined Village
One small outdoor area with a destructible village, a gatehouse, a short underground cellar/dungeon, 3–5 enemy archetypes and one unique boss. Two approaches to the boss: through the fortified gate or by destroying/excavating an alternate route. On return, all terrain damage and deaths remain.

## Character systems
- Start with one melee-focused class and three skills: basic attack, heavy impact, short-range explosive/ground-shock attack.
- Expand later to spellcaster and summoner archetypes. Every class must have basic access to terrain interaction; specialized skills add efficiency or tactics.
- Separate damage to creatures from material/structural damage; balance independently.
- Item affixes may affect combat, elemental effects and environmental interaction.

## Enemy and boss permanence
Every enemy has a stable region-local ID. Its death is a permanent state transition. Bosses have authored mechanics layered onto generated arenas and drop one-time rewards. Survivors may reposition or fortify but cannot multiply or silently respawn. Once a region is cleared, it remains clear unless a separately designed, clearly communicated story event introduces a new *distinct* population; defer such events beyond MVP.

## Replayability without respawns
The frontier generates additional regions with different layouts, biome combinations, encounters, materials and loot tables. Optional fresh world/character creation supports replaying the overall game; never reset a player's existing region as a farming mechanic.

## Destruction examples
- Destroy a gate instead of searching for a key.
- Collapse a watchtower to remove enemy ranged advantage.
- Dig around a barricade into a cellar.
- Create a crater that changes enemy pathfinding and movement.
- Find a sealed room behind destructible masonry.

## Visual direction
Readable isometric camera, dark fantasy atmosphere, high-quality lighting, fine-voxel terrain/architecture, conventional skinned 3D characters and weapons, restrained particles and silhouettes readable amid rubble. Design destructible materials to communicate resistance and fracture behavior.

## MVP exclusions
Multiplayer, region-wide fluid simulation, fully dynamic fire propagation, sophisticated building, hundreds of simultaneous physics fragments, procedural story generation and a multi-class endgame.

## Success criteria
A player can enter a newly generated region, fight and permanently kill its enemies, destroy at least two different structures, uncover an alternate route, defeat its boss, leave, restart and return to the identical modified world state.
