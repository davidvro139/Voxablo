//! Diablo I Tristram on the 51 m viewer map.
//!
//! The cathedral and its bell tower sit north, with a walled graveyard in front and a red
//! path out the door. A shallow river winds around the town. The west bank outside the river
//! is forest. Cottages are small and spaced on dirt tracks.
//!
//! A wall is several materials at once: stone plinth, daub, timber posts, thatch or slate,
//! a brick chimney, and a lit window. The 0.1 m cell already fits those courses on a 2 m
//! wall, so the index grid stays 512 and the town stays 51 m. y = 0 is never cleared.

use voxel_core_ffi::{Material, PieceVoxel, VoxelWorld, CHUNK_SIZE};

/// First air cell of the town. Feet stand here.
pub const GRADE: i32 = 24;
/// How far the river bed sits below [`GRADE`], in voxels (0.6 m).
pub const DROP: i32 = 6;

const BED: f32 = 5.0;
const BANK: f32 = 16.0;
const DOOR_H: i32 = 20;

const NAVE_X0: i32 = 256;
const NAVE_X1: i32 = 372;
const NAVE_Z0: i32 = 118;
const NAVE_Z1: i32 = 182;
const TOWER_X0: i32 = 210;
const TOWER_X1: i32 = 256;
const TOWER_Z0: i32 = 100;
const TOWER_Z1: i32 = 182;

/// Cathedral floor. The church sits on the town grade.
pub const CATH_FLOOR: i32 = GRADE;

/// South crossing over the river. The player starts here, facing the cathedral (−Z).
pub const SPAWN: (i32, i32) = (312, 448);
/// On the river centerline, west of the crossing. Read by the layout test.
#[cfg_attr(not(test), allow(dead_code))]
pub const RIVER: (i32, i32) = (185, 444);
/// On the crossing deck, north of the spawn. Read by the layout test.
#[cfg_attr(not(test), allow(dead_code))]
pub const BRIDGE: (i32, i32) = (312, 436);
/// Outer cell of the cathedral door. Read by the layout test.
#[cfg_attr(not(test), allow(dead_code))]
pub const DOOR: (i32, i32) = (312, NAVE_Z1 - 1);
/// Clear aisle inside the nave. Read by the layout test.
#[cfg_attr(not(test), allow(dead_code))]
pub const NAVE: (i32, i32) = (312, 150);
/// Outer cell of the tavern's west door. Read by the layout test.
#[cfg_attr(not(test), allow(dead_code))]
pub const TAVERN_DOOR: (i32, i32) = (332, 320);
pub const WELL: (i32, i32) = (290, 290);

/// Closed river centerline. North is low Z. The south vertex is the spawn.
const RIVER_LINE: [(i32, i32); 15] = [
    (360, 48),
    (240, 56),
    (160, 90),
    (130, 160),
    (124, 250),
    (130, 340),
    (140, 430),
    (230, 458),
    (312, 448),
    (400, 456),
    (470, 420),
    (486, 330),
    (480, 230),
    (450, 140),
    (400, 80),
];

/// Open ground, in roster order. Each cell has a floor at grade and 1.8 m of air.
pub const ENEMIES: [(i32, i32); 9] = [
    (312, 210),
    (260, 270),
    (190, 310),
    (200, 370),
    (280, 320),
    (360, 364),
    (200, 400),
    (400, 270),
    (312, 260),
];

/// Warm lamps. `post` plants a timber the light sits on.
pub const LAMPS: [Lamp; 4] = [
    Lamp { x: 300, y: GRADE + 12, z: 196, post: true },
    Lamp { x: 324, y: GRADE + 12, z: 308, post: true },
    Lamp { x: 220, y: GRADE + 12, z: 258, post: true },
    Lamp { x: 300, y: GRADE + 12, z: 400, post: true },
];

#[derive(Clone, Copy)]
pub struct Lamp {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub post: bool,
}

struct Stamp {
    voxels: Vec<PieceVoxel>,
    max_y: i32,
    max_xz: i32,
}

impl Stamp {
    fn set(&mut self, x: i32, y: i32, z: i32, material: Material) {
        // y = 0 is the structural anchor. Voxels at or above max_y are not meshed.
        if y < 1 || y >= self.max_y || x < 0 || z < 0 || x >= self.max_xz || z >= self.max_xz {
            return;
        }
        self.voxels.push(PieceVoxel { x, y, z, material });
    }

    fn fill(&mut self, x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, material: Material) {
        for y in y0..y1 {
            for z in z0..z1 {
                for x in x0..x1 {
                    self.set(x, y, z, material);
                }
            }
        }
    }
}

fn river_dist(x: i32, z: i32) -> f32 {
    let mut best = f32::MAX;
    let n = RIVER_LINE.len();
    for i in 0..n {
        let (ax, az) = RIVER_LINE[i];
        let (bx, bz) = RIVER_LINE[(i + 1) % n];
        let abx = (bx - ax) as f32;
        let abz = (bz - az) as f32;
        let apx = (x - ax) as f32;
        let apz = (z - az) as f32;
        let den = abx * abx + abz * abz;
        let t = if den == 0.0 { 0.0 } else { ((apx * abx + apz * abz) / den).clamp(0.0, 1.0) };
        let dx = apx - abx * t;
        let dz = apz - abz * t;
        best = best.min(dx * dx + dz * dz);
    }
    best.sqrt()
}

fn inside_town(x: i32, z: i32) -> bool {
    let mut inside = false;
    let n = RIVER_LINE.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, zi) = RIVER_LINE[i];
        let (xj, zj) = RIVER_LINE[j];
        if (zi > z) != (zj > z) {
            let xint = xi as f32 + (xj - xi) as f32 * (z - zi) as f32 / (zj - zi) as f32;
            if (x as f32) < xint {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn drop_at(d: f32) -> i32 {
    if d <= BED {
        DROP
    } else if d <= BANK {
        let t = (d - BED) / (BANK - BED);
        ((1.0 - t) * DROP as f32).round() as i32
    } else {
        0
    }
}

fn highest_solid(world: &VoxelWorld, region: u64, x: i32, z: i32, max_y: i32) -> i32 {
    for y in (0..max_y).rev() {
        if world.get_voxel(region, x, y, z) != Material::Air {
            return y;
        }
    }
    0
}

fn sculpt_ground(stamp: &mut Stamp, world: &VoxelWorld, region: u64) {
    for z in 0..stamp.max_xz {
        for x in 0..stamp.max_xz {
            let d = river_dist(x, z);
            let inside = inside_town(x, z);
            // West of the river is flattened too, so the forest sits on grade instead of the hills.
            if !inside && d > 42.0 && x >= 150 {
                continue;
            }
            let drop = drop_at(d);
            let feet = (GRADE - drop).clamp(2, stamp.max_y - 2);
            let top = feet - 1;
            let solid = highest_solid(world, region, x, z, stamp.max_y);
            if solid < top {
                for y in (solid + 1)..top {
                    stamp.set(x, y, z, Material::Dirt);
                }
            } else if solid > top {
                for y in (top + 1)..=solid {
                    stamp.set(x, y, z, Material::Air);
                }
            }
            let mat = if d <= BED {
                Material::Water
            } else if d <= BANK {
                Material::Stone
            } else {
                Material::Grass
            };
            stamp.set(x, top, z, mat);
            if top > 1 {
                let under = match mat {
                    Material::Water | Material::Stone => Material::Stone,
                    _ => Material::Dirt,
                };
                stamp.set(x, top - 1, z, under);
            }
        }
    }
}

fn track(stamp: &mut Stamp, x0: i32, z0: i32, x1: i32, z1: i32) {
    let (x0, x1) = (x0.min(x1), x0.max(x1));
    let (z0, z1) = (z0.min(z1), z0.max(z1));
    stamp.fill(x0, GRADE - 1, z0, x1, GRADE, z1, Material::Path);
}

fn disc(stamp: &mut Stamp, cx: i32, cz: i32, r2: i32, y0: i32, y1: i32, material: Material) {
    let r = (r2 as f32).sqrt() as i32 + 1;
    for z in (cz - r)..=(cz + r) {
        for x in (cx - r)..=(cx + r) {
            let dx = x - cx;
            let dz = z - cz;
            if dx * dx + dz * dz <= r2 {
                stamp.fill(x, y0, z, x + 1, y1, z + 1, material);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    North,
    South,
    East,
    West,
}

#[derive(Clone, Copy)]
struct Door {
    side: Side,
    at: i32,
    width: i32,
}

struct Hall {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    feet: i32,
    wall_top: i32,
    peak: i32,
    thick: i32,
    wall: Material,
    upper: Material,
    upper_from: i32,
    roof: Material,
    floor: Material,
    door: Option<Door>,
}

fn gable(stamp: &mut Stamp, hall: &Hall) {
    let mid = (hall.x0 + hall.x1) / 2;
    let half = ((hall.x1 - hall.x0) / 2).max(1);
    let rise = hall.peak - hall.wall_top;
    for z in hall.z0..hall.z1 {
        for x in hall.x0..hall.x1 {
            let y = hall.wall_top + rise * (half - (x - mid).abs()) / half;
            stamp.set(x, y, z, hall.roof);
            let end = z < hall.z0 + hall.thick || z >= hall.z1 - hall.thick;
            if end {
                for yy in hall.wall_top..y {
                    let material = if yy >= hall.upper_from { hall.upper } else { hall.wall };
                    stamp.set(x, yy, z, material);
                }
            }
        }
    }
}

fn cut_door(stamp: &mut Stamp, hall: &Hall, door: Door) {
    let half = door.width / 2;
    let y0 = hall.feet;
    let y1 = hall.feet + DOOR_H;
    let t = hall.thick;
    match door.side {
        Side::South => stamp.fill(door.at - half, y0, hall.z1 - t, door.at + half, y1, hall.z1, Material::Air),
        Side::North => stamp.fill(door.at - half, y0, hall.z0, door.at + half, y1, hall.z0 + t, Material::Air),
        Side::West => stamp.fill(hall.x0, y0, door.at - half, hall.x0 + t, y1, door.at + half, Material::Air),
        Side::East => stamp.fill(hall.x1 - t, y0, door.at - half, hall.x1, y1, door.at + half, Material::Air),
    }
}

fn shell(stamp: &mut Stamp, hall: &Hall) {
    // Reach into the grade so a hall is not sitting on a one-voxel gap.
    let base = (hall.feet - 3).min(GRADE - 2);
    stamp.fill(hall.x0, base, hall.z0, hall.x1, hall.feet - 1, hall.z1, Material::Stone);
    stamp.fill(hall.x0, hall.feet - 1, hall.z0, hall.x1, hall.feet, hall.z1, hall.floor);
    stamp.fill(hall.x0, hall.feet, hall.z0, hall.x1, hall.peak + 2, hall.z1, Material::Air);
    for y in hall.feet..hall.wall_top {
        let material = if y >= hall.upper_from { hall.upper } else { hall.wall };
        for z in hall.z0..hall.z1 {
            for x in hall.x0..hall.x1 {
                let edge = x < hall.x0 + hall.thick
                    || x >= hall.x1 - hall.thick
                    || z < hall.z0 + hall.thick
                    || z >= hall.z1 - hall.thick;
                if edge {
                    stamp.set(x, y, z, material);
                }
            }
        }
    }
    gable(stamp, hall);
    if let Some(door) = hall.door {
        cut_door(stamp, hall, door);
    }
}

fn eaves(stamp: &mut Stamp, hall: &Hall) {
    let y = hall.wall_top;
    let roof = hall.roof;
    stamp.fill(hall.x0 - 1, y, hall.z0 - 1, hall.x1 + 1, y + 1, hall.z0, roof);
    stamp.fill(hall.x0 - 1, y, hall.z1, hall.x1 + 1, y + 1, hall.z1 + 1, roof);
    stamp.fill(hall.x0 - 1, y, hall.z0, hall.x0, y + 1, hall.z1, roof);
    stamp.fill(hall.x1, y, hall.z0, hall.x1 + 1, y + 1, hall.z1, roof);
}

fn half_timber(stamp: &mut Stamp, hall: &Hall) {
    let y0 = hall.feet;
    let y1 = hall.wall_top;
    let mut x = hall.x0;
    while x < hall.x1 {
        stamp.fill(x, y0, hall.z0, x + 1, y1, hall.z0 + 1, Material::Wood);
        stamp.fill(x, y0, hall.z1 - 1, x + 1, y1, hall.z1, Material::Wood);
        x += 8;
    }
    let mut z = hall.z0;
    while z < hall.z1 {
        stamp.fill(hall.x0, y0, z, hall.x0 + 1, y1, z + 1, Material::Wood);
        stamp.fill(hall.x1 - 1, y0, z, hall.x1, y1, z + 1, Material::Wood);
        z += 8;
    }
    // Belt on the wall thickness only. A full slab would seal the room at chest height.
    let band0 = hall.upper_from - 1;
    let band1 = hall.upper_from + 1;
    let t = hall.thick;
    stamp.fill(hall.x0, band0, hall.z0, hall.x1, band1, hall.z0 + t, Material::Wood);
    stamp.fill(hall.x0, band0, hall.z1 - t, hall.x1, band1, hall.z1, Material::Wood);
    stamp.fill(hall.x0, band0, hall.z0, hall.x0 + t, band1, hall.z1, Material::Wood);
    stamp.fill(hall.x1 - t, band0, hall.z0, hall.x1, band1, hall.z1, Material::Wood);
    if let Some(door) = hall.door {
        cut_door(stamp, hall, door);
    }
}

fn warm_windows(stamp: &mut Stamp, hall: &Hall) {
    let y0 = hall.feet + 8;
    let y1 = hall.feet + 13;
    if y1 > hall.wall_top {
        return;
    }
    let blocked = |side: Side, at: i32| {
        hall.door.is_some_and(|door| door.side == side && (at - door.at).abs() <= door.width / 2 + 2)
    };
    let mut window = |side: Side, x0: i32, z0: i32, x1: i32, z1: i32| {
        let at = match side {
            Side::North | Side::South => (x0 + x1) / 2,
            Side::East | Side::West => (z0 + z1) / 2,
        };
        if !blocked(side, at) {
            stamp.fill(x0, y0, z0, x1, y1, z1, Material::Ember);
        }
    };
    let t = hall.thick;
    window(Side::South, hall.x0 + 4, hall.z1 - t, hall.x0 + 7, hall.z1);
    window(Side::South, hall.x1 - 7, hall.z1 - t, hall.x1 - 4, hall.z1);
    window(Side::North, hall.x0 + 4, hall.z0, hall.x0 + 7, hall.z0 + t);
    window(Side::North, hall.x1 - 7, hall.z0, hall.x1 - 4, hall.z0 + t);
    window(Side::West, hall.x0, hall.z0 + 4, hall.x0 + t, hall.z0 + 7);
    window(Side::West, hall.x0, hall.z1 - 7, hall.x0 + t, hall.z1 - 4);
    window(Side::East, hall.x1 - t, hall.z0 + 4, hall.x1, hall.z0 + 7);
    window(Side::East, hall.x1 - t, hall.z1 - 7, hall.x1, hall.z1 - 4);
}

fn door_frame(stamp: &mut Stamp, hall: &Hall) {
    let Some(door) = hall.door else {
        return;
    };
    let half = door.width / 2;
    let y0 = hall.feet;
    let y1 = hall.feet + DOOR_H;
    let t = hall.thick;
    let wood = Material::Wood;
    match door.side {
        Side::South => {
            stamp.fill(door.at - half - 1, y0, hall.z1 - t, door.at - half, y1, hall.z1, wood);
            stamp.fill(door.at + half, y0, hall.z1 - t, door.at + half + 1, y1, hall.z1, wood);
            stamp.fill(door.at - half, y1, hall.z1 - t, door.at + half, y1 + 1, hall.z1, wood);
        }
        Side::North => {
            stamp.fill(door.at - half - 1, y0, hall.z0, door.at - half, y1, hall.z0 + t, wood);
            stamp.fill(door.at + half, y0, hall.z0, door.at + half + 1, y1, hall.z0 + t, wood);
            stamp.fill(door.at - half, y1, hall.z0, door.at + half, y1 + 1, hall.z0 + t, wood);
        }
        Side::West => {
            stamp.fill(hall.x0, y0, door.at - half - 1, hall.x0 + t, y1, door.at - half, wood);
            stamp.fill(hall.x0, y0, door.at + half, hall.x0 + t, y1, door.at + half + 1, wood);
            stamp.fill(hall.x0, y1, door.at - half, hall.x0 + t, y1 + 1, door.at + half, wood);
        }
        Side::East => {
            stamp.fill(hall.x1 - t, y0, door.at - half - 1, hall.x1, y1, door.at - half, wood);
            stamp.fill(hall.x1 - t, y0, door.at + half, hall.x1, y1, door.at + half + 1, wood);
            stamp.fill(hall.x1 - t, y1, door.at - half, hall.x1, y1 + 1, door.at + half, wood);
        }
    }
}

fn cottage(
    stamp: &mut Stamp,
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    door: Door,
    wall_top: i32,
    peak: i32,
    upper: Material,
    roof: Material,
) {
    let hall = Hall {
        x0,
        z0,
        x1,
        z1,
        feet: GRADE,
        wall_top,
        peak,
        thick: 2,
        wall: Material::Stone,
        upper,
        upper_from: GRADE + 4,
        roof,
        floor: Material::Wood,
        door: Some(door),
    };
    shell(stamp, &hall);
    eaves(stamp, &hall);
    half_timber(stamp, &hall);
    warm_windows(stamp, &hall);
    door_frame(stamp, &hall);
}

fn chimney(stamp: &mut Stamp, x: i32, z: i32, top: i32) {
    stamp.fill(x, GRADE, z, x + 3, top, z + 3, Material::Brick);
    stamp.set(x + 1, top, z + 1, Material::Ember);
}

fn houses(stamp: &mut Stamp) {
    let wall = GRADE + 20;
    let peak = GRADE + 36;
    // Cain, west of the yard. Door faces the track.
    cottage(
        stamp,
        172,
        248,
        214,
        290,
        Door { side: Side::East, at: 268, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 174, 250, GRADE + 40);

    cottage(
        stamp,
        172,
        318,
        214,
        360,
        Door { side: Side::East, at: 348, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 174, 320, GRADE + 40);

    cottage(
        stamp,
        230,
        360,
        274,
        402,
        Door { side: Side::North, at: 252, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 232, 388, GRADE + 40);

    cottage(
        stamp,
        230,
        300,
        272,
        342,
        Door { side: Side::East, at: 320, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 232, 328, GRADE + 40);

    // Ogden's tavern, the large house east of the road. Door faces west.
    cottage(
        stamp,
        332,
        286,
        414,
        356,
        Door { side: Side::West, at: 320, width: 8 },
        GRADE + 24,
        GRADE + 52,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 390, 288, GRADE + 56);
    // Awning clears the door probe: it stops at the wall and sits above 1.8 m.
    stamp.fill(322, GRADE + 20, 312, 332, GRADE + 21, 328, Material::Cloth);
    post(stamp, 322, 312, GRADE + 20);
    post(stamp, 322, 327, GRADE + 20);
    stamp.fill(390, GRADE, 300, 408, GRADE + 4, 340, Material::Wood);
    stamp.fill(396, GRADE + 1, 330, 404, GRADE + 3, 338, Material::Ember);

    // Griswold. Brick over a stone plinth, slate roof, hearth at the back.
    cottage(
        stamp,
        336,
        372,
        400,
        414,
        Door { side: Side::West, at: 392, width: 8 },
        GRADE + 22,
        GRADE + 38,
        Material::Brick,
        Material::Tile,
    );
    chimney(stamp, 370, 400, GRADE + 44);
    stamp.fill(372, GRADE, 402, 384, GRADE + 5, 410, Material::Brick);
    stamp.fill(374, GRADE + 1, 404, 382, GRADE + 3, 408, Material::Ember);

    cottage(
        stamp,
        420,
        250,
        460,
        292,
        Door { side: Side::West, at: 270, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 422, 252, GRADE + 40);

    cottage(
        stamp,
        400,
        172,
        432,
        204,
        Door { side: Side::South, at: 416, width: 8 },
        wall,
        peak,
        Material::Plaster,
        Material::Thatch,
    );
    chimney(stamp, 402, 174, GRADE + 40);
}

fn cathedral(stamp: &mut Stamp) {
    let hall = Hall {
        x0: NAVE_X0,
        z0: NAVE_Z0,
        x1: NAVE_X1,
        z1: NAVE_Z1,
        feet: CATH_FLOOR,
        wall_top: GRADE + 30,
        peak: GRADE + 44,
        thick: 3,
        wall: Material::Stone,
        upper: Material::Stone,
        upper_from: GRADE + 30,
        roof: Material::Tile,
        floor: Material::Stone,
        door: Some(Door { side: Side::South, at: 312, width: 10 }),
    };
    shell(stamp, &hall);

    // Cloth runner, floor voxel only, so the aisle stays open.
    stamp.fill(308, CATH_FLOOR - 1, NAVE_Z0 + 10, 316, CATH_FLOOR, 176, Material::Cloth);

    // Gold jambs sit just outside the opening (x 307..317 is the cut).
    for x in [306, 317] {
        stamp.fill(x, CATH_FLOOR, NAVE_Z1 - 3, x + 1, CATH_FLOOR + DOOR_H, NAVE_Z1, Material::Gold);
    }
    stamp.fill(306, CATH_FLOOR + DOOR_H, NAVE_Z1 - 3, 318, CATH_FLOOR + DOOR_H + 2, NAVE_Z1, Material::Gold);

    // Red glass in the south wall, clear of the door, and a rose above the lintel.
    stamp.fill(268, CATH_FLOOR + 8, NAVE_Z1 - 3, 276, CATH_FLOOR + 16, NAVE_Z1, Material::Glass);
    stamp.fill(348, CATH_FLOOR + 8, NAVE_Z1 - 3, 360, CATH_FLOOR + 16, NAVE_Z1, Material::Glass);
    let rose = hall.wall_top;
    stamp.fill(296, rose, NAVE_Z1 - 3, 328, rose + 12, NAVE_Z1, Material::Glass);

    // Buttresses beside the path, not on it.
    stamp.fill(268, CATH_FLOOR, NAVE_Z1, 276, CATH_FLOOR + 14, NAVE_Z1 + 6, Material::Stone);
    stamp.fill(348, CATH_FLOOR, NAVE_Z1, 356, CATH_FLOOR + 14, NAVE_Z1 + 6, Material::Stone);

    for z in [140, 160] {
        for x in [276, 348] {
            stamp.fill(x, CATH_FLOOR, z, x + 2, CATH_FLOOR + 14, z + 2, Material::Stone);
        }
    }

    for z in [136, 144, 156, 164] {
        stamp.fill(268, CATH_FLOOR, z, 300, CATH_FLOOR + 3, z + 2, Material::Wood);
        stamp.fill(328, CATH_FLOOR, z, 360, CATH_FLOOR + 3, z + 2, Material::Wood);
        stamp.fill(268, CATH_FLOOR + 2, z, 300, CATH_FLOOR + 3, z + 2, Material::Cloth);
        stamp.fill(328, CATH_FLOOR + 2, z, 360, CATH_FLOOR + 3, z + 2, Material::Cloth);
    }
    stamp.fill(296, CATH_FLOOR, NAVE_Z0 + 6, 328, CATH_FLOOR + 4, NAVE_Z0 + 14, Material::Stone);
    stamp.fill(300, CATH_FLOOR + 3, NAVE_Z0 + 8, 324, CATH_FLOOR + 4, NAVE_Z0 + 12, Material::Gold);
    stamp.set(304, CATH_FLOOR + 4, NAVE_Z0 + 9, Material::Ember);
    stamp.set(320, CATH_FLOOR + 4, NAVE_Z0 + 10, Material::Ember);

    tower(stamp);
}

fn tower(stamp: &mut Stamp) {
    let belfry = GRADE + 46;
    let crown = GRADE + 58;
    let cap = GRADE + 70;
    stamp.fill(TOWER_X0, GRADE - 2, TOWER_Z0, TOWER_X1, belfry, TOWER_Z1, Material::Stone);
    stamp.fill(TOWER_X0 + 3, belfry, TOWER_Z0 + 3, TOWER_X1 - 3, crown, TOWER_Z1 - 3, Material::Air);
    stamp.fill(TOWER_X0, belfry, TOWER_Z0, TOWER_X1, crown, TOWER_Z0 + 3, Material::Stone);
    stamp.fill(TOWER_X0, belfry, TOWER_Z1 - 3, TOWER_X1, crown, TOWER_Z1, Material::Stone);
    stamp.fill(TOWER_X0, belfry, TOWER_Z0, TOWER_X0 + 3, crown, TOWER_Z1, Material::Stone);
    stamp.fill(TOWER_X1 - 3, belfry, TOWER_Z0, TOWER_X1, crown, TOWER_Z1, Material::Stone);
    stamp.fill(224, belfry + 2, TOWER_Z1 - 2, 242, belfry + 10, TOWER_Z1, Material::Glass);
    stamp.fill(TOWER_X0, belfry + 2, 130, TOWER_X0 + 2, belfry + 10, 148, Material::Glass);
    let rise = (cap - crown).max(1);
    let half = ((TOWER_X1 - TOWER_X0).max(TOWER_Z1 - TOWER_Z0) / 2).max(1);
    for y in crown..cap {
        let inset = (y - crown) * half / rise;
        let x0 = TOWER_X0 + inset;
        let z0 = TOWER_Z0 + inset;
        let x1 = TOWER_X1 - inset;
        let z1 = TOWER_Z1 - inset;
        if x0 < x1 && z0 < z1 {
            stamp.fill(x0, y, z0, x1, y + 1, z1, Material::Tile);
        }
    }
}

fn graveyard(stamp: &mut Stamp) {
    let y0 = GRADE;
    let y1 = GRADE + 5;
    stamp.fill(210, y0, 182, 214, y1, 252, Material::Stone);
    stamp.fill(372, y0, 182, 376, y1, 252, Material::Stone);
    stamp.fill(210, y0, 248, 304, y1, 252, Material::Stone);
    stamp.fill(324, y0, 248, 376, y1, 252, Material::Stone);
    for (x, z) in [(240, 200), (256, 222), (274, 200), (340, 200), (356, 222), (370, 204), (242, 230), (360, 230)] {
        grave(stamp, x, z);
    }
}

fn red_path(stamp: &mut Stamp) {
    // Same red as the cathedral glass. Window hearths stay on Ember, which reads gold.
    stamp.fill(306, GRADE - 1, 182, 318, GRADE, 252, Material::Glass);
    stamp.fill(308, GRADE - 1, 176, 316, GRADE, 182, Material::Glass);
}

fn well(stamp: &mut Stamp) {
    let (cx, cz) = WELL;
    disc(stamp, cx, cz, 25, GRADE - 2, GRADE, Material::Stone);
    // Mouth is narrower than a body, so a walker cannot drop in.
    disc(stamp, cx, cz, 4, GRADE, GRADE + 8, Material::Air);
    let r = 6;
    for z in (cz - r)..=(cz + r) {
        for x in (cx - r)..=(cx + r) {
            let d2 = (x - cx) * (x - cx) + (z - cz) * (z - cz);
            if (5..=25).contains(&d2) {
                stamp.fill(x, GRADE, z, x + 1, GRADE + 6, z + 1, Material::Stone);
            }
        }
    }
    for (dx, dz) in [(3, 4), (3, -4), (-3, 4), (-3, -4)] {
        post(stamp, cx + dx, cz + dz, GRADE + 11);
    }
    disc(stamp, cx, cz, 20, GRADE + 11, GRADE + 12, Material::Wood);
    stamp.set(cx, GRADE + 12, cz, Material::Gold);
}

fn causeway(stamp: &mut Stamp) {
    // Short solid crossing. No rails: the screenshot is a flat span, and the ends stay open.
    let (x0, x1, z0, z1) = (300, 326, 430, 468);
    stamp.fill(x0, GRADE - DROP, z0, x1, GRADE - 2, z1, Material::Stone);
    stamp.fill(x0, GRADE - 2, z0, x1, GRADE, z1, Material::Wood);
}

fn paths(stamp: &mut Stamp) {
    track(stamp, 308, 182, 316, 434);
    track(stamp, 214, 264, 308, 272);
    track(stamp, 214, 344, 308, 352);
    track(stamp, 248, 352, 256, 360);
    track(stamp, 272, 316, 308, 324);
    track(stamp, 316, 316, 332, 324);
    track(stamp, 316, 388, 336, 396);
    track(stamp, 316, 266, 420, 274);
}

fn grave(stamp: &mut Stamp, x: i32, z: i32) {
    stamp.fill(x, GRADE, z, x + 2, GRADE + 1, z + 1, Material::Stone);
    stamp.fill(x, GRADE + 1, z, x + 1, GRADE + 5, z + 1, Material::Stone);
}

fn post(stamp: &mut Stamp, x: i32, z: i32, top: i32) {
    stamp.fill(x, GRADE, z, x + 1, top, z + 1, Material::Wood);
}

fn dead_tree(stamp: &mut Stamp, x: i32, z: i32) {
    stamp.fill(x, GRADE, z, x + 2, GRADE + 14, z + 2, Material::Wood);
    stamp.fill(x + 2, GRADE + 9, z, x + 7, GRADE + 11, z + 2, Material::Wood);
    stamp.fill(x - 4, GRADE + 7, z, x, GRADE + 9, z + 2, Material::Wood);
    stamp.fill(x, GRADE + 11, z + 2, x + 2, GRADE + 13, z + 6, Material::Wood);
}

fn canopy(stamp: &mut Stamp, x: i32, z: i32) {
    stamp.fill(x, GRADE, z, x + 2, GRADE + 10, z + 2, Material::Wood);
    disc(stamp, x, z, 8, GRADE + 8, GRADE + 15, Material::Violet);
}

fn forest(stamp: &mut Stamp) {
    for z in (40..480).step_by(7) {
        for x in (16..120).step_by(7) {
            if inside_town(x, z) || river_dist(x, z) < 20.0 {
                continue;
            }
            if (x * 3 + z) % 11 != 0 {
                continue;
            }
            canopy(stamp, x, z);
        }
    }
    canopy(stamp, 48, 220);
}

fn pasture(stamp: &mut Stamp) {
    stamp.fill(378, GRADE, 128, 424, GRADE + 4, 132, Material::Stone);
    stamp.fill(420, GRADE, 132, 424, GRADE + 4, 162, Material::Stone);
    for (x, z) in [(392, 142), (406, 150)] {
        stamp.fill(x, GRADE, z, x + 3, GRADE + 2, z + 2, Material::Plaster);
    }
}

fn dress(stamp: &mut Stamp) {
    dead_tree(stamp, 148, 230);
    dead_tree(stamp, 200, 175);
    dead_tree(stamp, 430, 320);
    post(stamp, 318, 300, GRADE + 14);
    stamp.fill(319, GRADE + 8, 300, 326, GRADE + 14, 301, Material::Cloth);
    for lamp in LAMPS {
        if lamp.post {
            post(stamp, lamp.x, lamp.z, lamp.y);
            stamp.set(lamp.x, lamp.y, lamp.z, Material::Ember);
        }
    }
}

/// Stamps Tristram over terrain that is already generated for this region.
pub fn build(world: &VoxelWorld, region: u64) {
    let max_y = super::VERTICAL_CHUNKS * CHUNK_SIZE;
    let mut stamp = Stamp {
        voxels: Vec::with_capacity(2_000_000),
        max_y,
        max_xz: super::GRID * CHUNK_SIZE,
    };
    sculpt_ground(&mut stamp, world, region);
    paths(&mut stamp);
    red_path(&mut stamp);
    houses(&mut stamp);
    cathedral(&mut stamp);
    graveyard(&mut stamp);
    well(&mut stamp);
    forest(&mut stamp);
    pasture(&mut stamp);
    dress(&mut stamp);
    causeway(&mut stamp);
    world.set_voxels(region, &stamp.voxels);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn column_clear(world: &VoxelWorld, x: i32, feet: i32, z: i32, tall: i32) {
        assert_ne!(
            world.get_voxel(1, x, feet - 1, z),
            Material::Air,
            "expected a floor under ({x}, {z}) at y {}",
            feet - 1
        );
        for y in feet..(feet + tall) {
            assert_eq!(
                world.get_voxel(1, x, y, z),
                Material::Air,
                "({x}, {y}, {z}) should be walkable headroom"
            );
        }
    }

    #[test]
    fn tristram_stands_on_the_river_and_the_cathedral_door_is_open() {
        let world = VoxelWorld::new();
        let region = 1;
        world.load_region(region);
        let chunks = super::super::GRID;
        for x in 0..chunks {
            for z in 0..chunks {
                world.generate_terrain(region, x, 0, z);
            }
        }
        build(&world, region);

        column_clear(&world, SPAWN.0, GRADE, SPAWN.1, 18);
        column_clear(&world, BRIDGE.0, GRADE, BRIDGE.1, 12);
        for (x, z) in ENEMIES {
            column_clear(&world, x, GRADE, z, 18);
        }
        column_clear(&world, DOOR.0, CATH_FLOOR, DOOR.1, 18);
        column_clear(&world, NAVE.0, CATH_FLOOR, NAVE.1, 18);
        column_clear(&world, TAVERN_DOOR.0, GRADE, TAVERN_DOOR.1, 18);

        assert!(river_dist(RIVER.0, RIVER.1) < 0.6, "river probe left the centerline");
        assert_eq!(world.get_voxel(region, RIVER.0, GRADE - DROP - 1, RIVER.1), Material::Water);
        assert_eq!(world.get_voxel(region, RIVER.0, GRADE - DROP - 2, RIVER.1), Material::Stone);
        assert_eq!(world.get_voxel(region, RIVER.0, GRADE - DROP, RIVER.1), Material::Air);

        assert_eq!(world.get_voxel(region, SPAWN.0, GRADE - 1, SPAWN.1), Material::Wood);
        assert_eq!(world.get_voxel(region, 312, GRADE - 1, 300), Material::Path);
        assert_eq!(world.get_voxel(region, 190, GRADE - 1, 230), Material::Grass);

        assert_eq!(world.get_voxel(region, 271, CATH_FLOOR + 10, NAVE_Z1 - 1), Material::Glass);
        assert_eq!(world.get_voxel(region, 306, CATH_FLOOR, NAVE_Z1 - 1), Material::Gold);

        // Cain's south wall is a stack of materials, not one block.
        assert_eq!(world.get_voxel(region, 186, GRADE + 1, 289), Material::Stone);
        assert_eq!(world.get_voxel(region, 186, GRADE + 8, 289), Material::Plaster);
        assert_eq!(world.get_voxel(region, 180, GRADE + 8, 289), Material::Wood);
        assert_eq!(world.get_voxel(region, 177, GRADE + 10, 289), Material::Ember);
        assert_eq!(world.get_voxel(region, 193, GRADE + 36, 270), Material::Thatch);
        assert_eq!(world.get_voxel(region, 175, GRADE + 12, 251), Material::Brick);

        assert_eq!(world.get_voxel(region, 326, GRADE + 20, 320), Material::Cloth);
        assert_eq!(world.get_voxel(region, 250, GRADE + 2, 250), Material::Stone);
        assert_eq!(world.get_voxel(region, 312, GRADE + 2, 250), Material::Air);
        assert_eq!(world.get_voxel(region, 48, GRADE + 12, 220), Material::Violet);

        assert_eq!(world.get_voxel(region, WELL.0, GRADE, WELL.1), Material::Air);
        assert_ne!(world.get_voxel(region, WELL.0, GRADE - 1, WELL.1), Material::Air);
        assert_eq!(world.get_voxel(region, WELL.0 + 4, GRADE + 2, WELL.1), Material::Stone);

        assert_ne!(world.get_voxel(region, RIVER.0, 0, RIVER.1), Material::Air);
        assert_ne!(world.get_voxel(region, SPAWN.0, 0, SPAWN.1), Material::Air);
    }
}
