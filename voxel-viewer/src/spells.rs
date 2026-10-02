//! Voxel-breaking spells: Arcane Bolt (projectile), Disintegrate (beam), Fire Orb (lobbed grenade).
//! Positions are in metres; damage radii are in voxels, as the C++ core expects.

use crate::falling::{self, Blasted, FallingPiece};
use crate::player::{GRAVITY, VOXEL_SIZE};
use crate::raycast::{raycast, Hit};
use crate::{cursor_enemy_point, cursor_ray, is_solid, material_color, pick_voxel, Enemy, Player, Voxels, ENEMY_BODY_CENTER_Y};
use bevy::ecs::query::QueryFilter;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::collections::HashMap;
use voxel_core_ffi::{Material, VoxelWorld};

const CAST_HEIGHT: f32 = 1.3;
const CAST_FORWARD: f32 = 0.4;

const BOLT_SPEED: f32 = 28.0;
const BOLT_LIFETIME: f32 = 2.0;
const BOLT_COOLDOWN: f32 = 0.25;
const BOLT_RADIUS: f32 = 4.0;
const BOLT_ENERGY: f32 = 3.0;
const BOLT_DIRECT_PUSH: f32 = 3.0;

const BEAM_RANGE: f32 = 25.0;
const BEAM_TICK: f32 = 0.05;
const BEAM_WIDTH: f32 = 0.07;
const BEAM_RADIUS: f32 = 2.5;
const BEAM_ENERGY: f32 = 2.2;
const BEAM_DIRECT_PUSH: f32 = 0.6;

const ORB_COOLDOWN: f32 = 0.8;
const ORB_FUSE: f32 = 1.4;
const ORB_MIN_FLIGHT: f32 = 0.45;
const ORB_MAX_FLIGHT: f32 = 1.1;
const ORB_RESTITUTION: f32 = 0.35;
const ORB_FRICTION: f32 = 0.6;
const ORB_SIZE: f32 = 0.12;
const ORB_RADIUS: f32 = 14.0;
const ORB_ENERGY: f32 = 4.0;
const ORB_DIRECT_PUSH: f32 = 6.0;

const METEOR_COOLDOWN: f32 = 1.1;
const METEOR_RADIUS: f32 = 16.0;
const METEOR_ENERGY: f32 = 5.5;
const METEOR_DAMAGE: f32 = 120.0;

const NOVA_COOLDOWN: f32 = 0.75;
const NOVA_RADIUS: f32 = 9.0;
const NOVA_ENERGY: f32 = 2.0;
const NOVA_DAMAGE: f32 = 65.0;

const DEBRIS_LIFETIME: f32 = 2.5;
const DEBRIS_RESTITUTION: f32 = 0.3;
const ENEMY_HIT_RADIUS: f32 = 0.65;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Spell {
    #[default]
    ArcaneBolt,
    Disintegrate,
    FireOrb,
    Meteor,
    FrostNova,
}

const SPELLS: [(Spell, &str); 5] = [
    (Spell::ArcaneBolt, "Arcane Bolt"),
    (Spell::Disintegrate, "Disintegrate"),
    (Spell::FireOrb, "Fire Orb"),
    (Spell::Meteor, "Meteor"),
    (Spell::FrostNova, "Frost Nova"),
];

#[derive(Resource, Default)]
pub struct SpellState {
    selected: Spell,
    cooldown: f32,
    beam_tick: f32,
    /// Set while the disintegrate beam is on screen, so its crackle can play outside this system.
    beam_hum: bool,
}

#[derive(Resource, Default)]
pub struct CameraShake {
    pub trauma: f32,
}

impl CameraShake {
    fn add(&mut self, amount: f32) {
        self.trauma = (self.trauma + amount).min(1.0);
    }
}

/// xorshift64*; good enough for particle scatter.
#[derive(Resource)]
pub struct Rng(u64);

impl Rng {
    pub fn next_f32(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    pub fn unit_vector(&mut self) -> Vec3 {
        loop {
            let v = Vec3::new(self.range(-1.0, 1.0), self.range(-1.0, 1.0), self.range(-1.0, 1.0));
            let len = v.length();
            if len > 0.05 && len <= 1.0 {
                return v / len;
            }
        }
    }
}

#[derive(Resource)]
pub struct SpellAssets {
    sphere: Handle<Mesh>,
    cube: Handle<Mesh>,
    bolt: Handle<StandardMaterial>,
    orb: Handle<StandardMaterial>,
    meteor: Handle<StandardMaterial>,
    frost: Handle<StandardMaterial>,
    demon_debris: Handle<StandardMaterial>,
    debris: HashMap<u16, Handle<StandardMaterial>>,
}

#[derive(Component)]
pub struct Projectile {
    vel: Vec3,
    life: f32,
}

#[derive(Component)]
pub struct FireOrb {
    vel: Vec3,
    fuse: f32,
}

#[derive(Component)]
pub struct BeamVisual;

#[derive(Component)]
pub struct BeamLight;

#[derive(Component)]
pub struct Flash {
    age: f32,
    duration: f32,
    size: f32,
}

#[derive(Component)]
pub struct FlashLight {
    age: f32,
    duration: f32,
    intensity: f32,
}

#[derive(Component)]
pub struct Debris {
    vel: Vec3,
    spin: Vec3,
    life: f32,
    size: f32,
}

#[derive(Component)]
pub struct SpellHud;

/// Velocity that carries a body from `from` to `to` in `flight_time` under gravity.
pub fn launch_velocity(from: Vec3, to: Vec3, flight_time: f32, gravity: f32) -> Vec3 {
    (to - from) / flight_time + Vec3::Y * (0.5 * gravity * flight_time)
}

/// First solid voxel on the segment `from..to` (metres); returns the entry point and hit.
fn segment_hit(world: &VoxelWorld, from: Vec3, to: Vec3) -> Option<(Vec3, Hit)> {
    let delta = to - from;
    let len = delta.length();
    if len <= f32::EPSILON {
        return None;
    }
    let dir = delta / len;
    raycast(from / VOXEL_SIZE, dir, len / VOXEL_SIZE, |v| is_solid(world, v))
        .map(|hit| (from + dir * hit.distance * VOXEL_SIZE, hit))
}

fn nearer_point(from: Vec3, static_hit: Option<Vec3>, falling_hit: Option<Vec3>) -> Option<(Vec3, bool)> {
    match (static_hit, falling_hit) {
        (Some(a), Some(b)) => {
            if from.distance_squared(a) <= from.distance_squared(b) { Some((a, false)) } else { Some((b, true)) }
        }
        (Some(a), None) => Some((a, false)),
        (None, Some(b)) => Some((b, true)),
        (None, None) => None,
    }
}

/// Applies a damage sphere (centre in voxel units) and returns the voxels it destroyed.
pub fn destroy_sphere(world: &VoxelWorld, center: Vec3, radius: f32, energy: f32) -> Vec<(IVec3, Material)> {
    let lo = (center - radius).floor().as_ivec3();
    let hi = (center + radius).ceil().as_ivec3();
    let mut solid = Vec::new();
    for y in lo.y..=hi.y {
        for z in lo.z..=hi.z {
            for x in lo.x..=hi.x {
                let v = IVec3::new(x, y, z);
                if v.as_vec3().distance(center) <= radius {
                    let m = world.get_voxel(crate::REGION, x, y, z);
                    if m != Material::Air {
                        solid.push((v, m));
                    }
                }
            }
        }
    }

    world.apply_damage(center.x, center.y, center.z, radius, energy);
    solid.retain(|(v, _)| !is_solid(world, *v));
    solid
}

fn cast_origin(feet: Vec3, facing: f32) -> Vec3 {
    let forward = Quat::from_rotation_y(facing) * Vec3::NEG_Z;
    feet + Vec3::Y * CAST_HEIGHT + forward * CAST_FORWARD
}

fn facing_toward(from: Vec3, to: Vec3) -> Option<f32> {
    let d = Vec2::new(to.x - from.x, to.z - from.z);
    (d.length() > 0.05).then(|| f32::atan2(-d.x, -d.y))
}

fn aim_point<'a>(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform)>,
    voxels: &Voxels,
    enemies: impl Iterator<Item = (&'a Enemy, &'a Transform)>,
) -> Option<Vec3> {
    let ray = cursor_ray(windows, cameras)?;
    let hit = voxels.with(|w| pick_voxel(w, ray));
    let ground_m = hit.as_ref().map(|hit| hit.distance * VOXEL_SIZE);
    if let Some(point) = cursor_enemy_point(ray, ground_m, enemies) {
        return Some(point);
    }
    let hit = hit?;
    Some(ray.origin + *ray.direction * hit.distance * VOXEL_SIZE)
}

struct BlastStyle {
    flash: Option<(Color, f32, f32)>, // colour, size (m), light intensity
    max_debris: usize,
    shake: f32,
    /// Speed (m/s) given to pieces the blast cuts loose, away from the blast.
    push: f32,
}

pub(crate) fn spawn_enemy_debris(commands: &mut Commands, assets: &SpellAssets, rng: &mut Rng, hit: Vec3, enemy_pos: Vec3, amount: f32) {
    let count = ((amount / 8.0).ceil() as usize).clamp(3, 18);
    let body = enemy_pos + Vec3::Y * ENEMY_BODY_CENTER_Y;
    let outward = (hit - body).normalize_or_zero();
    let outward = if outward.length_squared() > 0.0 { outward } else { Vec3::Y };
    for _ in 0..count {
        let scatter = rng.unit_vector() * rng.range(0.4, 2.2);
        let vel = outward * rng.range(2.0, 5.5) + Vec3::Y * rng.range(1.0, 3.6) + scatter;
        let size = VOXEL_SIZE * rng.range(0.45, 0.9);
        commands.spawn((
            Debris {
                vel,
                spin: rng.unit_vector() * rng.range(6.0, 16.0),
                life: DEBRIS_LIFETIME * rng.range(0.6, 1.2),
                size,
            },
            PbrBundle {
                mesh: assets.cube.clone(),
                material: assets.demon_debris.clone(),
                transform: Transform::from_translation(hit + rng.unit_vector() * 0.12).with_scale(Vec3::splat(size)),
                ..default()
            },
        ));
    }
}

fn damage_enemy_report(
    commands: &mut Commands,
    assets: &SpellAssets,
    rng: &mut Rng,
    enemy_pos: Vec3,
    damage: f32,
    report: crate::combat::StrikeReport,
) {
    if let Some(impact) = report.impact {
        spawn_enemy_debris(commands, assets, rng, impact, enemy_pos, damage);
    }
}

fn damage_enemies<F: QueryFilter>(
    enemies: &mut Query<(&mut Enemy, &Transform), F>,
    center: Vec3,
    radius: f32,
    damage: f32,
    commands: &mut Commands,
    assets: &SpellAssets,
    rng: &mut Rng,
) {
    if damage <= 0.0 {
        return;
    }
    for (mut enemy, transform) in enemies {
        let body = transform.translation + Vec3::Y * ENEMY_BODY_CENTER_Y;
        let horizontal = Vec2::new(body.x - center.x, body.z - center.z).length();
        let vertical = (body.y - center.y).abs().min(ENEMY_BODY_CENTER_Y);
        let distance = Vec2::new(horizontal, vertical).length();
        if distance <= radius {
            let falloff = 1.0 - distance / radius.max(VOXEL_SIZE);
            let amount = damage * (0.35 + 0.65 * falloff);
            let report = enemy.damage_area(amount, center, radius, transform);
            damage_enemy_report(commands, assets, rng, transform.translation, amount, report);
        }
    }
}

fn enemy_body_point(transform: &Transform) -> Vec3 {
    transform.translation + Vec3::Y * ENEMY_BODY_CENTER_Y
}

fn point_segment_distance(point: Vec3, from: Vec3, to: Vec3) -> (f32, Vec3, f32) {
    let segment = to - from;
    let len2 = segment.length_squared();
    if len2 <= f32::EPSILON {
        return (point.distance(from), from, 0.0);
    }
    let t = ((point - from).dot(segment) / len2).clamp(0.0, 1.0);
    let closest = from + segment * t;
    (point.distance(closest), closest, t)
}

fn damage_enemy_segments<F: QueryFilter>(
    enemies: &mut Query<(&mut Enemy, &Transform), F>,
    from: Vec3,
    to: Vec3,
    radius: f32,
    damage: f32,
    commands: &mut Commands,
    assets: &SpellAssets,
    rng: &mut Rng,
) -> Option<Vec3> {
    let mut best: Option<(f32, Vec3, f32, usize)> = None;

    for (index, (_, transform)) in enemies.iter().enumerate() {
        let body = enemy_body_point(transform);
        let (distance, hit, t) = point_segment_distance(body, from, to);
        if distance <= radius && best.map_or(true, |(best_t, _, _, _)| t < best_t) {
            best = Some((t, hit, distance, index));
        }
    }

    let (_, _, _, wanted) = best?;
    for (index, (mut enemy, transform)) in enemies.iter_mut().enumerate() {
        if index == wanted {
            // A shot that meets a part deals its full damage to that part. Grazing the
            // body centre used to scale damage down and made clean limb hits fail to sever.
            let report = enemy.damage_along(damage, from, to, transform);
            let impact = report.impact;
            damage_enemy_report(commands, assets, rng, transform.translation, damage, report);
            return impact;
        }
    }
    None
}

/// A small tumbling voxel cube that bounces off the world and shrinks away.
pub fn spawn_debris(commands: &mut Commands, assets: &SpellAssets, rng: &mut Rng, pos: Vec3, vel: Vec3, material: Material) {
    let Some(mat) = assets.debris.get(&(material as u16)) else { return };
    let size = VOXEL_SIZE * rng.range(0.6, 1.0);
    commands.spawn((
        Debris {
            vel,
            spin: rng.unit_vector() * rng.range(4.0, 12.0),
            life: DEBRIS_LIFETIME * rng.range(0.6, 1.0),
            size,
        },
        PbrBundle {
            mesh: assets.cube.clone(),
            material: mat.clone(),
            transform: Transform::from_translation(pos).with_scale(Vec3::splat(size)),
            ..default()
        },
    ));
}

#[allow(clippy::too_many_arguments)]
fn explode(
    commands: &mut Commands,
    world: &VoxelWorld,
    assets: &SpellAssets,
    rng: &mut Rng,
    materials: &mut Assets<StandardMaterial>,
    blasts: &mut EventWriter<Blasted>,
    shake: &mut CameraShake,
    center: Vec3,
    radius: f32,
    energy: f32,
    style: BlastStyle,
) {
    let center_voxels = center / VOXEL_SIZE;
    let destroyed = destroy_sphere(world, center_voxels, radius, energy);
    blasts.send(Blasted { center: center_voxels, radius, push: style.push });
    shake.add(style.shake);

    if !destroyed.is_empty() && style.max_debris > 0 {
        let stride = (destroyed.len() / style.max_debris).max(1);
        let start = (rng.next_f32() * stride as f32) as usize;
        for (voxel, material) in destroyed.iter().skip(start).step_by(stride).take(style.max_debris) {
            let pos = (voxel.as_vec3() + 0.5) * VOXEL_SIZE;
            let outward = (pos - center).normalize_or_zero();
            let vel = outward * rng.range(2.5, 7.0) + Vec3::Y * rng.range(1.0, 4.0) + rng.unit_vector() * 1.5;
            spawn_debris(commands, assets, rng, pos, vel, *material);
        }
    }

    if let Some((color, size, intensity)) = style.flash {
        commands.spawn((
            Flash { age: 0.0, duration: 0.3, size },
            PbrBundle {
                mesh: assets.sphere.clone(),
                material: materials.add(StandardMaterial {
                    base_color: color,
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                }),
                transform: Transform::from_translation(center).with_scale(Vec3::splat(size * 0.2)),
                ..default()
            },
        ));
        commands.spawn((
            FlashLight { age: 0.0, duration: 0.35, intensity },
            PointLightBundle {
                point_light: PointLight {
                    color: Color::rgb(color.r().min(1.0), color.g().min(1.0), color.b().min(1.0)),
                    intensity,
                    range: size * 6.0,
                    ..default()
                },
                transform: Transform::from_translation(center),
                ..default()
            },
        ));
    }
}

pub struct SpellsPlugin;

impl Plugin for SpellsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpellState>()
            .init_resource::<CameraShake>()
            .insert_resource(Rng(0x9E37_79B9_7F4A_7C15))
            .add_systems(Startup, setup_spells);
    }
}

fn setup_spells(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let glow = |materials: &mut Assets<StandardMaterial>, color: Color| {
        materials.add(StandardMaterial { base_color: color, unlit: true, ..default() })
    };

    let debris = [Material::Dirt, Material::Wood, Material::Brick, Material::Stone]
        .into_iter()
        .map(|m| {
            let [r, g, b, _] = material_color(m, [0.0, 0.0, 0.0]);
            let handle = materials.add(StandardMaterial {
                base_color: Color::rgb(r, g, b),
                perceptual_roughness: 0.9,
                ..default()
            });
            (m as u16, handle)
        })
        .collect();

    let assets = SpellAssets {
        sphere: meshes.add(Sphere::new(1.0)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        bolt: glow(&mut materials, Color::rgb_linear(3.0, 2.0, 12.0)),
        orb: glow(&mut materials, Color::rgb_linear(14.0, 5.0, 1.0)),
        meteor: glow(&mut materials, Color::rgb_linear(18.0, 4.0, 0.4)),
        frost: glow(&mut materials, Color::rgb_linear(1.0, 7.0, 12.0)),
        demon_debris: materials.add(StandardMaterial {
            base_color: Color::rgb(0.72, 0.06, 0.035),
            emissive: Color::rgb_linear(0.18, 0.01, 0.0),
            perceptual_roughness: 0.8,
            ..default()
        }),
        debris,
    };

    commands.spawn((
        BeamVisual,
        PbrBundle {
            mesh: assets.cube.clone(),
            material: glow(&mut materials, Color::rgb_linear(6.0, 10.0, 16.0)),
            visibility: Visibility::Hidden,
            ..default()
        },
    ));
    commands.spawn((
        BeamLight,
        PointLightBundle {
            point_light: PointLight {
                color: Color::rgb(0.6, 0.85, 1.0),
                intensity: 80_000.0,
                range: 5.0,
                ..default()
            },
            visibility: Visibility::Hidden,
            ..default()
        },
    ));

    commands.spawn((
        SpellHud,
        TextBundle::from_section("", TextStyle { font_size: 20.0, color: Color::WHITE, ..default() }).with_style(
            Style {
                position_type: PositionType::Absolute,
                bottom: Val::Px(12.0),
                left: Val::Px(12.0),
                ..default()
            },
        ),
    ));

    commands.insert_resource(assets);
}

pub fn select_spell(
    keys: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<SpellState>,
    mut hud: Query<&mut Text, With<SpellHud>>,
) {
    for (key, spell) in [
        (KeyCode::Digit1, Spell::ArcaneBolt),
        (KeyCode::Digit2, Spell::Disintegrate),
        (KeyCode::Digit3, Spell::FireOrb),
        (KeyCode::Digit4, Spell::Meteor),
        (KeyCode::Digit5, Spell::FrostNova),
    ] {
        if keys.just_pressed(key) {
            state.selected = spell;
        }
    }

    for mut text in &mut hud {
        let label = SPELLS
            .iter()
            .enumerate()
            .map(|(i, (spell, name))| {
                if *spell == state.selected { format!("[{} {}]", i + 1, name) } else { format!(" {} {} ", i + 1, name) }
            })
            .collect::<Vec<_>>()
            .join("   ");
        if text.sections[0].value != label {
            text.sections[0].value = label;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn cast_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    voxels: Res<Voxels>,
    assets: Res<SpellAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<SpellState>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<CameraShake>,
    mut blasts: EventWriter<Blasted>,
    mut sfx: ResMut<crate::sfx::SfxBank>,
    mut players: Query<(&mut Player, &Transform)>,
    mut enemies: Query<(&mut Enemy, &Transform)>,
) {
    state.cooldown = (state.cooldown - time.delta_seconds()).max(0.0);
    if state.selected == Spell::Disintegrate || !buttons.pressed(MouseButton::Right) || state.cooldown > 0.0 {
        return;
    }
    let Some(aim) = aim_point(&windows, &cameras, &voxels, enemies.iter()) else { return };
    let Ok((mut player, transform)) = players.get_single_mut() else { return };

    if let Some(facing) = facing_toward(transform.translation, aim) {
        player.facing = facing;
    }
    player.cast_time = 0.28;
    let origin = cast_origin(transform.translation, player.facing);

    match state.selected {
        Spell::ArcaneBolt => {
            sfx.play(&mut commands, crate::sfx::Cue::Bolt);
            let dir = (aim - origin).normalize_or_zero();
            commands
                .spawn((
                    Projectile { vel: dir * BOLT_SPEED, life: BOLT_LIFETIME },
                    PbrBundle {
                        mesh: assets.sphere.clone(),
                        material: assets.bolt.clone(),
                        transform: Transform::from_translation(origin).with_scale(Vec3::splat(0.1)),
                        ..default()
                    },
                ))
                .with_children(|parent| {
                    parent.spawn(PointLightBundle {
                        point_light: PointLight {
                            color: Color::rgb(0.6, 0.5, 1.0),
                            intensity: 40_000.0,
                            range: 6.0,
                            ..default()
                        },
                        ..default()
                    });
                });
            state.cooldown = BOLT_COOLDOWN;
        }
        Spell::FireOrb => {
            sfx.play(&mut commands, crate::sfx::Cue::FireCast);
            let horizontal = Vec2::new(aim.x - origin.x, aim.z - origin.z).length();
            let flight_time = (horizontal / 9.0).clamp(ORB_MIN_FLIGHT, ORB_MAX_FLIGHT);
            let vel = launch_velocity(origin, aim + Vec3::Y * ORB_SIZE, flight_time, GRAVITY);
            commands
                .spawn((
                    FireOrb { vel, fuse: ORB_FUSE },
                    PbrBundle {
                        mesh: assets.sphere.clone(),
                        material: assets.orb.clone(),
                        transform: Transform::from_translation(origin).with_scale(Vec3::splat(ORB_SIZE)),
                        ..default()
                    },
                ))
                .with_children(|parent| {
                    parent.spawn(PointLightBundle {
                        point_light: PointLight {
                            color: Color::rgb(1.0, 0.55, 0.2),
                            intensity: 30_000.0,
                            range: 5.0,
                            ..default()
                        },
                        ..default()
                    });
                });
            state.cooldown = ORB_COOLDOWN;
        }
        Spell::Meteor => {
            sfx.play(&mut commands, crate::sfx::Cue::Meteor);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    aim,
                    METEOR_RADIUS,
                    METEOR_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(18.0, 4.0, 0.5, 1.0), 2.8, 900_000.0)),
                        max_debris: 100,
                        shake: 0.8,
                        push: 6.0,
                    },
                )
            });
            damage_enemies(&mut enemies, aim, METEOR_RADIUS * VOXEL_SIZE, METEOR_DAMAGE, &mut commands, &assets, &mut rng);
            commands.spawn((
                Flash { age: 0.0, duration: 0.5, size: 1.6 },
                PbrBundle {
                    mesh: assets.sphere.clone(),
                    material: assets.meteor.clone(),
                    transform: Transform::from_translation(aim + Vec3::Y * 2.0).with_scale(Vec3::splat(0.35)),
                    ..default()
                },
            ));
            state.cooldown = METEOR_COOLDOWN;
        }
        Spell::FrostNova => {
            sfx.play(&mut commands, crate::sfx::Cue::Frost);
            let center = transform.translation + Vec3::Y * 0.4;
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    center,
                    NOVA_RADIUS,
                    NOVA_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(0.7, 5.0, 12.0, 1.0), 2.4, 450_000.0)),
                        max_debris: 35,
                        shake: 0.25,
                        push: 2.5,
                    },
                )
            });
            damage_enemies(&mut enemies, center, NOVA_RADIUS * VOXEL_SIZE, NOVA_DAMAGE, &mut commands, &assets, &mut rng);
            commands.spawn((
                Flash { age: 0.0, duration: 0.45, size: 1.9 },
                PbrBundle {
                    mesh: assets.sphere.clone(),
                    material: assets.frost.clone(),
                    transform: Transform::from_translation(center).with_scale(Vec3::splat(0.2)),
                    ..default()
                },
            ));
            state.cooldown = NOVA_COOLDOWN;
        }
        Spell::Disintegrate => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn fire_beam(
    mut commands: Commands,
    time: Res<Time>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    voxels: Res<Voxels>,
    assets: Res<SpellAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut state: ResMut<SpellState>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<CameraShake>,
    mut blasts: EventWriter<Blasted>,
    mut players: Query<(&mut Player, &Transform), (Without<BeamVisual>, Without<BeamLight>)>,
    mut beam: Query<(&mut Transform, &mut Visibility), (With<BeamVisual>, Without<BeamLight>)>,
    mut light: Query<(&mut Transform, &mut Visibility), (With<BeamLight>, Without<BeamVisual>)>,
    mut targets: ParamSet<(
        Query<&mut FallingPiece>,
        Query<(&mut Enemy, &Transform), (Without<Player>, Without<BeamVisual>, Without<BeamLight>)>,
    )>,
) {
    state.beam_hum = false;
    let (Ok((mut beam_transform, mut beam_visibility)), Ok((mut light_transform, mut light_visibility))) =
        (beam.get_single_mut(), light.get_single_mut())
    else {
        return;
    };

    let firing = state.selected == Spell::Disintegrate && buttons.pressed(MouseButton::Right);
    let aim = if firing {
        let enemies = targets.p1();
        aim_point(&windows, &cameras, &voxels, enemies.iter())
    } else {
        None
    };
    let (Some(aim), Ok((mut player, transform))) = (aim, players.get_single_mut()) else {
        *beam_visibility = Visibility::Hidden;
        *light_visibility = Visibility::Hidden;
        state.beam_tick = 0.0;
        return;
    };

    // Channelled: the caster stands still and faces the target.
    player.target = None;
    player.cast_time = 0.16;
    if let Some(facing) = facing_toward(transform.translation, aim) {
        player.facing = facing;
    }
    let origin = cast_origin(transform.translation, player.facing);
    let dir = (aim - origin).normalize_or_zero();
    if dir == Vec3::ZERO {
        return;
    }

    let far = origin + dir * BEAM_RANGE;
    let static_hit = voxels.with(|w| segment_hit(w, origin, far).map(|(point, _)| point));
    let falling_hit = falling::hit_falling_piece(&mut targets.p0(), origin, far, 0.0).map(|hit| hit.point);
    let hit = nearer_point(origin, static_hit, falling_hit);
    let end = hit.map_or(far, |(point, _)| point);
    let length = origin.distance(end);
    let up = if dir.y.abs() > 0.99 { Vec3::Z } else { Vec3::Y };
    let width = BEAM_WIDTH * rng.range(0.75, 1.25);
    *beam_transform = Transform::from_translation((origin + end) * 0.5)
        .looking_to(dir, up)
        .with_scale(Vec3::new(width, width, length));
    *beam_visibility = Visibility::Visible;
    light_transform.translation = end - dir * 0.2;
    *light_visibility = Visibility::Visible;
    state.beam_hum = true;

    state.beam_tick -= time.delta_seconds();
    if let Some((point, dynamic)) = hit {
        if state.beam_tick <= 0.0 {
            state.beam_tick = BEAM_TICK;
            damage_enemy_segments(&mut targets.p1(), origin, point, ENEMY_HIT_RADIUS, 10.0, &mut commands, &assets, &mut rng);
            if dynamic {
                falling::hit_falling_piece(&mut targets.p0(), origin, point + dir * VOXEL_SIZE, BEAM_DIRECT_PUSH);
            }
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    point,
                    BEAM_RADIUS,
                    BEAM_ENERGY,
                    BlastStyle { flash: None, max_debris: 4, shake: 0.02, push: 0.3 },
                )
            });
        }
    } else if state.beam_tick <= 0.0 {
        state.beam_tick = BEAM_TICK;
        damage_enemy_segments(&mut targets.p1(), origin, end, ENEMY_HIT_RADIUS, 10.0, &mut commands, &assets, &mut rng);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_projectiles(
    mut commands: Commands,
    time: Res<Time>,
    voxels: Res<Voxels>,
    assets: Res<SpellAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<CameraShake>,
    mut blasts: EventWriter<Blasted>,
    mut sfx: ResMut<crate::sfx::SfxBank>,
    mut bolts: Query<(Entity, &mut Transform, &mut Projectile)>,
    mut falling_pieces: Query<&mut FallingPiece>,
    mut enemies: Query<(&mut Enemy, &Transform), Without<Projectile>>,
) {
    let dt = time.delta_seconds();
    for (entity, mut transform, mut bolt) in &mut bolts {
        bolt.life -= dt;
        let from = transform.translation;
        let to = from + bolt.vel * dt;

        if let Some(point) = damage_enemy_segments(&mut enemies, from, to, ENEMY_HIT_RADIUS, 45.0, &mut commands, &assets, &mut rng) {
            sfx.play(&mut commands, crate::sfx::Cue::BoltHit);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    point,
                    BOLT_RADIUS,
                    BOLT_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(3.0, 2.0, 10.0, 1.0), 0.6, 150_000.0)),
                        max_debris: 8,
                        shake: 0.08,
                        push: 0.8,
                    },
                )
            });
            commands.entity(entity).despawn_recursive();
            continue;
        }

        let static_hit = voxels.with(|w| segment_hit(w, from, to).map(|(point, _)| point));
        let falling_hit = falling::hit_falling_piece(&mut falling_pieces, from, to, 0.0).map(|hit| hit.point);
        if let Some((point, dynamic)) = nearer_point(from, static_hit, falling_hit) {
            if dynamic {
                falling::hit_falling_piece(&mut falling_pieces, from, to, BOLT_DIRECT_PUSH);
            }
            sfx.play(&mut commands, crate::sfx::Cue::BoltHit);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    point,
                    BOLT_RADIUS,
                    BOLT_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(3.0, 2.0, 10.0, 1.0), 0.6, 150_000.0)),
                        max_debris: 25,
                        shake: 0.15,
                        push: 1.5,
                    },
                )
            });
            damage_enemies(&mut enemies, point, BOLT_RADIUS * VOXEL_SIZE, 35.0, &mut commands, &assets, &mut rng);
            commands.entity(entity).despawn_recursive();
        } else if bolt.life <= 0.0 {
            commands.entity(entity).despawn_recursive();
        } else {
            transform.translation = to;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn update_fire_orbs(
    mut commands: Commands,
    time: Res<Time>,
    voxels: Res<Voxels>,
    assets: Res<SpellAssets>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut rng: ResMut<Rng>,
    mut shake: ResMut<CameraShake>,
    mut blasts: EventWriter<Blasted>,
    mut sfx: ResMut<crate::sfx::SfxBank>,
    mut orbs: Query<(Entity, &mut Transform, &mut FireOrb)>,
    mut falling_pieces: Query<&mut FallingPiece>,
    mut enemies: Query<(&mut Enemy, &Transform), Without<FireOrb>>,
) {
    let dt = time.delta_seconds();
    for (entity, mut transform, mut orb) in &mut orbs {
        orb.fuse -= dt;
        if orb.fuse <= 0.0 {
            sfx.play(&mut commands, crate::sfx::Cue::FireBurst);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    transform.translation,
                    ORB_RADIUS,
                    ORB_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(12.0, 5.0, 1.0, 1.0), 2.2, 600_000.0)),
                        max_debris: 60,
                        shake: 0.6,
                        push: 4.0,
                    },
                )
            });
            damage_enemies(&mut enemies, transform.translation, ORB_RADIUS * VOXEL_SIZE, 85.0, &mut commands, &assets, &mut rng);
            commands.entity(entity).despawn_recursive();
            continue;
        }

        orb.vel.y -= GRAVITY * dt;
        let from = transform.translation;
        let to = from + orb.vel * dt;
        if let Some(point) = damage_enemy_segments(&mut enemies, from, to, ENEMY_HIT_RADIUS + ORB_SIZE, 85.0, &mut commands, &assets, &mut rng) {
            sfx.play(&mut commands, crate::sfx::Cue::FireBurst);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    point,
                    ORB_RADIUS,
                    ORB_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(12.0, 5.0, 1.0, 1.0), 2.2, 600_000.0)),
                        max_debris: 60,
                        shake: 0.6,
                        push: 4.0,
                    },
                )
            });
            damage_enemies(&mut enemies, point, ORB_RADIUS * VOXEL_SIZE, 85.0, &mut commands, &assets, &mut rng);
            commands.entity(entity).despawn_recursive();
            continue;
        }
        let static_hit = voxels.with(|w| segment_hit(w, from, to));
        let falling_hit = falling::hit_falling_piece(&mut falling_pieces, from, to, 0.0);
        let dynamic_first = match (static_hit.as_ref(), falling_hit.as_ref()) {
            (Some((point, _)), Some(hit)) => hit.distance < from.distance(*point),
            (None, Some(_)) => true,
            _ => false,
        };
        if dynamic_first {
            let point = falling_hit.unwrap().point;
            falling::hit_falling_piece(&mut falling_pieces, from, to, ORB_DIRECT_PUSH);
            sfx.play(&mut commands, crate::sfx::Cue::FireBurst);
            voxels.with(|w| {
                explode(
                    &mut commands,
                    w,
                    &assets,
                    &mut rng,
                    &mut materials,
                    &mut blasts,
                    &mut shake,
                    point,
                    ORB_RADIUS,
                    ORB_ENERGY,
                    BlastStyle {
                        flash: Some((Color::rgba_linear(12.0, 5.0, 1.0, 1.0), 2.2, 600_000.0)),
                        max_debris: 60,
                        shake: 0.6,
                        push: 4.0,
                    },
                )
            });
            damage_enemies(&mut enemies, point, ORB_RADIUS * VOXEL_SIZE, 85.0, &mut commands, &assets, &mut rng);
            commands.entity(entity).despawn_recursive();
            continue;
        }

        match static_hit {
            Some((point, hit)) if hit.normal != IVec3::ZERO => {
                let n = hit.normal.as_vec3();
                let normal_part = orb.vel.dot(n) * n;
                orb.vel = (orb.vel - normal_part) * ORB_FRICTION - normal_part * ORB_RESTITUTION;
                transform.translation = point + n * 0.02;
            }
            Some(_) => orb.vel = Vec3::ZERO,
            None => transform.translation = to,
        }

        // Pulse faster as the fuse runs out.
        let urgency = 1.0 - orb.fuse / ORB_FUSE;
        let pulse = 1.0 + 0.25 * (time.elapsed_seconds() * (8.0 + 30.0 * urgency)).sin();
        transform.scale = Vec3::splat(ORB_SIZE * pulse);
    }
}

pub fn beam_audio(mut commands: Commands, time: Res<Time>, state: Res<SpellState>, mut sfx: ResMut<crate::sfx::SfxBank>) {
    if state.beam_hum {
        sfx.tick_beam(&mut commands, time.elapsed_seconds());
    } else {
        sfx.rest_beam();
    }
}

pub fn update_flashes(
    mut commands: Commands,
    time: Res<Time>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut flashes: Query<(Entity, &mut Transform, &mut Flash, &Handle<StandardMaterial>)>,
    mut lights: Query<(Entity, &mut PointLight, &mut FlashLight)>,
) {
    let dt = time.delta_seconds();
    for (entity, mut transform, mut flash, material) in &mut flashes {
        flash.age += dt;
        let t = flash.age / flash.duration;
        if t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let eased = 1.0 - (1.0 - t).powi(3);
        transform.scale = Vec3::splat(flash.size * (0.2 + 0.8 * eased));
        if let Some(m) = materials.get_mut(material) {
            m.base_color.set_a(1.0 - t);
        }
    }
    for (entity, mut light, mut flash) in &mut lights {
        flash.age += dt;
        let t = flash.age / flash.duration;
        if t >= 1.0 {
            commands.entity(entity).despawn();
        } else {
            light.intensity = flash.intensity * (1.0 - t) * (1.0 - t);
        }
    }
}

pub fn update_debris(
    mut commands: Commands,
    time: Res<Time>,
    voxels: Res<Voxels>,
    mut debris: Query<(Entity, &mut Transform, &mut Debris)>,
) {
    let dt = time.delta_seconds().min(0.05);
    voxels.with(|w| {
        for (entity, mut transform, mut d) in &mut debris {
            d.life -= dt;
            if d.life <= 0.0 {
                commands.entity(entity).despawn();
                continue;
            }

            d.vel.y -= GRAVITY * dt;
            let mut pos = transform.translation;
            for axis in 0..3 {
                let mut next = pos;
                next[axis] += d.vel[axis] * dt;
                if is_solid(w, (next / VOXEL_SIZE).floor().as_ivec3()) {
                    d.vel[axis] *= -DEBRIS_RESTITUTION;
                    if axis == 1 {
                        d.vel.x *= 0.7;
                        d.vel.z *= 0.7;
                        d.spin *= 0.7;
                    }
                } else {
                    pos = next;
                }
            }

            transform.translation = pos;
            transform.rotate(Quat::from_scaled_axis(d.spin * dt));
            transform.scale = Vec3::splat(d.size * (d.life / 0.4).min(1.0));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lobbed_orb_lands_on_target() {
        let (from, to, flight_time) = (Vec3::new(0.0, 1.3, 0.0), Vec3::new(6.0, 0.5, -3.0), 0.8);
        let mut pos = from;
        let mut vel = launch_velocity(from, to, flight_time, GRAVITY);
        let dt = 1.0 / 1000.0;
        for _ in 0..(flight_time / dt).round() as i32 {
            vel.y -= GRAVITY * dt;
            pos += vel * dt;
        }
        assert!(pos.distance(to) < 0.02, "landed at {pos}");
    }

    #[test]
    fn destroy_sphere_reports_only_voxels_it_removed() {
        let world = VoxelWorld::new();
        world.load_region(crate::REGION);
        world.generate_terrain(crate::REGION, 0, 0, 0);

        let surface = (0..32).rev().find(|&y| world.get_voxel(crate::REGION, 16, y, 16) != Material::Air).unwrap();
        let center = Vec3::new(16.0, surface as f32, 16.0);
        let destroyed = destroy_sphere(&world, center, 4.0, 3.0);

        assert!(!destroyed.is_empty());
        assert!(destroyed.iter().all(|(v, m)| *m != Material::Air && !is_solid(&world, *v)));
        assert!(destroyed.iter().all(|(v, _)| v.as_vec3().distance(center) <= 4.0));
    }

    #[test]
    fn rng_stays_in_range() {
        let mut rng = Rng(12345);
        for _ in 0..10_000 {
            let x = rng.next_f32();
            assert!((0.0..1.0).contains(&x));
            assert!((rng.unit_vector().length() - 1.0).abs() < 1e-4);
        }
    }
}
