# Combat, Enemies, Bosses and Loot

## Combat principles
Readable isometric action, deliberate attack commitment, and hits that change what a fighter can still do. Buildings keep a separate material and structural damage model so a fight does not erase the town. Creatures are not a single health bar.

## Limb battle
The battle rule comes from Clone Drone in the Danger Zone, translated into an isometric action RPG.

A fighter is a set of parts. Each part grants a capability. An attack that connects damages that part only. When its integrity is gone, the part comes off and the capability goes with it. The fighter then uses the best stance it can still perform. The fight ends when a vital is destroyed (head or torso core) or no remaining part can reach the player.

A committed attack locks facing for its windup. Stepping out of that arc makes it miss. Cutting the legs takes away the chase. Cutting the arms takes away the claws. The fighter switches weapons instead of playing a slowed-down version of the same attack.

The first body is the claw brute, the fallen demon in the viewer:

| Still attached | How they fight |
|---|---|
| Both arms, both legs | Circle and slash with both claws |
| One arm, both legs | Longer lunges with the remaining arm |
| Any arm, one leg | Hop in, plant, and swing. They cannot circle |
| No arms, horns, both legs | Charge and gore |
| No arms, horns, one leg | Hobble into a short charge |
| No arms, tail, at least one leg | Whip with the tail |
| At least one arm, no legs | Drop and crawl, swiping at point-blank range |
| Only a tail | Thrash in place. Stepping away is safe |
| Nothing that can reach, or the head or core destroyed | They can't fight |

Aim is the part under the cursor. Left click walks in and cuts that part. A bolt, beam, or nova hits the part the shot actually meets, so a low hit takes a leg and a high hit takes the head. One clean melee cut or a solid bolt removes a limb or the head. The torso core is tougher, so dismantling is how you change the fight and coring it is the finisher. A meteor on the body finishes them. A blast at the edge takes the part it lands on and they keep fighting.

Later bodies use the same rule with a different capability map, not a new health system. The viewer now fields four of them. None of them create extra enemies.

| Body | Still attached | How they fight |
|---|---|---|
| Fallen | as in the table above | Cleavers, horns, or the tail |
| Skeleton | Sword arm and shield arm | Circles and cuts. The shield catches a frontal hit aimed at the head or core |
| Skeleton | Shield arm only | Bashes |
| Skeleton | Sword arm only | Lunges |
| Skeleton | No arms, a leg left | Kicks. No legs either, and they are done |
| Zombie | Any arms | Slow slam or slap. A hit that connects rots the player for a few seconds |
| Zombie | No arms, head still on | Lunges and bites. The bite rots too |
| Corrupt rogue | Both arms | Stops and looses an arrow. The bow needs both arms |
| Corrupt rogue | One arm | Drops the bow and rushes |
| Corrupt rogue | No arms, a leg left | Kicks. No legs either, and they are done |

A hammer body is still only a map: only the hammer arm can hurt you, and that arm coming off ends them. The player uses this body too, later. A lost arm drops that hand's weapon. A lost leg forces the hop. This slice does not take the player's limbs.

Enemy wrecks stay where they fell until the world is reset. Deaths are not persisted yet, and neither is loot.

## Prototype character
One melee cut aimed at a body part, plus the existing spells. The smash and shockwave ideas remain the way heavy hits open brick and wood. Add more bodies after this brute can be dismantled in a fight.

## Enemy AI
The claw brute above is the first archetype. Ranged, shielded, and boss bodies come after it, each as a capability map on the same part rule. Each spawned enemy still has a stable ID and a persistent life state. AI uses a region-local nav graph and reacts to invalidated routes. Survivors can retreat to nearby surviving structures but may not create extra enemies.

## Boss encounter
A unique village warden in a partially destructible gatehouse. Boss phases respond to health and environment; breaking walls exposes alternate angles. Boss death permanently changes region quest state and awards a one-time item. Test boss accessibility if the arena is extensively destroyed.

## Loot model without farming
- One-time boss rewards, randomized equipment from finite enemy drops and discoverable caches.
- Resource gathering from terrain and dismantled structures, bounded to prevent infinite extraction exploits.
- Crafting/upgrading as a later system to make otherwise unlucky drops useful.
- New regions generate new enemies and loot; earlier regions do not reset.
- Persist loot ownership and claim IDs independently of physical drop objects.

## Balance challenges
A finite population makes missed or low-quality drops more consequential. Prototype safeguards: guaranteed starter upgrades, boss reward choice or deterministic minimum-quality rewards, and optional salvage. Do not promise endless item farming in the same world.

## Environment interaction
Wood burns, brick cracks, stone resists light attacks; high-impact attacks can collapse cover. Environmental kills grant appropriate progression and loot credit. Keep effects predictable and visually legible to avoid punishing players for using destruction.

## Future design decisions
Skill trees, multiplayer, friendly-fire building damage, repair/rebuilding, NPC settlement restoration, destructible quest-critical locations and whether unexplored regions scale to character level are intentionally undecided.
