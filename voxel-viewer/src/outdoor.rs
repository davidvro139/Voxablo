//! Seeded moor south of Tristram.
//!
//! The town stays a fixed preset. This level starts at the south crossing and runs
//! toward +Z. A stone cliff rings the field. The road from the bridge is the gate.
//! North is low Z, the same axis as the town. Scatter uses the seed; the gate does not.

use voxel_core_ffi::{Material, PieceVoxel, VoxelWorld, CHUNK_SIZE};

use crate::tristram::GRADE;

/// Chunks of moor beyond the town square.
pub const EXTRA_CHUNKS: i32 = 8;
/// Fixed until regions carry their own seed. Same seed, same moor.
pub const SEED: u64 = 0x7A11_5EED;

const THICK: i32 = 8;
const ROAD_X0: i32 = 308;
const ROAD_X1: i32 = 316;
const GATE_X0: i32 = 300;
const GATE_X1: i32 = 324;
/// North cliff. Far enough south of the river bank that the ring does not bury it.
const NORTH_Z0: i32 = 500;
/// Road leaves the bridge here. The bank between the bridge and the cliff is already at grade.
const ROAD_Z0: i32 = 468;
/// Full-width grade starts once the river bank is behind us.
const FLAT_Z0: i32 = 480;

/// Floor markers the roster appends after the town. Kept off the road and the circle.
pub const ENEMIES: [(i32, i32); 4] = [(240, 580), (400, 560), (250, 700), (420, 700)];

struct Stamp {
    voxels: Vec<PieceVoxel>,
    max_y: i32,
    max_x: i32,
    max_z: i32,
}

impl Stamp {
    fn set(&mut self, x: i32, y: i32, z: i32, material: Material) {
        if y < 1 || y >= self.max_y || x < 0 || z < 0 || x >= self.max_x || z >= self.max_z {
            return;
        }
        self.voxels.push(PieceVoxel { x, y, z, material });
    }

    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, material: Material) {
        for z in z0..z1 {
            for x in x0..x1 {
                for y in y0..y1 {
                    self.set(x, y, z, material);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Rect {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
}

impl Rect {
    fn contains(self, x: i32, z: i32) -> bool {
        x >= self.x0 && x < self.x1 && z >= self.z0 && z < self.z1
    }
}

pub struct Level {
    pub map_x: i32,
    pub map_z: i32,
    pub road: [Rect; 2],
    pub gate: Rect,
    pub circle: (i32, i32),
    pub trees: Vec<(i32, i32)>,
    pub rocks: Vec<(i32, i32)>,
}

fn map_x() -> i32 {
    super::GRID * CHUNK_SIZE
}

fn map_z() -> i32 {
    (super::GRID + EXTRA_CHUNKS) * CHUNK_SIZE
}

fn hash(seed: u64, x: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 27)
}

/// Road, gate, circle, and scatter for one seed. The gate stays on the bridge road.
pub fn plan(seed: u64) -> Level {
    let map_x = map_x();
    let map_z = map_z();
    let east = seed & 1 == 0;
    let circle = if east { (430, 640) } else { (180, 640) };
    let branch = if east {
        Rect { x0: ROAD_X1, z0: 636, x1: circle.0 - 16, z1: 644 }
    } else {
        Rect { x0: circle.0 + 16, z0: 636, x1: ROAD_X0, z1: 644 }
    };
    let road = [
        Rect { x0: ROAD_X0, z0: ROAD_Z0, x1: ROAD_X1, z1: map_z - 28 },
        branch,
    ];
    let gate = Rect { x0: GATE_X0, z0: NORTH_Z0, x1: GATE_X1, z1: NORTH_Z0 + THICK };
    let blocked = |x: i32, z: i32| {
        road.iter().any(|r| r.contains(x, z))
            || gate.contains(x, z)
            || (x - circle.0).pow(2) + (z - circle.1).pow(2) <= 22 * 22
            || ENEMIES.iter().any(|&(ex, ez)| (x - ex).abs() <= 6 && (z - ez).abs() <= 6)
    };
    let mut trees = Vec::new();
    let mut rocks = Vec::new();
    for z in (530..map_z - 36).step_by(24) {
        for x in (28..map_x - 28).step_by(24) {
            if blocked(x, z) {
                continue;
            }
            match hash(seed, x, z) % 6 {
                0 => trees.push((x, z)),
                1 => rocks.push((x, z)),
                _ => {}
            }
        }
    }
    Level { map_x, map_z, road, gate, circle, trees, rocks }
}

fn on_ring(level: &Level, x: i32, z: i32) -> bool {
    let west = x < THICK;
    let east = x >= level.map_x - THICK;
    let south = z >= level.map_z - THICK;
    let north = z >= NORTH_Z0 && z < NORTH_Z0 + THICK;
    if north && x >= GATE_X0 && x < GATE_X1 {
        return false;
    }
    (west || east || south || north) && z >= NORTH_Z0 && z < level.map_z && x >= 0 && x < level.map_x
}

fn cliff_top(level: &Level, x: i32, z: i32) -> i32 {
    let from_west = x;
    let from_east = level.map_x - 1 - x;
    let from_south = level.map_z - 1 - z;
    let from_north = z - NORTH_Z0;
    let inset = if z >= level.map_z - THICK {
        from_south
    } else if z < NORTH_Z0 + THICK {
        from_north
    } else if x < THICK {
        from_west
    } else {
        from_east
    };
    // Outer lip is taller. The inner face stays above a step, so the ring is a wall.
    GRADE + if inset < 3 { 34 } else if inset < 6 { 24 } else { 16 }
}

fn flatten(stamp: &mut Stamp, level: &Level) {
    // Terrain noise tops out under y = 30. Fill the gap and shave the peaks.
    for z in FLAT_Z0..level.map_z {
        for x in 0..level.map_x {
            for y in 16..GRADE {
                stamp.set(x, y, z, Material::Dirt);
            }
            stamp.set(x, GRADE - 1, z, Material::Grass);
            for y in GRADE..32 {
                stamp.set(x, y, z, Material::Air);
            }
        }
    }
}

fn cliffs(stamp: &mut Stamp, level: &Level) {
    for z in NORTH_Z0..level.map_z {
        for x in 0..level.map_x {
            if !on_ring(level, x, z) {
                continue;
            }
            let top = cliff_top(level, x, z);
            for y in 16..top {
                stamp.set(x, y, z, Material::Stone);
            }
        }
    }
}

fn roads(stamp: &mut Stamp, level: &Level) {
    for rect in level.road {
        stamp.fill(rect.x0, GRADE - 1, rect.z0, rect.x1, GRADE, rect.z1, Material::Path);
    }
    // Gate floor is path, and the opening stays air through the cliff lip.
    let gate = level.gate;
    stamp.fill(gate.x0, GRADE - 1, gate.z0, gate.x1, GRADE, gate.z1, Material::Path);
    stamp.fill(gate.x0, GRADE, gate.z0, gate.x1, GRADE + 40, gate.z1, Material::Air);
}

fn circle(stamp: &mut Stamp, center: (i32, i32)) {
    let (cx, cz) = center;
    for i in 0..10 {
        let angle = i as f32 / 10.0 * std::f32::consts::TAU;
        // Leave the side that faces the branch (toward the road at x = 312) open.
        let toward_road = (angle.cos() - (312.0 - cx as f32).signum()).abs() < 0.35 && angle.sin().abs() < 0.55;
        if toward_road {
            continue;
        }
        let x = cx + (angle.cos() * 16.0).round() as i32;
        let z = cz + (angle.sin() * 16.0).round() as i32;
        stamp.fill(x, GRADE, z, x + 2, GRADE + 9, z + 2, Material::Stone);
    }
}

fn tree(stamp: &mut Stamp, x: i32, z: i32) {
    stamp.fill(x, GRADE, z, x + 2, GRADE + 11, z + 2, Material::Wood);
    stamp.fill(x - 3, GRADE + 7, z, x, GRADE + 9, z + 2, Material::Wood);
    stamp.fill(x + 2, GRADE + 8, z, x + 6, GRADE + 10, z + 2, Material::Wood);
}

fn rock(stamp: &mut Stamp, x: i32, z: i32) {
    stamp.fill(x, GRADE, z, x + 3, GRADE + 3, z + 2, Material::Stone);
}

/// Stamps the moor over terrain that is already generated for this region.
pub fn build(world: &VoxelWorld, region: u64) {
    let level = plan(SEED);
    let max_y = super::VERTICAL_CHUNKS * CHUNK_SIZE;
    let mut stamp = Stamp {
        voxels: Vec::with_capacity(2_700_000),
        max_y,
        max_x: level.map_x,
        max_z: level.map_z,
    };
    flatten(&mut stamp, &level);
    cliffs(&mut stamp, &level);
    roads(&mut stamp, &level);
    circle(&mut stamp, level.circle);
    for (x, z) in level.trees {
        tree(&mut stamp, x, z);
    }
    for (x, z) in level.rocks {
        rock(&mut stamp, x, z);
    }
    world.set_voxels(region, &stamp.voxels);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gate_is_on_the_road_and_the_ring_closes_around_it() {
        let level = plan(SEED);
        assert!(level.gate.contains(312, NORTH_Z0 + 2));
        assert!(level.road[0].contains(312, 470));
        assert!(level.road[0].contains(312, 600));
        assert!(level.gate.x0 < ROAD_X0 && level.gate.x1 > ROAD_X1);
        assert!(on_ring(&level, 40, NORTH_Z0 + 2), "the north cliff should meet the town");
        assert!(!on_ring(&level, 312, NORTH_Z0 + 2), "the road should be the gap");
        assert!(on_ring(&level, 4, 600));
        assert!(on_ring(&level, 312, level.map_z - 3));
        assert!(!on_ring(&level, 312, 600));
        let (cx, cz) = level.circle;
        assert!(cx > THICK + 30 && cx < level.map_x - THICK - 30);
        assert!(cz > NORTH_Z0 + THICK + 40 && cz < level.map_z - THICK - 40);
        assert!(!level.trees.is_empty());
        for &(x, z) in level.trees.iter().chain(level.rocks.iter()) {
            assert!(!level.road.iter().any(|r| r.contains(x, z)), "scatter landed on the road at ({x}, {z})");
            assert!(!on_ring(&level, x, z));
        }
    }

    #[test]
    fn the_moor_road_leaves_the_bridge_and_the_cliffs_stop_the_edge() {
        let world = VoxelWorld::new();
        let region = 1u64;
        world.load_region(region);
        let chunks_x = super::super::GRID;
        let chunks_z = chunks_x + EXTRA_CHUNKS;
        for x in 0..chunks_x {
            for z in 0..chunks_z {
                world.generate_terrain(region, x, 0, z);
            }
        }
        crate::tristram::build(&world, region);
        build(&world, region);

        let level = plan(SEED);
        let clear = |x: i32, z: i32| {
            assert_ne!(world.get_voxel(region, x, GRADE - 1, z), Material::Air, "floor ({x}, {z})");
            for y in GRADE..(GRADE + 18) {
                assert_eq!(world.get_voxel(region, x, y, z), Material::Air, "headroom ({x}, {y}, {z})");
            }
        };
        clear(312, 470);
        clear(312, NORTH_Z0 + 2);
        clear(312, 600);
        for (x, z) in ENEMIES {
            clear(x, z);
        }

        assert_eq!(world.get_voxel(region, 312, GRADE - 1, 470), Material::Path);
        assert_eq!(world.get_voxel(region, 312, GRADE - 1, 600), Material::Path);
        assert_eq!(world.get_voxel(region, 200, GRADE - 1, 600), Material::Grass);
        assert_eq!(world.get_voxel(region, 40, GRADE + 8, NORTH_Z0 + 2), Material::Stone);
        assert_eq!(world.get_voxel(region, 312, GRADE + 8, NORTH_Z0 + 2), Material::Air);
        assert_eq!(world.get_voxel(region, 4, GRADE + 8, 600), Material::Stone);
        assert_eq!(world.get_voxel(region, 312, GRADE + 8, level.map_z - 3), Material::Stone);
        assert_ne!(world.get_voxel(region, 312, 0, 600), Material::Air);

        let (tx, tz) = level.trees[0];
        assert_eq!(world.get_voxel(region, tx, GRADE + 2, tz), Material::Wood);
        let (cx, cz) = level.circle;
        assert_eq!(world.get_voxel(region, cx, GRADE, cz), Material::Air);
    }
}
