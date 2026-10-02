mod combat;
mod falling;
mod outdoor;
mod sfx;
mod tristram;
mod occluder_fade;
mod player;
mod player_model;
mod raycast;
mod rig;
mod rigid;
mod spells;

use bevy::audio::AddAudioSource;
use bevy::app::AppExit;
use bevy::log::LogPlugin;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use bevy::render::camera::{Exposure, ScalingMode};
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use bevy::window::PrimaryWindow;
use occluder_fade::{FadingMaterial, OccluderFade, OccluderFadeParams, OccluderFadePlugin};
use bevy::core_pipeline::bloom::{BloomCompositeMode, BloomPrefilterSettings, BloomSettings};
use player::{walk_step, Body, ENEMY_BODY, HUMAN, VOXEL_SIZE};
use combat::{
    choose_cursor_body, plan_fight, CursorSample, Part, Species, Stance, CURSOR_DEPTH_SLACK, CURSOR_PICK_RADIUS, MELEE_CUT,
};
use spells::{CameraShake, SpellsPlugin};
use raycast::{raycast, Hit};
use std::collections::{HashMap, HashSet};
use std::f32::consts::{FRAC_PI_4, PI, TAU};
use std::sync::Mutex;
use voxel_core_ffi::{ChunkMesh, Material, VoxelWorld, CHUNK_SIZE};

const REGION: u64 = 1;
/// Town square, in chunks. 16 * 3.2 m = 51.2 m. The moor continues past +Z.
const GRID: i32 = 16;
/// Town chunks plus the outdoor level.
const GRID_Z: i32 = GRID + outdoor::EXTRA_CHUNKS;
/// Chunk layers shown above ground level: 3 * 3.2 m = 9.6 m. The cathedral spire stays inside this.
const VERTICAL_CHUNKS: i32 = 3;

const WALK_SPEED: f32 = 4.5;
const SPRINT_SPEED: f32 = 7.0;
const JUMP_SPEED: f32 = 4.5;
const ARRIVE_RADIUS: f32 = 0.1;
/// Give up on a destination after being blocked this long.
const STUCK_TIMEOUT: f32 = 0.5;
/// How far the drawn character may lag behind its collider when stepping up or down.
const MAX_VISUAL_LAG: f32 = 0.35;
const ENEMY_AGGRO_RANGE: f32 = 28.0;
const PLAYER_SEPARATION_RADIUS: f32 = 0.65;
const ENEMY_SEPARATION_RADIUS: f32 = 0.75;
pub const ENEMY_BODY_CENTER_Y: f32 = 0.75;
const PLAYER_HEALTH: f32 = 100.0;
const MELEE_RANGE: f32 = 1.6;
/// Sword swing, counted down. The blade connects as windup ends.
/// Windup / total stays at the pose's strike frame (0.36).
const SWING_WINDUP: f32 = 0.13;
const SWING_STRIKE: f32 = 0.09;
const SWING_RECOVER: f32 = 0.14;
/// Metres per second the body steps along the blade through the cut.
const STRIKE_LUNGE: f32 = 3.8;
/// Metres per second the body slides back once the blade has passed.
/// Matches the step-in so repeated cuts rock in place instead of walking through.
const RECOVER_SLIDE: f32 = 4.0;
const HIT_INVULN: f32 = 0.45;
/// A zombie's connecting hit keeps dealing this much health per second.
const ZOMBIE_POISON_DPS: f32 = 5.0;
const ZOMBIE_POISON_SECONDS: f32 = 3.0;
const DEATH_INVULN: f32 = 1.6;
const PICK_REACH: f32 = 80.0;

const CAMERA_YAW: f32 = FRAC_PI_4;
/// True isometric elevation: atan(1 / sqrt(2)).
const CAMERA_PITCH: f32 = 0.615_479_7;
const CAMERA_DISTANCE: f32 = 60.0;
const CAMERA_PIVOT_HEIGHT: f32 = 0.9;

/// Geometry within this distance of the camera-to-player line (and in front of the player) fades.
const FADE_RADIUS: f32 = 2.5;
/// 1.0 removes occluders completely at the centre; slightly less leaves a faint dithered outline.
const FADE_STRENGTH: f32 = 0.9;
const DEFAULT_VIEW_HEIGHT: f32 = 14.0;
const MAX_PICK_VOXELS: f32 = 3000.0;

struct WorldHandle(VoxelWorld);
// The raw C++ pointer is only ever touched through the Mutex below.
unsafe impl Send for WorldHandle {}

#[derive(Resource)]
struct Voxels(Mutex<WorldHandle>);

impl Voxels {
    fn with<R>(&self, f: impl FnOnce(&VoxelWorld) -> R) -> R {
        f(&self.0.lock().unwrap().0)
    }
}

fn is_solid(world: &VoxelWorld, v: IVec3) -> bool {
    world.get_voxel(REGION, v.x, v.y, v.z) != Material::Air
}

/// Rendered chunk entities; chunks with no visible faces have no entity.
#[derive(Resource, Default)]
struct ChunkViews(HashMap<IVec3, Entity>);

#[derive(Resource)]
struct TerrainMaterial(Handle<FadingMaterial>);

#[derive(Component)]
struct DestinationMarker;

#[derive(Component)]
struct Player {
    body: Body,
    facing: f32,
    /// Smoothed height the model is drawn at, so 10 cm steps don't jolt the character.
    visual_y: f32,
    target: Option<Vec3>,
    stuck_time: f32,
    cast_time: f32,
    health: f32,
    invuln: f32,
    /// Seconds of rot left from a zombie's hit. Drains health until it runs out.
    poison: f32,
    attack_target: Option<Entity>,
    aim: Option<Part>,
    /// Seconds left in the current sword swing. Zero means the arm is free.
    swing_left: f32,
    /// Blade height captured when the swing started, so the cut stays on the aimed part.
    swing_height: f32,
    /// Seconds left being shoved by a hit. While this is up, a new swing waits.
    recoil: f32,
    /// Horizontal shove from the last hit, in metres per second.
    shove: Vec3,
}

#[derive(Component, Default)]
struct CharacterAnim {
    phase: f32,
    /// 0 standing, 1 fully in the walk cycle. Eased so a stop doesn't snap.
    weight: f32,
    speed: f32,
    hurt: f32,
    last_hits: u32,
    idle_phase: f32,
}

/// Marker on rig entities so a rebuild can drop bones without the demon's light.
#[derive(Component)]
struct RigPart;

/// Material for the limb meshes. The body root itself has no mesh: an empty
/// mesh makes Bevy build a prepass pipeline whose vertex stage never writes
/// the position the fragment stage requires, and that pipeline panics.
#[derive(Component)]
struct BodyMaterial(Handle<StandardMaterial>);

#[derive(Clone, Copy)]
struct BoneLink {
    entity: Entity,
    driver: rig::Driver,
}

#[derive(Component)]
struct RigState {
    wounds: u8,
    spine: Entity,
    bones: Vec<BoneLink>,
    links: Vec<rig::ChainLink>,
    soles: Vec<(usize, Vec3)>,
}

#[derive(Component)]
struct BattleHud;

#[derive(Component)]
pub struct Enemy {
    fighter: combat::Fighter,
}

impl Enemy {
    fn of(species: Species) -> Self {
        Self { fighter: combat::Fighter::new(species) }
    }

    pub fn is_down(&self) -> bool {
        self.fighter.is_down()
    }

    pub fn is_winding(&self) -> bool {
        self.fighter.is_winding()
    }

    pub fn stance(&self) -> Stance {
        self.fighter.stance()
    }

    pub fn hits(&self) -> u32 {
        self.fighter.hits()
    }

    pub fn center(&self, part: Part) -> Option<Vec3> {
        self.fighter.center(part)
    }

    pub fn species(&self) -> Species {
        self.fighter.species()
    }

    pub fn part_name(&self, part: Part) -> &'static str {
        self.fighter.part_name(part)
    }

    pub fn status_line(&self) -> String {
        self.fighter.status_line()
    }

    pub fn damage_along(&mut self, amount: f32, from: Vec3, to: Vec3, transform: &Transform) -> combat::StrikeReport {
        let report = self.fighter.strike_along(to_local(transform, from), to_local(transform, to), amount, None, 2.0);
        to_world_report(transform, report)
    }

    pub fn damage_area(&mut self, amount: f32, center: Vec3, radius: f32, transform: &Transform) -> combat::StrikeReport {
        let report = self.fighter.strike_area(to_local(transform, center), radius, amount);
        to_world_report(transform, report)
    }

    pub fn damage_swing(
        &mut self,
        from: Vec3,
        to: Vec3,
        amount: f32,
        prefer: Option<Part>,
        reach: f32,
        transform: &Transform,
    ) -> combat::StrikeReport {
        let report = self.fighter.strike_along(to_local(transform, from), to_local(transform, to), amount, prefer, reach);
        to_world_report(transform, report)
    }
}

fn to_local(transform: &Transform, world: Vec3) -> Vec3 {
    transform.rotation.inverse() * (world - transform.translation)
}

fn to_world(transform: &Transform, local: Vec3) -> Vec3 {
    transform.translation + transform.rotation * local
}

fn to_world_report(transform: &Transform, mut report: combat::StrikeReport) -> combat::StrikeReport {
    if let Some(point) = report.impact {
        report.impact = Some(to_world(transform, point));
    }
    report
}

fn wound_mask(fighter: &combat::Fighter) -> u8 {
    let mut mask = 0;
    if !fighter.attached(Part::LeftArm) {
        mask |= player_model::DEMON_LEFT_ARM;
    }
    if !fighter.attached(Part::RightArm) {
        mask |= player_model::DEMON_RIGHT_ARM;
    }
    if !fighter.attached(Part::Horns) {
        mask |= player_model::DEMON_HORNS;
    }
    if !fighter.attached(Part::Tail) {
        mask |= player_model::DEMON_TAIL;
    }
    if !fighter.attached(Part::LeftLeg) {
        mask |= player_model::DEMON_LEFT_LEG;
    }
    if !fighter.attached(Part::RightLeg) {
        mask |= player_model::DEMON_RIGHT_LEG;
    }
    if !fighter.attached(Part::Head) {
        mask |= player_model::DEMON_HEAD;
    }
    mask
}

impl Player {
    fn spawn_at(pos: Vec3) -> Self {
        Self {
            body: Body { pos, ..default() },
            facing: 0.0,
            visual_y: pos.y,
            target: None,
            stuck_time: 0.0,
            cast_time: 0.0,
            health: PLAYER_HEALTH,
            invuln: 0.0,
            poison: 0.0,
            attack_target: None,
            aim: None,
            swing_left: 0.0,
            swing_height: 0.85,
            recoil: 0.0,
            shove: Vec3::ZERO,
        }
    }
}

#[derive(Resource)]
struct CameraRig {
    view_height: f32,
}

/// Inclusive range of chunk coordinates whose meshes are stale.
#[derive(Event)]
struct ChunksChanged {
    min: IVec3,
    max: IVec3,
}

impl ChunksChanged {
    fn all() -> Self {
        Self { min: IVec3::ZERO, max: IVec3::new(GRID - 1, VERTICAL_CHUNKS - 1, GRID_Z - 1) }
    }

    fn around_voxel(center: Vec3, radius: f32) -> Self {
        let to_chunk = |v: Vec3| (v / CHUNK_SIZE as f32).floor().as_ivec3();
        Self { min: to_chunk(center - radius), max: to_chunk(center + radius) }
    }

    /// Chunks covering the inclusive voxel box `min..=max`.
    fn voxel_box(min: IVec3, max: IVec3) -> Self {
        let size = IVec3::splat(CHUNK_SIZE);
        Self { min: min.div_euclid(size), max: max.div_euclid(size) }
    }

}

fn main() {
    App::new()
        // Black void, like the town sitting in darkness. The fill is moonlight, not a rust wash.
        .insert_resource(ClearColor(Color::rgb(0.0, 0.0, 0.0)))
        .insert_resource(AmbientLight { color: Color::rgb(0.38, 0.46, 0.62), brightness: 40.0 })
        .insert_resource(CameraRig { view_height: DEFAULT_VIEW_HEIGHT })
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Persistent Voxel ARPG".into(),
                resolution: (1280.0, 720.0).into(),
                ..default()
            }),
            ..default()
        }).set(LogPlugin {
            filter: "warn,wgpu_hal=off,log=off".into(),
            ..default()
        }))
        .add_plugins((OccluderFadePlugin, SpellsPlugin))
        .add_audio_source::<sfx::SfxClip>()
        .add_event::<ChunksChanged>()
        .add_event::<falling::Blasted>()
        .add_systems(Startup, (setup, sfx::load))
        .add_systems(
            Update,
            (
                handle_input,
                spells::select_spell,
                spells::cast_projectiles,
                spells::fire_beam,
                spells::beam_audio,
                player_controller,
                update_enemies,
                update_hostile_shots,
                spells::update_projectiles,
                spells::update_fire_orbs,
                pose_rigs,
                update_battle_hud,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                falling::detect_detached,
                falling::update_falling_pieces,
                spells::update_debris,
                spells::update_flashes,
                follow_camera,
                update_occluder_fade,
                update_marker,
                remesh_changed_chunks,
            )
                .chain(),
        )
        .run();
}

/// Unit vector from the scene toward the camera.
fn camera_offset_dir() -> Vec3 {
    Vec3::new(
        CAMERA_YAW.cos() * CAMERA_PITCH.cos(),
        CAMERA_PITCH.sin(),
        CAMERA_YAW.sin() * CAMERA_PITCH.cos(),
    )
}

fn to_bevy_mesh(positions: Vec<[f32; 3]>, normals: Vec<[f32; 3]>, colors: Vec<[f32; 4]>, indices: Vec<u32>) -> Mesh {
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_indices(Indices::U32(indices))
}

fn model_mesh(model: &player_model::VoxelModel) -> Mesh {
    let m = model.mesh(VOXEL_SIZE);
    to_bevy_mesh(m.positions, m.normals, m.colors, m.indices)
}

fn mesh_from_arrays(arrays: &player_model::MeshArrays) -> Mesh {
    to_bevy_mesh(arrays.positions.clone(), arrays.normals.clone(), arrays.colors.clone(), arrays.indices.clone())
}

fn material_color(material: Material, normal: [f32; 3]) -> [f32; 4] {
    let up = normal[1] > 0.5;
    match material {
        Material::Dirt if up => [0.14, 0.10, 0.06, 1.0],
        Material::Dirt => [0.10, 0.07, 0.04, 1.0],
        Material::Grass if up => [0.10, 0.13, 0.06, 1.0],
        Material::Grass => [0.07, 0.09, 0.04, 1.0],
        Material::Path if up => [0.24, 0.16, 0.08, 1.0],
        Material::Path => [0.16, 0.11, 0.05, 1.0],
        Material::Water if up => [0.05, 0.09, 0.18, 1.0],
        Material::Water => [0.03, 0.05, 0.10, 1.0],
        Material::Stone if up => [0.22, 0.21, 0.23, 1.0],
        Material::Stone => [0.30, 0.29, 0.31, 1.0],
        Material::Brick => [0.32, 0.12, 0.08, 1.0],
        // Slate. The cathedral roof reads dark, not red.
        Material::Tile if up => [0.15, 0.16, 0.19, 1.0],
        Material::Tile => [0.10, 0.11, 0.14, 1.0],
        Material::Thatch if up => [0.11, 0.08, 0.05, 1.0],
        Material::Thatch => [0.07, 0.05, 0.03, 1.0],
        // Daub, light enough to separate from stone and thatch.
        Material::Plaster => [0.40, 0.32, 0.24, 1.0],
        Material::Wood if up => [0.22, 0.12, 0.05, 1.0],
        Material::Wood => [0.14, 0.08, 0.03, 1.0],
        Material::Cloth => [0.32, 0.05, 0.04, 1.0],
        // Above 1 so the cathedral windows bloom red under the dark exposure.
        Material::Glass => [1.85, 0.10, 0.04, 1.0],
        Material::Gold => [0.62, 0.44, 0.14, 1.0],
        // West-forest canopy.
        Material::Violet => [0.07, 0.08, 0.13, 1.0],
        // Hearth and window light. Above 1 so it blooms gold. Cathedral glass stays the red.
        Material::Ember => [1.9, 1.15, 0.38, 1.0],
        Material::Air | Material::Unknown => [1.0, 0.0, 1.0, 1.0],
    }
}

fn terrain_mesh(chunk: ChunkMesh) -> Mesh {
    let colors = chunk
        .materials
        .iter()
        .zip(&chunk.normals)
        .zip(&chunk.positions)
        .map(|((&m, &n), &p)| {
            let mut color = material_color(m, n);
            // A channel above 1 is a light. Wobble would knock it under the bloom threshold.
            let glow = color[0] > 1.0 || color[1] > 1.0 || color[2] > 1.0;
            if !glow {
                let h = (p[0] * 12.9898 + p[2] * 78.233).sin() * 43758.5453;
                let wobble = 0.90 + (h - h.floor()) * 0.18;
                color[0] *= wobble;
                color[1] *= wobble;
                color[2] *= wobble;
            }
            color
        })
        .collect();
    to_bevy_mesh(chunk.positions, chunk.normals, colors, chunk.indices)
}

/// Feet position on the south causeway into Tristram.
fn spawn_point(world: &VoxelWorld) -> Vec3 {
    let (x, z) = tristram::SPAWN;
    let top = (0..VERTICAL_CHUNKS * CHUNK_SIZE)
        .rev()
        .find(|&y| is_solid(world, IVec3::new(x, y, z)))
        .unwrap_or(0);
    Vec3::new(x as f32 + 0.5, (top + 1) as f32, z as f32 + 0.5) * VOXEL_SIZE
}

fn surface_height(world: &VoxelWorld, x: i32, z: i32) -> i32 {
    (0..VERTICAL_CHUNKS * CHUNK_SIZE)
        .rev()
        .find(|&y| is_solid(world, IVec3::new(x, y, z)))
        .map_or(0, |y| y + 1)
}

/// Fills the (already loaded) region with terrain, then stamps Tristram on it.
fn build_world(world: &VoxelWorld) {
    for x in 0..GRID {
        for z in 0..GRID_Z {
            world.generate_terrain(REGION, x, 0, z);
        }
    }
    tristram::build(world, REGION);
    outdoor::build(world, REGION);
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut fading_materials: ResMut<Assets<FadingMaterial>>,
    mut changed: EventWriter<ChunksChanged>,
) {
    let world = VoxelWorld::new();
    world.load_region(REGION);
    build_world(&world);
    changed.send(ChunksChanged::all());

    let vertex_colored = StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        ..default()
    };
    commands.insert_resource(TerrainMaterial(fading_materials.add(FadingMaterial {
        base: vertex_colored.clone(),
        extension: OccluderFade::default(),
    })));
    commands.insert_resource(ChunkViews::default());
    let vertex_colored = materials.add(vertex_colored);

    let spawn = spawn_point(&world);
    commands
        .spawn((
            Player::spawn_at(spawn),
            CharacterAnim::default(),
            BodyMaterial(vertex_colored.clone()),
            SpatialBundle {
                transform: Transform::from_translation(spawn),
                ..default()
            },
        ))
        .with_children(|parent| {
            // Not a RigPart, so pose rebuilds leave it. Respawn replaces the Player component in place.
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: Color::rgb(1.0, 0.84, 0.55),
                    intensity: 14_000.0,
                    range: 5.0,
                    shadows_enabled: false,
                    ..default()
                },
                transform: Transform::from_xyz(0.0, 1.4, 0.0),
                ..default()
            });
        });

    commands.spawn((
        DestinationMarker,
        PbrBundle {
            mesh: meshes.add(model_mesh(&player_model::destination_marker())),
            material: materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, ..default() }),
            visibility: Visibility::Hidden,
            ..default()
        },
    ));

    commands.insert_resource(ArrowAssets {
        mesh: meshes.add(Mesh::from(Cuboid::from_size(Vec3::new(0.05, 0.05, 0.5)))),
        material: materials.add(StandardMaterial {
            base_color: Color::rgb(0.55, 0.36, 0.16),
            emissive: Color::rgb_linear(0.15, 0.08, 0.02),
            perceptual_roughness: 0.65,
            ..default()
        }),
    });
    for (index, pos) in enemy_spawn_points(&world).into_iter().enumerate() {
        let species = species_at(index);
        spawn_enemy(&mut commands, materials.add(species_material(species)), species, pos);
    }
    for lamp in tristram::LAMPS {
        commands.spawn(PointLightBundle {
            point_light: PointLight {
                color: Color::rgb(1.0, 0.82, 0.46),
                intensity: 9_000.0,
                range: 5.0,
                shadows_enabled: false,
                ..default()
            },
            transform: Transform::from_xyz(
                (lamp.x as f32 + 0.5) * VOXEL_SIZE,
                lamp.y as f32 * VOXEL_SIZE,
                (lamp.z as f32 + 0.5) * VOXEL_SIZE,
            ),
            ..default()
        });
    }

    commands.insert_resource(Voxels(Mutex::new(WorldHandle(world))));

    commands.spawn((
        Camera3dBundle {
            // HDR so spell colours brighter than white can bloom.
            // EV 10.6 is about 1.6× darker than the Blender default (9.7). Paired with the
            // cool 2200 lux moon this stays a readable night, not a black frame.
            camera: Camera { hdr: true, ..default() },
            exposure: Exposure { ev100: 10.6 },
            projection: OrthographicProjection {
                scaling_mode: ScalingMode::FixedVertical(DEFAULT_VIEW_HEIGHT),
                ..default()
            }
            .into(),
            ..default()
        },
        // Additive bloom only on HDR hotspots (spells, light cores). A zero threshold would haze the murk.
        BloomSettings {
            intensity: 0.35,
            low_frequency_boost: 0.65,
            low_frequency_boost_curvature: 0.9,
            high_pass_frequency: 1.0,
            prefilter_settings: BloomPrefilterSettings { threshold: 1.0, threshold_softness: 0.5 },
            composite_mode: BloomCompositeMode::Additive,
        },
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            color: Color::rgb(0.70, 0.76, 0.90),
            illuminance: 2200.0,
            shadows_enabled: true,
            ..default()
        },
        // About 20° elevation. Low enough for long shadows, high enough that voxel faces do not stripe.
        transform: Transform::from_xyz(46.0, 18.0, 12.0).looking_at(Vec3::ZERO, Vec3::Y),
        // An orthographic camera sits CAMERA_DISTANCE away, so put the one cascade around that depth.
        cascade_shadow_config: CascadeShadowConfigBuilder {
            num_cascades: 1,
            minimum_distance: CAMERA_DISTANCE - 30.0,
            first_cascade_far_bound: CAMERA_DISTANCE + 30.0,
            maximum_distance: CAMERA_DISTANCE + 30.0,
            ..default()
        }
        .build(),
        ..default()
    });

    commands.spawn(
        TextBundle::from_section(
            "Left click near an enemy: step in and cut. Click open ground to slip a swing.\n\
             Right click: spell at a nearby enemy, or at the ground. 1-5: spell. Shift sprint, Space jump.\n\
             Wheel: zoom. R: reset. Esc: quit.\n\
             You start on the south crossing into Tristram. The cathedral is north. The moor is south, through the cliff gate.\n\
             Fallen cleave, skeletons block, zombies rot, rogues shoot until an arm is gone.",
            TextStyle { font_size: 18.0, color: Color::WHITE, ..default() },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
    );
    commands.spawn((
        BattleHud,
        TextBundle::from_section(
            "",
            TextStyle { font_size: 18.0, color: Color::WHITE, ..default() },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(100.0),
            left: Val::Px(10.0),
            ..default()
        }),
    ));
}

fn enemy_spawn_points(world: &VoxelWorld) -> Vec<Vec3> {
    tristram::ENEMIES
        .into_iter()
        .chain(outdoor::ENEMIES)
        .map(|(x, z)| Vec3::new(x as f32 + 0.5, surface_height(world, x, z) as f32, z as f32 + 0.5) * VOXEL_SIZE)
        .collect()
}

fn cursor_ray(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform)>,
) -> Option<Ray3d> {
    let cursor = windows.get_single().ok()?.cursor_position()?;
    let (camera, transform) = cameras.get_single().ok()?;
    camera.viewport_to_world(transform, cursor)
}

fn pick_voxel(world: &VoxelWorld, ray: Ray3d) -> Option<Hit> {
    raycast(ray.origin / VOXEL_SIZE, *ray.direction, MAX_PICK_VOXELS, |v| is_solid(world, v))
}

fn handle_input(
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform)>,
    voxels: Res<Voxels>,
    mut players: Query<&mut Player>,
    mut changed: EventWriter<ChunksChanged>,
    mut exit: EventWriter<AppExit>,
    mut commands: Commands,
    falling_pieces: Query<Entity, With<falling::FallingPiece>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut CharacterAnim, &mut Transform)>,
    arrows: Query<Entity, With<HostileShot>>,
) {
    if buttons.pressed(MouseButton::Left) {
        if let Some(ray) = cursor_ray(&windows, &cameras) {
            let terrain = voxels.with(|w| pick_voxel(w, ray));
            let terrain_m = terrain.as_ref().map(|hit| hit.distance * VOXEL_SIZE);
            if let Some(hit) = pick_fighter(ray, terrain_m, &enemies) {
                for mut player in &mut players {
                    player.attack_target = Some(hit.entity);
                    player.aim = Some(hit.part);
                    player.target = None;
                    if buttons.just_pressed(MouseButton::Left) {
                        player.stuck_time = 0.0;
                    }
                }
            } else if let Some(hit) = terrain {
                // Stand in the empty cell in front of the clicked face.
                let cell = hit.voxel + hit.normal;
                let target = Vec3::new(cell.x as f32 + 0.5, cell.y as f32, cell.z as f32 + 0.5) * VOXEL_SIZE;
                for mut player in &mut players {
                    player.attack_target = None;
                    player.aim = None;
                    player.target = Some(target);
                    if buttons.just_pressed(MouseButton::Left) {
                        player.stuck_time = 0.0;
                    }
                }
            }
        }
    }

    if keys.just_pressed(KeyCode::KeyR) {
        let spawn = voxels.with(|w| {
            w.unload_region(REGION);
            w.load_region(REGION);
            build_world(w);
            spawn_point(w)
        });
        for mut player in &mut players {
            *player = Player::spawn_at(spawn);
        }
        for entity in &falling_pieces {
            commands.entity(entity).despawn();
        }
        for entity in &arrows {
            commands.entity(entity).despawn();
        }
        let enemy_spawns = voxels.with(enemy_spawn_points);
        let mut reset_count = 0;
        for (index, ((_entity, mut enemy, mut anim, mut transform), pos)) in
            enemies.iter_mut().zip(enemy_spawns.iter().copied()).enumerate()
        {
            *enemy = Enemy::of(species_at(index));
            *anim = CharacterAnim::default();
            transform.translation = pos;
            transform.scale = Vec3::ONE;
            reset_count += 1;
        }
        if reset_count < enemy_spawns.len() {
            for (index, pos) in enemy_spawns.into_iter().enumerate().skip(reset_count) {
                let species = species_at(index);
                spawn_enemy(&mut commands, materials.add(species_material(species)), species, pos);
            }
        }
        changed.send(ChunksChanged::all());
    }

    if keys.just_pressed(KeyCode::Escape) {
        exit.send(AppExit);
    }
}

fn species_at(index: usize) -> Species {
    const ROSTER: [Species; 13] = [
        Species::Fallen,
        Species::Skeleton,
        Species::Zombie,
        Species::Archer,
        Species::Fallen,
        Species::Skeleton,
        Species::Zombie,
        Species::Archer,
        Species::Skeleton,
        Species::Fallen,
        Species::Zombie,
        Species::Archer,
        Species::Skeleton,
    ];
    ROSTER.get(index).copied().unwrap_or(Species::Fallen)
}

fn species_material(species: Species) -> StandardMaterial {
    match species {
        Species::Fallen => StandardMaterial {
            base_color: Color::rgb(1.0, 0.72, 0.66),
            emissive: Color::rgb_linear(0.35, 0.02, 0.0),
            perceptual_roughness: 0.85,
            ..default()
        },
        Species::Skeleton => StandardMaterial {
            base_color: Color::rgb(0.92, 0.9, 0.82),
            emissive: Color::rgb_linear(0.04, 0.045, 0.03),
            perceptual_roughness: 0.72,
            ..default()
        },
        Species::Zombie => StandardMaterial {
            base_color: Color::rgb(0.78, 0.92, 0.68),
            emissive: Color::rgb_linear(0.02, 0.08, 0.0),
            perceptual_roughness: 0.92,
            ..default()
        },
        Species::Archer => StandardMaterial {
            base_color: Color::rgb(0.86, 0.8, 0.66),
            emissive: Color::rgb_linear(0.04, 0.03, 0.0),
            perceptual_roughness: 0.8,
            ..default()
        },
    }
}

fn species_light(species: Species) -> Color {
    match species {
        Species::Fallen => Color::rgb(1.0, 0.12, 0.04),
        Species::Skeleton => Color::rgb(0.55, 0.7, 0.45),
        Species::Zombie => Color::rgb(0.25, 0.65, 0.12),
        Species::Archer => Color::rgb(0.85, 0.5, 0.12),
    }
}

#[derive(Resource)]
struct ArrowAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

#[derive(Component)]
struct HostileShot {
    vel: Vec3,
    life: f32,
    damage: f32,
    prev: Vec3,
}

fn spawn_enemy(commands: &mut Commands, material: Handle<StandardMaterial>, species: Species, pos: Vec3) {
    let glow = if species == Species::Fallen { 14_000.0 } else { 7_000.0 };
    commands
        .spawn((
            Enemy::of(species),
            CharacterAnim::default(),
            BodyMaterial(material),
            SpatialBundle {
                transform: Transform::from_translation(pos),
                ..default()
            },
        ))
        .with_children(|parent| {
            parent.spawn(PointLightBundle {
                point_light: PointLight {
                    color: species_light(species),
                    intensity: glow,
                    range: 3.4,
                    ..default()
                },
                transform: Transform::from_xyz(0.0, 1.0, 0.0),
                ..default()
            });
        });
}

fn update_enemies(
    time: Res<Time>,
    voxels: Res<Voxels>,
    arrows: Res<ArrowAssets>,
    mut commands: Commands,
    mut sfx: ResMut<sfx::SfxBank>,
    mut players: Query<(&mut Player, &mut Transform), Without<Enemy>>,
    mut enemies: Query<(Entity, &mut Enemy, &mut CharacterAnim, &mut Transform), Without<Player>>,
) {
    let dt = time.delta_seconds().min(0.05);
    let enemy_positions: Vec<(Entity, Vec3)> = enemies.iter().map(|(entity, _, _, transform)| (entity, transform.translation)).collect();
    for (entity, mut enemy, mut anim, mut transform) in &mut enemies {
        let mut movement = Vec3::ZERO;
        let mut face_yaw = None;
        if !enemy.is_down() {
            if let Ok((mut player, mut player_transform)) = players.get_single_mut() {
                let player_pos = player_transform.translation;
                let to_player = Vec3::new(player_pos.x - transform.translation.x, 0.0, player_pos.z - transform.translation.z);
                let distance = to_player.length();
                if distance < ENEMY_AGGRO_RANGE {
                    let plan = plan_fight(&mut enemy.fighter, dt, transform.translation, player_pos);
                    movement += plan.velocity;
                    face_yaw = Some(plan.yaw);
                    movement += separation_push(transform.translation, player_pos, PLAYER_SEPARATION_RADIUS) * 2.0;
                    if plan.strike_damage > 0.0 && player.invuln <= 0.0 {
                        let killed = player.health <= plan.strike_damage;
                        player.health -= plan.strike_damage;
                        player.invuln = HIT_INVULN;
                        if enemy.species() == Species::Zombie {
                            player.poison = ZOMBIE_POISON_SECONDS;
                        }
                        let blow = if enemy.species() == Species::Skeleton { sfx::Cue::Clang } else { sfx::Cue::Hit };
                        sfx.play(&mut commands, blow);
                        sfx.play(&mut commands, if killed { sfx::Cue::Death } else { sfx::Cue::Hurt });
                        if !killed {
                            let away = Vec3::new(
                                player_pos.x - transform.translation.x,
                                0.0,
                                player_pos.z - transform.translation.z,
                            )
                            .normalize_or_zero();
                            player.shove = away * 3.6;
                            player.recoil = 0.18;
                        }
                        if killed {
                            let spawn = voxels.with(spawn_point);
                            *player = Player::spawn_at(spawn);
                            player.invuln = DEATH_INVULN;
                            player_transform.translation = spawn;
                        }
                    }
                    if plan.shot > 0.0 {
                        sfx.play(&mut commands, sfx::Cue::Arrow);
                        let forward = combat::forward_from_yaw(plan.yaw);
                        let origin = transform.translation + Vec3::Y * 1.2 + forward * 0.55;
                        commands.spawn((
                            HostileShot {
                                vel: forward * 18.0,
                                life: 1.35,
                                damage: plan.shot,
                                prev: origin,
                            },
                            PbrBundle {
                                mesh: arrows.mesh.clone(),
                                material: arrows.material.clone(),
                                transform: Transform::from_translation(origin).looking_at(origin + forward, Vec3::Y),
                                ..default()
                            },
                        ));
                    }
                }
            }
        }
        if face_yaw.is_none() && !enemy.is_down() {
            let wander = Vec3::new((anim.phase * TAU + entity.index() as f32).cos(), 0.0, (anim.phase * TAU * 0.7).sin());
            movement += wander.normalize_or_zero() * 0.8;
        }
        for (other, other_pos) in &enemy_positions {
            if *other != entity && !enemy.is_down() {
                movement += separation_push(transform.translation, *other_pos, ENEMY_SEPARATION_RADIUS) * 2.0;
            }
        }
        let delta = if movement.length_squared() > 0.0 { movement * dt } else { Vec3::ZERO };
        let before = transform.translation;
        let stepped = voxels.with(|w| walk_step(before, delta, &ENEMY_BODY, &|v| is_solid(w, v)));
        transform.translation = stepped;
        let horizontal = Vec3::new(stepped.x - before.x, 0.0, stepped.z - before.z);
        anim.speed = horizontal.length() / dt.max(0.0001);
        if let Some(yaw) = face_yaw {
            transform.rotation = Quat::from_rotation_y(yaw);
        } else if horizontal.length_squared() > 0.0001 {
            transform.rotation = Quat::from_rotation_y(f32::atan2(-horizontal.x, -horizontal.z));
        }

        if enemy.hits() != anim.last_hits {
            anim.hurt = 1.0;
        }
        anim.last_hits = enemy.hits();
        anim.hurt = (anim.hurt - dt * 4.0).max(0.0);
        // Wrecks flatten on the root. The walk itself lives on the rig, not a mesh scale.
        transform.scale = if enemy.is_down() { Vec3::new(1.18, 0.34, 1.18) } else { Vec3::ONE };
    }
}

fn update_hostile_shots(
    time: Res<Time>,
    voxels: Res<Voxels>,
    mut commands: Commands,
    mut sfx: ResMut<sfx::SfxBank>,
    mut shots: Query<(Entity, &mut HostileShot, &mut Transform)>,
    mut players: Query<(&mut Player, &mut Transform), Without<HostileShot>>,
) {
    let dt = time.delta_seconds().min(0.05);
    for (entity, mut shot, mut transform) in &mut shots {
        let from = shot.prev;
        let next = transform.translation + shot.vel * dt;
        shot.life -= dt;
        let delta = next - from;
        let len = delta.length();
        let wall = if len > 1.0e-4 {
            let dir = delta / len;
            voxels.with(|w| raycast(from / VOXEL_SIZE, dir, len / VOXEL_SIZE, |v| is_solid(w, v)).map(|hit| hit.distance * VOXEL_SIZE))
        } else {
            None
        };
        let mut struck = false;
        if let Ok((mut player, mut player_transform)) = players.get_single_mut() {
            let chest = player_transform.translation + Vec3::Y * 0.95;
            let (gap, along) = segment_gap(chest, from, next);
            let wall_first = wall.is_some_and(|distance| distance < along);
            if gap < 0.42 && !wall_first {
                struck = true;
                if player.invuln <= 0.0 {
                    let killed = player.health <= shot.damage;
                    player.health -= shot.damage;
                    player.invuln = HIT_INVULN;
                    sfx.play(&mut commands, if killed { sfx::Cue::Death } else { sfx::Cue::Hurt });
                    if !killed {
                        let away = (-shot.vel).normalize_or_zero();
                        player.shove = Vec3::new(away.x, 0.0, away.z) * 2.4;
                        player.recoil = 0.12;
                    }
                    if killed {
                        let spawn = voxels.with(spawn_point);
                        *player = Player::spawn_at(spawn);
                        player.invuln = DEATH_INVULN;
                        player_transform.translation = spawn;
                    }
                }
            }
        }
        let blocked = wall.is_some();
        if struck || blocked || shot.life <= 0.0 {
            if struck {
                sfx.play(&mut commands, sfx::Cue::ArrowHit);
            } else if blocked {
                sfx.play_scaled(&mut commands, sfx::Cue::ArrowHit, 0.45);
            }
            commands.entity(entity).despawn();
        } else {
            shot.prev = next;
            transform.translation = next;
        }
    }
}

fn segment_gap(point: Vec3, from: Vec3, to: Vec3) -> (f32, f32) {
    let segment = to - from;
    let len2 = segment.length_squared();
    if len2 <= 1.0e-8 {
        return (point.distance(from), 0.0);
    }
    let t = ((point - from).dot(segment) / len2).clamp(0.0, 1.0);
    let closest = from + segment * t;
    (point.distance(closest), from.distance(closest))
}

fn separation_push(pos: Vec3, other: Vec3, radius: f32) -> Vec3 {
    let away = Vec3::new(pos.x - other.x, 0.0, pos.z - other.z);
    let distance = away.length();
    if distance <= 0.001 || distance >= radius {
        Vec3::ZERO
    } else {
        away / distance * (1.0 - distance / radius)
    }
}

/// The late windup and the strike are the step through the blade.
fn sword_is_stepping(swing_left: f32) -> bool {
    let strike_at = SWING_RECOVER + SWING_STRIKE;
    let late_at = strike_at + SWING_WINDUP * 0.42;
    swing_left > SWING_RECOVER && swing_left <= late_at
}

/// Feet during a sword clock. Engaged: a short step in, then a slide back.
/// A ground click drops `engaged`, and the feet follow `approach` so the swing can be slipped.
fn sword_footwork(swing_left: f32, facing: Vec3, approach: Vec3, engaged: bool) -> Vec3 {
    if swing_left <= 0.0 {
        return approach;
    }
    let stepping = sword_is_stepping(swing_left);
    let recovering = swing_left <= SWING_RECOVER;
    if !engaged {
        if stepping {
            return approach * 0.55 + facing * (STRIKE_LUNGE * 0.35);
        }
        return approach;
    }
    if stepping {
        facing * STRIKE_LUNGE
    } else if recovering {
        -facing * RECOVER_SLIDE
    } else {
        approach * 0.4
    }
}

fn player_controller(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    voxels: Res<Voxels>,
    assets: Res<spells::SpellAssets>,
    mut rng: ResMut<spells::Rng>,
    mut sfx: ResMut<sfx::SfxBank>,
    mut players: Query<(&mut Player, &mut CharacterAnim, &mut Transform)>,
    mut enemies: Query<(Entity, &mut Enemy, &mut Transform), Without<Player>>,
) {
    let Ok((mut player, mut anim, mut transform)) = players.get_single_mut() else { return };
    let p = &mut *player;
    let dt = time.delta_seconds().min(0.05);
    if dt <= 0.0 {
        return;
    }
    if p.poison > 0.0 {
        p.poison = (p.poison - dt).max(0.0);
        p.health -= ZOMBIE_POISON_DPS * dt;
        if p.health <= 0.0 {
            sfx.play(&mut commands, sfx::Cue::Death);
            let spawn = voxels.with(spawn_point);
            *p = Player::spawn_at(spawn);
            p.invuln = DEATH_INVULN;
            transform.translation = spawn;
            return;
        }
    }

    let mut wish = Vec3::ZERO;
    p.cast_time = (p.cast_time - dt).max(0.0);
    p.invuln = (p.invuln - dt).max(0.0);
    p.recoil = (p.recoil - dt).max(0.0);
    let prev_swing = p.swing_left;
    if p.swing_left > 0.0 {
        p.swing_left = (p.swing_left - dt).max(0.0);
    }
    let top_speed = if keys.pressed(KeyCode::ShiftLeft) { SPRINT_SPEED } else { WALK_SPEED };
    let mut face_lock = None;
    let mut start_swing = false;
    let mut aim_height = 0.85;
    let mut gap: Option<f32> = None;
    if let Some(target_entity) = p.attack_target {
        let aim = p.aim;
        let engaged = enemies.get(target_entity).ok().map(|(_, enemy, enemy_transform)| {
            (enemy.is_down(), enemy_transform.translation, aim.and_then(|part| enemy.center(part)))
        });
        match engaged {
            Some((false, enemy_pos, part_center)) => {
                let to_enemy = Vec3::new(enemy_pos.x - p.body.pos.x, 0.0, enemy_pos.z - p.body.pos.z);
                let distance = to_enemy.length();
                if distance > 0.05 {
                    face_lock = Some(combat::yaw_toward(p.body.pos, enemy_pos));
                }
                gap = Some(distance);
                if distance > MELEE_RANGE {
                    wish = to_enemy.normalize_or_zero() * top_speed.min(distance * 6.0);
                } else if p.swing_left == 0.0 && p.recoil <= 0.0 {
                    aim_height = part_center.map(|center| center.y).unwrap_or(0.85);
                    start_swing = true;
                }
            }
            _ => {
                p.attack_target = None;
                p.aim = None;
            }
        }
    }
    if p.attack_target.is_none() {
        if let Some(target) = p.target {
        let to_target = Vec3::new(target.x - p.body.pos.x, 0.0, target.z - p.body.pos.z);
        let distance = to_target.length();
        if distance < ARRIVE_RADIUS {
            p.target = None;
        } else {
            let top_speed = if keys.pressed(KeyCode::ShiftLeft) { SPRINT_SPEED } else { WALK_SPEED };
            // Ease into the destination instead of overshooting it.
            wish = to_target / distance * top_speed.min(distance * 6.0);
        }
        }
    }

    if start_swing {
        p.swing_height = aim_height.clamp(0.3, 1.4);
        p.swing_left = SWING_WINDUP + SWING_STRIKE + SWING_RECOVER;
        sfx.play(&mut commands, sfx::Cue::Swing);
    }
    if let Some(yaw) = face_lock {
        p.facing = yaw;
    }
    wish = sword_footwork(p.swing_left, combat::forward_from_yaw(p.facing), wish, p.attack_target.is_some());
    if sword_is_stepping(p.swing_left) {
        if let Some(distance) = gap {
            let room = (distance - 0.55).max(0.0);
            let cap = room / dt.max(1.0e-3);
            if wish.length() > cap {
                wish = wish.normalize_or_zero() * cap;
            }
        }
    } else if p.recoil > 0.0 {
        wish = p.shove;
    }

    let responsiveness = if p.body.on_ground { 12.0 } else { 2.0 };
    let blend = 1.0 - (-responsiveness * dt).exp();
    p.body.vel.x += (wish.x - p.body.vel.x) * blend;
    p.body.vel.z += (wish.z - p.body.vel.z) * blend;

    if keys.just_pressed(KeyCode::Space) && p.body.on_ground {
        p.body.vel.y = JUMP_SPEED;
        p.body.on_ground = false;
    }

    let before = p.body.pos;
    let fell_off_world = voxels.with(|w| {
        p.body.step(&HUMAN, dt, &|v| is_solid(w, v));
        (p.body.pos.y < -10.0).then(|| spawn_point(w))
    });
    if let Some(spawn) = fell_off_world {
        sfx.play(&mut commands, sfx::Cue::Death);
        *p = Player::spawn_at(spawn);
    }

    let moved = Vec2::new(p.body.pos.x - before.x, p.body.pos.z - before.z).length() / dt;
    if wish.length() > 0.5 && moved < wish.length() * 0.25 {
        p.stuck_time += dt;
        if p.stuck_time > STUCK_TIMEOUT {
            p.target = None;
            if p.swing_left == 0.0 {
                p.attack_target = None;
                p.aim = None;
            }
        }
    } else {
        p.stuck_time = 0.0;
    }

    let horizontal = Vec2::new(p.body.vel.x, p.body.vel.z);
    anim.speed = if p.body.on_ground { horizontal.length() } else { 0.0 };
    if p.swing_left > 0.0 {
        anim.speed *= 0.35;
    }
    if face_lock.is_none() && horizontal.length() > 0.3 {
        let target_facing = f32::atan2(-horizontal.x, -horizontal.y);
        let diff = (target_facing - p.facing + PI).rem_euclid(TAU) - PI;
        p.facing += diff * (1.0 - (-15.0 * dt).exp());
    }

    let strike_at = SWING_RECOVER + SWING_STRIKE;
    let connect = fell_off_world.is_none() && prev_swing > strike_at && p.swing_left <= strike_at && p.swing_left > 0.0;
    if connect {
        let forward = combat::forward_from_yaw(p.facing);
        let height = p.swing_height;
        let origin = p.body.pos + Vec3::Y * height + forward * 0.35;
        let end = p.body.pos + Vec3::Y * height + forward * 1.7;
        let prefer_target = p.attack_target;
        let prefer_part = p.aim;
        let mut landed: Option<sfx::Cue> = None;
        for (entity, mut enemy, mut enemy_transform) in &mut enemies {
            let flat = Vec3::new(enemy_transform.translation.x - p.body.pos.x, 0.0, enemy_transform.translation.z - p.body.pos.z);
            let distance = flat.length();
            if distance > 2.2 || distance < 0.01 || flat.normalize().dot(forward) < 0.2 {
                continue;
            }
            let prefer = if Some(entity) == prefer_target { prefer_part } else { None };
            let report = enemy.damage_swing(origin, end, MELEE_CUT, prefer, 0.55, &enemy_transform);
            if let Some(impact) = report.impact {
                let push = if report.severed.is_empty() { 0.12 } else { 0.30 };
                let dest = voxels.with(|w| {
                    walk_step(enemy_transform.translation, forward * push, &ENEMY_BODY, &|v| is_solid(w, v))
                });
                enemy_transform.translation = dest;
                spells::spawn_enemy_debris(&mut commands, &assets, &mut rng, impact, enemy_transform.translation, MELEE_CUT);
                let cue = if !report.severed.is_empty() {
                    sfx::Cue::Sever
                } else if enemy.species() == Species::Skeleton {
                    sfx::Cue::Clang
                } else {
                    sfx::Cue::Hit
                };
                landed = Some(if landed == Some(sfx::Cue::Sever) { sfx::Cue::Sever } else { cue });
            }
        }
        if let Some(cue) = landed {
            sfx.play(&mut commands, cue);
        }
    }

    let rate = if p.body.on_ground { 12.0 } else { 25.0 };
    p.visual_y += (p.body.pos.y - p.visual_y) * (1.0 - (-rate * dt).exp());
    p.visual_y = p.visual_y.clamp(p.body.pos.y - MAX_VISUAL_LAG, p.body.pos.y + MAX_VISUAL_LAG);

    transform.translation = Vec3::new(p.body.pos.x, p.visual_y, p.body.pos.z);
    transform.rotation = Quat::from_rotation_y(p.facing);
    transform.scale = Vec3::ONE;
}

fn pose_rigs(
    mut commands: Commands,
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut players: Query<(Entity, &Player, &mut CharacterAnim, &BodyMaterial, Option<&Children>), With<Player>>,
    mut enemies: Query<(Entity, &Enemy, &mut CharacterAnim, &BodyMaterial, Option<&Children>), (With<Enemy>, Without<Player>)>,
    rigs: Query<&RigState>,
    parts: Query<(), With<RigPart>>,
    mut bones: Query<&mut Transform, With<RigPart>>,
) {
    let dt = time.delta_seconds().min(0.05);
    for (entity, player, mut anim, material, children) in &mut players {
        let (phase, weight) = rig::advance_gait(anim.phase, anim.weight, anim.speed, dt);
        anim.phase = phase;
        anim.weight = weight;
        anim.idle_phase += dt * 1.6;
        let attack = rig::player_swing_progress(player.swing_left);
        let input = rig::PoseInput {
            phase,
            weight,
            speed: anim.speed,
            demon: false,
            attack,
            kind: if attack.is_some() { rig::AttackKind::Sword } else { rig::AttackKind::None },
            hop: false,
            hurt: anim.hurt,
            breathe: anim.idle_phase,
            cast: if attack.is_some() { 0.0 } else { (player.cast_time / 0.28).clamp(0.0, 1.0) },
        };
        sync_rig(&mut commands, &mut meshes, &rigs, &parts, &mut bones, entity, children, material.0.clone(), rig::BodyKind::Hero, 0, &input);
    }
    for (entity, enemy, mut anim, material, children) in &mut enemies {
        let gait_speed = if enemy.is_down() { 0.0 } else { anim.speed };
        let (phase, weight) = rig::advance_gait(anim.phase, anim.weight, gait_speed, dt);
        anim.phase = phase;
        anim.weight = weight;
        anim.idle_phase += dt * 1.4;
        let mask = wound_mask(&enemy.fighter);
        let attack = if enemy.is_down() { None } else { enemy.fighter.attack_progress() };
        let species = enemy.species();
        let input = rig::PoseInput {
            phase,
            weight,
            speed: gait_speed,
            demon: matches!(species, Species::Fallen | Species::Zombie),
            attack,
            kind: attack_kind(species, enemy.stance(), mask, attack.is_some()),
            hop: matches!(
                enemy.stance(),
                Stance::HopSwing
                    | Stance::HobbleCharge
                    | Stance::HopTail
                    | Stance::SkeletonHop
                    | Stance::ZombieHop
                    | Stance::ArcherHobble
            ),
            hurt: anim.hurt,
            breathe: anim.idle_phase,
            cast: 0.0,
        };
        sync_rig(
            &mut commands,
            &mut meshes,
            &rigs,
            &parts,
            &mut bones,
            entity,
            children,
            material.0.clone(),
            body_kind(species),
            mask,
            &input,
        );
    }
}

fn body_kind(species: Species) -> rig::BodyKind {
    match species {
        Species::Fallen => rig::BodyKind::Fallen,
        Species::Skeleton => rig::BodyKind::Skeleton,
        Species::Zombie => rig::BodyKind::Zombie,
        Species::Archer => rig::BodyKind::Archer,
    }
}

fn attack_kind(species: Species, stance: Stance, mask: u8, attacking: bool) -> rig::AttackKind {
    use rig::AttackKind::{Bash, Bow, Cleavers, Gore, Kick, None, Sword};
    if !attacking {
        return None;
    }
    let right = mask & player_model::DEMON_RIGHT_ARM == 0;
    let left = mask & player_model::DEMON_LEFT_ARM == 0;
    match species {
        Species::Fallen => demon_attack_kind(mask, true),
        Species::Skeleton | Species::Archer => match stance {
            Stance::ArrowVolley | Stance::ArcherHobble => Bow,
            Stance::BoneKick | Stance::RogueKick => Kick,
            Stance::ShieldBash => Bash,
            Stance::SwordShield | Stance::SwordLunge => Sword,
            _ if right => Sword,
            _ if left => Bash,
            _ => Kick,
        },
        Species::Zombie => {
            if (left || right) && stance != Stance::ZombieBite {
                Cleavers
            } else {
                Gore
            }
        }
    }
}

fn demon_attack_kind(mask: u8, attacking: bool) -> rig::AttackKind {
    if !attacking {
        return rig::AttackKind::None;
    }
    let arms = mask & player_model::DEMON_LEFT_ARM == 0 || mask & player_model::DEMON_RIGHT_ARM == 0;
    if arms {
        return rig::AttackKind::Cleavers;
    }
    let horns = mask & player_model::DEMON_HEAD == 0 && mask & player_model::DEMON_HORNS == 0;
    if horns {
        return rig::AttackKind::Gore;
    }
    if mask & player_model::DEMON_TAIL == 0 {
        rig::AttackKind::Tail
    } else {
        rig::AttackKind::None
    }
}

fn sync_rig(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    rigs: &Query<&RigState>,
    parts: &Query<(), With<RigPart>>,
    bones: &mut Query<&mut Transform, With<RigPart>>,
    entity: Entity,
    children: Option<&Children>,
    material: Handle<StandardMaterial>,
    kind: rig::BodyKind,
    mask: u8,
    input: &rig::PoseInput,
) {
    let rebuild = !matches!(rigs.get(entity), Ok(state) if state.wounds == mask);
    if rebuild {
        if let Some(children) = children {
            let old: Vec<Entity> = children.iter().copied().filter(|child| parts.get(*child).is_ok()).collect();
            for child in old {
                commands.entity(child).despawn_recursive();
            }
        }
        let figure = rig::figure(kind, mask);
        let links = rig::links_of(&figure.bones);
        let soles = rig::soles_of(&figure.bones);
        let pose = rig::pose(input, &links, &soles);
        let (spine, bones) = spawn_figure(commands, meshes, entity, &figure, &material, &pose);
        commands.entity(entity).insert(RigState { wounds: mask, spine, bones, links, soles });
        return;
    }
    let state = rigs.get(entity).unwrap();
    let pose = rig::pose(input, &state.links, &state.soles);
    let spine = state.spine;
    let links = state.bones.clone();
    if let Ok(mut transform) = bones.get_mut(spine) {
        transform.translation = Vec3::new(0.0, pose.bob, 0.0);
        transform.rotation = Quat::from_rotation_z(pose.roll);
        transform.scale = Vec3::ONE;
    }
    for link in links {
        if let Ok(mut transform) = bones.get_mut(link.entity) {
            transform.rotation = rig::driver_rotation(link.driver, &pose);
        }
    }
}

fn spawn_figure(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    root: Entity,
    figure: &rig::Figure,
    material: &Handle<StandardMaterial>,
    pose: &rig::Pose,
) -> (Entity, Vec<BoneLink>) {
    let spine = commands
        .spawn((
            RigPart,
            SpatialBundle {
                transform: Transform {
                    translation: Vec3::new(0.0, pose.bob, 0.0),
                    rotation: Quat::from_rotation_z(pose.roll),
                    ..default()
                },
                ..default()
            },
        ))
        .id();
    commands.entity(root).add_child(spine);
    let mut ids = Vec::with_capacity(figure.bones.len());
    let mut links = Vec::with_capacity(figure.bones.len());
    for bone in &figure.bones {
        let id = commands
            .spawn((
                RigPart,
                PbrBundle {
                    mesh: meshes.add(mesh_from_arrays(&bone.mesh)),
                    material: material.clone(),
                    transform: Transform {
                        translation: bone.local_pos,
                        rotation: rig::driver_rotation(bone.driver, pose),
                        ..default()
                    },
                    ..default()
                },
            ))
            .id();
        let parent = bone.parent.and_then(|index| ids.get(index).copied()).unwrap_or(spine);
        commands.entity(parent).add_child(id);
        ids.push(id);
        links.push(BoneLink { entity: id, driver: bone.driver });
    }
    (spine, links)
}

struct AimHit {
    entity: Entity,
    part: Part,
}

fn best_cursor<'a, T: Copy>(
    ray: Ray3d,
    ground_m: Option<f32>,
    enemies: impl Iterator<Item = (T, &'a Enemy, &'a Transform)>,
) -> Option<(T, Part, Vec3)> {
    let end = ray.origin + *ray.direction * PICK_REACH;
    let mut samples = Vec::new();
    let mut kept = Vec::new();
    for (tag, enemy, transform) in enemies {
        let from = to_local(transform, ray.origin);
        let to = to_local(transform, end);
        let Some(pick) = enemy.fighter.pick_part(from, to, None, CURSOR_PICK_RADIUS) else { continue };
        let local = enemy.center(pick.part).unwrap_or(Vec3::Y * ENEMY_BODY_CENTER_Y);
        samples.push(CursorSample {
            down: enemy.is_down(),
            along: pick.t * PICK_REACH,
            miss: pick.distance,
        });
        kept.push((tag, pick.part, to_world(transform, local)));
    }
    let index = choose_cursor_body(&samples, ground_m, CURSOR_DEPTH_SLACK)?;
    kept.get(index).copied()
}

fn pick_fighter(
    ray: Ray3d,
    ground_m: Option<f32>,
    enemies: &Query<(Entity, &mut Enemy, &mut CharacterAnim, &mut Transform)>,
) -> Option<AimHit> {
    best_cursor(ray, ground_m, enemies.iter().map(|(entity, enemy, _, transform)| (entity, enemy, transform))).map(
        |(entity, part, _)| AimHit { entity, part },
    )
}

pub(crate) fn cursor_enemy_point<'a>(
    ray: Ray3d,
    ground_m: Option<f32>,
    enemies: impl Iterator<Item = (&'a Enemy, &'a Transform)>,
) -> Option<Vec3> {
    best_cursor(ray, ground_m, enemies.map(|(enemy, transform)| ((), enemy, transform))).map(|(_, _, point)| point)
}

fn update_battle_hud(
    players: Query<(&Player, &Transform)>,
    enemies: Query<(Entity, &Enemy, &Transform), Without<Player>>,
    mut texts: Query<&mut Text, With<BattleHud>>,
) {
    let Ok((player, player_transform)) = players.get_single() else { return };
    let Ok(mut text) = texts.get_single_mut() else { return };
    let mut nearest: Option<(f32, String)> = None;
    for (_, enemy, transform) in &enemies {
        if enemy.is_down() {
            continue;
        }
        let distance = Vec2::new(
            transform.translation.x - player_transform.translation.x,
            transform.translation.z - player_transform.translation.z,
        )
        .length();
        if nearest.as_ref().map_or(true, |(best, _)| distance < *best) {
            nearest = Some((distance, enemy.status_line()));
        }
    }
    let foe = if let Some((_, line)) = nearest {
        let aim = match (player.attack_target, player.aim) {
            (Some(target), Some(part)) => enemies
                .iter()
                .find(|(entity, _, _)| *entity == target)
                .map(|(_, enemy, _)| format!("Cutting the {} — ", enemy.part_name(part)))
                .unwrap_or_default(),
            _ => String::new(),
        };
        format!("{aim}Foe: {line}")
    } else {
        "No one left that can fight.".to_string()
    };
    let rot = if player.poison > 0.0 { "  rotting" } else { "" };
    let line = format!("You {:.0}/{:.0}{rot}\n{foe}", player.health.max(0.0), PLAYER_HEALTH);
    text.sections[0].style.color = if player.health < 30.0 { Color::rgb(1.0, 0.45, 0.35) } else { Color::WHITE };
    if text.sections[0].value != line {
        text.sections[0].value = line;
    }
}

fn follow_camera(
    time: Res<Time>,
    mut rig: ResMut<CameraRig>,
    mut shake: ResMut<CameraShake>,
    mut wheel: EventReader<MouseWheel>,
    players: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut cameras: Query<(&mut Transform, &mut Projection), (With<Camera3d>, Without<Player>)>,
) {
    let old_height = rig.view_height;
    for e in wheel.read() {
        let lines = match e.unit {
            MouseScrollUnit::Line => e.y,
            MouseScrollUnit::Pixel => e.y / 100.0,
        };
        rig.view_height = (rig.view_height * (1.0 - lines * 0.1)).clamp(5.0, 60.0);
    }

    let (Ok(player), Ok((mut transform, mut projection))) = (players.get_single(), cameras.get_single_mut()) else {
        return;
    };

    let pivot = player.translation + Vec3::Y * CAMERA_PIVOT_HEIGHT;
    let offset = camera_offset_dir() * CAMERA_DISTANCE;
    *transform = Transform::from_translation(pivot + offset).looking_at(pivot, Vec3::Y);

    // Shake scales with trauma squared so small hits barely register and big blasts jolt.
    shake.trauma = (shake.trauma - time.delta_seconds() * 1.5).max(0.0);
    let t = time.elapsed_seconds();
    let amount = shake.trauma * shake.trauma * 0.3;
    let (right, up): (Vec3, Vec3) = (*transform.right(), *transform.up());
    let jitter = right * (t * 37.0).sin() + up * (t * 29.0).cos();
    transform.translation += jitter * amount;

    if rig.view_height != old_height {
        if let Projection::Orthographic(ortho) = &mut *projection {
            ortho.scaling_mode = ScalingMode::FixedVertical(rig.view_height);
        }
    }
}

fn update_occluder_fade(
    players: Query<&Transform, With<Player>>,
    terrain: Res<TerrainMaterial>,
    mut materials: ResMut<Assets<FadingMaterial>>,
) {
    let Ok(player) = players.get_single() else { return };
    let params = OccluderFadeParams::new(
        player.translation,
        CAMERA_PIVOT_HEIGHT,
        -camera_offset_dir(),
        FADE_RADIUS,
        FADE_STRENGTH,
    );
    // Only touch the asset when something moved; mutating it re-uploads the bind group.
    if materials.get(&terrain.0).is_some_and(|m| m.extension.params != params) {
        if let Some(material) = materials.get_mut(&terrain.0) {
            material.extension.params = params;
        }
    }
}

fn update_marker(
    players: Query<&Player>,
    mut markers: Query<(&mut Transform, &mut Visibility), With<DestinationMarker>>,
) {
    let (Ok(player), Ok((mut transform, mut visibility))) = (players.get_single(), markers.get_single_mut()) else {
        return;
    };
    match player.target {
        Some(target) => {
            transform.translation = target;
            *visibility = Visibility::Visible;
        }
        None => *visibility = Visibility::Hidden,
    }
}

fn remesh_changed_chunks(
    mut commands: Commands,
    mut events: EventReader<ChunksChanged>,
    voxels: Res<Voxels>,
    material: Res<TerrainMaterial>,
    mut views: ResMut<ChunkViews>,
    mut meshes: ResMut<Assets<Mesh>>,
    handles: Query<&Handle<Mesh>>,
) {
    let world_max = IVec3::new(GRID - 1, VERTICAL_CHUNKS - 1, GRID_Z - 1);
    let mut stale = HashSet::new();
    for change in events.read() {
        let (min, max) = (change.min.max(IVec3::ZERO), change.max.min(world_max));
        for y in min.y..=max.y {
            for z in min.z..=max.z {
                for x in min.x..=max.x {
                    stale.insert(IVec3::new(x, y, z));
                }
            }
        }
    }

    let chunk_size_m = CHUNK_SIZE as f32 * VOXEL_SIZE;
    for coord in stale {
        let data = voxels.with(|w| w.get_chunk_mesh(REGION, coord.x, coord.y, coord.z));
        match (data.is_empty(), views.0.get(&coord).copied()) {
            (true, None) => {}
            (true, Some(entity)) => {
                commands.entity(entity).despawn();
                views.0.remove(&coord);
            }
            (false, Some(entity)) => {
                if let Some(mesh) = handles.get(entity).ok().and_then(|h| meshes.get_mut(h)) {
                    *mesh = terrain_mesh(data);
                }
            }
            (false, None) => {
                let entity = commands
                    .spawn(MaterialMeshBundle::<FadingMaterial> {
                        mesh: meshes.add(terrain_mesh(data)),
                        material: material.0.clone(),
                        transform: Transform::from_translation(coord.as_vec3() * chunk_size_m)
                            .with_scale(Vec3::splat(VOXEL_SIZE)),
                        ..default()
                    })
                    .id();
                views.0.insert(coord, entity);
            }
        }
    }
}

#[cfg(test)]
mod combat_feel {
    use super::*;

    #[test]
    fn a_sword_swing_steps_in_then_back_and_a_ground_click_can_leave() {
        let facing = combat::forward_from_yaw(0.0);
        assert!((facing - Vec3::new(0.0, 0.0, -1.0)).length() < 1.0e-4);
        let approach = Vec3::new(4.5, 0.0, 0.0);
        let total = SWING_WINDUP + SWING_STRIKE + SWING_RECOVER;
        assert!((total - rig::SWORD_SWING).abs() < 1.0e-4);

        let early = sword_footwork(total - 0.01, facing, approach, true);
        assert!(early.x > 0.5 && early.x < approach.x * 0.6, "windup should still be able to drift, got {early}");

        let late = sword_footwork(SWING_RECOVER + SWING_STRIKE + SWING_WINDUP * 0.2, facing, approach, true);
        assert!(late.z < -3.0, "the cut should step along the blade, got {late}");
        assert!(late.x.abs() < 0.05, "{late}");

        let back = sword_footwork(SWING_RECOVER * 0.5, facing, approach, true);
        assert!(back.z > 3.0, "recover should slide back off the blade, got {back}");

        let dodge = sword_footwork(total - 0.01, facing, approach, false);
        assert!((dodge - approach).length() < 1.0e-3, "a ground click during the tell keeps full speed, got {dodge}");

        let slipped = sword_footwork(SWING_RECOVER + SWING_STRIKE * 0.5, facing, approach, false);
        assert!(slipped.x > 2.0, "slipping during the cut still carries the click, got {slipped}");

        let progress = 1.0 - (SWING_RECOVER + SWING_STRIKE) / rig::SWORD_SWING;
        assert!((progress - rig::STRIKE_PROGRESS).abs() < 0.02, "blade pose at the hit, progress {progress}");
    }
}
