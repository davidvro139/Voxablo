//! Small colored voxel models (characters), meshed face-by-face.

use bevy::math::{IVec3, Vec3};

pub type Color = [f32; 4];

pub struct MeshArrays {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub colors: Vec<Color>,
    pub indices: Vec<u32>,
}

pub struct VoxelModel {
    size: IVec3,
    /// Model-local x/z of voxel (0, 0), in voxels. Defaults to the canvas centre.
    /// A padded canvas keeps this on the unpadded body so part boxes stay aligned.
    anchor: [f32; 2],
    voxels: Vec<Option<Color>>,
    /// Parallel to `voxels`. Empty cells are 0 and are ignored by [`Self::limb`].
    limbs: Vec<u8>,
}

/// Segment ids for the jointed rig. The live walk rotates these; attack sculpts
/// still fill the same ids so a later overwrite keeps the visible part.
pub const LIMB_BODY: u8 = 0;
pub const LIMB_HEAD: u8 = 1;
pub const LIMB_THIGH_L: u8 = 2;
pub const LIMB_SHIN_L: u8 = 3;
pub const LIMB_FOOT_L: u8 = 4;
pub const LIMB_THIGH_R: u8 = 5;
pub const LIMB_SHIN_R: u8 = 6;
pub const LIMB_FOOT_R: u8 = 7;
pub const LIMB_ARM_L: u8 = 8;
pub const LIMB_FORE_L: u8 = 9;
pub const LIMB_ARM_R: u8 = 10;
pub const LIMB_FORE_R: u8 = 11;
pub const LIMB_SWORD: u8 = 12;
pub const LIMB_CLEAVER_L: u8 = 13;
pub const LIMB_CLEAVER_R: u8 = 14;
pub const LIMB_TAIL: u8 = 15;
pub const LIMB_HORN: u8 = 16;
pub const LIMB_COUNT: usize = 17;

/// Coarse whole-body poses. Attack poses commit a weapon through the swing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motion {
    Idle,
    StepLeft,
    StepRight,
    /// Both arms forward. The live cast is a shoulder blend; this pose remains for the sculpt.
    #[allow(dead_code)]
    Cast,
    Windup,
    Strike,
    /// Follow-through. The live recover is interpolated; this pose remains for the sculpt.
    #[allow(dead_code)]
    Recover,
}

pub const DEMON_LEFT_ARM: u8 = 1 << 0;
pub const DEMON_RIGHT_ARM: u8 = 1 << 1;
pub const DEMON_HORNS: u8 = 1 << 2;
pub const DEMON_TAIL: u8 = 1 << 3;
pub const DEMON_LEFT_LEG: u8 = 1 << 4;
pub const DEMON_RIGHT_LEG: u8 = 1 << 5;
pub const DEMON_HEAD: u8 = 1 << 6;

/// Neighbour offset and the unit-cube corners of that face, counter-clockwise from outside.
const FACES: [([i32; 3], [[f32; 3]; 4]); 6] = [
    ([1, 0, 0], [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]]),
    ([-1, 0, 0], [[0., 0., 1.], [0., 1., 1.], [0., 1., 0.], [0., 0., 0.]]),
    ([0, 1, 0], [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]]),
    ([0, -1, 0], [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]]),
    ([0, 0, 1], [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]]),
    ([0, 0, -1], [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]]),
];

impl VoxelModel {
    pub fn new(size: IVec3) -> Self {
        Self::with_anchor(size, [size.x as f32 / 2.0, size.z as f32 / 2.0])
    }

    pub fn with_anchor(size: IVec3, anchor: [f32; 2]) -> Self {
        let n = (size.x * size.y * size.z) as usize;
        Self { size, anchor, voxels: vec![None; n], limbs: vec![0; n] }
    }

    fn index(&self, p: IVec3) -> Option<usize> {
        (p.cmpge(IVec3::ZERO).all() && p.cmplt(self.size).all())
            .then(|| (p.x + p.z * self.size.x + p.y * self.size.x * self.size.z) as usize)
    }

    pub fn get(&self, p: IVec3) -> Option<Color> {
        self.index(p).and_then(|i| self.voxels[i])
    }

    #[allow(dead_code)]
    pub fn scaled(&self, factor: i32) -> Self {
        let factor = factor.max(1);
        let mut out = Self::with_anchor(
            self.size * factor,
            [self.anchor[0] * factor as f32, self.anchor[1] * factor as f32],
        );
        for y in 0..self.size.y {
            for z in 0..self.size.z {
                for x in 0..self.size.x {
                    let p = IVec3::new(x, y, z);
                    let Some(color) = self.get(p) else { continue };
                    let limb = self.limb(p).unwrap_or(LIMB_BODY);
                    out.fill_part(
                        [x * factor, y * factor, z * factor],
                        [(x + 1) * factor, (y + 1) * factor, (z + 1) * factor],
                        color,
                        limb,
                    );
                }
            }
        }
        out
    }

    /// Fills the box `min..max` (max exclusive) as body.
    pub fn fill(&mut self, min: [i32; 3], max: [i32; 3], color: Color) {
        self.fill_part(min, max, color, LIMB_BODY);
    }

    /// Fills the box and tags every voxel with a rig segment.
    pub fn fill_part(&mut self, min: [i32; 3], max: [i32; 3], color: Color, limb: u8) {
        for y in min[1]..max[1] {
            for z in min[2]..max[2] {
                for x in min[0]..max[0] {
                    if let Some(i) = self.index(IVec3::new(x, y, z)) {
                        self.voxels[i] = Some(color);
                        self.limbs[i] = limb;
                    }
                }
            }
        }
    }

    /// Segment id of an occupied in-bounds voxel. Empty and out-of-range are `None`,
    /// so a default limb of 0 is not confused with the body.
    pub fn limb(&self, p: IVec3) -> Option<u8> {
        let i = self.index(p)?;
        self.voxels[i].map(|_| self.limbs[i])
    }

    pub(crate) fn each_solid(&self, mut f: impl FnMut(IVec3, Color, u8)) {
        for y in 0..self.size.y {
            for z in 0..self.size.z {
                for x in 0..self.size.x {
                    let p = IVec3::new(x, y, z);
                    if let Some(color) = self.get(p) {
                        f(p, color, self.limbs[self.index(p).unwrap()]);
                    }
                }
            }
        }
    }

    /// Inclusive-exclusive metre bounds of one segment. `color` limits the box
    /// (weapon grips sit above a hanging blade).
    pub fn part_aabb(&self, voxel_size: f32, part: u8, color: Option<Color>) -> Option<(Vec3, Vec3)> {
        let offset = Vec3::new(-self.anchor[0], 0.0, -self.anchor[1]);
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        let mut any = false;
        self.each_solid(|p, voxel_color, limb| {
            if limb != part || color.is_some_and(|want| voxel_color != want) {
                return;
            }
            any = true;
            let a = (p.as_vec3() + offset) * voxel_size;
            let b = (p.as_vec3() + Vec3::ONE + offset) * voxel_size;
            min = min.min(a);
            max = max.max(b);
        });
        any.then_some((min, max))
    }

    /// One segment, in character metres (same anchor as [`Self::mesh`]).
    /// Faces are culled only against the same segment, so a swung limb stays closed.
    pub fn mesh_part(&self, voxel_size: f32, part: u8) -> MeshArrays {
        let offset = Vec3::new(-self.anchor[0], 0.0, -self.anchor[1]);
        let mut out = MeshArrays { positions: vec![], normals: vec![], colors: vec![], indices: vec![] };
        for y in 0..self.size.y {
            for z in 0..self.size.z {
                for x in 0..self.size.x {
                    let p = IVec3::new(x, y, z);
                    if self.limb(p) != Some(part) {
                        continue;
                    }
                    let Some(color) = self.get(p) else { continue };
                    for (dir, corners) in FACES {
                        let dir = IVec3::from(dir);
                        if self.limb(p + dir) == Some(part) {
                            continue;
                        }
                        let base = out.positions.len() as u32;
                        for c in corners {
                            let v = (p.as_vec3() + Vec3::from(c) + offset) * voxel_size;
                            out.positions.push(v.into());
                            out.normals.push(dir.as_vec3().into());
                            out.colors.push(color);
                        }
                        out.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
        out
    }

    /// Mesh in metres. Voxel (0, 0) sits at `-anchor`, so the base stays on y = 0
    /// and a padded canvas does not shift the body.
    pub fn mesh(&self, voxel_size: f32) -> MeshArrays {
        self.mesh_offset(voxel_size, Vec3::new(-self.anchor[0], 0.0, -self.anchor[1]))
    }

    /// Mesh in metres with voxel (0, 0, 0) spanning 0..voxel_size.
    pub fn mesh_from_corner(&self, voxel_size: f32) -> MeshArrays {
        self.mesh_offset(voxel_size, Vec3::ZERO)
    }

    fn mesh_offset(&self, voxel_size: f32, offset: Vec3) -> MeshArrays {
        let mut out = MeshArrays { positions: vec![], normals: vec![], colors: vec![], indices: vec![] };

        for y in 0..self.size.y {
            for z in 0..self.size.z {
                for x in 0..self.size.x {
                    let p = IVec3::new(x, y, z);
                    let Some(color) = self.get(p) else { continue };

                    for (dir, corners) in FACES {
                        let dir = IVec3::from(dir);
                        if self.get(p + dir).is_some() {
                            continue;
                        }
                        let base = out.positions.len() as u32;
                        for c in corners {
                            let v = (p.as_vec3() + Vec3::from(c) + offset) * voxel_size;
                            out.positions.push(v.into());
                            out.normals.push(dir.as_vec3().into());
                            out.colors.push(color);
                        }
                        out.indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
                    }
                }
            }
        }
        out
    }
}

/// Sword steel. Distinct from armour so tests can find the blade.
const SWORD_BLADE: Color = [0.78, 0.84, 0.92, 1.0];
const SWORD_EDGE: Color = [0.94, 0.97, 1.0, 1.0];
pub(crate) const SWORD_GRIP: Color = [0.32, 0.18, 0.10, 1.0];
const SWORD_GUARD: Color = [0.74, 0.60, 0.24, 1.0];

/// Cleaver steel. Distinct from the horn/claw colour.
const CLEAVER_BLADE: Color = [0.62, 0.67, 0.74, 1.0];
const CLEAVER_EDGE: Color = [0.84, 0.88, 0.93, 1.0];
pub(crate) const CLEAVER_GRIP: Color = [0.22, 0.09, 0.05, 1.0];

/// Extra voxels in front of the old body so a blade can reach unpadded z ≈ -12
/// without moving the torso. Anchor is the old centre plus this pad.
const PLAYER_PAD_X: i32 = 2;
const PLAYER_PAD_Z: i32 = 12;
const DEMON_PAD_X: i32 = 2;
const DEMON_PAD_Z: i32 = 10;

fn stamp(
    m: &mut VoxelModel,
    min: [i32; 3],
    max: [i32; 3],
    dx: i32,
    dy: i32,
    dz: i32,
    drop: i32,
    pad_x: i32,
    pad_z: i32,
    color: Color,
    limb: u8,
) {
    let y0 = min[1] + dy - drop;
    let y1 = max[1] + dy - drop;
    if y1 <= 0 {
        return;
    }
    m.fill_part(
        [min[0] + dx + pad_x, y0.max(0), min[2] + dz + pad_z],
        [max[0] + dx + pad_x, y1, max[2] + dz + pad_z],
        color,
        limb,
    );
}

/// A 1.8 m adventurer facing -Z, sword held in the right hand.
#[allow(dead_code)]
pub fn adventurer() -> VoxelModel {
    adventurer_motion(Motion::Idle)
}

pub fn adventurer_motion(motion: Motion) -> VoxelModel {
    const SKIN: Color = [0.86, 0.66, 0.50, 1.0];
    const SKIN_DARK: Color = [0.62, 0.42, 0.32, 1.0];
    const HAIR: Color = [0.25, 0.16, 0.10, 1.0];
    const EYES: Color = [0.08, 0.08, 0.10, 1.0];
    const TUNIC: Color = [0.25, 0.34, 0.42, 1.0];
    const TRIM: Color = [0.65, 0.50, 0.24, 1.0];
    const BELT: Color = [0.20, 0.14, 0.09, 1.0];
    const PANTS: Color = [0.26, 0.27, 0.32, 1.0];
    const BOOTS: Color = [0.14, 0.10, 0.08, 1.0];
    const STEEL: Color = [0.55, 0.58, 0.62, 1.0];
    const CLOAK: Color = [0.18, 0.22, 0.28, 1.0];

    const Z_HOME: i32 = 4;
    // +1 is the left step: left foot toward -Z, right arm (the sword) toward -Z.
    let stride = match motion {
        Motion::StepLeft => 1,
        Motion::StepRight => -1,
        _ => 0,
    };
    let left_leg_swing = -stride;
    let right_leg_swing = stride;
    let left_arm_swing = stride;
    let right_arm_swing = -stride;

    let place = |m: &mut VoxelModel, min: [i32; 3], max: [i32; 3], dx: i32, dy: i32, dz: i32, color: Color, limb: u8| {
        let y0 = min[1] + dy;
        let y1 = max[1] + dy;
        if y1 <= 0 {
            return;
        }
        m.fill_part(
            [min[0] + dx + PLAYER_PAD_X, y0.max(0), min[2] + Z_HOME + dz + PLAYER_PAD_Z],
            [max[0] + dx + PLAYER_PAD_X, y1, max[2] + Z_HOME + dz + PLAYER_PAD_Z],
            color,
            limb,
        );
    };

    // Old body is 14×36×15. Pad x by 2 each side and z by 12 in front, and
    // leave room overhead for a raised sword. Anchor (9, 19.5) = old centre (7, 7.5) + pad.
    let mut m = VoxelModel::with_anchor(IVec3::new(18, 48, 32), [9.0, 19.5]);
    for (leg_x, side, swing, thigh, shin, foot) in [
        (3, -1, left_leg_swing, LIMB_THIGH_L, LIMB_SHIN_L, LIMB_FOOT_L),
        (8, 1, right_leg_swing, LIMB_THIGH_R, LIMB_SHIN_R, LIMB_FOOT_R),
    ] {
        place(&mut m, [leg_x, 9, 1], [leg_x + 3, 16, 5], side, 0, swing, PANTS, thigh);
        place(&mut m, [leg_x, 4, 1], [leg_x + 3, 10, 5], side * 2, 0, swing * 2, PANTS, shin);
        place(&mut m, [leg_x, 0, 1], [leg_x + 3, 4, 6], side * 2, 0, swing * 4, BOOTS, foot);
        place(&mut m, [leg_x - 1, 0, 0], [leg_x + 4, 2, 3], side * 2, 0, swing * 5, BOOTS, foot);
        place(&mut m, [leg_x, 10, 0], [leg_x + 3, 12, 1], side, 0, swing, STEEL, thigh);
    }

    place(&mut m, [3, 16, 0], [11, 18, 6], 0, 0, 0, BELT, LIMB_BODY);
    place(&mut m, [4, 17, 0], [10, 18, 1], 0, 0, 0, TRIM, LIMB_BODY);
    place(&mut m, [3, 18, 1], [11, 28, 6], 0, 0, 0, TUNIC, LIMB_BODY);
    place(&mut m, [2, 22, 2], [12, 28, 6], 0, 0, 0, TUNIC, LIMB_BODY);
    place(&mut m, [4, 18, 0], [10, 25, 1], 0, 0, 0, TRIM, LIMB_BODY);
    place(&mut m, [6, 17, 0], [8, 19, 1], 0, 0, 0, STEEL, LIMB_BODY);
    place(&mut m, [2, 18, 5], [12, 27, 7], 0, 0, 0, CLOAK, LIMB_BODY);

    // (dx, dy, dz) for upper, forearm, hand. Right side is the sword arm.
    let (left_arm, right_arm, sword) = match motion {
        Motion::Windup => (
            [(-1, 2, -3), (-2, 4, -6), (-2, 4, -7)],
            [(1, 6, 3), (2, 12, 5), (2, 16, 6)],
            (2, 16, 6, false, false),
        ),
        Motion::Strike => (
            [(-1, 0, 3), (-2, 0, 5), (-2, 1, 6)],
            [(1, 1, -3), (2, 2, -6), (2, 2, -8)],
            (2, 2, -8, true, false),
        ),
        Motion::Recover => (
            [(-1, 4, -1), (-2, 8, -2), (-2, 10, -2)],
            [(0, -1, -1), (-1, -3, -1), (-1, -4, -1)],
            (-1, -4, -1, false, true),
        ),
        Motion::Cast => (
            [(-1, 3, -2), (-2, 7, -5), (-2, 10, -6)],
            [(1, 3, -2), (2, 7, -5), (2, 10, -6)],
            (2, 10, -6, false, false),
        ),
        _ => (
            [(-1, 0, left_arm_swing), (-2, 0, left_arm_swing * 3), (-2, 0, left_arm_swing * 5)],
            [(1, 0, right_arm_swing), (2, 0, right_arm_swing * 3), (2, 0, right_arm_swing * 5)],
            (2, 0, right_arm_swing * 5, false, false),
        ),
    };

    // Rest boxes are the unshifted limb. Offsets above are the full dx/dy/dz,
    // including the outward step the walk used to bake in (side, side*2).
    {
        let mut paint_arm = |upper_x: i32, fore_x: i32, hand_x: i32, offsets: [(i32, i32, i32); 3], upper: u8, fore: u8| {
            let (ux, uy, uz) = offsets[0];
            let (fx, fy, fz) = offsets[1];
            let (hx, hy, hz) = offsets[2];
            place(&mut m, [upper_x, 20, 2], [upper_x + 2, 27, 5], ux, uy, uz, TUNIC, upper);
            place(&mut m, [upper_x, 26, 1], [upper_x + 2, 29, 5], ux, uy, uz, STEEL, upper);
            place(&mut m, [fore_x, 13, 2], [fore_x + 2, 20, 5], fx, fy, fz, SKIN, fore);
            place(&mut m, [hand_x, 10, 1], [hand_x + 2, 14, 5], hx, hy, hz, SKIN_DARK, fore);
            place(&mut m, [hand_x, 9, 1], [hand_x + 1, 10, 2], hx, hy, hz, SKIN, fore);
            place(&mut m, [hand_x + 1, 9, 3], [hand_x + 2, 10, 4], hx, hy, hz, SKIN, fore);
        };
        paint_arm(1, 0, 0, left_arm, LIMB_ARM_L, LIMB_FORE_L);
        paint_arm(11, 12, 12, right_arm, LIMB_ARM_R, LIMB_FORE_R);
    }

    let (sx, sy, sz, thrust, low) = sword;
    // Grip sits in the right hand. The hand's rest x is 12, plus the same dx the arm uses.
    place(&mut m, [12, 11, 0], [14, 15, 3], sx, sy, sz, SWORD_GRIP, LIMB_SWORD);
    place(&mut m, [11, 14, -1], [15, 16, 3], sx, sy, sz, SWORD_GUARD, LIMB_SWORD);
    if thrust {
        // Blade runs out through -Z, past the body front, on the strike frame.
        place(&mut m, [12, 12, -8], [14, 16, -2], sx, sy, sz, SWORD_BLADE, LIMB_SWORD);
        place(&mut m, [12, 12, -8], [14, 14, -6], sx, sy, sz, SWORD_EDGE, LIMB_SWORD);
    } else if low {
        place(&mut m, [4, 8, 0], [14, 11, 3], sx, sy, sz, SWORD_BLADE, LIMB_SWORD);
        place(&mut m, [4, 8, -1], [14, 9, 1], sx, sy, sz, SWORD_EDGE, LIMB_SWORD);
    } else if matches!(motion, Motion::Windup | Motion::Cast) {
        place(&mut m, [12, 16, 1], [14, 30, 4], sx, sy, sz, SWORD_BLADE, LIMB_SWORD);
        place(&mut m, [12, 26, 0], [14, 30, 2], sx, sy, sz, SWORD_EDGE, LIMB_SWORD);
    } else {
        // Guard and walk: blade up the right side of the chest, still under the 1.8 m crown.
        place(&mut m, [12, 16, -1], [14, 30, 2], sx, sy, sz, SWORD_BLADE, LIMB_SWORD);
        place(&mut m, [12, 16, -2], [14, 30, -1], sx, sy, sz, SWORD_EDGE, LIMB_SWORD);
    }

    // Neck, head, nose, hair cap, eyebrows and eyes.
    place(&mut m, [6, 28, 2], [8, 30, 5], 0, 0, 0, SKIN, LIMB_HEAD);
    place(&mut m, [4, 30, 1], [10, 36, 6], 0, 0, 0, SKIN, LIMB_HEAD);
    place(&mut m, [6, 32, 0], [8, 34, 2], 0, 0, 0, SKIN_DARK, LIMB_HEAD);
    place(&mut m, [4, 35, 1], [10, 36, 6], 0, 0, 0, HAIR, LIMB_HEAD);
    place(&mut m, [3, 33, 2], [5, 36, 6], 0, 0, 0, HAIR, LIMB_HEAD);
    place(&mut m, [9, 33, 2], [11, 36, 6], 0, 0, 0, HAIR, LIMB_HEAD);
    place(&mut m, [4, 31, 5], [10, 35, 7], 0, 0, 0, HAIR, LIMB_HEAD);
    place(&mut m, [4, 33, 0], [6, 34, 1], 0, 0, 0, EYES, LIMB_HEAD);
    place(&mut m, [8, 33, 0], [10, 34, 1], 0, 0, 0, EYES, LIMB_HEAD);
    place(&mut m, [4, 34, 0], [6, 35, 1], 0, 0, 0, HAIR, LIMB_HEAD);
    place(&mut m, [8, 34, 0], [10, 35, 1], 0, 0, 0, HAIR, LIMB_HEAD);
    m
}

/// Hunched horned demon. Idle keeps the cleavers low so the standing height stays 1.5 m.
pub fn fallen_demon(wounds: u8) -> VoxelModel {
    demon(wounds, Motion::Idle)
}

/// Same body as [`fallen_demon`], posed. Each remaining arm holds a cleaver.
/// With both arms gone, a strike leans the horns or throws the tail forward.
pub fn demon(wounds: u8, motion: Motion) -> VoxelModel {
    const SKIN: Color = [0.48, 0.06, 0.04, 1.0];
    const DARK: Color = [0.16, 0.02, 0.02, 1.0];
    const HORN: Color = [0.78, 0.68, 0.50, 1.0];
    const EYE: Color = [1.0, 0.75, 0.08, 1.0];
    const CLAW: Color = [0.86, 0.80, 0.66, 1.0];
    const CLOTH: Color = [0.22, 0.12, 0.07, 1.0];
    const SKIN_HI: Color = [0.68, 0.10, 0.07, 1.0];
    const WART: Color = [0.08, 0.22, 0.05, 1.0];

    // Both legs gone drops the body onto the ground so a crawler is not floating.
    let prone = wounds & DEMON_LEFT_LEG != 0 && wounds & DEMON_RIGHT_LEG != 0;
    let drop = if prone { 4 } else { 0 };
    let arms_on = wounds & DEMON_LEFT_ARM == 0 || wounds & DEMON_RIGHT_ARM == 0;
    let horns_on = wounds & DEMON_HEAD == 0 && wounds & DEMON_HORNS == 0;
    let tail_on = wounds & DEMON_TAIL == 0;
    // Horns outrank the tail, matching the stance ladder.
    let gore = !arms_on && horns_on;
    let whip = !arms_on && tail_on && !gore;

    let (left_leg_dz, right_leg_dz) = match motion {
        Motion::StepLeft => (-2, 2),
        Motion::StepRight => (2, -2),
        Motion::Strike => (-2, 0),
        Motion::Windup => (1, 1),
        _ => (0, 0),
    };
    // (dy, dz) for upper arm, forearm, hand. StepLeft swings the right cleaver forward.
    let (left_arm, right_arm) = match motion {
        Motion::StepLeft => ([(0, 2), (0, 2), (0, 2)], [(0, -2), (0, -2), (0, -2)]),
        Motion::StepRight => ([(0, -2), (0, -2), (0, -2)], [(0, 2), (0, 2), (0, 2)]),
        Motion::Windup => ([(3, 2), (8, 3), (12, 3)], [(3, 2), (8, 3), (12, 3)]),
        Motion::Strike => ([(1, -2), (2, -4), (2, -4)], [(1, -2), (2, -4), (2, -4)]),
        Motion::Recover => ([(-1, 1), (-2, 1), (-3, 0)], [(-1, 1), (-2, 1), (-3, 0)]),
        _ => ([(0, 0); 3], [(0, 0); 3]),
    };
    let (head_dy, head_dz) = if gore {
        match motion {
            Motion::Windup => (1, 2),
            Motion::Strike => (0, -4),
            Motion::Recover => (0, 1),
            _ => (0, 0),
        }
    } else {
        (0, 0)
    };
    let (tail_base, tail_mid, tail_tip) = if whip {
        match motion {
            Motion::Windup => (1, 2, 3),
            Motion::Strike => (-2, -7, -12),
            Motion::Recover => (0, 1, 2),
            _ => (0, 0, 0),
        }
    } else {
        match motion {
            Motion::StepLeft => (0, 1, 1),
            Motion::StepRight => (0, -1, -1),
            _ => (0, 0, 0),
        }
    };

    // Old body is 22×30×14, centre (11, 7). Pad keeps that body on the combat boxes.
    let mut m = VoxelModel::with_anchor(IVec3::new(26, 42, 32), [13.0, 17.0]);
    let put = |m: &mut VoxelModel, min: [i32; 3], max: [i32; 3], dy: i32, dz: i32, color: Color, limb: u8| {
        stamp(m, min, max, 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, color, limb);
    };

    // Thigh plus one lower bone (hoof and claws). A separate ankle fights the short shin.
    for (leg_x, flag, dz, thigh, shin) in [
        (6, DEMON_LEFT_LEG, left_leg_dz, LIMB_THIGH_L, LIMB_SHIN_L),
        (12, DEMON_RIGHT_LEG, right_leg_dz, LIMB_THIGH_R, LIMB_SHIN_R),
    ] {
        if wounds & flag != 0 {
            continue;
        }
        put(&mut m, [leg_x, 0, 4], [leg_x + 4, 4, 10], 0, dz, DARK, shin);
        put(&mut m, [leg_x + 1, 4, 4], [leg_x + 4, 12, 8], 0, dz, SKIN, thigh);
        put(&mut m, [leg_x - 2, 0, 2], [leg_x + 4, 2, 6], 0, dz, CLAW, shin);
        put(&mut m, [leg_x - 2, 0, 1], [leg_x - 1, 1, 3], 0, dz, CLAW, shin);
        put(&mut m, [leg_x + 1, 0, 1], [leg_x + 2, 1, 3], 0, dz, CLAW, shin);
    }

    put(&mut m, [6, 10, 4], [16, 16, 10], 0, 0, CLOTH, LIMB_BODY);
    put(&mut m, [4, 15, 2], [18, 24, 12], 0, 0, SKIN, LIMB_BODY);
    put(&mut m, [6, 24, 4], [16, 26, 10], 0, 0, DARK, LIMB_BODY);
    put(&mut m, [7, 17, 1], [15, 19, 3], 0, 0, SKIN_HI, LIMB_BODY);
    put(&mut m, [8, 11, 2], [14, 13, 4], 0, 0, HORN, LIMB_BODY);
    put(&mut m, [5, 22, 3], [7, 23, 5], 0, 0, WART, LIMB_BODY);
    put(&mut m, [15, 20, 9], [17, 21, 11], 0, 0, WART, LIMB_BODY);

    let paint_cleaver = |m: &mut VoxelModel, hand_x: i32, dy: i32, dz: i32, limb: u8| {
        stamp(m, [hand_x + 1, 5, 0], [hand_x + 3, 8, 3], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_GRIP, limb);
        match motion {
            Motion::Windup => {
                stamp(m, [hand_x, 10, 1], [hand_x + 4, 26, 5], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_BLADE, limb);
                stamp(m, [hand_x, 22, 0], [hand_x + 4, 26, 2], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_EDGE, limb);
            }
            Motion::Strike => {
                stamp(m, [hand_x, 6, -6], [hand_x + 4, 11, -1], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_BLADE, limb);
                stamp(m, [hand_x, 6, -6], [hand_x + 4, 8, -4], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_EDGE, limb);
            }
            Motion::Recover => {
                stamp(m, [hand_x, 2, -1], [hand_x + 4, 5, 4], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_BLADE, limb);
            }
            _ => {
                stamp(m, [hand_x, 3, -5], [hand_x + 4, 11, 1], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_BLADE, limb);
                stamp(m, [hand_x, 7, -6], [hand_x + 4, 10, -4], 0, dy, dz, drop, DEMON_PAD_X, DEMON_PAD_Z, CLEAVER_EDGE, limb);
            }
        }
    };

    if wounds & DEMON_LEFT_ARM == 0 {
        let (uy, uz) = left_arm[0];
        let (fy, fz) = left_arm[1];
        let (hy, hz) = left_arm[2];
        put(&mut m, [2, 12, 4], [6, 22, 10], uy, uz, SKIN, LIMB_ARM_L);
        put(&mut m, [0, 7, 3], [4, 14, 8], fy, fz, SKIN, LIMB_FORE_L);
        put(&mut m, [0, 5, 1], [4, 7, 8], hy, hz, CLAW, LIMB_FORE_L);
        put(&mut m, [0, 4, 1], [1, 5, 3], hy, hz, CLAW, LIMB_FORE_L);
        put(&mut m, [2, 4, 3], [3, 5, 5], hy, hz, CLAW, LIMB_FORE_L);
        paint_cleaver(&mut m, 0, hy, hz, LIMB_CLEAVER_L);
    }
    if wounds & DEMON_RIGHT_ARM == 0 {
        let (uy, uz) = right_arm[0];
        let (fy, fz) = right_arm[1];
        let (hy, hz) = right_arm[2];
        put(&mut m, [16, 12, 4], [20, 22, 10], uy, uz, SKIN, LIMB_ARM_R);
        put(&mut m, [18, 7, 3], [22, 14, 8], fy, fz, SKIN, LIMB_FORE_R);
        put(&mut m, [18, 5, 1], [22, 7, 8], hy, hz, CLAW, LIMB_FORE_R);
        put(&mut m, [19, 4, 1], [20, 5, 3], hy, hz, CLAW, LIMB_FORE_R);
        put(&mut m, [21, 4, 3], [22, 5, 5], hy, hz, CLAW, LIMB_FORE_R);
        paint_cleaver(&mut m, 18, hy, hz, LIMB_CLEAVER_R);
    }

    if wounds & DEMON_HEAD == 0 {
        put(&mut m, [6, 22, 2], [16, 30, 12], head_dy, head_dz, SKIN, LIMB_HEAD);
        put(&mut m, [6, 26, 2], [16, 30, 6], head_dy, head_dz, DARK, LIMB_HEAD);
        put(&mut m, [6, 24, 0], [16, 28, 4], head_dy, head_dz, SKIN_HI, LIMB_HEAD);
        put(&mut m, [8, 26, 0], [10, 28, 1], head_dy, head_dz, EYE, LIMB_HEAD);
        put(&mut m, [12, 26, 0], [14, 28, 1], head_dy, head_dz, EYE, LIMB_HEAD);
        put(&mut m, [9, 22, 0], [10, 24, 2], head_dy, head_dz, CLAW, LIMB_HEAD);
        put(&mut m, [12, 22, 0], [13, 24, 2], head_dy, head_dz, CLAW, LIMB_HEAD);
        put(&mut m, [10, 24, 0], [12, 26, 2], head_dy, head_dz, DARK, LIMB_HEAD);
        if wounds & DEMON_HORNS == 0 {
            let horn_dz = head_dz + if gore && motion == Motion::Strike { -2 } else { 0 };
            put(&mut m, [2, 26, 4], [6, 30, 8], head_dy, horn_dz, HORN, LIMB_HORN);
            put(&mut m, [16, 26, 4], [20, 30, 8], head_dy, horn_dz, HORN, LIMB_HORN);
            put(&mut m, [0, 28, 6], [4, 30, 10], head_dy, horn_dz, HORN, LIMB_HORN);
            put(&mut m, [18, 28, 6], [22, 30, 10], head_dy, horn_dz, HORN, LIMB_HORN);
            put(&mut m, [0, 29, 9], [2, 30, 12], head_dy, horn_dz, HORN, LIMB_HORN);
            put(&mut m, [20, 29, 9], [22, 30, 12], head_dy, horn_dz, HORN, LIMB_HORN);
        }
    }

    if wounds & DEMON_TAIL == 0 {
        put(&mut m, [10, 9, 10], [12, 16, 14], 0, tail_base, SKIN, LIMB_TAIL);
        put(&mut m, [10, 7, 12], [12, 10, 14], 0, tail_mid, SKIN, LIMB_TAIL);
        put(&mut m, [10, 6, 13], [12, 7, 14], 0, tail_tip, CLAW, LIMB_TAIL);
    }

    m
}

/// Bone warrior on the demon canvas. Sword in the right hand, shield on the left.
/// The shield is the left-hand weapon mesh, so it leaves with that arm.
pub fn skeleton(wounds: u8) -> VoxelModel {
    const BONE: Color = [0.86, 0.82, 0.70, 1.0];
    const BONE_DARK: Color = [0.40, 0.36, 0.30, 1.0];
    const SOCKET: Color = [0.05, 0.04, 0.04, 1.0];
    const WOOD: Color = [0.40, 0.24, 0.11, 1.0];
    const RIM: Color = [0.52, 0.55, 0.58, 1.0];
    const RUST: Color = [0.48, 0.22, 0.10, 1.0];

    let prone = wounds & DEMON_LEFT_LEG != 0 && wounds & DEMON_RIGHT_LEG != 0;
    let drop = if prone { 4 } else { 0 };
    let mut m = VoxelModel::with_anchor(IVec3::new(26, 42, 32), [13.0, 17.0]);
    let put = |m: &mut VoxelModel, min: [i32; 3], max: [i32; 3], color: Color, limb: u8| {
        stamp(m, min, max, 0, 0, 0, drop, DEMON_PAD_X, DEMON_PAD_Z, color, limb);
    };

    for (leg_x, flag, thigh, shin, foot) in [
        (6, DEMON_LEFT_LEG, LIMB_THIGH_L, LIMB_SHIN_L, LIMB_FOOT_L),
        (12, DEMON_RIGHT_LEG, LIMB_THIGH_R, LIMB_SHIN_R, LIMB_FOOT_R),
    ] {
        if wounds & flag != 0 {
            continue;
        }
        put(&mut m, [leg_x, 9, 3], [leg_x + 4, 16, 7], BONE, thigh);
        put(&mut m, [leg_x, 3, 3], [leg_x + 3, 9, 6], BONE, shin);
        put(&mut m, [leg_x - 1, 0, 2], [leg_x + 4, 3, 8], BONE_DARK, foot);
    }

    put(&mut m, [7, 15, 3], [15, 18, 8], BONE, LIMB_BODY);
    put(&mut m, [8, 18, 4], [14, 26, 8], BONE, LIMB_BODY);
    put(&mut m, [10, 18, 6], [12, 26, 8], BONE_DARK, LIMB_BODY);
    for rib in [19, 21, 23] {
        put(&mut m, [8, rib, 3], [14, rib + 1, 5], BONE_DARK, LIMB_BODY);
    }

    if wounds & DEMON_HEAD == 0 {
        put(&mut m, [8, 26, 3], [14, 34, 9], BONE, LIMB_HEAD);
        put(&mut m, [9, 26, 2], [13, 28, 4], BONE, LIMB_HEAD);
        put(&mut m, [8, 29, 2], [10, 31, 4], SOCKET, LIMB_HEAD);
        put(&mut m, [12, 29, 2], [14, 31, 4], SOCKET, LIMB_HEAD);
        put(&mut m, [10, 27, 2], [12, 29, 3], SOCKET, LIMB_HEAD);
        put(&mut m, [9, 33, 3], [13, 34, 8], BONE_DARK, LIMB_HEAD);
    }

    if wounds & DEMON_LEFT_ARM == 0 {
        put(&mut m, [3, 18, 3], [7, 26, 7], BONE, LIMB_ARM_L);
        put(&mut m, [2, 12, 3], [6, 19, 7], BONE, LIMB_FORE_L);
        put(&mut m, [2, 14, 2], [6, 17, 5], CLEAVER_GRIP, LIMB_CLEAVER_L);
        put(&mut m, [0, 10, 1], [8, 24, 3], WOOD, LIMB_CLEAVER_L);
        put(&mut m, [0, 10, 0], [8, 24, 1], RIM, LIMB_CLEAVER_L);
        put(&mut m, [3, 16, 0], [5, 19, 1], RUST, LIMB_CLEAVER_L);
    }
    if wounds & DEMON_RIGHT_ARM == 0 {
        put(&mut m, [15, 18, 3], [19, 26, 7], BONE, LIMB_ARM_R);
        put(&mut m, [16, 12, 3], [20, 19, 7], BONE, LIMB_FORE_R);
        put(&mut m, [17, 13, 2], [19, 16, 5], SWORD_GRIP, LIMB_SWORD);
        put(&mut m, [16, 15, 1], [20, 17, 6], SWORD_GUARD, LIMB_SWORD);
        put(&mut m, [17, 16, 1], [19, 33, 4], SWORD_BLADE, LIMB_SWORD);
        put(&mut m, [17, 28, 0], [19, 33, 2], SWORD_EDGE, LIMB_SWORD);
    }
    m
}

/// Bloated shambler. No weapon: the arms slam, and the jaw bites once they are gone.
pub fn zombie(wounds: u8) -> VoxelModel {
    const ROT: Color = [0.42, 0.52, 0.28, 1.0];
    const ROT_DARK: Color = [0.24, 0.30, 0.14, 1.0];
    const WOUND: Color = [0.48, 0.10, 0.08, 1.0];
    const SHROUD: Color = [0.50, 0.46, 0.34, 1.0];
    const EYE: Color = [0.95, 0.18, 0.08, 1.0];
    const TEETH: Color = [0.86, 0.82, 0.68, 1.0];

    let prone = wounds & DEMON_LEFT_LEG != 0 && wounds & DEMON_RIGHT_LEG != 0;
    let drop = if prone { 4 } else { 0 };
    let mut m = VoxelModel::with_anchor(IVec3::new(26, 42, 32), [13.0, 17.0]);
    let put = |m: &mut VoxelModel, min: [i32; 3], max: [i32; 3], color: Color, limb: u8| {
        stamp(m, min, max, 0, 0, 0, drop, DEMON_PAD_X, DEMON_PAD_Z, color, limb);
    };

    for (leg_x, flag, thigh, shin, foot) in [
        (5, DEMON_LEFT_LEG, LIMB_THIGH_L, LIMB_SHIN_L, LIMB_FOOT_L),
        (12, DEMON_RIGHT_LEG, LIMB_THIGH_R, LIMB_SHIN_R, LIMB_FOOT_R),
    ] {
        if wounds & flag != 0 {
            continue;
        }
        put(&mut m, [leg_x, 8, 4], [leg_x + 5, 15, 9], ROT_DARK, thigh);
        put(&mut m, [leg_x + 1, 3, 4], [leg_x + 4, 9, 8], ROT, shin);
        put(&mut m, [leg_x, 0, 3], [leg_x + 5, 3, 9], ROT_DARK, foot);
    }

    put(&mut m, [5, 14, 3], [17, 24, 12], ROT, LIMB_BODY);
    put(&mut m, [4, 16, 6], [18, 23, 13], SHROUD, LIMB_BODY);
    put(&mut m, [8, 18, 2], [12, 21, 4], WOUND, LIMB_BODY);
    put(&mut m, [13, 15, 4], [15, 17, 6], WOUND, LIMB_BODY);

    if wounds & DEMON_HEAD == 0 {
        put(&mut m, [7, 22, 1], [15, 30, 8], ROT, LIMB_HEAD);
        put(&mut m, [8, 22, 0], [14, 25, 3], ROT_DARK, LIMB_HEAD);
        put(&mut m, [9, 22, 0], [13, 24, 2], TEETH, LIMB_HEAD);
        put(&mut m, [8, 26, 0], [10, 28, 2], EYE, LIMB_HEAD);
        put(&mut m, [12, 26, 0], [14, 28, 2], EYE, LIMB_HEAD);
        put(&mut m, [10, 28, 1], [12, 30, 4], ROT_DARK, LIMB_HEAD);
    }

    if wounds & DEMON_LEFT_ARM == 0 {
        put(&mut m, [1, 12, 3], [6, 22, 8], ROT, LIMB_ARM_L);
        put(&mut m, [0, 4, 2], [5, 13, 7], ROT_DARK, LIMB_FORE_L);
        put(&mut m, [0, 4, 1], [2, 6, 3], TEETH, LIMB_FORE_L);
        put(&mut m, [3, 4, 1], [5, 6, 3], TEETH, LIMB_FORE_L);
    }
    if wounds & DEMON_RIGHT_ARM == 0 {
        put(&mut m, [16, 12, 3], [21, 22, 8], ROT, LIMB_ARM_R);
        put(&mut m, [17, 4, 2], [22, 13, 7], ROT_DARK, LIMB_FORE_R);
        put(&mut m, [17, 4, 1], [19, 6, 3], TEETH, LIMB_FORE_R);
        put(&mut m, [20, 4, 1], [22, 6, 3], TEETH, LIMB_FORE_R);
    }
    m
}

/// Hooded archer. Both arms hold the bow. One arm left, and that hand keeps a dagger instead.
pub fn corrupt_rogue(wounds: u8) -> VoxelModel {
    const LEATHER: Color = [0.28, 0.16, 0.09, 1.0];
    const HOOD: Color = [0.12, 0.16, 0.11, 1.0];
    const CLOTH: Color = [0.22, 0.30, 0.18, 1.0];
    const SKIN: Color = [0.72, 0.54, 0.40, 1.0];
    const BOOT: Color = [0.10, 0.08, 0.06, 1.0];
    const BOW: Color = [0.46, 0.28, 0.12, 1.0];
    const STRING: Color = [0.75, 0.72, 0.62, 1.0];
    const EYE: Color = [0.85, 0.72, 0.30, 1.0];

    let both_arms = wounds & DEMON_LEFT_ARM == 0 && wounds & DEMON_RIGHT_ARM == 0;
    let prone = wounds & DEMON_LEFT_LEG != 0 && wounds & DEMON_RIGHT_LEG != 0;
    let drop = if prone { 4 } else { 0 };
    let mut m = VoxelModel::with_anchor(IVec3::new(26, 42, 32), [13.0, 17.0]);
    let put = |m: &mut VoxelModel, min: [i32; 3], max: [i32; 3], color: Color, limb: u8| {
        stamp(m, min, max, 0, 0, 0, drop, DEMON_PAD_X, DEMON_PAD_Z, color, limb);
    };

    for (leg_x, flag, thigh, shin, foot) in [
        (6, DEMON_LEFT_LEG, LIMB_THIGH_L, LIMB_SHIN_L, LIMB_FOOT_L),
        (12, DEMON_RIGHT_LEG, LIMB_THIGH_R, LIMB_SHIN_R, LIMB_FOOT_R),
    ] {
        if wounds & flag != 0 {
            continue;
        }
        put(&mut m, [leg_x, 9, 3], [leg_x + 4, 16, 7], CLOTH, thigh);
        put(&mut m, [leg_x, 3, 3], [leg_x + 3, 9, 6], CLOTH, shin);
        put(&mut m, [leg_x - 1, 0, 2], [leg_x + 4, 3, 7], BOOT, foot);
    }

    put(&mut m, [6, 15, 3], [16, 26, 8], LEATHER, LIMB_BODY);
    put(&mut m, [7, 16, 2], [15, 22, 4], CLOTH, LIMB_BODY);
    put(&mut m, [8, 18, 8], [14, 28, 11], LEATHER, LIMB_BODY);
    put(&mut m, [9, 20, 10], [13, 27, 12], BOW, LIMB_BODY);

    if wounds & DEMON_HEAD == 0 {
        put(&mut m, [7, 26, 2], [15, 34, 9], HOOD, LIMB_HEAD);
        put(&mut m, [8, 27, 2], [14, 32, 5], SKIN, LIMB_HEAD);
        put(&mut m, [8, 29, 1], [10, 31, 3], EYE, LIMB_HEAD);
        put(&mut m, [12, 29, 1], [14, 31, 3], EYE, LIMB_HEAD);
        put(&mut m, [6, 30, 3], [8, 34, 9], HOOD, LIMB_HEAD);
        put(&mut m, [14, 30, 3], [16, 34, 9], HOOD, LIMB_HEAD);
    }

    if wounds & DEMON_LEFT_ARM == 0 {
        put(&mut m, [3, 18, 3], [7, 26, 7], LEATHER, LIMB_ARM_L);
        put(&mut m, [2, 12, 3], [6, 19, 7], SKIN, LIMB_FORE_L);
        if both_arms {
            put(&mut m, [3, 13, 1], [6, 16, 4], CLEAVER_GRIP, LIMB_CLEAVER_L);
            put(&mut m, [3, 8, 2], [5, 13, 4], BOW, LIMB_CLEAVER_L);
            put(&mut m, [3, 16, 2], [5, 32, 4], BOW, LIMB_CLEAVER_L);
            put(&mut m, [4, 9, 0], [5, 30, 1], STRING, LIMB_CLEAVER_L);
        } else {
            put(&mut m, [2, 12, 0], [4, 14, 3], SWORD_BLADE, LIMB_FORE_L);
            put(&mut m, [2, 12, -2], [4, 13, 1], SWORD_EDGE, LIMB_FORE_L);
        }
    }
    if wounds & DEMON_RIGHT_ARM == 0 {
        put(&mut m, [15, 18, 3], [19, 26, 7], LEATHER, LIMB_ARM_R);
        put(&mut m, [16, 12, 3], [20, 19, 7], SKIN, LIMB_FORE_R);
        if !both_arms {
            put(&mut m, [17, 12, 1], [19, 15, 4], SWORD_GRIP, LIMB_SWORD);
            put(&mut m, [17, 13, -4], [19, 15, 2], SWORD_BLADE, LIMB_SWORD);
            put(&mut m, [17, 13, -5], [19, 14, -2], SWORD_EDGE, LIMB_SWORD);
        }
    }
    m
}

/// Flat 0.5 m ring shown at the click-to-move destination.
pub fn destination_marker() -> VoxelModel {
    const GOLD: Color = [1.0, 0.8, 0.25, 1.0];
    let mut m = VoxelModel::new(IVec3::new(5, 1, 5));
    m.fill([1, 0, 0], [4, 1, 1], GOLD);
    m.fill([1, 0, 4], [4, 1, 5], GOLD);
    m.fill([0, 0, 1], [1, 1, 4], GOLD);
    m.fill([4, 0, 1], [5, 1, 4], GOLD);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adventurer_is_1_8_metres_tall_and_standing_on_origin() {
        let mesh = adventurer().mesh(0.05);
        let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(min_y.abs() < 1e-5);
        assert!((max_y - 1.8).abs() < 1e-5, "height {max_y}");
    }

    #[test]
    fn fallen_demon_is_readable_target_size() {
        let mesh = fallen_demon(0).mesh(0.05);
        let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(min_y.abs() < 1e-5);
        assert!((1.4..=1.5).contains(&max_y), "height {max_y}");
    }

    #[test]
    fn lost_limbs_change_the_silhouette() {
        let intact = fallen_demon(0).mesh(0.05).positions.len();
        assert!(fallen_demon(DEMON_LEFT_ARM).mesh(0.05).positions.len() < intact);
        assert!(fallen_demon(DEMON_HEAD).mesh(0.05).positions.len() < intact);
        let standing = fallen_demon(0).mesh(0.05);
        let prone = fallen_demon(DEMON_LEFT_LEG | DEMON_RIGHT_LEG).mesh(0.05);
        let max_y = |mesh: &MeshArrays| mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
        assert!(max_y(&prone) < max_y(&standing) - 0.15);
    }

    #[test]
    fn the_other_bodies_stand_and_lose_the_weapon_with_the_arm() {
        let standing = |model: VoxelModel, lo: f32, hi: f32| {
            let mesh = model.mesh(0.05);
            let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            assert!(min_y.abs() < 1e-4, "min {min_y}");
            assert!((lo..=hi).contains(&max_y), "height {max_y}");
        };
        standing(skeleton(0), 1.6, 1.8);
        standing(zombie(0), 1.4, 1.6);
        standing(corrupt_rogue(0), 1.6, 1.8);

        assert!(!skeleton(0).mesh_part(0.05, LIMB_SWORD).positions.is_empty());
        assert!(skeleton(DEMON_RIGHT_ARM).mesh_part(0.05, LIMB_SWORD).positions.is_empty());
        assert!(!skeleton(0).mesh_part(0.05, LIMB_CLEAVER_L).positions.is_empty());
        assert!(skeleton(DEMON_LEFT_ARM).mesh_part(0.05, LIMB_CLEAVER_L).positions.is_empty());
        assert!(skeleton(0).mesh_part(0.05, LIMB_TAIL).positions.is_empty());

        assert!(zombie(0).mesh_part(0.05, LIMB_SWORD).positions.is_empty());
        assert!(zombie(DEMON_LEFT_ARM | DEMON_RIGHT_ARM).mesh_part(0.05, LIMB_ARM_L).positions.is_empty());
        assert!(!zombie(DEMON_LEFT_ARM | DEMON_RIGHT_ARM).mesh_part(0.05, LIMB_HEAD).positions.is_empty());

        let rogue = corrupt_rogue(0);
        assert!(!rogue.mesh_part(0.05, LIMB_CLEAVER_L).positions.is_empty());
        assert!(rogue.mesh_part(0.05, LIMB_SWORD).positions.is_empty());
        let one_arm = corrupt_rogue(DEMON_LEFT_ARM);
        assert!(one_arm.mesh_part(0.05, LIMB_CLEAVER_L).positions.is_empty());
        assert!(!one_arm.mesh_part(0.05, LIMB_SWORD).positions.is_empty());
    }

    #[test]
    fn faces_wind_counter_clockwise_from_outside() {
        let mesh = adventurer().mesh(0.05);
        for tri in mesh.indices.chunks_exact(3) {
            let p = |i: u32| Vec3::from(mesh.positions[i as usize]);
            let n = Vec3::from(mesh.normals[tri[0] as usize]);
            let cross = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
            assert!(cross.dot(n) > 0.0);
        }
    }

    #[test]
    fn hidden_faces_are_culled() {
        let mut solid = VoxelModel::new(IVec3::splat(2));
        solid.fill([0, 0, 0], [2, 2, 2], [1.0; 4]);
        assert_eq!(solid.mesh(1.0).indices.len(), 6 * 4 * 6);
    }

    #[test]
    fn scaling_preserves_world_size_when_voxel_size_halves() {
        let lo = adventurer().mesh(0.05);
        let hi = adventurer().scaled(2).mesh(0.025);
        let height = |mesh: &MeshArrays| {
            let min_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
            let max_y = mesh.positions.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
            max_y - min_y
        };
        assert!((height(&lo) - height(&hi)).abs() < 1e-5);
        assert!(hi.positions.len() > lo.positions.len());
    }

    fn centroid(mesh: &MeshArrays, color: Color) -> Vec3 {
        let mut sum = Vec3::ZERO;
        let mut n = 0.0;
        for (position, vertex_color) in mesh.positions.iter().zip(mesh.colors.iter()) {
            if *vertex_color == color {
                sum += Vec3::from(*position);
                n += 1.0;
            }
        }
        assert!(n > 0.0, "no voxels of that colour");
        sum / n
    }

    #[test]
    fn the_sword_and_cleavers_are_in_their_hands() {
        let hero = adventurer_motion(Motion::Idle).mesh(0.05);
        let sword = centroid(&hero, SWORD_BLADE);
        assert!(sword.y > 0.7 && sword.y < 1.7, "guard blade {sword}");
        assert!(sword.x > 0.15, "sword should sit in the right hand, got {sword}");

        let brute = demon(0, Motion::Idle).mesh(0.05);
        assert!(brute.colors.iter().any(|color| *color == CLEAVER_BLADE));
        let left = brute.positions.iter().zip(brute.colors.iter()).any(|(p, c)| *c == CLEAVER_BLADE && p[0] < -0.2);
        let right = brute.positions.iter().zip(brute.colors.iter()).any(|(p, c)| *c == CLEAVER_BLADE && p[0] > 0.2);
        assert!(left && right, "each hand should hold a cleaver");

        let one_arm = demon(DEMON_LEFT_ARM, Motion::Idle).mesh(0.05);
        let cleaver_on_left = one_arm.positions.iter().zip(one_arm.colors.iter()).any(|(p, c)| *c == CLEAVER_BLADE && p[0] < -0.2);
        assert!(!cleaver_on_left);
    }

    #[test]
    fn the_strike_carries_the_weapon_forward_and_the_windup_raises_it() {
        let guard = centroid(&adventurer_motion(Motion::Idle).mesh(0.05), SWORD_BLADE);
        let windup = centroid(&adventurer_motion(Motion::Windup).mesh(0.05), SWORD_BLADE);
        let strike = centroid(&adventurer_motion(Motion::Strike).mesh(0.05), SWORD_BLADE);
        assert!(windup.y > guard.y + 0.15, "windup {windup} guard {guard}");
        assert!(strike.z < guard.z - 0.15, "strike {strike} guard {guard}");

        let demon_guard = centroid(&demon(0, Motion::Idle).mesh(0.05), CLEAVER_BLADE);
        let demon_windup = centroid(&demon(0, Motion::Windup).mesh(0.05), CLEAVER_BLADE);
        let demon_strike = centroid(&demon(0, Motion::Strike).mesh(0.05), CLEAVER_BLADE);
        assert!(demon_windup.y > demon_guard.y + 0.1, "cleaver windup {demon_windup} guard {demon_guard}");
        assert!(demon_strike.z < demon_guard.z - 0.1, "cleaver strike {demon_strike} guard {demon_guard}");
    }

    #[test]
    fn a_left_step_puts_the_left_foot_forward() {
        let foot_z = |motion: Motion| {
            let mesh = adventurer_motion(motion).mesh(0.05);
            let mut sum = 0.0;
            let mut n = 0.0;
            for position in &mesh.positions {
                if position[1] < 0.08 && position[0] < -0.05 {
                    sum += position[2];
                    n += 1.0;
                }
            }
            assert!(n > 0.0);
            sum / n
        };
        let left = foot_z(Motion::StepLeft);
        let right = foot_z(Motion::StepRight);
        assert!(right - left > 0.02, "left step {left} should be more -Z than right step {right}");
    }
}
