//! Rigid voxel bodies against the static voxel grid: gravity, contact impulses with friction,
//! sleeping, and writing a resting body back onto the grid. Units are metres.

use crate::player::{GRAVITY, VOXEL_SIZE};
use bevy::math::{IVec3, Mat3, Quat, Vec3};
use std::collections::{HashMap, HashSet};

const RESTITUTION: f32 = 0.15;
/// Impacts slower than this (m/s) don't bounce at all.
const BOUNCE_THRESHOLD: f32 = 1.0;
const FRICTION: f32 = 0.6;
const SOLVER_ITERATIONS: usize = 8;
/// Fraction of penetration removed per substep.
const POSITION_CORRECTION: f32 = 0.8;
const LINEAR_DAMPING: f32 = 0.05;
const ANGULAR_DAMPING: f32 = 0.5;
const SLEEP_LINEAR: f32 = 0.15;
const SLEEP_ANGULAR: f32 = 0.3;
const SLEEP_TIME: f32 = 0.35;
/// Large pieces collide using an even sample of their surface voxels.
const MAX_CONTACT_POINTS: usize = 6000;
const MAX_SUBSTEPS: u32 = 16;
/// Bodies closer than this to an axis-aligned orientation are squared up when written back.
const SNAP_ANGLE: f32 = 25.0 * std::f32::consts::PI / 180.0;
const EPS: f32 = 1e-4;

const NEIGHBOURS: [IVec3; 6] = [IVec3::X, IVec3::NEG_X, IVec3::Y, IVec3::NEG_Y, IVec3::Z, IVec3::NEG_Z];

/// Mass properties and collision points of a voxel body. Voxels are local grid indices;
/// voxel `v` spans `v * VOXEL_SIZE .. (v + 1) * VOXEL_SIZE` in local space.
pub struct RigidShape {
    pub voxels: Vec<IVec3>,
    /// Centre of mass in local metres.
    pub com: Vec3,
    /// Surface voxel centres relative to the centre of mass.
    points: Vec<Vec3>,
    inv_mass: f32,
    inv_inertia: Mat3,
    radius: f32,
    occupied: HashSet<IVec3>,
}

impl RigidShape {
    pub fn new(voxels: Vec<IVec3>) -> Self {
        let centre = |v: &IVec3| (v.as_vec3() + 0.5) * VOXEL_SIZE;
        let mass = voxels.len() as f32;
        let com = voxels.iter().map(centre).sum::<Vec3>() / mass;

        // Sum of unit-mass cubes: point-mass term plus each cube's own inertia.
        let cube = VOXEL_SIZE * VOXEL_SIZE / 6.0;
        let mut inertia = Mat3::ZERO;
        for v in &voxels {
            let r = centre(v) - com;
            inertia += Mat3::from_diagonal(Vec3::splat(r.length_squared() + cube)) - outer(r, r);
        }

        let occupied: HashSet<IVec3> = voxels.iter().copied().collect();
        let surface: Vec<Vec3> = voxels
            .iter()
            .filter(|v| NEIGHBOURS.iter().any(|d| !occupied.contains(&(**v + *d))))
            .map(|v| centre(v) - com)
            .collect();
        let stride = surface.len().div_ceil(MAX_CONTACT_POINTS).max(1);
        let points: Vec<Vec3> = surface.into_iter().step_by(stride).collect();
        let radius = voxels.iter().map(|v| (centre(v) - com).length()).fold(0.0, f32::max) + VOXEL_SIZE;

        Self { voxels, com, points, inv_mass: 1.0 / mass, inv_inertia: inertia.inverse(), radius, occupied }
    }

    fn world_inv_inertia(&self, rot: Quat) -> Mat3 {
        let r = Mat3::from_quat(rot);
        r * self.inv_inertia * r.transpose()
    }

    pub fn nearest_surface_point(&self, state: &RigidState, point: Vec3) -> Vec3 {
        self.points
            .iter()
            .map(|q| state.pos + state.rot * *q)
            .min_by(|a, b| a.distance_squared(point).total_cmp(&b.distance_squared(point)))
            .unwrap_or(state.pos)
    }

    pub fn radius(&self) -> f32 {
        self.radius
    }

    pub fn segment_hit(&self, state: &RigidState, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        let delta = to - from;
        let len2 = delta.length_squared();
        if len2 <= f32::EPSILON {
            return None;
        }
        if distance_point_segment_squared(state.pos, from, to) > (self.radius + VOXEL_SIZE).powi(2) {
            return None;
        }

        let sample_radius = VOXEL_SIZE * 0.75;
        let mut best = None;
        for q in &self.points {
            let point = state.pos + state.rot * *q;
            let t = (point - from).dot(delta) / len2;
            if !(0.0..=1.0).contains(&t) {
                continue;
            }
            let closest = from + delta * t;
            if point.distance_squared(closest) <= sample_radius * sample_radius
                && best.map_or(true, |(best_t, _)| t < best_t)
            {
                best = Some((t, closest));
            }
        }
        best
    }
}

fn distance_point_segment_squared(point: Vec3, from: Vec3, to: Vec3) -> f32 {
    let delta = to - from;
    let len2 = delta.length_squared();
    if len2 <= f32::EPSILON {
        return point.distance_squared(from);
    }
    let t = ((point - from).dot(delta) / len2).clamp(0.0, 1.0);
    point.distance_squared(from + delta * t)
}

fn outer(a: Vec3, b: Vec3) -> Mat3 {
    Mat3::from_cols(a * b.x, a * b.y, a * b.z)
}

#[derive(Clone, Copy, Debug)]
pub struct RigidState {
    /// World position of the centre of mass.
    pub pos: Vec3,
    pub rot: Quat,
    pub vel: Vec3,
    pub ang: Vec3,
    /// Fastest normal impact speed seen so far (m/s).
    pub max_impact: f32,
    rest_time: f32,
}

struct Contact {
    r: Vec3,
    normal: Vec3,
    depth: f32,
}

impl RigidState {
    pub fn new(pos: Vec3) -> Self {
        Self { pos, rot: Quat::IDENTITY, vel: Vec3::ZERO, ang: Vec3::ZERO, max_impact: 0.0, rest_time: 0.0 }
    }

    pub fn at_rest(&self) -> bool {
        self.rest_time >= SLEEP_TIME
    }

    fn apply_impulse(&mut self, shape: &RigidShape, inv_inertia: &Mat3, r: Vec3, impulse: Vec3) {
        self.vel += impulse * shape.inv_mass;
        self.ang += *inv_inertia * r.cross(impulse);
    }

    /// Mass the body presents to an impulse along `dir` at offset `r` from its centre of mass.
    fn inv_effective_mass(shape: &RigidShape, inv_inertia: &Mat3, r: Vec3, dir: Vec3) -> f32 {
        shape.inv_mass + dir.dot((*inv_inertia * r.cross(dir)).cross(r))
    }

    /// Changes the velocity of the body point at world offset `r` by `speed` along `dir`.
    pub fn push_point(&mut self, shape: &RigidShape, r: Vec3, dir: Vec3, speed: f32) {
        self.rest_time = 0.0;
        let inv_inertia = shape.world_inv_inertia(self.rot);
        let k = Self::inv_effective_mass(shape, &inv_inertia, r, dir);
        self.apply_impulse(shape, &inv_inertia, r, dir * (speed / k));
    }

    pub fn step(&mut self, shape: &RigidShape, dt: f32, is_solid: &impl Fn(IVec3) -> bool) {
        let motion = self.vel.length() * dt + self.ang.length() * shape.radius * dt + GRAVITY * dt * dt;
        let substeps = ((motion / (VOXEL_SIZE * 0.4)).ceil() as u32).clamp(1, MAX_SUBSTEPS);
        let h = dt / substeps as f32;

        let mut touching = false;
        for _ in 0..substeps {
            touching |= self.substep(shape, h, is_solid);
        }

        self.vel *= (-LINEAR_DAMPING * dt).exp();
        self.ang *= (-ANGULAR_DAMPING * dt).exp();
        if touching && self.vel.length() < SLEEP_LINEAR && self.ang.length() < SLEEP_ANGULAR {
            self.rest_time += dt;
        } else {
            self.rest_time = 0.0;
        }
    }

    fn substep(&mut self, shape: &RigidShape, h: f32, is_solid: &impl Fn(IVec3) -> bool) -> bool {
        self.vel.y -= GRAVITY * h;
        self.pos += self.vel * h;
        self.rot = (Quat::from_scaled_axis(self.ang * h) * self.rot).normalize();

        let rot = Mat3::from_quat(self.rot);
        let inv_inertia = rot * shape.inv_inertia * rot.transpose();
        // Vertical half-extent of a rotated voxel: test each voxel's lowest point against the grid.
        let half_y = 0.5 * VOXEL_SIZE * (rot.x_axis.y.abs() + rot.y_axis.y.abs() + rot.z_axis.y.abs());

        let mut contacts = Vec::new();
        for q in &shape.points {
            let centre = self.pos + rot * *q;
            let low = Vec3::new(centre.x, centre.y - half_y + EPS, centre.z);
            let cell = (low / VOXEL_SIZE).floor().as_ivec3();
            if is_solid(cell) {
                let (normal, depth) = contact_normal(low, cell, is_solid);
                contacts.push(Contact { r: low - self.pos, normal, depth });
            }
        }
        if contacts.is_empty() {
            return false;
        }

        // Projected Gauss-Seidel with accumulated impulses: each contact's total normal impulse stays
        // non-negative and its friction stays inside the cone, so load spreads across all contacts.
        let bounce: Vec<f32> = contacts
            .iter()
            .map(|c| {
                let vn = (self.vel + self.ang.cross(c.r)).dot(c.normal);
                self.max_impact = self.max_impact.max(-vn);
                if -vn > BOUNCE_THRESHOLD { -RESTITUTION * vn } else { 0.0 }
            })
            .collect();
        let normal_mass: Vec<f32> =
            contacts.iter().map(|c| Self::inv_effective_mass(shape, &inv_inertia, c.r, c.normal)).collect();
        let mut normal_total = vec![0.0f32; contacts.len()];
        let mut friction_total = vec![Vec3::ZERO; contacts.len()];

        for _ in 0..SOLVER_ITERATIONS {
            for (i, c) in contacts.iter().enumerate() {
                let vn = (self.vel + self.ang.cross(c.r)).dot(c.normal);
                let total = (normal_total[i] + (bounce[i] - vn) / normal_mass[i]).max(0.0);
                let delta = total - normal_total[i];
                normal_total[i] = total;
                self.apply_impulse(shape, &inv_inertia, c.r, c.normal * delta);

                let v = self.vel + self.ang.cross(c.r);
                let tangential = v - c.normal * v.dot(c.normal);
                let slide = tangential.length();
                if slide > 1e-6 {
                    let t = tangential / slide;
                    let kt = Self::inv_effective_mass(shape, &inv_inertia, c.r, t);
                    let wanted = friction_total[i] - t * (slide / kt);
                    let limit = FRICTION * normal_total[i];
                    let clamped = if wanted.length() > limit { wanted.normalize_or_zero() * limit } else { wanted };
                    self.apply_impulse(shape, &inv_inertia, c.r, clamped - friction_total[i]);
                    friction_total[i] = clamped;
                }
            }
        }

        let push = contacts.iter().map(|c| c.normal * c.depth).sum::<Vec3>() / contacts.len() as f32;
        self.pos += push * POSITION_CORRECTION;
        true
    }
}

/// Resolve sampled surface contacts between two moving pieces. Occupancy is checked in
/// each body's local voxel grid, so holes in a piece do not become solid bounding boxes.
/// Surface samples are spheres inscribed in voxels; this is an approximate collider.
pub fn collide_pair(sa: &RigidShape, a: &mut RigidState, sb: &RigidShape, b: &mut RigidState) {
    if a.pos.distance_squared(b.pos) > (sa.radius + sb.radius).powi(2) {
        return;
    }
    let mut contacts = Vec::new();
    pair_contacts(sa, a, sb, b, false, &mut contacts);
    pair_contacts(sb, b, sa, a, true, &mut contacts);
    if contacts.is_empty() {
        return;
    }
    let ia = sa.world_inv_inertia(a.rot);
    let ib = sb.world_inv_inertia(b.rot);
    let mut totals = vec![0.0f32; contacts.len()];
    let mut friction = vec![Vec3::ZERO; contacts.len()];
    let bounce: Vec<f32> = contacts.iter().map(|(p, n, _)| {
        let speed = (a.vel + a.ang.cross(*p - a.pos) - b.vel - b.ang.cross(*p - b.pos)).dot(*n);
        a.max_impact = a.max_impact.max(-speed);
        b.max_impact = b.max_impact.max(-speed);
        if speed < -BOUNCE_THRESHOLD { -speed * RESTITUTION } else { 0.0 }
    }).collect();
    for _ in 0..SOLVER_ITERATIONS {
        for (i, (p, n, _)) in contacts.iter().enumerate() {
            let ra = *p - a.pos;
            let rb = *p - b.pos;
            let relative = a.vel + a.ang.cross(ra) - b.vel - b.ang.cross(rb);
            let k = RigidState::inv_effective_mass(sa, &ia, ra, *n)
                + RigidState::inv_effective_mass(sb, &ib, rb, *n);
            let total = (totals[i] + (bounce[i] - relative.dot(*n)) / k).max(0.0);
            let impulse = *n * (total - totals[i]);
            totals[i] = total;
            a.apply_impulse(sa, &ia, ra, impulse);
            b.apply_impulse(sb, &ib, rb, -impulse);
            let relative = a.vel + a.ang.cross(ra) - b.vel - b.ang.cross(rb);
            let slide = relative - *n * relative.dot(*n);
            if slide.length_squared() > 1e-12 {
                let t = slide.normalize();
                let kt = RigidState::inv_effective_mass(sa, &ia, ra, t)
                    + RigidState::inv_effective_mass(sb, &ib, rb, t);
                let next = (friction[i] - slide / kt).clamp_length_max(FRICTION * total);
                a.apply_impulse(sa, &ia, ra, next - friction[i]);
                b.apply_impulse(sb, &ib, rb, friction[i] - next);
                friction[i] = next;
            }
        }
    }
    let correction = contacts.iter().map(|(_, n, d)| *n * (*d - EPS).max(0.0)).sum::<Vec3>()
        * (POSITION_CORRECTION / contacts.len() as f32);
    let mass = sa.inv_mass + sb.inv_mass;
    a.pos += correction * (sa.inv_mass / mass);
    b.pos -= correction * (sb.inv_mass / mass);
    // Wake disturbed bodies, but let a quiet body supported by terrain settle first.
    for state in [a, b] {
        if state.vel.length() >= SLEEP_LINEAR || state.ang.length() >= SLEEP_ANGULAR
            || correction.length() > 0.002 {
            state.rest_time = 0.0;
        }
    }
}

fn pair_contacts(
    source: &RigidShape, a: &RigidState, target: &RigidShape, b: &RigidState,
    reverse: bool, contacts: &mut Vec<(Vec3, Vec3, f32)>,
) {
    let inverse = b.rot.inverse();
    let radius = VOXEL_SIZE * 0.5;
    // Bound narrow-phase work per pair. Large surfaces use evenly spaced samples.
    let stride = source.points.len().div_ceil(512).max(1);
    for q in source.points.iter().step_by(stride) {
        let world = a.pos + a.rot * *q;
        let local = inverse * (world - b.pos) + target.com;
        let lo = ((local - radius) / VOXEL_SIZE).floor().as_ivec3();
        let hi = ((local + radius) / VOXEL_SIZE).floor().as_ivec3();
        let mut best: Option<(Vec3, f32)> = None;
        for y in lo.y..=hi.y {
            for z in lo.z..=hi.z {
                for x in lo.x..=hi.x {
                    let cell = IVec3::new(x, y, z);
                    if !target.occupied.contains(&cell) { continue; }
                    let low = cell.as_vec3() * VOXEL_SIZE;
                    let delta = local - local.clamp(low, low + VOXEL_SIZE);
                    let distance = delta.length();
                    let (normal, depth) = if distance > EPS {
                        (delta / distance, radius - distance)
                    } else {
                        let (n, d) = contact_normal(local, cell, &|v| target.occupied.contains(&v));
                        (n, radius + d)
                    };
                    if depth > EPS && best.map_or(true, |(_, d)| depth > d) {
                        best = Some((normal, depth));
                    }
                }
            }
        }
        if let Some((normal, depth)) = best {
            let normal = b.rot * normal;
            let point = world - normal * (radius - depth * 0.5);
            contacts.push((point, if reverse { -normal } else { normal }, depth));
        }
    }
}

/// For a point inside solid `cell`: the direction out through the nearest face that opens
/// onto air, and how far the point is from that face.
fn contact_normal(point: Vec3, cell: IVec3, is_solid: &impl Fn(IVec3) -> bool) -> (Vec3, f32) {
    let local = point - cell.as_vec3() * VOXEL_SIZE;
    let faces = [
        (IVec3::Y, VOXEL_SIZE - local.y),
        (IVec3::NEG_Y, local.y),
        (IVec3::X, VOXEL_SIZE - local.x),
        (IVec3::NEG_X, local.x),
        (IVec3::Z, VOXEL_SIZE - local.z),
        (IVec3::NEG_Z, local.z),
    ];
    faces
        .iter()
        .filter(|(dir, _)| !is_solid(cell + *dir))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(dir, depth)| (dir.as_vec3(), *depth))
        .unwrap_or((Vec3::Y, VOXEL_SIZE - local.y))
}

fn snap_axis(v: Vec3) -> Vec3 {
    let a = v.abs();
    if a.x >= a.y && a.x >= a.z {
        Vec3::X * v.x.signum()
    } else if a.y >= a.z {
        Vec3::Y * v.y.signum()
    } else {
        Vec3::Z * v.z.signum()
    }
}

/// Squares a nearly axis-aligned body up to the grid so it writes back without aliasing.
pub fn snap_to_grid(shape: &RigidShape, state: &RigidState) -> RigidState {
    let rot = Mat3::from_quat(state.rot);
    let (c0, c1) = (snap_axis(rot.x_axis), snap_axis(rot.y_axis));
    if c0.dot(c1).abs() > 0.5 {
        return *state;
    }
    let snapped = Quat::from_mat3(&Mat3::from_cols(c0, c1, c0.cross(c1)));
    if snapped.angle_between(state.rot) > SNAP_ANGLE {
        return *state;
    }

    // Move so voxel centres land exactly on cell centres.
    let reference = (shape.voxels[0].as_vec3() + 0.5) * VOXEL_SIZE;
    let world = state.pos + snapped * (reference - shape.com);
    let aligned = ((world / VOXEL_SIZE).floor() + 0.5) * VOXEL_SIZE;
    RigidState { pos: state.pos + (aligned - world), rot: snapped, ..*state }
}

/// World cells covered by the body, each paired with the index of the voxel that covers it.
pub fn rasterize(shape: &RigidShape, state: &RigidState) -> Vec<(IVec3, usize)> {
    let index: HashMap<IVec3, usize> = shape.voxels.iter().enumerate().map(|(i, v)| (*v, i)).collect();
    let (lo, hi) = shape.voxels.iter().fold((IVec3::MAX, IVec3::MIN), |(lo, hi), v| (lo.min(*v), hi.max(*v)));

    let (mut world_lo, mut world_hi) = (Vec3::MAX, Vec3::MIN);
    for corner in 0..8 {
        let pick = |bit: i32, a: i32, b: i32| if corner & bit == 0 { a } else { b + 1 };
        let local = IVec3::new(pick(1, lo.x, hi.x), pick(2, lo.y, hi.y), pick(4, lo.z, hi.z)).as_vec3() * VOXEL_SIZE;
        let world = state.pos + state.rot * (local - shape.com);
        world_lo = world_lo.min(world);
        world_hi = world_hi.max(world);
    }

    let inverse = state.rot.inverse();
    let (cell_lo, cell_hi) = ((world_lo / VOXEL_SIZE).floor().as_ivec3(), (world_hi / VOXEL_SIZE).floor().as_ivec3());
    let mut cells = Vec::new();
    for y in cell_lo.y..=cell_hi.y {
        for z in cell_lo.z..=cell_hi.z {
            for x in cell_lo.x..=cell_hi.x {
                let cell = IVec3::new(x, y, z);
                let centre = (cell.as_vec3() + 0.5) * VOXEL_SIZE;
                let local = inverse * (centre - state.pos) + shape.com;
                if let Some(&i) = index.get(&(local / VOXEL_SIZE).floor().as_ivec3()) {
                    cells.push((cell, i));
                }
            }
        }
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_cubes_exchange_momentum_and_separate() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let mut a = RigidState::new(Vec3::ZERO);
        let mut b = RigidState::new(Vec3::new(0.38, 0.0, 0.0));
        a.vel = Vec3::X * 2.0;
        let momentum = a.vel + b.vel;
        collide_pair(&shape, &mut a, &shape, &mut b);
        assert!(b.vel.x > 0.5, "no momentum transferred: {b:?}");
        assert!((a.vel + b.vel - momentum).length() < 1e-4);
        assert!(b.pos.x - a.pos.x > 0.38);
        assert!(a.max_impact > 1.0 && b.max_impact > 1.0);
    }

    #[test]
    fn separating_cubes_receive_no_attractive_impulse() {
        let shape = RigidShape::new(block(IVec3::splat(2)));
        let mut a = RigidState::new(Vec3::ZERO);
        let mut b = RigidState::new(Vec3::new(0.19, 0.0, 0.0));
        a.vel = -Vec3::X;
        b.vel = Vec3::X;
        collide_pair(&shape, &mut a, &shape, &mut b);
        assert!((a.vel + Vec3::X).length() < 1e-5);
        assert!((b.vel - Vec3::X).length() < 1e-5);
    }

    #[test]
    fn hollow_piece_does_not_collide_in_its_empty_interior() {
        let shell = RigidShape::new(block(IVec3::splat(10)).into_iter().filter(|v|
            v.x == 0 || v.x == 9 || v.y == 0 || v.y == 9 || v.z == 0 || v.z == 9
        ).collect());
        let small = RigidShape::new(block(IVec3::splat(2)));
        let mut a = RigidState::new(Vec3::ZERO);
        let mut b = RigidState::new(Vec3::ZERO);
        b.rot = Quat::from_rotation_y(0.4);
        b.vel = Vec3::X;
        collide_pair(&shell, &mut a, &small, &mut b);
        assert_eq!(a.pos, Vec3::ZERO);
        assert_eq!(a.vel, Vec3::ZERO);
        assert_eq!(b.vel, Vec3::X);
    }

    #[test]
    fn off_centre_collision_spins_target() {
        let small = RigidShape::new(block(IVec3::splat(2)));
        let large = RigidShape::new(block(IVec3::splat(6)));
        let mut a = RigidState::new(Vec3::new(-0.38, 0.22, 0.0));
        let mut b = RigidState::new(Vec3::ZERO);
        a.vel = Vec3::X * 3.0;
        collide_pair(&small, &mut a, &large, &mut b);
        assert!(b.ang.z.abs() > 0.1, "no rotational response: {b:?}");
        let momentum = a.vel * 8.0 + b.vel * 216.0;
        assert!((momentum - Vec3::X * 24.0).length() < 1e-3);
    }

    #[test]
    fn segment_hit_finds_piece_surface() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let state = RigidState::new(Vec3::ZERO);
        let hit = shape.segment_hit(&state, Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));

        assert!(hit.is_some(), "segment passed through piece without a hit");
        assert!(hit.unwrap().1.x < 0.0);
    }

    #[test]
    fn segment_hit_misses_clear_space() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let state = RigidState::new(Vec3::ZERO);

        assert!(shape.segment_hit(&state, Vec3::new(-1.0, 1.0, 0.0), Vec3::new(1.0, 1.0, 0.0)).is_none());
    }

    fn block(size: IVec3) -> Vec<IVec3> {
        let mut v = Vec::new();
        for y in 0..size.y {
            for z in 0..size.z {
                for x in 0..size.x {
                    v.push(IVec3::new(x, y, z));
                }
            }
        }
        v
    }

    fn simulate(shape: &RigidShape, state: &mut RigidState, seconds: f32, world: impl Fn(IVec3) -> bool) {
        for _ in 0..(seconds * 60.0) as i32 {
            state.step(shape, 1.0 / 60.0, &world);
            if state.at_rest() {
                return;
            }
        }
    }

    /// Floor top at y = 0.5 m.
    fn floor(v: IVec3) -> bool {
        v.y < 5
    }

    #[test]
    fn dropped_cube_comes_to_rest_upright_on_the_floor() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let mut state = RigidState::new(Vec3::new(1.0, 1.5, 1.0));
        simulate(&shape, &mut state, 5.0, floor);

        assert!(state.at_rest(), "still moving: {state:?}");
        assert!((state.pos.y - 0.7).abs() < 0.03, "centre at {}", state.pos.y);
        assert!(state.rot.angle_between(Quat::IDENTITY) < 0.05, "twisted: {state:?}");
        let drift = Vec3::new(state.pos.x - 1.0, 0.0, state.pos.z - 1.0).length();
        assert!(drift < 0.05, "slid {drift} m");
        assert!(state.max_impact > 3.0);
    }

    #[test]
    fn pushed_slab_topples_over() {
        // 0.2 m x 2 m x 0.6 m slab standing on the floor, nudged at the top.
        let shape = RigidShape::new(block(IVec3::new(2, 20, 6)));
        let mut state = RigidState::new(Vec3::new(1.0, 0.5 + 1.0 + 0.001, 1.0));
        state.push_point(&shape, Vec3::new(0.0, 1.0, 0.0), Vec3::X, 1.5);
        simulate(&shape, &mut state, 8.0, floor);

        assert!(state.at_rest(), "still moving: {state:?}");
        assert!(state.pos.y < 0.5 + 0.2, "slab still standing, centre at {}", state.pos.y);
    }

    #[test]
    fn unbalanced_plank_tips_over_a_ledge() {
        // 3 m plank on a 2 m ledge that ends 1 m in; 2 m hangs over the drop to a floor at 0.5 m.
        // It should pivot on the edge until its far end reaches the floor (about 48 degrees).
        let shape = RigidShape::new(block(IVec3::new(30, 2, 4)));
        let ledge = |v: IVec3| (v.x < 10 && v.y < 20) || v.y < 5;
        let mut state = RigidState::new(Vec3::new(1.5, 2.0 + 0.1 + 0.001, 0.2));
        simulate(&shape, &mut state, 8.0, ledge);

        let tilt = state.rot.angle_between(Quat::IDENTITY);
        assert!(tilt > 0.5, "plank did not tip: {tilt} rad, {state:?}");
        assert!(state.pos.y < 1.9, "centre at {}", state.pos.y);
    }

    #[test]
    fn upright_body_rasterizes_back_to_the_same_voxels() {
        let voxels = block(IVec3::new(3, 5, 2));
        let shape = RigidShape::new(voxels.clone());
        let state = RigidState::new(shape.com + Vec3::new(1.0, 2.0, 3.0));
        let cells = rasterize(&shape, &state);

        assert_eq!(cells.len(), voxels.len());
        let expected: HashSet<IVec3> = voxels.iter().map(|v| *v + IVec3::new(10, 20, 30)).collect();
        assert!(cells.iter().all(|(c, _)| expected.contains(c)));
    }

    #[test]
    fn nearly_quarter_turned_body_snaps_and_keeps_every_voxel() {
        let voxels = block(IVec3::new(6, 2, 3));
        let shape = RigidShape::new(voxels.clone());
        let mut state = RigidState::new(Vec3::new(2.03, 1.01, 0.98));
        state.rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 + 0.2);

        let snapped = snap_to_grid(&shape, &state);
        assert!(snapped.rot.angle_between(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)) < 1e-3);
        let cells = rasterize(&shape, &snapped);
        assert_eq!(cells.len(), voxels.len());
        assert_eq!(cells.iter().map(|(c, _)| *c).collect::<HashSet<_>>().len(), voxels.len());
    }
}
