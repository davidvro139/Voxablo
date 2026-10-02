//! Exact voxel ray traversal (Amanatides & Woo). All inputs are in voxel units.

use bevy::math::{IVec3, Vec3};

pub struct Hit {
    pub voxel: IVec3,
    /// Face of `voxel` the ray entered through; zero if the ray started inside it.
    pub normal: IVec3,
    /// Distance along the (normalised) ray to the point where it entered `voxel`.
    pub distance: f32,
}

pub fn raycast(origin: Vec3, dir: Vec3, max_dist: f32, is_solid: impl Fn(IVec3) -> bool) -> Option<Hit> {
    let dir = dir.normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }

    let mut voxel = origin.floor().as_ivec3();
    let mut step = IVec3::ZERO;
    let mut t_max = Vec3::splat(f32::INFINITY);
    let mut t_delta = Vec3::splat(f32::INFINITY);
    for axis in 0..3 {
        if dir[axis] > 0.0 {
            step[axis] = 1;
            t_max[axis] = (voxel[axis] as f32 + 1.0 - origin[axis]) / dir[axis];
            t_delta[axis] = 1.0 / dir[axis];
        } else if dir[axis] < 0.0 {
            step[axis] = -1;
            t_max[axis] = (voxel[axis] as f32 - origin[axis]) / dir[axis];
            t_delta[axis] = -1.0 / dir[axis];
        }
    }

    let mut normal = IVec3::ZERO;
    let mut distance = 0.0;
    loop {
        if is_solid(voxel) {
            return Some(Hit { voxel, normal, distance });
        }
        let axis = if t_max.x < t_max.y {
            if t_max.x < t_max.z { 0 } else { 2 }
        } else if t_max.y < t_max.z {
            1
        } else {
            2
        };
        if t_max[axis] > max_dist {
            return None;
        }
        distance = t_max[axis];
        voxel[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        normal = IVec3::ZERO;
        normal[axis] = -step[axis];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floor(v: IVec3) -> bool {
        v.y < 10
    }

    #[test]
    fn straight_down_hits_top_face() {
        let hit = raycast(Vec3::new(0.5, 20.5, 0.5), Vec3::NEG_Y, 100.0, floor).unwrap();
        assert_eq!(hit.voxel, IVec3::new(0, 9, 0));
        assert_eq!(hit.normal, IVec3::Y);
        assert!((hit.distance - 10.5).abs() < 1e-4, "distance {}", hit.distance);
    }

    #[test]
    fn starting_inside_solid_hits_immediately() {
        let hit = raycast(Vec3::new(0.5, 5.5, 0.5), Vec3::X, 100.0, floor).unwrap();
        assert_eq!(hit.voxel, IVec3::new(0, 5, 0));
        assert_eq!(hit.normal, IVec3::ZERO);
        assert_eq!(hit.distance, 0.0);
    }

    #[test]
    fn diagonal_ray_hits_floor_at_expected_column() {
        // Reaches y = 10 (top of the floor) at x = 10.8.
        let hit = raycast(Vec3::new(0.3, 20.5, 0.5), Vec3::new(1.0, -1.0, 0.0), 100.0, floor).unwrap();
        assert_eq!(hit.voxel, IVec3::new(10, 9, 0));
        assert_eq!(hit.normal, IVec3::Y);
    }

    #[test]
    fn hits_wall_side_before_floor() {
        let wall = |v: IVec3| v.y < 10 || v.x >= 5;
        let hit = raycast(Vec3::new(0.5, 12.5, 0.5), Vec3::X, 100.0, wall).unwrap();
        assert_eq!(hit.voxel, IVec3::new(5, 12, 0));
        assert_eq!(hit.normal, IVec3::NEG_X);
    }

    #[test]
    fn misses_when_pointing_away_or_out_of_range() {
        assert!(raycast(Vec3::new(0.5, 20.5, 0.5), Vec3::Y, 100.0, floor).is_none());
        assert!(raycast(Vec3::new(0.5, 20.5, 0.5), Vec3::NEG_Y, 5.0, floor).is_none());
    }
}
