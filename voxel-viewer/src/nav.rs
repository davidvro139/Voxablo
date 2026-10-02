//! Navigation over the destructible voxel world.
//!
//! Nodes are standable floors on a 0.5 m grid, up to four per column so a house's storeys and a
//! bridge deck over a ravine stay separate. Links are one-way and found by walking a probe the
//! way `player::walk_step` does: drops of any height, climbs of one step at a time.
//! One Dijkstra fill from the player (a "Dijkstra map") serves every enemy; each just steps to
//! the neighbour with the lowest value. Blasting a wall rebuilds the cells under it, so a new
//! hole is a new route on the next fill. The flee map is Brogue's: the chase map scaled by -1.2
//! and relaxed again, so fleeing bodies run for open ground instead of into dead ends.

use bevy::prelude::*;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::player::VOXEL_SIZE;

/// Voxels per cell side: 0.5 m.
pub const CELL: i32 = 5;
pub const CELL_M: f32 = CELL as f32 * VOXEL_SIZE;
const LEVELS: usize = 4;
/// Air above a floor for an enemy to stand: 1.7 m.
const HEADROOM: i32 = 17;
/// Voxels climbed per step: 0.3 m, the same as the walker.
const STEP: i32 = 3;
/// Feet this far from a floor still count as standing on it.
const FLOOR_SNAP: i32 = 4;
const NO_FLOOR: i16 = -1;
const NO_LINK: u8 = u8::MAX;
const DIRS: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
const OPPOSITE: [usize; 8] = [1, 0, 3, 2, 7, 6, 5, 4];
/// The two straight moves a diagonal cuts between. Both must be open, so no corner-cutting through walls.
const DIAGONAL_SIDES: [(usize, usize); 4] = [(0, 2), (0, 3), (1, 2), (1, 3)];
/// Climbing costs a little extra, so stairs are taken only when they pay.
const CLIMB_COST_PER_VOXEL: f32 = 0.05;
/// Brogue's flee coefficient.
const FLEE_SCALE: f32 = -1.2;
/// A floor with solid this close overhead is under a roof: shelter for a body falling back.
const COVER_REACH: i32 = 30;

pub struct NavGrid {
    cells_x: i32,
    cells_z: i32,
    height: i32,
    floors: Vec<[i16; LEVELS]>,
    /// Per floor: something solid overhead within `COVER_REACH`.
    covered: Vec<[bool; LEVELS]>,
    links: Vec<[u8; 8]>,
}

#[derive(Copy, Clone, PartialEq)]
struct Visit {
    cost: f32,
    node: usize,
}

impl Eq for Visit {}

impl Ord for Visit {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.total_cmp(&self.cost).then_with(|| self.node.cmp(&other.node))
    }
}

impl PartialOrd for Visit {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl NavGrid {
    /// `height` is in voxels; everything at or above it is open air.
    pub fn build(cells_x: i32, cells_z: i32, height: i32, is_solid: &impl Fn(IVec3) -> bool) -> Self {
        let cells = (cells_x * cells_z) as usize;
        let mut grid = Self {
            cells_x,
            cells_z,
            height,
            floors: vec![[NO_FLOOR; LEVELS]; cells],
            covered: vec![[false; LEVELS]; cells],
            links: vec![[NO_LINK; 8]; cells * LEVELS],
        };
        grid.rebuild(IVec2::ZERO, IVec2::new(cells_x - 1, cells_z - 1), is_solid);
        grid
    }

    pub fn node_count(&self) -> usize {
        self.links.len()
    }

    /// Recompute floors in the cell box, then the links in and around it.
    pub fn rebuild(&mut self, min: IVec2, max: IVec2, is_solid: &impl Fn(IVec3) -> bool) {
        let lo = min.max(IVec2::ZERO);
        let hi = max.min(IVec2::new(self.cells_x - 1, self.cells_z - 1));
        for cz in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                let (x, z) = center_voxel(cx, cz);
                let (floors, covered) = self.scan_column(x, z, is_solid);
                let cell = self.cell(cx, cz);
                self.floors[cell] = floors;
                self.covered[cell] = covered;
            }
        }
        // Neighbours just outside the box link into it, so they are relinked too.
        let lo = (lo - IVec2::ONE).max(IVec2::ZERO);
        let hi = (hi + IVec2::ONE).min(IVec2::new(self.cells_x - 1, self.cells_z - 1));
        for cz in lo.y..=hi.y {
            for cx in lo.x..=hi.x {
                self.link_cell(cx, cz, is_solid);
            }
        }
    }

    /// Cells covering a box of voxels, for rebuilding after an edit.
    pub fn cells_for_voxels(min: IVec3, max: IVec3) -> (IVec2, IVec2) {
        (IVec2::new(min.x.div_euclid(CELL), min.z.div_euclid(CELL)), IVec2::new(max.x.div_euclid(CELL), max.z.div_euclid(CELL)))
    }

    fn cell(&self, cx: i32, cz: i32) -> usize {
        (cz * self.cells_x + cx) as usize
    }

    fn in_bounds(&self, cx: i32, cz: i32) -> bool {
        cx >= 0 && cz >= 0 && cx < self.cells_x && cz < self.cells_z
    }

    fn cell_xz(&self, cell: usize) -> (i32, i32) {
        let cell = cell as i32;
        (cell % self.cells_x, cell / self.cells_x)
    }

    fn clear_above(&self, x: i32, s: i32, z: i32, is_solid: &impl Fn(IVec3) -> bool) -> bool {
        (0..HEADROOM).all(|k| s + k >= self.height || !is_solid(IVec3::new(x, s + k, z)))
    }

    /// The lowest floors with headroom in a column, ascending, and whether each is under a roof.
    fn scan_column(&self, x: i32, z: i32, is_solid: &impl Fn(IVec3) -> bool) -> ([i16; LEVELS], [bool; LEVELS]) {
        let mut found = Vec::new();
        let mut air = HEADROOM;
        // Nothing solid seen yet from the top: open sky.
        let mut sky = true;
        for y in (0..self.height).rev() {
            if is_solid(IVec3::new(x, y, z)) {
                if air >= HEADROOM {
                    found.push(((y + 1) as i16, !sky && air <= COVER_REACH));
                }
                air = 0;
                sky = false;
            } else {
                air += 1;
            }
        }
        found.reverse();
        let mut floors = [NO_FLOOR; LEVELS];
        let mut covered = [false; LEVELS];
        for (level, (floor, roof)) in found.into_iter().take(LEVELS).enumerate() {
            floors[level] = floor;
            covered[level] = roof;
        }
        (floors, covered)
    }

    /// Where the walker ends up in a column coming from feet height `from`: the highest floor within
    /// a step up that has headroom, falling as far as it takes. Mirrors `player::stand_height`.
    fn stand(&self, x: i32, z: i32, from: i32, is_solid: &impl Fn(IVec3) -> bool) -> Option<i32> {
        let top = (from + STEP).min(self.height);
        (1..=top).rev().find(|&s| is_solid(IVec3::new(x, s - 1, z)) && self.clear_above(x, s, z, is_solid))
    }

    /// Level index in a cell whose floor is exactly `y`.
    fn level_of(&self, cell: usize, y: i32) -> Option<usize> {
        self.floors[cell].iter().position(|&floor| floor != NO_FLOOR && floor as i32 == y)
    }

    fn link_cell(&mut self, cx: i32, cz: i32, is_solid: &impl Fn(IVec3) -> bool) {
        let cell = self.cell(cx, cz);
        let (ax, az) = center_voxel(cx, cz);
        for level in 0..LEVELS {
            let node = cell * LEVELS + level;
            self.links[node] = [NO_LINK; 8];
            let floor = self.floors[cell][level];
            if floor == NO_FLOOR {
                continue;
            }
            let from = floor as i32;
            for (dir, &(dx, dz)) in DIRS.iter().enumerate() {
                let (nx, nz) = (cx + dx, cz + dz);
                if !self.in_bounds(nx, nz) {
                    continue;
                }
                if dir >= 4 {
                    let (a, b) = DIAGONAL_SIDES[dir - 4];
                    if self.links[node][a] == NO_LINK || self.links[node][b] == NO_LINK {
                        continue;
                    }
                }
                // Every voxel column on the way, so a 20 cm wall between two centres is not stepped over.
                let mut feet = Some(from);
                for i in 1..=CELL {
                    feet = feet.and_then(|y| self.stand(ax + dx * i, az + dz * i, y, is_solid));
                }
                let Some(end) = feet else { continue };
                if let Some(target) = self.level_of(self.cell(nx, nz), end) {
                    self.links[node][dir] = target as u8;
                }
            }
        }
    }

    fn link_cost(&self, from: usize, dir: usize, to: usize) -> f32 {
        let base = if dir >= 4 { CELL_M * std::f32::consts::SQRT_2 } else { CELL_M };
        let rise = self.floor_of(to) - self.floor_of(from);
        base + rise.max(0) as f32 * CLIMB_COST_PER_VOXEL
    }

    fn floor_of(&self, node: usize) -> i32 {
        self.floors[node / LEVELS][node % LEVELS] as i32
    }

    fn neighbour(&self, node: usize, dir: usize) -> Option<usize> {
        let level = self.links[node][dir];
        if level == NO_LINK {
            return None;
        }
        let (cx, cz) = self.cell_xz(node / LEVELS);
        let (dx, dz) = DIRS[dir];
        Some(self.cell(cx + dx, cz + dz) * LEVELS + level as usize)
    }

    /// The floor under a body's feet, if it is standing on one.
    pub fn node_at(&self, pos: Vec3) -> Option<usize> {
        let cx = (pos.x / CELL_M).floor() as i32;
        let cz = (pos.z / CELL_M).floor() as i32;
        if !self.in_bounds(cx, cz) {
            return None;
        }
        let cell = self.cell(cx, cz);
        let feet = (pos.y / VOXEL_SIZE).round() as i32;
        self.floors[cell]
            .iter()
            .enumerate()
            .filter(|(_, &floor)| floor != NO_FLOOR && (floor as i32 - feet).abs() <= FLOOR_SNAP)
            .min_by_key(|(_, &floor)| (floor as i32 - feet).abs())
            .map(|(level, _)| cell * LEVELS + level)
    }

    fn node_center(&self, node: usize) -> Vec3 {
        let (cx, cz) = self.cell_xz(node / LEVELS);
        let (x, z) = center_voxel(cx, cz);
        Vec3::new((x as f32 + 0.5) * VOXEL_SIZE, self.floor_of(node) as f32 * VOXEL_SIZE, (z as f32 + 0.5) * VOXEL_SIZE)
    }

    /// Metres from every node to `goal`, following links in their walking direction.
    pub fn chase_map(&self, goal: usize) -> Vec<f32> {
        let mut dist = vec![f32::INFINITY; self.node_count()];
        dist[goal] = 0.0;
        self.relax(&mut dist, vec![Visit { cost: 0.0, node: goal }]);
        dist
    }

    /// Metres from every node to the nearest floor under a roof.
    pub fn shelter_map(&self) -> Vec<f32> {
        let mut dist = vec![f32::INFINITY; self.node_count()];
        let mut seeds = Vec::new();
        for (cell, roofs) in self.covered.iter().enumerate() {
            for (level, &roof) in roofs.iter().enumerate() {
                if roof && self.floors[cell][level] != NO_FLOOR {
                    let node = cell * LEVELS + level;
                    dist[node] = 0.0;
                    seeds.push(Visit { cost: 0.0, node });
                }
            }
        }
        self.relax(&mut dist, seeds);
        dist
    }

    /// The floor in the column under `pos` closest to `pos.y`, standing on it or not.
    /// For goals picked in the air, such as a point on a ring around the player.
    pub fn floor_near(&self, pos: Vec3) -> Option<(usize, Vec3)> {
        let cx = (pos.x / CELL_M).floor() as i32;
        let cz = (pos.z / CELL_M).floor() as i32;
        if !self.in_bounds(cx, cz) {
            return None;
        }
        let cell = self.cell(cx, cz);
        let want = pos.y / VOXEL_SIZE;
        let level = self.floors[cell]
            .iter()
            .enumerate()
            .filter(|(_, &floor)| floor != NO_FLOOR)
            .min_by(|a, b| (*a.1 as f32 - want).abs().total_cmp(&(*b.1 as f32 - want).abs()))
            .map(|(level, _)| level)?;
        let node = cell * LEVELS + level;
        Some((node, self.node_center(node)))
    }

    /// Brogue's safety map: low values are far from the threat and well connected.
    pub fn flee_map(&self, chase: &[f32]) -> Vec<f32> {
        let mut dist: Vec<f32> = chase.iter().map(|&d| if d.is_finite() { d * FLEE_SCALE } else { f32::INFINITY }).collect();
        let seeds = dist
            .iter()
            .enumerate()
            .filter(|(_, d)| d.is_finite())
            .map(|(node, &cost)| Visit { cost, node })
            .collect();
        self.relax(&mut dist, seeds);
        dist
    }

    /// Dijkstra over reversed links: a value is the cost of walking from that node to a seed.
    fn relax(&self, dist: &mut [f32], seeds: Vec<Visit>) {
        let mut heap = BinaryHeap::from(seeds);
        while let Some(Visit { cost, node }) = heap.pop() {
            if cost > dist[node] {
                continue;
            }
            let (cx, cz) = self.cell_xz(node / LEVELS);
            for (dir, &(dx, dz)) in DIRS.iter().enumerate() {
                let (px, pz) = (cx + dx, cz + dz);
                if !self.in_bounds(px, pz) {
                    continue;
                }
                // A neighbour that walks back to this node in the opposite direction.
                let back = OPPOSITE[dir];
                let base = self.cell(px, pz) * LEVELS;
                for level in 0..LEVELS {
                    let prev = base + level;
                    if self.neighbour(prev, back) != Some(node) {
                        continue;
                    }
                    let next = cost + self.link_cost(prev, back, node);
                    if next < dist[prev] {
                        dist[prev] = next;
                        heap.push(Visit { cost: next, node: prev });
                    }
                }
            }
        }
    }

    /// Value of a map under a body, if it stands on a reachable floor.
    pub fn value_at(&self, map: &[f32], pos: Vec3) -> Option<f32> {
        let node = self.node_at(pos)?;
        map[node].is_finite().then_some(map[node])
    }

    /// Flat unit direction to the neighbour that lowers the map the most. None at a minimum,
    /// off the grid, or where the map has no route.
    pub fn next_step(&self, map: &[f32], pos: Vec3) -> Option<Vec3> {
        let node = self.node_at(pos)?;
        let here = map[node];
        if !here.is_finite() {
            return None;
        }
        let best = (0..8)
            .filter_map(|dir| self.neighbour(node, dir))
            .filter(|&next| map[next] < here - 1.0e-4)
            .min_by(|a, b| map[*a].total_cmp(&map[*b]))?;
        let to = self.node_center(best) - pos;
        let flat = Vec3::new(to.x, 0.0, to.z);
        Some(if flat.length_squared() > 1.0e-6 { flat.normalize() } else { (self.node_center(best) - self.node_center(node)).normalize_or_zero() })
    }
}

fn center_voxel(cx: i32, cz: i32) -> (i32, i32) {
    (cx * CELL + CELL / 2, cz * CELL + CELL / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 10 x 10 cells (5 m square), 3 m tall, a ground floor at y = 1.
    struct World {
        extra: Vec<Box<dyn Fn(IVec3) -> bool>>,
        holes: Vec<Box<dyn Fn(IVec3) -> bool>>,
    }

    impl World {
        fn flat() -> Self {
            Self { extra: Vec::new(), holes: Vec::new() }
        }

        fn solid(&self, v: IVec3) -> bool {
            if self.holes.iter().any(|hole| hole(v)) {
                return false;
            }
            v.y == 0 || self.extra.iter().any(|shape| shape(v))
        }

        fn grid(&self) -> NavGrid {
            NavGrid::build(10, 10, 30, &|v| self.solid(v))
        }
    }

    fn at(cx: i32, cz: i32, floor: i32) -> Vec3 {
        Vec3::new((cx as f32 + 0.5) * CELL_M, floor as f32 * VOXEL_SIZE, (cz as f32 + 0.5) * CELL_M)
    }

    /// A 2.5 m wall along x = 25 voxels (the cell boundary between cx 4 and 5), with an optional door.
    fn wall(door: bool) -> Box<dyn Fn(IVec3) -> bool> {
        Box::new(move |v: IVec3| {
            let in_door = door && (40..45).contains(&v.z);
            (25..27).contains(&v.x) && (1..26).contains(&v.y) && !in_door
        })
    }

    #[test]
    fn open_ground_points_straight_at_the_goal() {
        let grid = World::flat().grid();
        let goal = grid.node_at(at(9, 5, 1)).unwrap();
        let map = grid.chase_map(goal);
        let step = grid.next_step(&map, at(1, 5, 1)).unwrap();
        assert!(step.x > 0.9, "{step}");
        let far = grid.value_at(&map, at(1, 5, 1)).unwrap();
        assert!((far - 4.0).abs() < 0.01, "{far}");
    }

    #[test]
    fn a_wall_with_a_door_routes_through_the_door() {
        let mut world = World::flat();
        world.extra.push(wall(true));
        let grid = world.grid();
        let map = grid.chase_map(grid.node_at(at(8, 1, 1)).unwrap());
        let start = at(1, 1, 1);
        // Straight across is 3.5 m; the door at z 8..9 makes it much longer.
        assert!(grid.value_at(&map, start).unwrap() > 6.0);
        let step = grid.next_step(&map, start).unwrap();
        assert!(step.z > 0.5, "should head for the door, got {step}");
    }

    #[test]
    fn a_solid_wall_has_no_route_until_it_is_blasted() {
        let mut world = World::flat();
        world.extra.push(wall(false));
        let mut grid = world.grid();
        let goal = grid.node_at(at(8, 1, 1)).unwrap();
        assert!(grid.value_at(&grid.chase_map(goal), at(1, 1, 1)).is_none());

        world.holes.push(Box::new(|v: IVec3| (25..27).contains(&v.x) && (5..15).contains(&v.z) && v.y > 0));
        let (min, max) = NavGrid::cells_for_voxels(IVec3::new(25, 0, 5), IVec3::new(26, 29, 14));
        grid.rebuild(min, max, &|v| world.solid(v));
        let map = grid.chase_map(goal);
        assert!(grid.value_at(&map, at(1, 1, 1)).is_some());
    }

    #[test]
    fn a_ledge_can_be_dropped_from_but_not_climbed() {
        let mut world = World::flat();
        // A 1 m platform over the east half.
        world.extra.push(Box::new(|v: IVec3| v.x >= 25 && v.y <= 10));
        let grid = world.grid();
        let low = at(2, 5, 1);
        let high = at(7, 5, 11);
        let to_low = grid.chase_map(grid.node_at(low).unwrap());
        assert!(grid.value_at(&to_low, high).is_some());
        let to_high = grid.chase_map(grid.node_at(high).unwrap());
        assert!(grid.value_at(&to_high, low).is_none());
    }

    #[test]
    fn stairs_are_climbed_one_step_at_a_time() {
        let mut world = World::flat();
        // A 2 voxel rise every cell eastward: walkable.
        world.extra.push(Box::new(|v: IVec3| v.y <= (v.x / CELL) * 2));
        let grid = world.grid();
        let top = grid.chase_map(grid.node_at(at(9, 5, 19)).unwrap());
        assert!(grid.value_at(&top, at(0, 5, 1)).is_some());
    }

    #[test]
    fn upper_floors_are_separate_nodes() {
        let mut world = World::flat();
        // A slab at y 20 over everything: a second storey with headroom below.
        world.extra.push(Box::new(|v: IVec3| v.y == 20));
        let grid = world.grid();
        let ground = grid.node_at(at(3, 3, 1)).unwrap();
        let upstairs = grid.node_at(at(3, 3, 21)).unwrap();
        assert_ne!(ground, upstairs);
        // No stairs, so the floors do not connect.
        assert!(grid.chase_map(ground)[upstairs].is_infinite());
    }

    #[test]
    fn the_shelter_map_leads_under_a_roof() {
        let mut world = World::flat();
        // A 2 m high roof slab over the east two cells' worth of columns.
        world.extra.push(Box::new(|v: IVec3| v.y == 20 && v.x >= 40));
        let grid = world.grid();
        let shelter = grid.shelter_map();
        assert_eq!(grid.value_at(&shelter, at(9, 5, 1)), Some(0.0));
        let step = grid.next_step(&shelter, at(2, 5, 1)).unwrap();
        assert!(step.x > 0.5, "{step}");
        // The roof's top is open sky, so it is no shelter.
        assert!(grid.value_at(&shelter, at(9, 5, 21)).unwrap() > 0.0 || grid.node_at(at(9, 5, 21)).is_none());
    }

    #[test]
    fn floor_near_picks_the_storey_closest_to_the_height_asked() {
        let mut world = World::flat();
        world.extra.push(Box::new(|v: IVec3| v.y == 20));
        let grid = world.grid();
        let (_, low) = grid.floor_near(at(3, 3, 5)).unwrap();
        let (_, high) = grid.floor_near(at(3, 3, 18)).unwrap();
        assert!((low.y - 0.1).abs() < 1.0e-4, "{low}");
        assert!((high.y - 2.1).abs() < 1.0e-4, "{high}");
    }

    /// `cargo test --release -p voxel-viewer nav::tests::fill_cost -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn fill_cost_on_a_town_sized_grid() {
        let solid = |v: IVec3| v.y == 0 || (v.x % 60 < 2 && v.z % 90 > 10 && v.y < 25);
        let grid = NavGrid::build(102, 153, 96, &solid);
        let goal = grid.node_at(at(50, 70, 1)).unwrap();
        let started = std::time::Instant::now();
        for _ in 0..20 {
            let chase = grid.chase_map(goal);
            std::hint::black_box(grid.flee_map(&chase));
        }
        println!("chase + flee fill: {:.2} ms", started.elapsed().as_secs_f32() * 1000.0 / 20.0);
    }

    #[test]
    fn the_flee_map_leads_away_from_the_threat() {
        let grid = World::flat().grid();
        let chase = grid.chase_map(grid.node_at(at(2, 5, 1)).unwrap());
        let flee = grid.flee_map(&chase);
        let step = grid.next_step(&flee, at(4, 5, 1)).unwrap();
        assert!(step.x > 0.5, "{step}");
    }
}
