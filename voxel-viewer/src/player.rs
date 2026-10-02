//! Character physics against the voxel grid. Units are metres; voxels are 10 cm.

use bevy::math::{IVec3, Vec3};

pub const VOXEL_SIZE: f32 = 0.1;
pub const GRAVITY: f32 = 9.81;
const MAX_FALL_SPEED: f32 = 40.0;
/// In voxel units; keeps a body resting exactly on a voxel face from counting as inside it.
const EPS: f32 = 1e-3;

#[derive(Clone, Copy)]
pub struct Shape {
    pub half_width: f32,
    pub height: f32,
    pub max_step: f32,
}

/// Human-sized collider: 1.8 m tall, 0.5 m wide, climbs ledges up to 30 cm.
pub const HUMAN: Shape = Shape { half_width: 0.25, height: 1.8, max_step: 0.3 };
/// Monsters use the same step limit, so a wall is a wall and a stair is a stair.
pub const ENEMY_BODY: Shape = Shape { half_width: 0.28, height: 1.7, max_step: 0.3 };

/// `pos` is the centre of the feet.
#[derive(Clone, Copy, Default)]
pub struct Body {
    pub pos: Vec3,
    pub vel: Vec3,
    pub on_ground: bool,
}

impl Body {
    pub fn step(&mut self, shape: &Shape, dt: f32, is_solid: &impl Fn(IVec3) -> bool) {
        if overlaps(self.pos, shape, is_solid) {
            // Embedded (e.g. terrain regenerated around us): rise out one voxel per step.
            self.pos.y += VOXEL_SIZE;
            self.vel = Vec3::ZERO;
            return;
        }

        self.vel.y = (self.vel.y - GRAVITY * dt).max(-MAX_FALL_SPEED);

        // Never move more than half a voxel at a time, so fast bodies can't tunnel.
        let delta = self.vel * dt;
        let substeps = (delta.abs().max_element() / (VOXEL_SIZE * 0.5)).ceil().max(1.0) as u32;
        let d = delta / substeps as f32;

        let was_grounded = self.on_ground;
        self.on_ground = false;
        for _ in 0..substeps {
            self.move_vertical(shape, d.y, is_solid);
            let can_step = was_grounded || self.on_ground;
            self.move_horizontal(shape, 0, d.x, can_step, is_solid);
            self.move_horizontal(shape, 2, d.z, can_step, is_solid);
        }

        if was_grounded && !self.on_ground && self.vel.y <= 0.0 {
            self.snap_down(shape, is_solid);
        }
    }

    /// Keeps a walking body glued to ground up to `max_step` below instead of free-falling off
    /// every small ledge.
    fn snap_down(&mut self, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) {
        let base = (self.pos.y / VOXEL_SIZE).round() * VOXEL_SIZE;
        let max_voxels = (shape.max_step / VOXEL_SIZE).round() as i32;
        for k in 0..=max_voxels {
            let at = Vec3::new(self.pos.x, base - k as f32 * VOXEL_SIZE, self.pos.z);
            if overlaps(at, shape, is_solid) {
                return;
            }
            if overlaps(at - Vec3::Y * (VOXEL_SIZE * 0.5), shape, is_solid) {
                self.pos = at;
                self.vel.y = 0.0;
                self.on_ground = true;
                return;
            }
        }
    }

    fn move_vertical(&mut self, shape: &Shape, dy: f32, is_solid: &impl Fn(IVec3) -> bool) {
        if dy == 0.0 {
            return;
        }
        let next = self.pos + Vec3::Y * dy;
        if !overlaps(next, shape, is_solid) {
            self.pos = next;
            return;
        }
        if dy < 0.0 {
            let feet_voxel = (next.y / VOXEL_SIZE + EPS).floor();
            self.pos.y = (feet_voxel + 1.0) * VOXEL_SIZE;
            self.on_ground = true;
        }
        self.vel.y = 0.0;
    }

    fn move_horizontal(
        &mut self,
        shape: &Shape,
        axis: usize,
        amount: f32,
        can_step: bool,
        is_solid: &impl Fn(IVec3) -> bool,
    ) {
        if amount == 0.0 {
            return;
        }
        let mut next = self.pos;
        next[axis] += amount;
        if !overlaps(next, shape, is_solid) {
            self.pos = next;
            return;
        }

        if can_step {
            let base = (self.pos.y / VOXEL_SIZE).round() * VOXEL_SIZE;
            let max_voxels = (shape.max_step / VOXEL_SIZE).round() as i32;
            for k in 1..=max_voxels {
                let lift = base + k as f32 * VOXEL_SIZE;
                let raised_here = Vec3::new(self.pos.x, lift, self.pos.z);
                let raised_next = Vec3::new(next.x, lift, next.z);
                if !overlaps(raised_here, shape, is_solid) && !overlaps(raised_next, shape, is_solid) {
                    self.pos = raised_next;
                    self.on_ground = true;
                    return;
                }
            }
        }

        self.vel[axis] = 0.0;
    }
}

/// Move `delta` on the ground. A column whose floor is more than `max_step` above the feet
/// is refused, and so is any move that would leave the body inside solid. Slides along a wall
/// when the full step is blocked.
pub fn walk_step(pos: Vec3, delta: Vec3, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) -> Vec3 {
    if let Some(next) = grounded_move(pos, delta, shape, is_solid) {
        return next;
    }
    if delta.x != 0.0 {
        if let Some(next) = grounded_move(pos, Vec3::new(delta.x, 0.0, 0.0), shape, is_solid) {
            return next;
        }
    }
    if delta.z != 0.0 {
        if let Some(next) = grounded_move(pos, Vec3::new(0.0, 0.0, delta.z), shape, is_solid) {
            return next;
        }
    }
    grounded_move(pos, Vec3::ZERO, shape, is_solid).unwrap_or(pos)
}

fn grounded_move(pos: Vec3, delta: Vec3, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) -> Option<Vec3> {
    let next = Vec3::new(pos.x + delta.x, pos.y, pos.z + delta.z);
    // The body's front meets a riser while the centre column is still on the lower floor.
    // The highest legal surface under the footprint is the step; a wall column has none.
    let stand = footprint_stand(next, shape, is_solid)?;
    if stand > pos.y + shape.max_step + 1e-3 {
        return None;
    }
    let at = Vec3::new(next.x, stand, next.z);
    if overlaps(at, shape, is_solid) {
        return None;
    }
    Some(at)
}

/// Highest floor under the body that `stand_height` will accept. Columns that are a wall
/// (no surface within a step, with room for the body) are skipped.
fn footprint_stand(pos: Vec3, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) -> Option<f32> {
    let x0 = ((pos.x - shape.half_width) / VOXEL_SIZE + EPS).floor() as i32;
    let x1 = ((pos.x + shape.half_width) / VOXEL_SIZE - EPS).floor() as i32;
    let z0 = ((pos.z - shape.half_width) / VOXEL_SIZE + EPS).floor() as i32;
    let z1 = ((pos.z + shape.half_width) / VOXEL_SIZE - EPS).floor() as i32;
    let mut best: Option<f32> = None;
    for z in z0..=z1 {
        for x in x0..=x1 {
            let sample = Vec3::new((x as f32 + 0.5) * VOXEL_SIZE, pos.y, (z as f32 + 0.5) * VOXEL_SIZE);
            if let Some(height) = stand_height(sample, shape, is_solid) {
                best = Some(best.map_or(height, |had| had.max(height)));
            }
        }
    }
    best
}

/// Highest floor within a step of the feet that still leaves the body in air.
/// A roof far above the feet is ignored, so standing under a ceiling uses the floor.
fn stand_height(pos: Vec3, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) -> Option<f32> {
    let x = (pos.x / VOXEL_SIZE).floor() as i32;
    let z = (pos.z / VOXEL_SIZE).floor() as i32;
    let feet_v = (pos.y / VOXEL_SIZE).floor() as i32;
    let max_up = (shape.max_step / VOXEL_SIZE).round() as i32;
    let head_v = (shape.height / VOXEL_SIZE).round() as i32;
    let top = (feet_v + max_up).max(1);
    for surface in (1..=top).rev() {
        if !is_solid(IVec3::new(x, surface - 1, z)) {
            continue;
        }
        let clear = (0..head_v).all(|k| !is_solid(IVec3::new(x, surface + k, z)));
        if clear {
            return Some(surface as f32 * VOXEL_SIZE);
        }
    }
    None
}

pub fn overlaps(pos: Vec3, shape: &Shape, is_solid: &impl Fn(IVec3) -> bool) -> bool {
    let half = Vec3::new(shape.half_width, 0.0, shape.half_width);
    let lo = ((pos - half) / VOXEL_SIZE + EPS).floor().as_ivec3();
    let hi = ((pos + half + Vec3::Y * shape.height) / VOXEL_SIZE - EPS).floor().as_ivec3();

    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                if is_solid(IVec3::new(x, y, z)) {
                    return true;
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 60.0;

    fn simulate(body: &mut Body, frames: u32, walk_x: f32, world: impl Fn(IVec3) -> bool) {
        for _ in 0..frames {
            body.vel.x = walk_x;
            body.step(&HUMAN, DT, &world);
        }
    }

    fn floor_at_1m(v: IVec3) -> bool {
        v.y < 10
    }

    #[test]
    fn falls_from_height_and_lands_on_surface() {
        let mut body = Body { pos: Vec3::new(0.0, 50.0, 0.0), ..Default::default() };
        simulate(&mut body, 400, 0.0, floor_at_1m);
        assert!(body.on_ground);
        assert!((body.pos.y - 1.0).abs() < 1e-3, "feet at {}", body.pos.y);
    }

    #[test]
    fn walks_up_a_20cm_ledge() {
        let mut body = Body { pos: Vec3::new(0.0, 1.0, 0.0), ..Default::default() };
        simulate(&mut body, 60, 3.0, |v| v.y < 10 || (v.x >= 5 && v.y < 12));
        assert!(body.pos.x > 1.0, "x = {}", body.pos.x);
        assert!((body.pos.y - 1.2).abs() < 1e-3, "feet at {}", body.pos.y);
    }

    #[test]
    fn is_blocked_by_a_wall_taller_than_step_height() {
        let mut body = Body { pos: Vec3::new(0.0, 1.0, 0.0), ..Default::default() };
        simulate(&mut body, 60, 3.0, |v| v.y < 10 || (v.x >= 5 && v.y < 14));
        assert!(body.pos.x <= 0.25 + 1e-3, "walked into wall: x = {}", body.pos.x);
        assert!((body.pos.y - 1.0).abs() < 1e-3);
    }

    #[test]
    fn walks_down_a_20cm_ledge_without_leaving_the_ground() {
        let world = |v: IVec3| v.y < 10 || (v.x < 5 && v.y < 12);
        let mut body = Body { pos: Vec3::new(0.0, 1.2, 0.0), on_ground: true, ..Default::default() };
        for _ in 0..60 {
            body.vel.x = 3.0;
            body.step(&HUMAN, DT, &world);
            assert!(body.on_ground, "airborne at x = {}, y = {}", body.pos.x, body.pos.y);
        }
        assert!((body.pos.y - 1.0).abs() < 1e-3, "feet at {}", body.pos.y);
    }

    #[test]
    fn a_walker_stops_at_a_wall_and_still_uses_a_door() {
        let wall = |v: IVec3| v.y < 10 || ((10..13).contains(&v.x) && v.y < 40 && !(-3..4).contains(&v.z));
        let mut blocked = Vec3::new(0.4, 1.0, 2.0);
        for _ in 0..40 {
            blocked = walk_step(blocked, Vec3::new(0.08, 0.0, 0.0), &HUMAN, &wall);
        }
        assert!(blocked.x < 0.85, "climbed or crossed the wall at x {}", blocked.x);
        assert!((blocked.y - 1.0).abs() < 1e-3, "feet left the ground at {}", blocked.y);

        let mut through = Vec3::new(0.4, 1.0, 0.05);
        for _ in 0..40 {
            through = walk_step(through, Vec3::new(0.08, 0.0, 0.0), &HUMAN, &wall);
        }
        assert!(through.x > 1.5, "door stopped the walker at x {}", through.x);
    }

    #[test]
    fn a_walker_climbs_a_step_and_not_a_roof() {
        let ledge = |v: IVec3| v.y < 10 || (v.x >= 6 && v.y < 12);
        let mut pos = Vec3::new(0.2, 1.0, 0.2);
        for _ in 0..40 {
            pos = walk_step(pos, Vec3::new(0.08, 0.0, 0.0), &HUMAN, &ledge);
        }
        assert!(pos.x > 0.8, "x {}", pos.x);
        assert!((pos.y - 1.2).abs() < 1e-3, "feet {}", pos.y);

        let building = |v: IVec3| v.y < 10 || (v.x >= 8 && v.y < 50);
        let mut stuck = Vec3::new(0.3, 1.0, 0.3);
        for _ in 0..40 {
            stuck = walk_step(stuck, Vec3::new(0.1, 0.0, 0.0), &ENEMY_BODY, &building);
        }
        assert!(stuck.x < 0.7, "walked up the building to x {}", stuck.x);
        assert!(stuck.y < 1.2, "stood on the wall at y {}", stuck.y);
    }

    #[test]
    fn falls_into_a_hole() {
        let mut body = Body { pos: Vec3::new(1.0, 1.0, 1.0), ..Default::default() };
        simulate(&mut body, 120, 0.0, |v| v.y < 10 && !(0..20).contains(&v.x));
        assert!(body.pos.y < 0.0, "still standing at {}", body.pos.y);
    }
}
