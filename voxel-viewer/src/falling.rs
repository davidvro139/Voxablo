//! Voxel groups cut loose from the ground become rigid bodies: they fall, tip and tumble, and
//! once at rest are written back into the world.

use crate::occluder_fade::FadingMaterial;
use crate::player::VOXEL_SIZE;
use crate::player_model::VoxelModel;
use crate::rigid::{collide_pair, rasterize, snap_to_grid, RigidShape, RigidState};
use crate::spells::{spawn_debris, Rng, SpellAssets};
use crate::{is_solid, material_color, to_bevy_mesh, ChunksChanged, TerrainMaterial, Voxels, REGION};
use bevy::prelude::*;
use voxel_core_ffi::{Material, PieceVoxel};

/// Bigger groups are treated as supported, which bounds the connectivity search.
const MAX_PIECE_VOXELS: u32 = 150_000;
/// Landing faster than this (m/s) starts shattering the impact side into debris.
const SHATTER_SPEED: f32 = 5.0;
const MAX_SHATTER_FRACTION: f32 = 0.5;
const MAX_DEBRIS_PER_LANDING: usize = 80;
/// After a shattering landing, recheck pieces up to this size for bits the shatter cut loose.
const RECHECK_MAX_VOXELS: usize = 20_000;
/// Settle anyway after this long, so a jittering body can't stay dynamic forever.
const MAX_AGE: f32 = 12.0;
/// Below this height a body has fallen out of the world.
const KILL_Y: f32 = -20.0;
/// Existing falling bodies inside a blast get this fraction of the spell's launch speed.
const LIVE_BLAST_PUSH: f32 = 1.25;
/// Very near misses still shove large bodies when the blast overlaps their bounding sphere.
const LIVE_BLAST_EDGE_FALLOFF: f32 = 0.25;

/// Voxels were destroyed around `center` (voxel units); check what lost its support.
/// `push` is the speed (m/s) given to freed pieces at their point nearest the blast.
#[derive(Event)]
pub struct Blasted {
    pub center: Vec3,
    pub radius: f32,
    pub push: f32,
}

#[derive(Component)]
pub struct FallingPiece {
    shape: RigidShape,
    materials: Vec<Material>,
    state: RigidState,
    age: f32,
}

pub struct FallingHit {
    pub point: Vec3,
    pub distance: f32,
}

impl FallingPiece {
    pub fn center(&self) -> Vec3 {
        self.state.pos
    }

    pub fn speed(&self) -> f32 {
        self.state.vel.length()
    }

    pub fn velocity(&self) -> Vec3 {
        self.state.vel
    }

    pub fn radius(&self) -> f32 {
        self.shape.radius()
    }

    pub fn voxel_count(&self) -> usize {
        self.shape.voxels.len()
    }

    /// Closest sampled point of the piece's surface to `point`.
    pub fn nearest_surface_point(&self, point: Vec3) -> Vec3 {
        self.shape.nearest_surface_point(&self.state, point)
    }
}

fn piece_mesh(shape: &RigidShape, materials: &[Material]) -> Mesh {
    let size = shape.voxels.iter().fold(IVec3::ZERO, |acc, v| acc.max(*v)) + IVec3::ONE;
    let mut model = VoxelModel::new(size);
    for (v, m) in shape.voxels.iter().zip(materials) {
        model.fill((*v).into(), (*v + IVec3::ONE).into(), material_color(*m, [0.0, 0.0, 0.0]));
    }
    let mut arrays = model.mesh_from_corner(VOXEL_SIZE);
    for p in &mut arrays.positions {
        *p = (Vec3::from(*p) - shape.com).into();
    }
    to_bevy_mesh(arrays.positions, arrays.normals, arrays.colors, arrays.indices)
}

pub fn detect_detached(
    mut commands: Commands,
    mut blasts: EventReader<Blasted>,
    voxels: Res<Voxels>,
    mut meshes: ResMut<Assets<Mesh>>,
    terrain: Res<TerrainMaterial>,
    mut changed: EventWriter<ChunksChanged>,
) {
    for blast in blasts.read() {
        changed.send(ChunksChanged::around_voxel(blast.center, blast.radius));
        let c = blast.center;
        let pieces = voxels.with(|w| w.extract_detached(REGION, c.x, c.y, c.z, blast.radius, MAX_PIECE_VOXELS));

        for piece in pieces {
            let min = piece.iter().fold(IVec3::MAX, |acc, v| acc.min(IVec3::new(v.x, v.y, v.z)));
            let max = piece.iter().fold(IVec3::MIN, |acc, v| acc.max(IVec3::new(v.x, v.y, v.z)));
            changed.send(ChunksChanged::voxel_box(min, max));

            let shape = RigidShape::new(piece.iter().map(|v| IVec3::new(v.x, v.y, v.z) - min).collect());
            let materials: Vec<Material> = piece.iter().map(|v| v.material).collect();
            let mut state = RigidState::new(min.as_vec3() * VOXEL_SIZE + shape.com);

            if blast.push > 0.0 {
                // Shove the piece away from the blast at its nearest point, so it tips away.
                let blast_pos = blast.center * VOXEL_SIZE;
                let nearest = piece
                    .iter()
                    .map(|v| (IVec3::new(v.x, v.y, v.z).as_vec3() + 0.5) * VOXEL_SIZE)
                    .min_by(|a, b| a.distance_squared(blast_pos).total_cmp(&b.distance_squared(blast_pos)))
                    .unwrap_or(state.pos);
                let dir = (nearest - blast_pos).try_normalize().unwrap_or(Vec3::Y);
                state.push_point(&shape, nearest - state.pos, dir, blast.push);
            }

            commands.spawn((
                MaterialMeshBundle::<FadingMaterial> {
                    mesh: meshes.add(piece_mesh(&shape, &materials)),
                    material: terrain.0.clone(),
                    transform: Transform::from_translation(state.pos),
                    ..default()
                },
                FallingPiece { shape, materials, state, age: 0.0 },
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_falling_pieces(
    mut commands: Commands,
    time: Res<Time>,
    voxels: Res<Voxels>,
    assets: Res<SpellAssets>,
    mut rng: ResMut<Rng>,
    mut changed: EventWriter<ChunksChanged>,
    mut blast_events: ParamSet<(EventReader<Blasted>, EventWriter<Blasted>)>,
    mut pieces: Query<(Entity, &mut Transform, &mut FallingPiece)>,
) {
    for blast in blast_events.p0().read() {
        if blast.push <= 0.0 {
            continue;
        }
        for (_, _, mut piece) in &mut pieces {
            apply_blast_to_piece(blast, &mut piece);
        }
    }

    let dt = time.delta_seconds().min(0.05);
    // Advance all pieces together, then resolve mutual contacts before deciding which settle.
    // Small shared steps reduce tunnelling between fast moving fragments.
    let steps = (dt / (1.0 / 240.0)).ceil().max(1.0) as u32;
    let h = dt / steps as f32;
    for _ in 0..steps {
        for (_, _, mut piece) in &mut pieces {
            let piece = &mut *piece;
            voxels.with(|w| piece.state.step(&piece.shape, h, &|v| is_solid(w, v)));
        }
        let mut pairs = pieces.iter_combinations_mut();
        while let Some([(_, _, mut a), (_, _, mut b)]) = pairs.fetch_next() {
            let a = &mut *a;
            let b = &mut *b;
            collide_pair(&a.shape, &mut a.state, &b.shape, &mut b.state);
        }
    }
    for (entity, mut transform, mut piece) in &mut pieces {
        let piece = &mut *piece;
        piece.age += dt;
        transform.translation = piece.state.pos;
        transform.rotation = piece.state.rot;

        if piece.state.pos.y < KILL_Y {
            commands.entity(entity).despawn();
        } else if piece.state.at_rest() || piece.age > MAX_AGE {
            settle(&mut commands, &voxels, &assets, &mut rng, &mut changed, &mut blast_events.p1(), piece);
            commands.entity(entity).despawn();
        }
    }
}

fn apply_blast_to_piece(blast: &Blasted, piece: &mut FallingPiece) {
    let center = blast.center * VOXEL_SIZE;
    let radius = blast.radius * VOXEL_SIZE;
    let nearest = piece.shape.nearest_surface_point(&piece.state, center);
    let distance = nearest.distance(center);
    if distance > radius + piece.shape.radius() {
        return;
    }

    let blast_reach = radius + piece.shape.radius() * LIVE_BLAST_EDGE_FALLOFF;
    let falloff = (1.0 - distance / blast_reach.max(VOXEL_SIZE)).clamp(0.0, 1.0);
    if falloff <= 0.0 {
        return;
    }

    let dir = (nearest - center).try_normalize().unwrap_or(Vec3::Y);
    let speed = blast.push * LIVE_BLAST_PUSH * falloff;
    piece.state.push_point(&piece.shape, nearest - piece.state.pos, dir, speed);
}

fn push_piece_at(piece: &mut FallingPiece, point: Vec3, dir: Vec3, speed: f32) {
    let FallingPiece { shape, state, .. } = piece;
    let nearest = shape.nearest_surface_point(state, point);
    state.push_point(shape, nearest - state.pos, dir, speed);
}

pub fn hit_falling_piece(
    pieces: &mut Query<&mut FallingPiece>,
    from: Vec3,
    to: Vec3,
    push_speed: f32,
) -> Option<FallingHit> {
    let delta = to - from;
    let length = delta.length();
    if length <= f32::EPSILON {
        return None;
    }
    let dir = delta / length;
    let mut best: Option<(usize, f32, Vec3)> = None;
    for (i, piece) in pieces.iter_mut().enumerate() {
        if let Some((t, point)) = piece.shape.segment_hit(&piece.state, from, to) {
            if best.map_or(true, |(_, best_t, _)| t < best_t) {
                best = Some((i, t, point));
            }
        }
    }

    let (hit_i, hit_t, point) = best?;
    for (i, mut piece) in pieces.iter_mut().enumerate() {
        if i == hit_i {
            if push_speed > 0.0 {
                push_piece_at(&mut piece, point, dir, push_speed);
            }
            break;
        }
    }
    Some(FallingHit { point, distance: length * hit_t })
}

#[allow(clippy::too_many_arguments)]
fn settle(
    commands: &mut Commands,
    voxels: &Voxels,
    assets: &SpellAssets,
    rng: &mut Rng,
    changed: &mut EventWriter<ChunksChanged>,
    blasts: &mut EventWriter<Blasted>,
    piece: &FallingPiece,
) {
    let resting = snap_to_grid(&piece.shape, &piece.state);
    let cells = rasterize(&piece.shape, &resting);
    if cells.is_empty() {
        return;
    }
    let lo = cells.iter().fold(IVec3::MAX, |acc, (c, _)| acc.min(*c));
    let hi = cells.iter().fold(IVec3::MIN, |acc, (c, _)| acc.max(*c));

    let fraction = ((piece.state.max_impact - SHATTER_SPEED) / 12.0).clamp(0.0, MAX_SHATTER_FRACTION);
    let height = (hi.y - lo.y + 1) as f32;
    let mut kept = Vec::with_capacity(cells.len());
    let mut shattered = Vec::new();
    voxels.with(|w| {
        for &(cell, i) in &cells {
            if is_solid(w, cell) {
                continue;
            }
            let voxel = PieceVoxel { x: cell.x, y: cell.y, z: cell.z, material: piece.materials[i] };
            // The impact side takes the damage: the lowest voxels break up to twice the average rate.
            let low = 1.0 - (cell.y - lo.y) as f32 / height;
            if rng.next_f32() < fraction * 2.0 * low {
                shattered.push(voxel);
            } else {
                kept.push(voxel);
            }
        }
        w.set_voxels(REGION, &kept);
    });
    changed.send(ChunksChanged::voxel_box(lo, hi));

    let stride = (shattered.len() / MAX_DEBRIS_PER_LANDING).max(1);
    for v in shattered.iter().step_by(stride) {
        let pos = (IVec3::new(v.x, v.y, v.z).as_vec3() + 0.5) * VOXEL_SIZE;
        let spray = Vec3::new(rng.range(-1.0, 1.0), 0.0, rng.range(-1.0, 1.0)).normalize_or_zero();
        let vel = spray * rng.range(1.0, 3.5) + Vec3::Y * rng.range(0.5, 1.0) * piece.state.max_impact * 0.3;
        spawn_debris(commands, assets, rng, pos, vel, v.material);
    }

    if !shattered.is_empty() && cells.len() <= RECHECK_MAX_VOXELS {
        let (lo, hi) = (lo.as_vec3(), hi.as_vec3());
        blasts.send(Blasted { center: (lo + hi) * 0.5, radius: (hi - lo).length() * 0.5, push: 0.0 });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core_ffi::VoxelWorld;

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

    #[test]
    fn live_blast_pushes_falling_piece_away_from_nearest_face() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let mut piece = FallingPiece {
            state: RigidState::new(Vec3::new(1.0, 0.2, 0.0)),
            materials: vec![Material::Brick; shape.voxels.len()],
            shape,
            age: 0.0,
        };
        let blast = Blasted { center: Vec3::ZERO, radius: 12.0, push: 4.0 };

        apply_blast_to_piece(&blast, &mut piece);

        assert!(piece.state.vel.x > 0.05, "blast did not push piece away: {:?}", piece.state);
        assert!(piece.state.ang.length() > 0.1, "off-centre blast did not tip piece: {:?}", piece.state);
    }

    #[test]
    fn distant_live_blast_does_not_wake_piece() {
        let shape = RigidShape::new(block(IVec3::splat(4)));
        let mut piece = FallingPiece {
            state: RigidState::new(Vec3::ZERO),
            materials: vec![Material::Brick; shape.voxels.len()],
            shape,
            age: 0.0,
        };
        let blast = Blasted { center: Vec3::new(200.0, 0.0, 0.0), radius: 2.0, push: 4.0 };

        apply_blast_to_piece(&blast, &mut piece);

        assert_eq!(piece.state.vel, Vec3::ZERO);
        assert_eq!(piece.state.ang, Vec3::ZERO);
    }

    #[test]
    fn cut_tower_top_falls_and_is_written_back() {
        let world = VoxelWorld::new();
        world.load_region(REGION);
        let mut build = Vec::new();
        for x in 0..10 {
            for z in 0..10 {
                build.push(PieceVoxel { x, y: 0, z, material: Material::Dirt });
            }
        }
        for y in 1..=12 {
            build.push(PieceVoxel { x: 5, y, z: 5, material: Material::Brick });
        }
        world.set_voxels(REGION, &build);

        // Knock out one brick at y = 4; bricks 5..=12 lose their support.
        world.apply_damage(5.0, 4.0, 5.0, 0.5, 5.0);
        let pieces = world.extract_detached(REGION, 5.0, 4.0, 5.0, 0.5, 1000);
        assert_eq!(pieces.len(), 1);
        assert_eq!(pieces[0].len(), 8);
        assert!(pieces[0].iter().all(|v| v.x == 5 && v.z == 5 && v.material == Material::Brick));
        assert_eq!(world.get_voxel(REGION, 5, 12, 5), Material::Air);

        let min = IVec3::new(5, 5, 5);
        let shape = RigidShape::new(pieces[0].iter().map(|v| IVec3::new(v.x, v.y, v.z) - min).collect());
        let mut state = RigidState::new(min.as_vec3() * VOXEL_SIZE + shape.com);
        for _ in 0..600 {
            state.step(&shape, 1.0 / 60.0, &|v| is_solid(&world, v));
            if state.at_rest() {
                break;
            }
        }
        assert!(state.at_rest());

        // The column either stays balanced on its stub or tips over; either way every voxel must
        // be written back into empty cells on or above the ground.
        let cells = rasterize(&shape, &snap_to_grid(&shape, &state));
        assert_eq!(cells.len(), 8);
        let landed: Vec<_> = cells
            .iter()
            .map(|(c, _)| PieceVoxel { x: c.x, y: c.y, z: c.z, material: Material::Brick })
            .collect();
        assert!(landed.iter().all(|v| v.y >= 1));
        assert!(landed.iter().all(|v| world.get_voxel(REGION, v.x, v.y, v.z) == Material::Air));
        world.set_voxels(REGION, &landed);
        assert!(landed.iter().all(|v| world.get_voxel(REGION, v.x, v.y, v.z) == Material::Brick));
    }
}
