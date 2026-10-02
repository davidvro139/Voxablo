//! Jointed walk and weapon swings.
//!
//! Each foot is placed on a step the leg can reach. While that foot is down,
//! its character-space z moves backward at the same speed the body moves
//! forward, so the sole stays planted in the world. A 4.5 m/s move is a run:
//! the step is longer than a stroll, the foot is down for less than half its
//! cycle, and both feet leave the ground between footfalls. The swing knee
//! bends to put the airborne foot on that arc. The visual root then drops
//! until the lowest sole vertex sits on the ground, or stays up through the
//! flight. Positive rotation about local +X swings a point below the pivot
//! toward −Z, which is forward. The physics root keeps the one yaw from
//! `combat::yaw_toward`; this module does not apply another heading.

use bevy::math::{Quat, Vec3};
use bevy::transform::components::Transform;
use std::f32::consts::{PI, TAU};

use crate::player_model::{
    self, Color, MeshArrays, VoxelModel, CLEAVER_GRIP, LIMB_ARM_L, LIMB_ARM_R, LIMB_BODY, LIMB_CLEAVER_L,
    LIMB_CLEAVER_R, LIMB_COUNT, LIMB_FOOT_L, LIMB_FOOT_R, LIMB_FORE_L, LIMB_FORE_R, LIMB_HEAD, LIMB_HORN,
    LIMB_SHIN_L, LIMB_SHIN_R, LIMB_SWORD, LIMB_TAIL, LIMB_THIGH_L, LIMB_THIGH_R, SWORD_GRIP,
};

const VISUAL_VOXEL: f32 = 0.05;
const SPRINT_SPEED: f32 = 7.0;
/// Player sword clock: 0.13 windup + 0.09 strike + 0.14 recover.
pub const SWORD_SWING: f32 = 0.36;
/// Progress at which the blade is through and the hit is allowed to land.
pub const STRIKE_PROGRESS: f32 = 0.36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Driver {
    None,
    HipL,
    HipR,
    KneeL,
    KneeR,
    AnkleL,
    AnkleR,
    ShoulderL,
    ShoulderR,
    ElbowL,
    ElbowR,
    Head,
    Tail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackKind {
    None,
    Sword,
    Cleavers,
    Gore,
    Tail,
    /// Shield arm shoves straight forward.
    Bash,
    /// Both feet snap forward. Used when the arms are gone.
    Kick,
    /// Bow arm stays out. The drawing arm pulls back, then lets go.
    Bow,
}

#[derive(Clone, Copy, Debug)]
pub struct PoseInput {
    pub phase: f32,
    pub weight: f32,
    /// Horizontal speed in metres per second. The step length is this speed
    /// divided into the step rate, so the planted foot matches the body.
    pub speed: f32,
    pub demon: bool,
    pub attack: Option<f32>,
    pub kind: AttackKind,
    pub hop: bool,
    pub hurt: f32,
    pub breathe: f32,
    pub cast: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Pose {
    pub hip_l: f32,
    pub hip_r: f32,
    pub knee_l: f32,
    pub knee_r: f32,
    pub ankle_l: f32,
    pub ankle_r: f32,
    pub shoulder_l: f32,
    pub shoulder_r: f32,
    pub elbow_l: f32,
    pub elbow_r: f32,
    pub head: f32,
    pub tail: f32,
    pub bob: f32,
    pub roll: f32,
}

pub struct Bone {
    pub part: u8,
    pub parent: Option<usize>,
    pub local_pos: Vec3,
    pub driver: Driver,
    pub mesh: MeshArrays,
}

/// Rest transform of one bone. The game stores this so a step can plant the real sole
/// without rebuilding the voxel mesh.
#[derive(Clone, Copy, Debug)]
pub struct ChainLink {
    pub local_pos: Vec3,
    pub driver: Driver,
    pub parent: Option<usize>,
}

pub struct Figure {
    pub bones: Vec<Bone>,
}

struct PartGeom {
    pivot: Vec3,
    mesh: MeshArrays,
}

/// How far the body travels between one footfall and the next.
/// A stroll is near 0.6 m. 4.5 m/s is a run near 1.35 m. A sprint is near 1.8 m.
pub fn step_length(speed: f32) -> f32 {
    if speed < 0.08 {
        return 0.0;
    }
    (0.40 + 0.21 * speed).clamp(0.45, 1.85)
}

/// Footfalls per second. This is speed / step length, so a planted sole that
/// covers one step length is stationary in the world.
pub fn step_rate(speed: f32) -> f32 {
    let step = step_length(speed);
    if step <= 0.0 { 0.0 } else { speed / step }
}

pub fn advance_gait(phase: f32, weight: f32, speed: f32, dt: f32) -> (f32, f32) {
    let target = if speed > 0.2 { 1.0 } else { 0.0 };
    let blend = 1.0 - (-6.0 * dt).exp();
    let weight = weight + (target - weight) * blend;
    let phase = phase + step_rate(speed) * PI * dt;
    (phase, weight)
}

pub fn player_swing_progress(swing_left: f32) -> Option<f32> {
    if swing_left <= 0.0 {
        None
    } else {
        Some((1.0 - swing_left / SWORD_SWING).clamp(0.0, 1.0))
    }
}

pub fn hero_figure() -> Figure {
    figure_from(&player_model::adventurer())
}

pub fn demon_figure(wounds: u8) -> Figure {
    figure_from(&player_model::fallen_demon(wounds))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    Hero,
    Fallen,
    Skeleton,
    Zombie,
    Archer,
}

pub fn figure(kind: BodyKind, wounds: u8) -> Figure {
    match kind {
        BodyKind::Hero => hero_figure(),
        BodyKind::Fallen => demon_figure(wounds),
        BodyKind::Skeleton => figure_from(&player_model::skeleton(wounds)),
        BodyKind::Zombie => figure_from(&player_model::zombie(wounds)),
        BodyKind::Archer => figure_from(&player_model::corrupt_rogue(wounds)),
    }
}

pub fn pose(input: &PoseInput, links: &[ChainLink], soles: &[(usize, Vec3)]) -> Pose {
    let weight = input.weight.clamp(0.0, 1.0);
    let run = ((input.speed - 2.2) / (SPRINT_SPEED - 2.2)).clamp(0.0, 1.0);
    let stand_knee = if input.demon { 0.40 } else { 0.0 };
    let rest = if input.demon { 0.20 } else { 0.15 };
    let bend = 0.65 + run * 0.35;
    let arm_amp = (0.38 + run * 0.42) * weight;
    let mut shoulder_l = -input.phase.cos() * arm_amp;
    let mut shoulder_r = -shoulder_l;
    let mut elbow_l = rest + shoulder_l.max(0.0) * bend;
    let mut elbow_r = rest + shoulder_r.max(0.0) * bend;
    let mut head = input.phase.sin() * 0.05 * weight;
    let mut tail = input.phase.sin() * 0.22 * weight;

    if let Some(t) = input.attack {
        let t = t.clamp(0.0, 1.0);
        match input.kind {
            AttackKind::Sword => {
                let swing = sample_swing(t);
                shoulder_r = swing.0;
                elbow_r = swing.1;
                shoulder_l = -swing.0 * 0.45;
                elbow_l = rest + shoulder_l.max(0.0) * 0.5;
            }
            AttackKind::Cleavers => {
                let swing = sample_swing(t);
                shoulder_l = swing.0;
                shoulder_r = swing.0;
                elbow_l = swing.1;
                elbow_r = swing.1;
            }
            AttackKind::Gore => head = sample_keys(t, &GORE_HEAD),
            AttackKind::Tail => tail = sample_keys(t, &TAIL_WHIP),
            AttackKind::Bash => {
                shoulder_l = sample_keys(t, &BASH_SHOULDER);
                elbow_l = sample_keys(t, &BASH_ELBOW);
                shoulder_r = -shoulder_l * 0.25;
            }
            AttackKind::Bow => {
                shoulder_l = sample_keys(t, &BOW_L);
                shoulder_r = sample_keys(t, &BOW_R);
                elbow_l = sample_keys(t, &BOW_ELBOW_L);
                elbow_r = sample_keys(t, &BOW_ELBOW_R);
            }
            AttackKind::Kick | AttackKind::None => {}
        }
    } else if input.cast > 0.0 && !input.demon {
        let cast = input.cast.clamp(0.0, 1.0);
        shoulder_l += (1.05 - shoulder_l) * cast;
        shoulder_r += (1.05 - shoulder_r) * cast;
        elbow_l += (0.35 - elbow_l) * cast;
        elbow_r += (0.35 - elbow_r) * cast;
    }

    let mut pose = Pose {
        hip_l: 0.0,
        hip_r: 0.0,
        knee_l: stand_knee,
        knee_r: stand_knee,
        ankle_l: stand_knee,
        ankle_r: stand_knee,
        shoulder_l,
        shoulder_r,
        elbow_l,
        elbow_r,
        head,
        tail,
        bob: 0.0,
        roll: input.phase.sin() * 0.06 * weight + input.hurt * 0.12,
    };

    let legs = legs_from(links, soles);
    let moving = input.speed >= 0.08 && weight > 0.001 && !legs.is_empty();
    let desired = if moving {
        plant_feet(input, links, &legs, weight, stand_knee, &mut pose)
    } else {
        0.0
    };
    // Mesh correction: the solved point is the sole patch, and claws or toes
    // can hang below it. Shift the visual root so the lowest vertex lands on
    // `desired` (the ground, or the flight arc).
    pose.bob += sole_drop(links, soles, &pose) + desired;
    pose.bob += (1.0 - weight) * input.breathe.sin() * 0.012;
    pose.bob += input.hurt * 0.03;
    if input.hop {
        pose.bob += input.phase.sin().max(0.0) * 0.12 * weight.max(0.45);
    }
    // After the plant, so the sole correction does not pin the kicking foot back down.
    if input.kind == AttackKind::Kick {
        if let Some(t) = input.attack {
            let t = t.clamp(0.0, 1.0);
            let hip = sample_keys(t, &KICK_HIP);
            let knee = sample_keys(t, &KICK_KNEE);
            pose.hip_l = hip;
            pose.hip_r = hip;
            pose.knee_l = knee;
            pose.knee_r = knee;
            pose.ankle_l = knee - hip;
            pose.ankle_r = pose.ankle_l;
        }
    }
    pose
}

pub fn driver_rotation(driver: Driver, pose: &Pose) -> Quat {
    let angle = match driver {
        Driver::HipL => pose.hip_l,
        Driver::HipR => pose.hip_r,
        Driver::KneeL => -pose.knee_l,
        Driver::KneeR => -pose.knee_r,
        Driver::AnkleL => pose.ankle_l,
        Driver::AnkleR => pose.ankle_r,
        Driver::ShoulderL => pose.shoulder_l,
        Driver::ShoulderR => pose.shoulder_r,
        Driver::ElbowL => -pose.elbow_l,
        Driver::ElbowR => -pose.elbow_r,
        Driver::Head => pose.head,
        Driver::Tail => pose.tail,
        Driver::None => 0.0,
    };
    Quat::from_rotation_x(angle)
}

pub struct BakedPart {
    pub part: u8,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub indices: Vec<u32>,
}

pub fn bake_parts(figure: &Figure, pose: &Pose) -> Vec<BakedPart> {
    let worlds = bone_worlds(figure, pose);
    figure
        .bones
        .iter()
        .zip(worlds)
        .map(|(bone, world)| {
            let mut positions = Vec::with_capacity(bone.mesh.positions.len());
            let mut normals = Vec::with_capacity(bone.mesh.normals.len());
            for (position, normal) in bone.mesh.positions.iter().zip(&bone.mesh.normals) {
                positions.push(world.transform_point(Vec3::from(*position)).into());
                normals.push((world.rotation * Vec3::from(*normal)).into());
            }
            BakedPart {
                part: bone.part,
                positions,
                normals,
                indices: bone.mesh.indices.clone(),
            }
        })
        .collect()
}

fn bone_worlds(figure: &Figure, pose: &Pose) -> Vec<Transform> {
    link_worlds(&links_of(&figure.bones), pose)
}

pub fn links_of(bones: &[Bone]) -> Vec<ChainLink> {
    bones
        .iter()
        .map(|bone| ChainLink { local_pos: bone.local_pos, driver: bone.driver, parent: bone.parent })
        .collect()
}

/// Vertices that can touch the ground: the foot, or the whole lower leg when there is no foot.
pub fn soles_of(bones: &[Bone]) -> Vec<(usize, Vec3)> {
    let has_foot_l = bones.iter().any(|bone| bone.part == LIMB_FOOT_L);
    let has_foot_r = bones.iter().any(|bone| bone.part == LIMB_FOOT_R);
    let mut soles = Vec::new();
    for (index, bone) in bones.iter().enumerate() {
        let contact = match bone.part {
            LIMB_FOOT_L | LIMB_FOOT_R => true,
            LIMB_SHIN_L => !has_foot_l,
            LIMB_SHIN_R => !has_foot_r,
            _ => false,
        };
        if !contact {
            continue;
        }
        soles.extend(bone.mesh.positions.iter().map(|position| (index, Vec3::from(*position))));
    }
    soles
}

fn link_worlds(links: &[ChainLink], pose: &Pose) -> Vec<Transform> {
    let spine = Transform {
        translation: Vec3::new(0.0, pose.bob, 0.0),
        rotation: Quat::from_rotation_z(pose.roll),
        scale: Vec3::ONE,
    };
    let mut worlds = Vec::with_capacity(links.len());
    for link in links {
        let local = Transform {
            translation: link.local_pos,
            rotation: driver_rotation(link.driver, pose),
            scale: Vec3::ONE,
        };
        let parent = link.parent.and_then(|index| worlds.get(index).copied()).unwrap_or(spine);
        worlds.push(parent.mul_transform(local));
    }
    worlds
}

/// How far to drop the visual root so the lowest sole vertex sits on y = 0.
/// Claws and toes are included, which a straight-leg axis misses once the knee bends.
fn sole_drop(links: &[ChainLink], soles: &[(usize, Vec3)], pose: &Pose) -> f32 {
    if soles.is_empty() {
        return 0.0;
    }
    let worlds = link_worlds(links, pose);
    let mut lowest = f32::MAX;
    for &(index, local) in soles {
        lowest = lowest.min(worlds[index].transform_point(local).y);
    }
    -lowest
}

struct LegIk {
    left: bool,
    sole_index: usize,
    sole_local: Vec3,
    hip_y: f32,
    hip_z: f32,
    reach: f32,
}

struct FootGoal {
    z: f32,
    y: f32,
    stance: bool,
}

fn legs_from(links: &[ChainLink], soles: &[(usize, Vec3)]) -> Vec<LegIk> {
    let rest = link_worlds(links, &Pose {
        hip_l: 0.0,
        hip_r: 0.0,
        knee_l: 0.0,
        knee_r: 0.0,
        ankle_l: 0.0,
        ankle_r: 0.0,
        shoulder_l: 0.0,
        shoulder_r: 0.0,
        elbow_l: 0.0,
        elbow_r: 0.0,
        head: 0.0,
        tail: 0.0,
        bob: 0.0,
        roll: 0.0,
    });
    let mut legs = Vec::new();
    for left in [true, false] {
        let (hip_driver, knee_driver, ankle_driver) = if left {
            (Driver::HipL, Driver::KneeL, Driver::AnkleL)
        } else {
            (Driver::HipR, Driver::KneeR, Driver::AnkleR)
        };
        let Some(hip_index) = index_of(links, hip_driver) else { continue };
        let Some(knee_index) = index_of(links, knee_driver) else { continue };
        let ankle_index = index_of(links, ankle_driver);
        let sole_index = if ankle_index.is_some_and(|index| sole_patch(soles, index).is_some()) {
            ankle_index.unwrap()
        } else if sole_patch(soles, knee_index).is_some() {
            knee_index
        } else {
            continue;
        };
        let Some(sole_local) = sole_patch(soles, sole_index) else { continue };
        let hip = rest[hip_index].translation;
        let sole = rest[sole_index].transform_point(sole_local);
        legs.push(LegIk {
            left,
            sole_index,
            sole_local,
            hip_y: hip.y,
            hip_z: hip.z,
            reach: hip.distance(sole).max(0.05),
        });
    }
    legs
}

fn index_of(links: &[ChainLink], driver: Driver) -> Option<usize> {
    links.iter().position(|link| link.driver == driver)
}

/// Centre of the vertices that actually touch the ground, not a corner claw.
fn sole_patch(soles: &[(usize, Vec3)], index: usize) -> Option<Vec3> {
    let mut min_y = f32::MAX;
    for &(bone, vertex) in soles {
        if bone == index {
            min_y = min_y.min(vertex.y);
        }
    }
    if min_y == f32::MAX {
        return None;
    }
    let mut sum = Vec3::ZERO;
    let mut count = 0.0;
    for &(bone, vertex) in soles {
        if bone == index && vertex.y <= min_y + 0.004 {
            sum += vertex;
            count += 1.0;
        }
    }
    (count > 0.0).then_some(sum / count)
}

/// Fraction of one foot's cycle spent on the ground. A long step on a short
/// leg drops below one half, which is the flight between running steps.
fn stance_duty(step: f32, reach: f32) -> f32 {
    let max_contact = reach.max(0.2) * 0.85;
    (max_contact / (2.0 * step.max(0.05))).clamp(0.18, 0.62)
}

fn swing_lift(duty: f32) -> f32 {
    0.07 + (0.62 - duty).max(0.0) * 0.55
}

/// `(z, y, planted)`. Phase 0 is heel contact: the foot is forward, then it
/// tracks backward in a straight line, then it arcs through to the next contact.
fn foot_target(local_phase: f32, hip_z: f32, step: f32, duty: f32, lift: f32) -> (f32, f32, bool) {
    let cycle = local_phase.rem_euclid(TAU);
    let stance = duty * TAU;
    let half = duty * step;
    if cycle <= stance || stance >= TAU - 1.0e-3 {
        let u = if stance > 1.0e-4 { cycle / stance } else { 0.0 };
        (hip_z + half * (2.0 * u - 1.0), 0.0, true)
    } else {
        let swing = (TAU - stance).max(1.0e-4);
        let u = (cycle - stance) / swing;
        let z = hip_z + half * (u * PI).cos();
        let y = (u * PI).sin() * lift;
        (z, y, false)
    }
}

fn hip_y_for(hip_z: f32, reach: f32, target_z: f32, target_y: f32) -> f32 {
    let d = (target_z - hip_z).abs().min(reach * 0.92);
    let vertical = (reach * reach - d * d).max(0.0).sqrt();
    target_y + vertical * 0.99
}

/// Solve both legs so their sole patches sit on the step, and return the y the
/// lowest sole should keep (0 on the ground, or the flight arc).
fn plant_feet(
    input: &PoseInput,
    links: &[ChainLink],
    legs: &[LegIk],
    weight: f32,
    stand_knee: f32,
    pose: &mut Pose,
) -> f32 {
    let step = step_length(input.speed);
    let reach = legs.iter().map(|leg| leg.reach).fold(f32::MAX, f32::min);
    let duty = stance_duty(step, reach);
    let lift = swing_lift(duty);
    let goals: Vec<FootGoal> = legs
        .iter()
        .map(|leg| {
            let local = input.phase + if leg.left { 0.0 } else { PI };
            let (z, y, stance) = foot_target(local, leg.hip_z, step, duty, lift);
            FootGoal { z, y, stance }
        })
        .collect();

    let mut any_stance = false;
    let mut air_min = f32::MAX;
    let mut hip_y = f32::MAX;
    for (leg, goal) in legs.iter().zip(&goals) {
        air_min = air_min.min(goal.y);
        if goal.stance {
            any_stance = true;
            hip_y = hip_y.min(hip_y_for(leg.hip_z, leg.reach, goal.z, goal.y));
        }
    }
    if !any_stance {
        hip_y = f32::MAX;
        for (leg, goal) in legs.iter().zip(&goals) {
            hip_y = hip_y.min(hip_y_for(leg.hip_z, leg.reach, goal.z, goal.y));
        }
    }
    let rest_y = legs[0].hip_y;
    if !hip_y.is_finite() {
        hip_y = rest_y;
    }
    hip_y = hip_y.clamp(rest_y * 0.45, rest_y + lift + 0.05);
    pose.bob = hip_y - rest_y;

    let mut solved = Vec::with_capacity(legs.len());
    for (leg, goal) in legs.iter().zip(&goals) {
        solved.push(solve_leg(links, leg, pose, goal.y, goal.z));
    }
    // A short crouch buys reach when the first hip height left the sole short of the step.
    if sole_miss(links, legs, pose, &goals) > 0.025 {
        pose.bob -= 0.03;
        solved.clear();
        for (leg, goal) in legs.iter().zip(&goals) {
            solved.push(solve_leg(links, leg, pose, goal.y, goal.z));
        }
    }

    for (leg, (hip, knee)) in legs.iter().zip(solved) {
        let hip = hip * weight;
        let knee = stand_knee + (knee - stand_knee) * weight;
        write_leg(pose, leg.left, hip, knee);
    }

    if weight > 0.98 && !any_stance { air_min } else { 0.0 }
}

fn sole_miss(links: &[ChainLink], legs: &[LegIk], pose: &Pose, goals: &[FootGoal]) -> f32 {
    let mut worst = 0.0_f32;
    for (leg, goal) in legs.iter().zip(goals) {
        let point = leg_sole(links, leg, pose);
        worst = worst.max((point.z - goal.z).abs()).max((point.y - goal.y).abs());
    }
    worst
}

fn solve_leg(links: &[ChainLink], leg: &LegIk, pose: &mut Pose, target_y: f32, target_z: f32) -> (f32, f32) {
    let mut hip = ((leg.hip_z - target_z) * 1.5).clamp(-1.1, 1.1);
    let mut knee = 0.3;
    write_leg(pose, leg.left, hip, knee);
    for _ in 0..16 {
        let point = leg_sole(links, leg, pose);
        let ez = target_z - point.z;
        let ey = target_y - point.y;
        if ez * ez + ey * ey < 1.0e-8 {
            break;
        }
        let eps = 1.0e-3;
        write_leg(pose, leg.left, hip + eps, knee);
        let bumped_hip = leg_sole(links, leg, pose);
        write_leg(pose, leg.left, hip, knee + eps);
        let bumped_knee = leg_sole(links, leg, pose);
        let dz_h = (bumped_hip.z - point.z) / eps;
        let dy_h = (bumped_hip.y - point.y) / eps;
        let dz_k = (bumped_knee.z - point.z) / eps;
        let dy_k = (bumped_knee.y - point.y) / eps;
        let det = dz_h * dy_k - dz_k * dy_h;
        if det.abs() < 1.0e-5 {
            break;
        }
        hip += ((ez * dy_k - ey * dz_k) / det).clamp(-0.45, 0.45) * 0.8;
        knee += ((dz_h * ey - dy_h * ez) / det).clamp(-0.45, 0.45) * 0.8;
        hip = hip.clamp(-1.4, 1.4);
        knee = knee.clamp(0.0, 2.4);
        write_leg(pose, leg.left, hip, knee);
    }
    (hip, knee)
}

fn write_leg(pose: &mut Pose, left: bool, hip: f32, knee: f32) {
    if left {
        pose.hip_l = hip;
        pose.knee_l = knee;
        pose.ankle_l = knee - hip;
    } else {
        pose.hip_r = hip;
        pose.knee_r = knee;
        pose.ankle_r = knee - hip;
    }
}

fn leg_sole(links: &[ChainLink], leg: &LegIk, pose: &Pose) -> Vec3 {
    let worlds = link_worlds(links, pose);
    worlds[leg.sole_index].transform_point(leg.sole_local)
}

/// Shoulder (hand forward when positive) and elbow (fold back when positive).
const SWING_SHOULDER: [(f32, f32); 6] = [
    (0.0, 0.22),
    (0.20, -2.10),
    (STRIKE_PROGRESS, 1.40),
    (0.55, 1.15),
    (0.80, 0.40),
    (1.0, 0.22),
];
const SWING_ELBOW: [(f32, f32); 6] = [
    (0.0, 0.22),
    (0.20, 0.85),
    (STRIKE_PROGRESS, 0.12),
    (0.55, 0.15),
    (0.80, 0.80),
    (1.0, 0.22),
];
/// Head pitch. Negative tips the face toward −Z, because the skull sits above the neck.
const GORE_HEAD: [(f32, f32); 5] = [(0.0, 0.0), (0.20, 0.40), (STRIKE_PROGRESS, -0.90), (0.80, -0.10), (1.0, 0.0)];
/// Tail pitch. Negative throws the tail up and forward from its rest behind the body.
const TAIL_WHIP: [(f32, f32); 5] = [(0.0, 0.0), (0.20, 0.45), (STRIKE_PROGRESS, -2.0), (0.80, -0.30), (1.0, 0.0)];
/// Shield bash. Positive shoulder sends the board forward at the strike frame.
const BASH_SHOULDER: [(f32, f32); 5] = [(0.0, 0.15), (0.18, -0.5), (STRIKE_PROGRESS, 1.55), (0.72, 0.55), (1.0, 0.15)];
const BASH_ELBOW: [(f32, f32); 5] = [(0.0, 0.3), (0.18, 0.65), (STRIKE_PROGRESS, 0.1), (0.72, 0.25), (1.0, 0.3)];
/// Bow. The left arm holds the stave forward. The right arm draws back, then releases.
const BOW_L: [(f32, f32); 5] = [(0.0, 0.7), (0.20, 1.15), (STRIKE_PROGRESS, 1.05), (0.75, 0.55), (1.0, 0.35)];
const BOW_R: [(f32, f32); 5] = [(0.0, 0.15), (0.20, -0.9), (STRIKE_PROGRESS, 1.3), (0.75, 0.35), (1.0, 0.15)];
const BOW_ELBOW_L: [(f32, f32); 5] = [(0.0, 0.25), (0.20, 0.2), (STRIKE_PROGRESS, 0.2), (0.75, 0.25), (1.0, 0.25)];
const BOW_ELBOW_R: [(f32, f32); 5] = [(0.0, 0.3), (0.20, 0.75), (STRIKE_PROGRESS, 0.15), (0.75, 0.3), (1.0, 0.3)];
/// Kick. Negative hip chambers the foot back; positive hip snaps it forward.
const KICK_HIP: [(f32, f32); 5] = [(0.0, 0.05), (0.18, -0.45), (STRIKE_PROGRESS, 1.25), (0.72, 0.35), (1.0, 0.05)];
const KICK_KNEE: [(f32, f32); 5] = [(0.0, 0.15), (0.18, 1.15), (STRIKE_PROGRESS, 0.2), (0.72, 0.4), (1.0, 0.15)];

fn sample_swing(t: f32) -> (f32, f32) {
    (sample_keys(t, &SWING_SHOULDER), sample_keys(t, &SWING_ELBOW))
}

fn sample_keys(t: f32, keys: &[(f32, f32)]) -> f32 {
    if t <= keys[0].0 {
        return keys[0].1;
    }
    for pair in keys.windows(2) {
        if t <= pair[1].0 {
            let span = (pair[1].0 - pair[0].0).max(1.0e-4);
            let u = (t - pair[0].0) / span;
            return pair[0].1 + (pair[1].1 - pair[0].1) * u;
        }
    }
    keys[keys.len() - 1].1
}

fn figure_from(model: &VoxelModel) -> Figure {
    let mut geom: [Option<PartGeom>; LIMB_COUNT] = std::array::from_fn(|_| None);
    for part in 0..LIMB_COUNT as u8 {
        let Some((min, max)) = model.part_aabb(VISUAL_VOXEL, part, None) else { continue };
        let mut mesh = model.mesh_part(VISUAL_VOXEL, part);
        if mesh.positions.is_empty() {
            continue;
        }
        let grip = grip_color(part).and_then(|color| model.part_aabb(VISUAL_VOXEL, part, Some(color)));
        let pivot = pivot_for(part, min, max, grip);
        for position in &mut mesh.positions {
            position[0] -= pivot.x;
            position[1] -= pivot.y;
            position[2] -= pivot.z;
        }
        geom[part as usize] = Some(PartGeom { pivot, mesh });
    }

    const ORDER: [u8; LIMB_COUNT] = [
        LIMB_BODY,
        LIMB_HEAD,
        LIMB_HORN,
        LIMB_THIGH_L,
        LIMB_SHIN_L,
        LIMB_FOOT_L,
        LIMB_THIGH_R,
        LIMB_SHIN_R,
        LIMB_FOOT_R,
        LIMB_ARM_L,
        LIMB_FORE_L,
        LIMB_CLEAVER_L,
        LIMB_ARM_R,
        LIMB_FORE_R,
        LIMB_SWORD,
        LIMB_CLEAVER_R,
        LIMB_TAIL,
    ];
    let mut index_of: [Option<usize>; LIMB_COUNT] = [None; LIMB_COUNT];
    let mut bones = Vec::new();
    for part in ORDER {
        let Some(part_geom) = geom[part as usize].take() else { continue };
        let parent_part = resolve_parent(part, &index_of);
        let parent = parent_part.and_then(|id| index_of[id as usize]);
        // Earlier bones store pivot-relative translations, so the chain sums back to character space.
        let parent_pivot = parent.map(|index| pivot_in_character(&bones, index)).unwrap_or(Vec3::ZERO);
        let local_pos = part_geom.pivot - parent_pivot;
        index_of[part as usize] = Some(bones.len());
        bones.push(Bone {
            part,
            parent,
            local_pos,
            driver: driver_of(part),
            mesh: part_geom.mesh,
        });
    }

    Figure { bones }
}

fn pivot_in_character(bones: &[Bone], index: usize) -> Vec3 {
    let mut cursor = Some(index);
    let mut offset = Vec3::ZERO;
    while let Some(current) = cursor {
        offset += bones[current].local_pos;
        cursor = bones[current].parent;
    }
    offset
}

fn resolve_parent(part: u8, index_of: &[Option<usize>; LIMB_COUNT]) -> Option<u8> {
    let mut parent = nominal_parent(part);
    while let Some(id) = parent {
        if index_of[id as usize].is_some() {
            return Some(id);
        }
        parent = nominal_parent(id);
    }
    None
}

fn nominal_parent(part: u8) -> Option<u8> {
    Some(match part {
        LIMB_SHIN_L => LIMB_THIGH_L,
        LIMB_FOOT_L => LIMB_SHIN_L,
        LIMB_SHIN_R => LIMB_THIGH_R,
        LIMB_FOOT_R => LIMB_SHIN_R,
        LIMB_FORE_L => LIMB_ARM_L,
        LIMB_FORE_R => LIMB_ARM_R,
        LIMB_SWORD => LIMB_FORE_R,
        LIMB_CLEAVER_L => LIMB_FORE_L,
        LIMB_CLEAVER_R => LIMB_FORE_R,
        LIMB_HORN => LIMB_HEAD,
        _ => return None,
    })
}

fn driver_of(part: u8) -> Driver {
    match part {
        LIMB_THIGH_L => Driver::HipL,
        LIMB_SHIN_L => Driver::KneeL,
        LIMB_FOOT_L => Driver::AnkleL,
        LIMB_THIGH_R => Driver::HipR,
        LIMB_SHIN_R => Driver::KneeR,
        LIMB_FOOT_R => Driver::AnkleR,
        LIMB_ARM_L => Driver::ShoulderL,
        LIMB_FORE_L => Driver::ElbowL,
        LIMB_ARM_R => Driver::ShoulderR,
        LIMB_FORE_R => Driver::ElbowR,
        LIMB_HEAD => Driver::Head,
        LIMB_TAIL => Driver::Tail,
        _ => Driver::None,
    }
}

fn grip_color(part: u8) -> Option<Color> {
    match part {
        LIMB_SWORD => Some(SWORD_GRIP),
        LIMB_CLEAVER_L | LIMB_CLEAVER_R => Some(CLEAVER_GRIP),
        _ => None,
    }
}

fn pivot_for(part: u8, min: Vec3, max: Vec3, grip: Option<(Vec3, Vec3)>) -> Vec3 {
    let mid_x = (min.x + max.x) * 0.5;
    let mid_y = (min.y + max.y) * 0.5;
    let mid_z = (min.z + max.z) * 0.5;
    match part {
        LIMB_BODY => Vec3::ZERO,
        LIMB_HEAD => Vec3::new(mid_x, min.y, mid_z),
        LIMB_TAIL => Vec3::new(mid_x, mid_y, min.z),
        LIMB_SWORD | LIMB_CLEAVER_L | LIMB_CLEAVER_R => {
            let (gmin, gmax) = grip.unwrap_or((min, max));
            Vec3::new((gmin.x + gmax.x) * 0.5, gmin.y, (gmin.z + gmax.z) * 0.5)
        }
        _ => Vec3::new(mid_x, max.y, mid_z),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_model::{DEMON_LEFT_ARM, DEMON_LEFT_LEG, DEMON_RIGHT_LEG};

    fn posed(
        figure: &Figure,
        phase: f32,
        weight: f32,
        attack: Option<f32>,
        kind: AttackKind,
        demon: bool,
        speed: f32,
    ) -> Pose {
        pose(&input(phase, weight, attack, kind, demon, speed), &links_of(&figure.bones), &soles_of(&figure.bones))
    }

    fn input(phase: f32, weight: f32, attack: Option<f32>, kind: AttackKind, demon: bool, speed: f32) -> PoseInput {
        PoseInput {
            phase,
            weight,
            speed,
            demon,
            attack,
            kind,
            hop: false,
            hurt: 0.0,
            breathe: 0.0,
            cast: 0.0,
        }
    }

    fn part<'a>(baked: &'a [BakedPart], id: u8) -> &'a BakedPart {
        baked.iter().find(|part| part.part == id).unwrap_or_else(|| panic!("missing part {id}"))
    }

    fn mean(part: &BakedPart) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for position in &part.positions {
            sum += Vec3::from(*position);
        }
        sum / part.positions.len() as f32
    }

    fn min_y(part: &BakedPart) -> f32 {
        part.positions.iter().map(|p| p[1]).fold(f32::MAX, f32::min)
    }

    #[test]
    fn positive_x_rotation_swings_a_hanging_bone_forward() {
        let swung = Quat::from_rotation_x(0.6) * Vec3::new(0.0, -1.0, 0.0);
        assert!(swung.z < -0.4, "forward is -Z, got {swung}");
        assert!(swung.y < 0.0 && swung.y > -1.0, "{swung}");
    }

    #[test]
    fn step_rate_matches_a_run_and_does_not_buzz() {
        let stroll = step_rate(1.5);
        let run = step_rate(4.5);
        let sprint = step_rate(7.0);
        assert!((1.6..=2.6).contains(&stroll), "stroll steps/s {stroll}");
        assert!((2.8..=4.2).contains(&run), "run steps/s {run}");
        assert!((3.2..=4.8).contains(&sprint), "sprint steps/s {sprint}");
        assert!(sprint > run && run > stroll);
        assert!((step_length(4.5) * run - 4.5).abs() < 1.0e-3, "step length times rate is the speed");
        let (phase, _) = advance_gait(0.0, 1.0, 4.5, 1.0);
        assert!(phase > 8.0 && phase < 18.0, "one second of running advanced {phase} rad");
    }

    #[test]
    fn contact_plants_the_front_foot_and_the_arms_oppose_the_legs() {
        let figure = hero_figure();
        let contact = posed(&figure, 0.0, 1.0, None, AttackKind::None, false, 4.5);
        assert!(contact.knee_l < 0.55, "planted knee {}", contact.knee_l);
        assert!(contact.knee_r > contact.knee_l + 0.2, "swing knee {} plant {}", contact.knee_r, contact.knee_l);
        assert!(contact.hip_l > 0.15 && contact.shoulder_l < -0.15, "left arm should oppose the left leg");
        assert!(contact.shoulder_r > 0.15, "sword arm forward on the left step");

        let baked = bake_parts(&figure, &contact);
        let left = mean(part(&baked, LIMB_FOOT_L));
        let right = mean(part(&baked, LIMB_FOOT_R));
        assert!(left.z < right.z - 0.02, "left foot {left} should lead the right {right}");
        let left_sole = min_y(part(&baked, LIMB_FOOT_L));
        let right_sole = min_y(part(&baked, LIMB_FOOT_R));
        assert!(left_sole.abs() < 0.04, "planted sole should sit on the ground, y={left_sole}");
        assert!(right_sole > left_sole + 0.04, "swing foot {right_sole} should have left the ground");
        assert_wound(part(&baked, LIMB_FOOT_L));
    }

    fn assert_wound(part: &BakedPart) {
        for tri in part.indices.chunks_exact(3) {
            let p = |i: u32| Vec3::from(part.positions[i as usize]);
            let n = Vec3::from(part.normals[tri[0] as usize]);
            let cross = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0]));
            assert!(cross.dot(n) > 0.0, "a posed foot face is wound backwards");
        }
    }

    #[test]
    fn the_swing_foot_clears_and_the_stance_foot_does_not_skate() {
        let figure = hero_figure();
        let speed = 4.5;
        let rate = step_rate(speed);
        let mut max_knee = 0.0_f32;
        let mut max_clear = f32::MIN;
        for step in 0..24 {
            let phase = step as f32 / 24.0 * TAU;
            let pose = posed(&figure, phase, 1.0, None, AttackKind::None, false, speed);
            max_knee = max_knee.max(pose.knee_r).max(pose.knee_l);
            let baked = bake_parts(&figure, &pose);
            let left = min_y(part(&baked, LIMB_FOOT_L));
            let right = min_y(part(&baked, LIMB_FOOT_R));
            max_clear = max_clear.max(right - left).max(left - right);
        }
        assert!(max_knee > 0.7, "swing knee {max_knee}");
        assert!(max_clear > 0.06, "a swing foot should clear the ground by {max_clear}");

        // Early left stance. The sole's character-space z must advance at the
        // body speed, which is what keeps that foot still in the world.
        let early = 0.08_f32;
        let later = 0.42_f32;
        let foot_z = |phase: f32| {
            let baked = bake_parts(&figure, &posed(&figure, phase, 1.0, None, AttackKind::None, false, speed));
            mean(part(&baked, LIMB_FOOT_L)).z
        };
        let dz = foot_z(later) - foot_z(early);
        let expected = speed * (later - early) / (rate * PI);
        assert!(dz > 0.0, "stance foot should travel backward, dz {dz}");
        assert!((dz - expected).abs() < expected * 0.30, "skate dz {dz} expected {expected}");
    }

    #[test]
    fn the_sword_stays_in_the_hand_and_the_strike_carries_it_forward() {
        let figure = hero_figure();
        let idle = posed(&figure, 0.0, 0.0, None, AttackKind::None, false, 0.0);
        let windup = posed(&figure, 0.0, 0.2, Some(0.20), AttackKind::Sword, false, 0.0);
        let strike = posed(&figure, 0.0, 0.2, Some(STRIKE_PROGRESS), AttackKind::Sword, false, 0.0);
        let idle_mesh = bake_parts(&figure, &idle);
        let windup_mesh = bake_parts(&figure, &windup);
        let strike_mesh = bake_parts(&figure, &strike);
        let guard = mean(part(&idle_mesh, LIMB_SWORD));
        let cocked = mean(part(&windup_mesh, LIMB_SWORD));
        let through = mean(part(&strike_mesh, LIMB_SWORD));
        assert!(guard.x > 0.15, "sword in the right hand, {guard}");
        assert!((0.7..1.7).contains(&guard.y), "guard blade {guard}");
        assert!(cocked.y > guard.y + 0.15, "windup {cocked} guard {guard}");
        assert!(through.z < guard.z - 0.15, "strike {through} guard {guard}");
    }

    #[test]
    fn cleavers_stay_in_remaining_hands_and_chop_forward() {
        let intact = demon_figure(0);
        let idle = posed(&intact, 0.0, 0.0, None, AttackKind::None, true, 0.0);
        let strike = posed(&intact, 0.0, 0.2, Some(STRIKE_PROGRESS), AttackKind::Cleavers, true, 0.0);
        let idle_mesh = bake_parts(&intact, &idle);
        let strike_mesh = bake_parts(&intact, &strike);
        let left = mean(part(&idle_mesh, LIMB_CLEAVER_L));
        let right = mean(part(&idle_mesh, LIMB_CLEAVER_R));
        assert!(left.x < -0.2 && right.x > 0.2, "cleavers {left} {right}");
        let hoof = min_y(part(&idle_mesh, LIMB_SHIN_L)).min(min_y(part(&idle_mesh, LIMB_SHIN_R)));
        assert!(hoof.abs() < 0.05, "a standing demon should plant its hooves, y={hoof}");
        let running = posed(&intact, 0.0, 1.0, None, AttackKind::None, true, 4.5);
        let running_mesh = bake_parts(&intact, &running);
        let left_hoof = min_y(part(&running_mesh, LIMB_SHIN_L));
        let right_hoof = min_y(part(&running_mesh, LIMB_SHIN_R));
        let planted_hoof = left_hoof.min(right_hoof);
        assert!(planted_hoof.abs() < 0.05, "a running demon should plant a hoof, y={planted_hoof}");
        assert!(
            (left_hoof - right_hoof).abs() > 0.03,
            "one hoof should be off the ground, left {left_hoof} right {right_hoof}"
        );
        let chopped = mean(part(&strike_mesh, LIMB_CLEAVER_R));
        assert!(chopped.z < right.z - 0.1, "chop {chopped} guard {right}");

        let one = demon_figure(DEMON_LEFT_ARM);
        assert!(one.bones.iter().all(|bone| bone.part != LIMB_CLEAVER_L));
        let one_mesh = bake_parts(&one, &posed(&one, 0.0, 0.0, None, AttackKind::None, true, 0.0));
        assert!(one_mesh.iter().all(|part| part.part != LIMB_CLEAVER_L));
    }

    #[test]
    fn bash_bow_and_kick_each_move_their_own_limb() {
        let at = |kind, t| pose(&input(0.0, 0.0, Some(t), kind, false, 0.0), &[], &[]);
        let bash = at(AttackKind::Bash, STRIKE_PROGRESS);
        assert!(bash.shoulder_l > 1.0, "shield {}", bash.shoulder_l);
        let drawn = at(AttackKind::Bow, 0.20);
        assert!(drawn.shoulder_l > 0.8, "bow arm {}", drawn.shoulder_l);
        assert!(drawn.shoulder_r < -0.4, "draw {}", drawn.shoulder_r);
        let released = at(AttackKind::Bow, STRIKE_PROGRESS);
        assert!(released.shoulder_r > 0.8, "release {}", released.shoulder_r);
        let kick = at(AttackKind::Kick, STRIKE_PROGRESS);
        assert!(kick.hip_r > 0.8, "kick {}", kick.hip_r);
        assert!(kick.hip_l > 0.8, "kick {}", kick.hip_l);
    }

    #[test]
    fn a_legless_demon_is_not_sunk_a_second_time() {
        let prone = demon_figure(DEMON_LEFT_LEG | DEMON_RIGHT_LEG);
        assert!(prone.bones.iter().all(|bone| bone.part != LIMB_THIGH_L && bone.part != LIMB_THIGH_R));
        let pose = posed(&prone, 0.4, 1.0, None, AttackKind::None, true, 4.5);
        assert!(pose.bob.abs() < 0.03, "crawl drop is already in the sculpt, bob {}", pose.bob);
    }
}
