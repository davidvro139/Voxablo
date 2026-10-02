//! Feedback around the limb rule: hit-stop on the blows that matter, and a ground telegraph
//! for every committed windup so stepping out of it is a decision the player can read.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;

use crate::combat::ARC_DOT;
use crate::Enemy;

/// Freeze lengths in real seconds. Beam ticks never stop the game; only these events do.
pub const STOP_SEVER: f32 = 0.07;
pub const STOP_STAGGER: f32 = 0.05;
pub const STOP_DOWN: f32 = 0.11;
pub const STOP_PLAYER_HURT: f32 = 0.06;
/// Game speed during a stop. Not zero, so systems that divide by dt stay finite.
const STOP_SPEED: f32 = 0.05;

/// Pending freeze. Overlapping kicks take the longest instead of adding up,
/// so a meteor through a crowd is one stop, not five.
#[derive(Resource, Default)]
pub struct HitStop {
    left: f32,
}

impl HitStop {
    pub fn kick(&mut self, seconds: f32) {
        self.left = self.left.max(seconds);
    }
}

pub fn run_hit_stop(real: Res<Time<Real>>, mut virt: ResMut<Time<Virtual>>, mut stop: ResMut<HitStop>) {
    if stop.left > 0.0 {
        stop.left = (stop.left - real.delta_seconds()).max(0.0);
        virt.set_relative_speed(STOP_SPEED);
    } else if virt.relative_speed() != 1.0 {
        virt.set_relative_speed(1.0);
    }
}

/// One of four decals under each enemy: the faint full reach and the filling part,
/// for the melee arc and for the arrow lane.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DecalKind {
    /// A melee swing's arc.
    Arc,
    /// An arrow's lane.
    Lane,
    /// A volatile body's burst radius.
    Burst,
}

#[derive(Component)]
pub struct TelegraphDecal {
    kind: DecalKind,
    fill: bool,
}

#[derive(Resource)]
pub struct TelegraphAssets {
    arc: Handle<Mesh>,
    lane: Handle<Mesh>,
    disc: Handle<Mesh>,
    melee_zone: Handle<StandardMaterial>,
    melee_fill: Handle<StandardMaterial>,
    ranged_zone: Handle<StandardMaterial>,
    ranged_fill: Handle<StandardMaterial>,
    burst_zone: Handle<StandardMaterial>,
    burst_fill: Handle<StandardMaterial>,
}

impl TelegraphAssets {
    fn parts(&self, kind: DecalKind, fill: bool) -> (Handle<Mesh>, Handle<StandardMaterial>) {
        let (mesh, zone, filled) = match kind {
            DecalKind::Arc => (&self.arc, &self.melee_zone, &self.melee_fill),
            DecalKind::Lane => (&self.lane, &self.ranged_zone, &self.ranged_fill),
            DecalKind::Burst => (&self.disc, &self.burst_zone, &self.burst_fill),
        };
        (mesh.clone(), if fill { filled.clone() } else { zone.clone() })
    }
}

/// Unit disc for a burst radius.
fn disc_mesh() -> Mesh {
    const STEPS: u32 = 40;
    let mut positions = vec![[0.0, 0.0, 0.0]];
    for i in 0..=STEPS {
        let angle = std::f32::consts::TAU * i as f32 / STEPS as f32;
        positions.push([angle.cos(), 0.0, angle.sin()]);
    }
    let indices = (0..STEPS).flat_map(|i| [0, i + 2, i + 1]).collect();
    flat_mesh(positions, indices)
}

fn decal_material(color: Color) -> StandardMaterial {
    StandardMaterial {
        base_color: color,
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

/// Unit-radius sector facing -Z, as wide as the arc a committed swing can still connect in.
fn arc_mesh() -> Mesh {
    const STEPS: u32 = 24;
    let half = ARC_DOT.acos();
    let mut positions = vec![[0.0, 0.0, 0.0]];
    for i in 0..=STEPS {
        let angle = -half + 2.0 * half * i as f32 / STEPS as f32;
        positions.push([-angle.sin(), 0.0, -angle.cos()]);
    }
    let mut indices = Vec::new();
    for i in 0..STEPS {
        indices.extend_from_slice(&[0, i + 2, i + 1]);
    }
    flat_mesh(positions, indices)
}

/// Unit-length strip facing -Z, scaled along Z to the arrow's range.
fn lane_mesh() -> Mesh {
    const HALF_WIDTH: f32 = 0.22;
    let positions = vec![[-HALF_WIDTH, 0.0, 0.0], [HALF_WIDTH, 0.0, 0.0], [HALF_WIDTH, 0.0, -1.0], [-HALF_WIDTH, 0.0, -1.0]];
    flat_mesh(positions, vec![0, 2, 1, 0, 3, 2])
}

fn flat_mesh(positions: Vec<[f32; 3]>, indices: Vec<u32>) -> Mesh {
    let count = positions.len();
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; count])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0]; count])
        .with_inserted_indices(Indices::U32(indices))
}

pub fn setup_telegraphs(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(TelegraphAssets {
        arc: meshes.add(arc_mesh()),
        lane: meshes.add(lane_mesh()),
        disc: meshes.add(disc_mesh()),
        melee_zone: materials.add(decal_material(Color::rgba(1.0, 0.18, 0.06, 0.14))),
        melee_fill: materials.add(decal_material(Color::rgba(1.0, 0.3, 0.1, 0.42))),
        ranged_zone: materials.add(decal_material(Color::rgba(1.0, 0.7, 0.15, 0.12))),
        ranged_fill: materials.add(decal_material(Color::rgba(1.0, 0.78, 0.25, 0.4))),
        burst_zone: materials.add(decal_material(Color::rgba(1.0, 0.45, 0.05, 0.18))),
        burst_fill: materials.add(decal_material(Color::rgba(1.0, 0.85, 0.3, 0.45))),
    });
}

pub fn attach_telegraphs(mut commands: Commands, assets: Res<TelegraphAssets>, added: Query<Entity, Added<Enemy>>) {
    for entity in &added {
        commands.entity(entity).with_children(|parent| {
            for kind in [DecalKind::Arc, DecalKind::Lane, DecalKind::Burst] {
                for fill in [false, true] {
                    let (mesh, material) = assets.parts(kind, fill);
                    // The fill sits a hair above the zone so the two never fight for depth.
                    let lift = if fill { 0.05 } else { 0.04 };
                    parent.spawn((
                        TelegraphDecal { kind, fill },
                        NotShadowCaster,
                        PbrBundle {
                            mesh,
                            material,
                            transform: Transform::from_xyz(0.0, lift, 0.0),
                            visibility: Visibility::Hidden,
                            ..default()
                        },
                    ));
                }
            }
        });
    }
}

/// The enemy root already faces the locked swing direction during a windup,
/// so the decals only need their reach and fill.
pub fn update_telegraphs(
    enemies: Query<(&Enemy, &Children, &Transform)>,
    mut decals: Query<(&TelegraphDecal, &mut Transform, &mut Visibility), Without<Enemy>>,
) {
    for (enemy, children, root) in &enemies {
        let swing = enemy.telegraph();
        let burst = enemy.burst_telegraph();
        // A wreck's root is squashed flat; undo that so the burst ring keeps its true size.
        let unsquash = Vec3::new(1.0 / root.scale.x.max(1.0e-3), 1.0, 1.0 / root.scale.z.max(1.0e-3));
        for &child in children.iter() {
            let Ok((decal, mut transform, mut visibility)) = decals.get_mut(child) else { continue };
            let shown = match decal.kind {
                DecalKind::Arc | DecalKind::Lane => swing
                    .filter(|shown| shown.ranged == (decal.kind == DecalKind::Lane))
                    .map(|shown| (shown.range, shown.fill)),
                DecalKind::Burst => burst,
            };
            match shown {
                Some((range, fill)) => {
                    let reach = if decal.fill { range * fill.max(0.02) } else { range };
                    transform.scale = match decal.kind {
                        DecalKind::Lane => Vec3::new(1.0, 1.0, reach),
                        DecalKind::Arc => Vec3::new(reach, 1.0, reach),
                        DecalKind::Burst => Vec3::new(reach, 1.0, reach) * unsquash,
                    };
                    *visibility = Visibility::Inherited;
                }
                None => *visibility = Visibility::Hidden,
            }
        }
    }
}
