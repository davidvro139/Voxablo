//! Body-part combat. Each species is a capability map over the same parts.
//!
//! A fighter is the parts they can still use. Cutting a part removes only what
//! that part could do, and the stance is recomputed from whatever remains.
//! The fight ends when a vital is destroyed or no remaining part can reach the
//! player. Part boxes are in model space: metres, feet at y = 0, facing -Z,
//! centred on x/z. Fallen boxes match the unpadded ranges in
//! `player_model::fallen_demon` at 5 cm voxels. The other species use the same
//! canvas, so the same metre conversion applies. The mesh anchor absorbs canvas
//! padding, and these boxes do not move when a body drops to crawl.

use bevy::math::Vec3;

/// One clean melee cut. Tuned so it severs a limb or the head and only chips the torso core.
pub const MELEE_CUT: f32 = 52.0;
/// Other parts inside a blast take this fraction of the hit that landed on the closest part.
pub const AOE_SPLASH: f32 = 0.30;

const PARTS: usize = 8;
/// A limb this close to a closer vital still wins, so a slash through an arm does not core the chest.
const LIMB_SLACK: f32 = 0.14;
/// A committed aim wins when it is nearly as close as the nearest part.
const PREFER_SLACK: f32 = 0.22;
pub const ARC_DOT: f32 = 0.35;

/// Seconds a body reels after its poise breaks. Nothing is swung or stepped while it lasts.
pub const STAGGER_SECONDS: f32 = 0.6;
/// A part coming off is a shorter flinch: the stance change is already the big consequence.
pub const SEVER_STAGGER_SECONDS: f32 = 0.4;
/// After a stagger, poise damage cannot stagger again for this long, so a fight can't be stunlocked.
const STAGGER_GUARD: f32 = 1.2;
/// Poise starts refilling after this long without a hit.
const POISE_REGEN_DELAY: f32 = 1.2;
const POISE_REGEN_PER_SECOND: f32 = 35.0;
/// How much further out a melee body waits while another one has the attack.
pub const WAIT_RING: f32 = 1.6;

/// How far beside a part box the cursor still grabs that body.
/// Sword reach stays on `strike_along`; this is only the click magnet.
pub const CURSOR_PICK_RADIUS: f32 = 0.90;
/// Metres of ray depth a body may sit behind the first solid and still take the click.
/// Feet share the ground the ray hits, so a few centimetres of bias turned those clicks into walks.
pub const CURSOR_DEPTH_SLACK: f32 = 1.20;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Part {
    Head = 0,
    Torso = 1,
    LeftArm = 2,
    RightArm = 3,
    LeftLeg = 4,
    RightLeg = 5,
    Tail = 6,
    Horns = 7,
}

impl Part {
    pub const ALL: [Part; PARTS] = [
        Part::Head,
        Part::Torso,
        Part::LeftArm,
        Part::RightArm,
        Part::LeftLeg,
        Part::RightLeg,
        Part::Tail,
        Part::Horns,
    ];

    pub fn from_index(index: usize) -> Self {
        Self::ALL[index]
    }

    pub fn is_vital(self) -> bool {
        matches!(self, Part::Head | Part::Torso)
    }

    /// Integrity tuned to the viewer's spells: bolt 45, beam 10 per tick, nova 65, meteor 120.
    pub fn max_integrity(self) -> f32 {
        match self {
            Part::Head => 44.0,
            Part::Torso => 96.0,
            Part::LeftArm | Part::RightArm => 32.0,
            Part::LeftLeg | Part::RightLeg => 40.0,
            Part::Tail => 28.0,
            Part::Horns => 24.0,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Part::Head => "head",
            Part::Torso => "core",
            Part::LeftArm => "left arm",
            Part::RightArm => "right arm",
            Part::LeftLeg => "left leg",
            Part::RightLeg => "right leg",
            Part::Tail => "tail",
            Part::Horns => "horns",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DownReason {
    Head,
    Torso,
    NoWeapons,
}

/// Which body this fighter is. The parts are shared; the map is not.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Species {
    Fallen,
    Skeleton,
    Zombie,
    Archer,
}

impl Species {
    pub fn name(self) -> &'static str {
        match self {
            Species::Fallen => "fallen",
            Species::Skeleton => "skeleton",
            Species::Zombie => "zombie",
            Species::Archer => "corrupt rogue",
        }
    }

    fn has(self, part: Part) -> bool {
        match part {
            Part::Tail | Part::Horns => self == Species::Fallen,
            _ => true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SwingPose {
    Guard,
    Windup,
    Strike,
    Recover,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stance {
    CircleSlash,
    LungeSlash,
    HopSwing,
    GoreCharge,
    HobbleCharge,
    TailWhip,
    HopTail,
    CrawlSwipe,
    Thrash,
    /// Sword arm and shield arm both still on.
    SwordShield,
    /// Shield arm is gone; the sword arm still reaches.
    SwordLunge,
    /// Sword arm is gone; the shield arm bashes.
    ShieldBash,
    /// One leg, and an arm that can still strike or bash.
    SkeletonHop,
    /// Both arms gone. A leg can still kick.
    BoneKick,
    /// Legs gone, an arm still on the ground.
    SkeletonCrawl,
    /// Slow overhead slam with both arms.
    ZombieSlam,
    /// One rotting arm left.
    ZombieSlap,
    /// Both arms gone. The head bites.
    ZombieBite,
    /// One leg. Slam, slap, or bite, depending on the arms.
    ZombieHop,
    /// Flat on the ground, still able to slap or bite.
    ZombieCrawl,
    /// Both arms draw the bow.
    ArrowVolley,
    /// One leg, still able to loose an arrow.
    ArcherHobble,
    /// One arm left. The bow is gone; they rush with what that hand holds.
    DaggerRush,
    /// Both arms gone. A boot is the only weapon left.
    RogueKick,
    /// Legs gone. They stab from the ground.
    RogueCrawl,
    Down,
}

impl Stance {
    pub fn line(self, reason: Option<DownReason>) -> &'static str {
        match self {
            Stance::CircleSlash => "circling and slashing with both cleavers",
            Stance::LungeSlash => "lunging with the cleaver they have left",
            Stance::HopSwing => "hopping on one leg and still swinging a cleaver",
            Stance::GoreCharge => "no arms — charging with their horns",
            Stance::HobbleCharge => "one leg, no arms — hobbling into a horn charge",
            Stance::TailWhip => "no arms — whipping with their tail",
            Stance::HopTail => "one leg — whipping with their tail",
            Stance::CrawlSwipe => "legs gone — crawling in to swipe",
            Stance::Thrash => "only a tail left — thrashing if you stand on them",
            Stance::SwordShield => "circling, sword in one hand and shield in the other",
            Stance::SwordLunge => "shield gone — lunging with the sword arm",
            Stance::ShieldBash => "sword arm gone — bashing with the shield",
            Stance::SkeletonHop => "one leg — still striking with the arm they have",
            Stance::BoneKick => "arms gone — kicking",
            Stance::SkeletonCrawl => "legs gone — crawling and still swinging",
            Stance::ZombieSlam => "shambling in for a slow slam. The hit rots",
            Stance::ZombieSlap => "one arm left — a slow slap that still rots",
            Stance::ZombieBite => "arms gone — lunging to bite. The bite rots",
            Stance::ZombieHop => "one leg — still coming in, and the hit rots",
            Stance::ZombieCrawl => "flat on the ground — still able to rot you",
            Stance::ArrowVolley => "keeping distance and loosing arrows",
            Stance::ArcherHobble => "one leg — still loosing arrows",
            Stance::DaggerRush => "bow ruined — rushing with the arm they have left",
            Stance::RogueKick => "arms gone — kicking",
            Stance::RogueCrawl => "legs gone — stabbing from the ground",
            Stance::Down => match reason {
                Some(DownReason::Head) => "head gone — they can't fight",
                Some(DownReason::Torso) => "core split — they can't fight",
                _ => "nothing left that can reach you",
            },
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct AttackProfile {
    pub speed: f32,
    pub range: f32,
    pub preferred: f32,
    pub stop: f32,
    pub windup: f32,
    pub recover: f32,
    pub damage: f32,
    pub lunge: f32,
    pub strafe: bool,
}

pub fn profile(stance: Stance) -> AttackProfile {
    let mut p = AttackProfile {
        speed: 0.0,
        range: 0.0,
        preferred: 0.0,
        stop: 0.0,
        windup: 0.0,
        recover: 0.0,
        damage: 0.0,
        lunge: 0.0,
        strafe: false,
    };
    match stance {
        Stance::CircleSlash => {
            p.speed = 2.2;
            p.range = 1.5;
            p.preferred = 1.25;
            p.stop = 0.9;
            p.windup = 0.36;
            p.recover = 0.32;
            p.damage = 14.0;
            p.lunge = 3.8;
            p.strafe = true;
        }
        Stance::LungeSlash => {
            p.speed = 2.5;
            p.range = 1.65;
            p.preferred = 1.3;
            p.stop = 0.7;
            p.windup = 0.58;
            p.recover = 0.48;
            p.damage = 18.0;
            p.lunge = 5.4;
        }
        Stance::HopSwing => {
            p.speed = 1.05;
            p.range = 1.4;
            p.preferred = 1.15;
            p.stop = 0.6;
            p.windup = 0.70;
            p.recover = 0.62;
            p.damage = 16.0;
            p.lunge = 2.2;
        }
        Stance::GoreCharge => {
            p.speed = 3.5;
            p.range = 1.7;
            p.preferred = 1.4;
            p.stop = 0.5;
            p.windup = 0.78;
            p.recover = 0.70;
            p.damage = 26.0;
            p.lunge = 6.4;
        }
        Stance::HobbleCharge => {
            p.speed = 1.15;
            p.range = 1.45;
            p.preferred = 1.2;
            p.stop = 0.5;
            p.windup = 0.84;
            p.recover = 0.70;
            p.damage = 20.0;
            p.lunge = 2.6;
        }
        Stance::TailWhip => {
            p.speed = 2.3;
            p.range = 1.25;
            p.preferred = 1.05;
            p.stop = 0.7;
            p.windup = 0.40;
            p.recover = 0.42;
            p.damage = 12.0;
            p.lunge = 3.6;
        }
        Stance::HopTail => {
            p.speed = 1.0;
            p.range = 1.15;
            p.preferred = 0.95;
            p.stop = 0.55;
            p.windup = 0.7;
            p.recover = 0.85;
            p.damage = 11.0;
            p.lunge = 1.8;
        }
        Stance::CrawlSwipe => {
            p.speed = 0.55;
            p.range = 0.95;
            p.preferred = 0.8;
            p.stop = 0.35;
            p.windup = 0.62;
            p.recover = 0.85;
            p.damage = 13.0;
            p.lunge = 0.9;
        }
        Stance::Thrash => {
            p.speed = 0.0;
            p.range = 0.8;
            p.preferred = 0.6;
            p.stop = 0.0;
            p.windup = 0.4;
            p.recover = 0.45;
            p.damage = 9.0;
        }
        Stance::SwordShield => {
            p.speed = 1.9;
            p.range = 1.6;
            p.preferred = 1.3;
            p.stop = 0.85;
            p.windup = 0.40;
            p.recover = 0.34;
            p.damage = 15.0;
            p.lunge = 3.4;
            p.strafe = true;
        }
        Stance::SwordLunge => {
            p.speed = 2.2;
            p.range = 1.65;
            p.preferred = 1.25;
            p.stop = 0.7;
            p.windup = 0.62;
            p.recover = 0.75;
            p.damage = 17.0;
            p.lunge = 4.2;
        }
        Stance::ShieldBash => {
            p.speed = 1.6;
            p.range = 1.2;
            p.preferred = 1.0;
            p.stop = 0.6;
            p.windup = 0.42;
            p.recover = 0.5;
            p.damage = 12.0;
            p.lunge = 2.8;
        }
        Stance::SkeletonHop => {
            p.speed = 0.95;
            p.range = 1.4;
            p.preferred = 1.15;
            p.stop = 0.55;
            p.windup = 0.7;
            p.recover = 0.85;
            p.damage = 14.0;
            p.lunge = 1.8;
        }
        Stance::BoneKick | Stance::RogueKick => {
            p.speed = 1.25;
            p.range = 1.15;
            p.preferred = 0.95;
            p.stop = 0.5;
            p.windup = 0.55;
            p.recover = 0.7;
            p.damage = 9.0;
            p.lunge = 2.2;
        }
        Stance::SkeletonCrawl | Stance::RogueCrawl => {
            p.speed = 0.5;
            p.range = 0.95;
            p.preferred = 0.75;
            p.stop = 0.3;
            p.windup = 0.58;
            p.recover = 0.7;
            p.damage = 11.0;
            p.lunge = 0.8;
        }
        Stance::ZombieSlam => {
            p.speed = 0.75;
            p.range = 1.4;
            p.preferred = 1.15;
            p.stop = 0.7;
            p.windup = 1.05;
            p.recover = 0.55;
            p.damage = 20.0;
            p.lunge = 1.8;
        }
        Stance::ZombieSlap => {
            p.speed = 0.7;
            p.range = 1.35;
            p.preferred = 1.1;
            p.stop = 0.65;
            p.windup = 0.85;
            p.recover = 0.8;
            p.damage = 16.0;
            p.lunge = 1.4;
        }
        Stance::ZombieBite => {
            p.speed = 1.15;
            p.range = 1.0;
            p.preferred = 0.85;
            p.stop = 0.4;
            p.windup = 0.48;
            p.recover = 0.6;
            p.damage = 12.0;
            p.lunge = 2.8;
        }
        Stance::ZombieHop => {
            p.speed = 0.55;
            p.range = 1.2;
            p.preferred = 1.0;
            p.stop = 0.45;
            p.windup = 0.95;
            p.recover = 0.9;
            p.damage = 16.0;
            p.lunge = 1.1;
        }
        Stance::ZombieCrawl => {
            p.speed = 0.4;
            p.range = 0.9;
            p.preferred = 0.7;
            p.stop = 0.25;
            p.windup = 0.7;
            p.recover = 0.75;
            p.damage = 12.0;
            p.lunge = 0.6;
        }
        Stance::ArrowVolley => {
            p.speed = 1.7;
            p.range = 12.0;
            p.preferred = 7.5;
            p.stop = 4.8;
            p.windup = 0.62;
            p.recover = 0.48;
            p.damage = 13.0;
        }
        Stance::ArcherHobble => {
            p.speed = 0.7;
            p.range = 11.0;
            p.preferred = 6.5;
            p.stop = 4.2;
            p.windup = 0.85;
            p.recover = 0.9;
            p.damage = 12.0;
        }
        Stance::DaggerRush => {
            p.speed = 2.4;
            p.range = 1.5;
            p.preferred = 1.2;
            p.stop = 0.65;
            p.windup = 0.36;
            p.recover = 0.32;
            p.damage = 15.0;
            p.lunge = 4.6;
        }
        Stance::Down => {}
    }
    p
}

fn shoots(stance: Stance) -> bool {
    matches!(stance, Stance::ArrowVolley | Stance::ArcherHobble)
}

#[derive(Clone, Copy, Debug)]
struct Volume {
    part: Part,
    min: Vec3,
    max: Vec3,
}

fn vx(x: i32) -> f32 {
    -0.55 + x as f32 * 0.05
}
fn vy(y: i32) -> f32 {
    y as f32 * 0.05
}
fn vz(z: i32) -> f32 {
    -0.35 + z as f32 * 0.05
}
fn vol(part: Part, min: [i32; 3], max: [i32; 3]) -> Volume {
    Volume {
        part,
        min: Vec3::new(vx(min[0]), vy(min[1]), vz(min[2])),
        max: Vec3::new(vx(max[0]), vy(max[1]), vz(max[2])),
    }
}

/// Unpadded voxel boxes. Fallen matches `fallen_demon`. The others match the
/// boxes stamped by `skeleton`, `zombie`, and `corrupt_rogue` on the same canvas.
fn for_each_volume(species: Species, mut visit: impl FnMut(Volume)) {
    let boxes: &[(Part, [i32; 3], [i32; 3])] = match species {
        Species::Fallen => &[
            (Part::LeftArm, [0, 4, 1], [6, 22, 10]),
            (Part::RightArm, [16, 4, 1], [22, 22, 10]),
            (Part::LeftLeg, [4, 0, 1], [10, 12, 10]),
            (Part::RightLeg, [12, 0, 1], [16, 12, 10]),
            (Part::Head, [7, 22, 1], [15, 30, 11]),
            (Part::Torso, [6, 12, 3], [16, 24, 11]),
            (Part::Tail, [10, 6, 10], [12, 16, 14]),
            (Part::Horns, [0, 26, 4], [6, 30, 12]),
            (Part::Horns, [16, 26, 4], [22, 30, 12]),
        ],
        Species::Skeleton => &[
            (Part::LeftArm, [0, 10, 0], [8, 26, 7]),
            (Part::RightArm, [15, 11, 0], [20, 33, 7]),
            (Part::LeftLeg, [6, 0, 2], [11, 16, 8]),
            (Part::RightLeg, [12, 0, 2], [17, 16, 8]),
            (Part::Head, [8, 26, 2], [14, 34, 9]),
            (Part::Torso, [7, 15, 3], [15, 26, 8]),
        ],
        Species::Zombie => &[
            (Part::LeftArm, [0, 4, 2], [6, 22, 8]),
            (Part::RightArm, [16, 4, 2], [22, 22, 8]),
            (Part::LeftLeg, [5, 0, 3], [10, 15, 9]),
            (Part::RightLeg, [12, 0, 3], [17, 15, 9]),
            (Part::Head, [7, 22, 0], [15, 30, 8]),
            (Part::Torso, [4, 14, 3], [18, 24, 13]),
        ],
        Species::Archer => &[
            (Part::LeftArm, [2, 6, 0], [7, 32, 7]),
            (Part::RightArm, [15, 12, 0], [20, 26, 7]),
            (Part::LeftLeg, [6, 0, 2], [10, 16, 8]),
            (Part::RightLeg, [12, 0, 2], [16, 16, 8]),
            (Part::Head, [7, 26, 2], [15, 34, 9]),
            (Part::Torso, [6, 15, 3], [16, 28, 11]),
        ],
    };
    for &(part, min, max) in boxes {
        visit(vol(part, min, max));
    }
}

fn volume_center(species: Species, part: Part) -> Vec3 {
    let mut sum = Vec3::ZERO;
    let mut n = 0.0;
    for_each_volume(species, |volume| {
        if volume.part == part {
            sum += (volume.min + volume.max) * 0.5;
            n += 1.0;
        }
    });
    if n == 0.0 { Vec3::ZERO } else { sum / n }
}

/// Poise per body. One melee cut (52) that takes no part staggers a fallen or an archer,
/// two land it on a skeleton, and a zombie shrugs off a single cut.
fn max_poise(species: Species) -> f32 {
    match species {
        Species::Fallen => 45.0,
        Species::Skeleton => 70.0,
        Species::Zombie => 95.0,
        Species::Archer => 40.0,
    }
}

fn integrity_of(species: Species, part: Part) -> f32 {
    match (species, part) {
        (Species::Skeleton, Part::LeftArm) => 46.0,
        (Species::Skeleton, Part::Torso) => 84.0,
        (Species::Zombie, Part::Torso) => 120.0,
        (Species::Zombie, Part::Head) => 50.0,
        (Species::Zombie, Part::LeftArm | Part::RightArm) => 38.0,
        (Species::Archer, Part::LeftArm | Part::RightArm) => 28.0,
        (Species::Archer, Part::Torso) => 72.0,
        (_, part) => part.max_integrity(),
    }
}

#[derive(Clone, Copy, Debug)]
struct Approach {
    distance: f32,
    t: f32,
    on_volume: Vec3,
}

fn segment_approach(from: Vec3, to: Vec3, min: Vec3, max: Vec3) -> Approach {
    let mut best = Approach { distance: f32::MAX, t: 0.0, on_volume: min };
    for step in 0..=16 {
        let t = step as f32 / 16.0;
        let point = from.lerp(to, t);
        let on_volume = point.clamp(min, max);
        let distance = point.distance(on_volume);
        if distance < best.distance {
            best = Approach { distance, t, on_volume };
        }
    }
    best
}

/// Closest point on a segment to a box. Squared distance to an AABB is convex along the segment,
/// so ternary search finds it. The cursor ray is tens of metres; the 17-sample sweep above is for short swings.
fn segment_approach_exact(from: Vec3, to: Vec3, min: Vec3, max: Vec3) -> Approach {
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..48 {
        let left = lo + (hi - lo) / 3.0;
        let right = hi - (hi - lo) / 3.0;
        let left_point = from.lerp(to, left);
        let right_point = from.lerp(to, right);
        let left_d = left_point.distance_squared(left_point.clamp(min, max));
        let right_d = right_point.distance_squared(right_point.clamp(min, max));
        if left_d < right_d {
            hi = right;
        } else {
            lo = left;
        }
    }
    let t = (lo + hi) * 0.5;
    let point = from.lerp(to, t);
    let on_volume = point.clamp(min, max);
    Approach { distance: point.distance(on_volume), t, on_volume }
}

#[derive(Clone, Debug)]
pub struct StrikeReport {
    pub hit: Option<Part>,
    pub severed: Vec<Part>,
    /// Impact on the part, in the same space as the query that produced it.
    pub impact: Option<Vec3>,
    pub became_down: bool,
    /// The hit broke poise or took a part, and the body is now reeling.
    pub staggered: bool,
    pub stance: Stance,
    pub reason: Option<DownReason>,
}

impl StrikeReport {
    fn none(stance: Stance, reason: Option<DownReason>) -> Self {
        Self { hit: None, severed: Vec::new(), impact: None, became_down: false, staggered: false, stance, reason }
    }
}

/// What a committed attack covers on the ground, for the windup telegraph.
#[derive(Clone, Copy, Debug)]
pub struct Telegraph {
    pub yaw: f32,
    pub range: f32,
    /// An arrow flies down a lane instead of sweeping the melee arc.
    pub ranged: bool,
    /// 0 when the windup starts, 1 when the blow lands.
    pub fill: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct PartPick {
    pub part: Part,
    pub t: f32,
    /// Perpendicular miss from the segment to the part box, in metres.
    pub distance: f32,
}

/// One body under a cursor ray. `along` is metres from the ray origin; `miss` is metres off the part box.
#[derive(Clone, Copy, Debug)]
pub struct CursorSample {
    pub down: bool,
    pub along: f32,
    pub miss: f32,
}

/// Living body the cursor should take. Samples behind cover are dropped before the miss sort,
/// so a graze behind a wall cannot hide a body the ray actually reaches.
pub fn choose_cursor_body(samples: &[CursorSample], ground_along: Option<f32>, slack: f32) -> Option<usize> {
    let mut best: Option<(usize, f32, f32)> = None;
    for (index, sample) in samples.iter().enumerate() {
        if sample.down {
            continue;
        }
        if let Some(ground) = ground_along {
            if sample.along > ground + slack {
                continue;
            }
        }
        let take = match best {
            None => true,
            Some((_, miss, along)) => sample.miss < miss || (sample.miss == miss && sample.along < along),
        };
        if take {
            best = Some((index, sample.miss, sample.along));
        }
    }
    best.map(|(index, _, _)| index)
}

#[derive(Clone, Copy, Debug)]
struct AttackClock {
    timer: f32,
    locked_yaw: Option<f32>,
    /// Lengths this attack was committed with; a frenzied body swings faster than its profile.
    windup_len: f32,
    recover_len: f32,
}

impl Default for AttackClock {
    fn default() -> Self {
        Self { timer: 0.0, locked_yaw: None, windup_len: 0.0, recover_len: 0.0 }
    }
}

pub struct Fighter {
    species: Species,
    integrity: [f32; PARTS],
    attached: [bool; PARTS],
    hits: u32,
    stance: Stance,
    reason: Option<DownReason>,
    attack: AttackClock,
    poise: f32,
    /// Seconds left reeling. Positive means no attack and no movement of its own.
    stagger: f32,
    /// Seconds left in which poise damage cannot stagger again.
    guard: f32,
    /// Seconds since the last hit, for poise regeneration.
    quiet: f32,
    /// Poise damage gathered by the strike being resolved.
    pending_poise: f32,
}

impl Fighter {
    pub fn claw_brute() -> Self {
        Self::new(Species::Fallen)
    }

    pub fn new(species: Species) -> Self {
        let mut fighter = Self {
            species,
            integrity: [0.0; PARTS],
            attached: [false; PARTS],
            hits: 0,
            stance: Stance::Down,
            reason: None,
            attack: AttackClock::default(),
            poise: max_poise(species),
            stagger: 0.0,
            guard: 0.0,
            quiet: 0.0,
            pending_poise: 0.0,
        };
        for part in Part::ALL {
            if species.has(part) {
                fighter.integrity[part as usize] = integrity_of(species, part);
                fighter.attached[part as usize] = true;
            }
        }
        fighter.recompute(Stance::CircleSlash);
        fighter
    }

    pub fn species(&self) -> Species {
        self.species
    }

    pub fn part_name(&self, part: Part) -> &'static str {
        match (self.species, part) {
            (Species::Skeleton, Part::LeftArm) => "shield",
            (Species::Skeleton, Part::RightArm) => "sword arm",
            (Species::Archer, Part::LeftArm) => "bow arm",
            (Species::Archer, Part::RightArm) => "drawing arm",
            _ => part.label(),
        }
    }

    pub fn status_line(&self) -> String {
        format!("{} — {}", self.species.name(), self.stance.line(self.reason))
    }

    pub fn stance(&self) -> Stance {
        self.stance
    }

    pub fn down_reason(&self) -> Option<DownReason> {
        self.reason
    }

    pub fn is_down(&self) -> bool {
        self.stance == Stance::Down
    }

    pub fn is_winding(&self) -> bool {
        self.attack.timer > 0.0
    }

    /// How fast the current stance can walk, in m/s.
    pub fn move_speed(&self) -> f32 {
        profile(self.stance).speed
    }

    /// Drop a windup that was committed against a player the body can no longer sense.
    pub fn calm(&mut self) {
        if self.attack.timer > 0.0 {
            self.attack = AttackClock::default();
        }
    }

    pub fn parts_left(&self) -> u32 {
        self.attached.iter().filter(|on| **on).count() as u32
    }

    pub fn is_staggered(&self) -> bool {
        self.stagger > 0.0
    }

    /// The arc or lane the current windup will cover, while one is committed.
    pub fn telegraph(&self) -> Option<Telegraph> {
        let yaw = self.attack.locked_yaw?;
        if self.attack.timer <= 0.0 || self.is_down() {
            return None;
        }
        let profile = profile(self.stance);
        let fill = 1.0 - (self.attack.timer / self.windup_len().max(1.0e-3)).clamp(0.0, 1.0);
        Some(Telegraph { yaw, range: profile.range, ranged: shoots(self.stance), fill })
    }

    /// Windup of the attack in progress, or the stance's own when none was committed.
    fn windup_len(&self) -> f32 {
        if self.attack.windup_len > 0.0 {
            self.attack.windup_len
        } else {
            profile(self.stance).windup
        }
    }

    fn recover_len(&self) -> f32 {
        if self.attack.recover_len > 0.0 {
            self.attack.recover_len
        } else {
            profile(self.stance).recover
        }
    }

    /// Melee bodies share a smaller pool than archers. Stances that cannot move never queue for one.
    pub fn wants_token(&self) -> Option<bool> {
        if self.is_down() || profile(self.stance).speed <= 0.0 {
            return None;
        }
        Some(shoots(self.stance))
    }

    /// A windup or recovery is running, so a token held for it must not be taken away.
    pub fn mid_attack(&self) -> bool {
        self.attack.timer != 0.0
    }

    /// 0 at the start of a swing, 0.36 when the blow lands, 1 after recover.
    /// The landing frame is the same instant `plan_fight` deals damage (timer crosses 0).
    pub fn attack_progress(&self) -> Option<f32> {
        let timer = self.attack.timer;
        if timer > 0.0 {
            let windup = self.windup_len().max(1.0e-3);
            let u = 1.0 - (timer / windup).clamp(0.0, 1.0);
            Some(u * 0.36)
        } else if timer < 0.0 {
            let recover = self.recover_len().max(1.0e-3);
            let u = (1.0 + timer / recover).clamp(0.0, 1.0);
            Some(0.36 + u * 0.64)
        } else {
            None
        }
    }

    /// Which part of a committed swing the body should show. Guard is everything else.
    pub fn swing_pose(&self) -> SwingPose {
        let windup = self.windup_len();
        let timer = self.attack.timer;
        if timer > 0.0 && windup > 0.0 && timer > windup * 0.38 {
            SwingPose::Windup
        } else if timer > 0.0 {
            SwingPose::Strike
        } else if timer < 0.0 {
            SwingPose::Recover
        } else {
            SwingPose::Guard
        }
    }

    pub fn hits(&self) -> u32 {
        self.hits
    }

    pub fn attached(&self, part: Part) -> bool {
        self.attached[part as usize]
    }

    pub fn center(&self, part: Part) -> Option<Vec3> {
        self.attached(part).then(|| volume_center(self.species, part))
    }

    pub fn pick_part(&self, from: Vec3, to: Vec3, prefer: Option<Part>, reach: f32) -> Option<PartPick> {
        let (part, approach) = self.choose_part(from, to, prefer, reach, true)?;
        Some(PartPick { part, t: approach.t, distance: approach.distance })
    }

    pub fn strike_part(&mut self, part: Part, damage: f32) -> StrikeReport {
        let before = self.stance;
        let severed = self.apply_damage(part, damage);
        let severed = if severed { vec![part] } else { Vec::new() };
        self.finish(before, Some(part), Some(volume_center(self.species, part)), severed)
    }

    pub fn strike_along(&mut self, from: Vec3, to: Vec3, damage: f32, prefer: Option<Part>, reach: f32) -> StrikeReport {
        let Some((part, approach)) = self.choose_part(from, to, prefer, reach, false) else {
            return StrikeReport::none(self.stance, self.reason);
        };
        let before = self.stance;
        let severed = self.apply_damage(part, damage);
        let severed = if severed { vec![part] } else { Vec::new() };
        self.finish(before, Some(part), Some(approach.on_volume), severed)
    }

    /// Full damage on the closest part inside `radius`, splash on the other parts inside it.
    pub fn strike_area(&mut self, center: Vec3, radius: f32, damage: f32) -> StrikeReport {
        if damage <= 0.0 || radius <= 0.0 {
            return StrikeReport::none(self.stance, self.reason);
        }
        let mut best = [(f32::MAX, Vec3::ZERO); PARTS];
        let species = self.species;
        for_each_volume(species, |volume| {
            let index = volume.part as usize;
            if self.attached[index] {
                let on_volume = center.clamp(volume.min, volume.max);
                let distance = center.distance(on_volume);
                if distance < best[index].0 {
                    best[index] = (distance, on_volume);
                }
            }
        });
        let mut order: Vec<usize> = (0..PARTS).filter(|&index| self.attached[index] && best[index].0 <= radius).collect();
        order.sort_by(|a, b| best[*a].0.total_cmp(&best[*b].0));
        if order.is_empty() {
            return StrikeReport::none(self.stance, self.reason);
        }
        let before = self.stance;
        let primary = Part::from_index(order[0]);
        let impact = best[order[0]].1;
        let mut severed = Vec::new();
        for (nth, index) in order.iter().enumerate() {
            let amount = if nth == 0 { damage } else { damage * AOE_SPLASH };
            let part = Part::from_index(*index);
            if self.apply_damage(part, amount) {
                severed.push(part);
            }
        }
        self.finish(before, Some(primary), Some(impact), severed)
    }

    fn choose_part(
        &self,
        from: Vec3,
        to: Vec3,
        prefer: Option<Part>,
        reach: f32,
        precise: bool,
    ) -> Option<(Part, Approach)> {
        let mut approaches: [Option<Approach>; PARTS] = [None; PARTS];
        let species = self.species;
        for_each_volume(species, |volume| {
            let index = volume.part as usize;
            if self.attached[index] {
                let approach = if precise {
                    segment_approach_exact(from, to, volume.min, volume.max)
                } else {
                    segment_approach(from, to, volume.min, volume.max)
                };
                let closer = approaches[index].map_or(true, |prev| approach.distance < prev.distance);
                if closer {
                    approaches[index] = Some(approach);
                }
            }
        });
        let best_dist = approaches.iter().filter_map(|slot| slot.map(|a| a.distance)).fold(f32::MAX, f32::min);
        if best_dist > reach {
            return None;
        }
        let mut chosen: Option<(Part, Approach)> = None;
        if let Some(part) = prefer {
            if let Some(approach) = approaches[part as usize] {
                if approach.distance <= best_dist + PREFER_SLACK && approach.distance <= reach {
                    chosen = Some((part, approach));
                }
            }
        }
        if chosen.is_none() {
            let near = best_dist + LIMB_SLACK;
            for (index, slot) in approaches.iter().enumerate() {
                let Some(approach) = slot else { continue };
                if approach.distance > near || approach.distance > reach {
                    continue;
                }
                let part = Part::from_index(index);
                let better = match chosen {
                    None => true,
                    Some((prev, prev_approach)) => {
                        if prev.is_vital() && !part.is_vital() {
                            true
                        } else if !prev.is_vital() && part.is_vital() {
                            false
                        } else {
                            approach.distance < prev_approach.distance
                        }
                    }
                };
                if better {
                    chosen = Some((part, *approach));
                }
            }
        }
        let (part, _) = chosen?;
        // A skeleton's shield catches a cut aimed at the head or core from in front.
        // A swing from the side or from behind still meets the part it actually reaches.
        if self.shield_covers(from, part) {
            if let Some(approach) = approaches[Part::LeftArm as usize] {
                if approach.distance <= reach {
                    return Some((Part::LeftArm, approach));
                }
            }
        }
        chosen
    }

    /// Frontal hits on a vital, while the shield arm is still attached.
    fn shield_covers(&self, from: Vec3, part: Part) -> bool {
        self.species == Species::Skeleton
            && self.attached[Part::LeftArm as usize]
            && part.is_vital()
            && from.z < -0.05
            && from.z.abs() >= from.x.abs() * 0.5
    }

    fn apply_damage(&mut self, part: Part, amount: f32) -> bool {
        let index = part as usize;
        if !self.attached[index] || amount <= 0.0 {
            return false;
        }
        self.integrity[index] -= amount;
        self.hits += 1;
        self.pending_poise += amount;
        if self.integrity[index] <= 0.0 {
            self.attached[index] = false;
            true
        } else {
            false
        }
    }

    fn finish(&mut self, before: Stance, hit: Option<Part>, impact: Option<Vec3>, severed: Vec<Part>) -> StrikeReport {
        self.recompute(before);
        let poise_damage = std::mem::take(&mut self.pending_poise);
        let mut staggered = false;
        if self.stance != Stance::Down && hit.is_some() {
            self.quiet = 0.0;
            self.poise -= poise_damage;
            // A part coming off breaks the swing that was already committed.
            if !severed.is_empty() {
                staggered = self.begin_stagger(SEVER_STAGGER_SECONDS);
            } else if self.poise <= 0.0 && self.guard <= 0.0 {
                staggered = self.begin_stagger(STAGGER_SECONDS);
            }
            self.poise = self.poise.max(0.0);
        }
        StrikeReport {
            hit,
            severed,
            impact,
            became_down: before != Stance::Down && self.stance == Stance::Down,
            staggered,
            stance: self.stance,
            reason: self.reason,
        }
    }

    /// Cancel the committed attack and reel. Returns false while an earlier stagger is still running.
    fn begin_stagger(&mut self, seconds: f32) -> bool {
        if self.stagger > 0.0 {
            return false;
        }
        self.attack = AttackClock::default();
        self.stagger = seconds;
        self.poise = max_poise(self.species);
        true
    }

    /// Stagger, guard, and poise clocks. `plan_fight` calls this once per frame.
    fn tick(&mut self, dt: f32) {
        if self.stagger > 0.0 {
            self.stagger = (self.stagger - dt).max(0.0);
            if self.stagger == 0.0 {
                self.guard = STAGGER_GUARD;
            }
        } else {
            self.guard = (self.guard - dt).max(0.0);
        }
        self.quiet += dt;
        if self.quiet >= POISE_REGEN_DELAY {
            self.poise = (self.poise + POISE_REGEN_PER_SECOND * dt).min(max_poise(self.species));
        }
    }

    fn recompute(&mut self, before: Stance) {
        let (stance, reason) = match self.species {
            Species::Fallen => stance_claw_brute(self.attached),
            Species::Skeleton => stance_skeleton(self.attached),
            Species::Zombie => stance_zombie(self.attached),
            Species::Archer => stance_archer(self.attached),
        };
        if before != Stance::Down && stance == Stance::Down {
            self.attack = AttackClock::default();
        }
        self.stance = stance;
        self.reason = reason;
    }
}

/// Capability map for the fallen demon. Another archetype is a different function over the same parts.
fn stance_claw_brute(attached: [bool; PARTS]) -> (Stance, Option<DownReason>) {
    let on = |part: Part| attached[part as usize];
    if !on(Part::Head) {
        return (Stance::Down, Some(DownReason::Head));
    }
    if !on(Part::Torso) {
        return (Stance::Down, Some(DownReason::Torso));
    }
    let arms = on(Part::LeftArm) as u8 + on(Part::RightArm) as u8;
    let legs = on(Part::LeftLeg) as u8 + on(Part::RightLeg) as u8;
    let horns = on(Part::Horns);
    let tail = on(Part::Tail);
    let stance = if legs == 0 && arms > 0 {
        Stance::CrawlSwipe
    } else if legs == 0 && tail {
        Stance::Thrash
    } else if legs == 0 {
        Stance::Down
    } else if legs == 1 && arms > 0 {
        Stance::HopSwing
    } else if legs == 1 && horns {
        Stance::HobbleCharge
    } else if legs == 1 && tail {
        Stance::HopTail
    } else if legs == 1 {
        Stance::Down
    } else if arms == 2 {
        Stance::CircleSlash
    } else if arms == 1 {
        Stance::LungeSlash
    } else if horns {
        Stance::GoreCharge
    } else if tail {
        Stance::TailWhip
    } else {
        Stance::Down
    };
    let reason = (stance == Stance::Down).then_some(DownReason::NoWeapons);
    // Head and torso already returned. A Down from this ladder is "nothing left that can reach".
    if stance == Stance::Down {
        (Stance::Down, reason)
    } else {
        (stance, None)
    }
}

fn alive(attached: [bool; PARTS]) -> Option<DownReason> {
    let on = |part: Part| attached[part as usize];
    if !on(Part::Head) {
        Some(DownReason::Head)
    } else if !on(Part::Torso) {
        Some(DownReason::Torso)
    } else {
        None
    }
}

fn counts(attached: [bool; PARTS]) -> (u8, u8, bool) {
    let on = |part: Part| attached[part as usize];
    let arms = on(Part::LeftArm) as u8 + on(Part::RightArm) as u8;
    let legs = on(Part::LeftLeg) as u8 + on(Part::RightLeg) as u8;
    (arms, legs, on(Part::RightArm))
}

/// Sword arm strikes, shield arm bashes. Both arms gone, a kick remains. No kick ends them.
fn stance_skeleton(attached: [bool; PARTS]) -> (Stance, Option<DownReason>) {
    if let Some(reason) = alive(attached) {
        return (Stance::Down, Some(reason));
    }
    let (arms, legs, right) = counts(attached);
    let stance = if legs == 0 && arms == 0 {
        Stance::Down
    } else if legs == 0 {
        Stance::SkeletonCrawl
    } else if arms == 0 {
        Stance::BoneKick
    } else if legs == 1 {
        Stance::SkeletonHop
    } else if arms == 2 {
        Stance::SwordShield
    } else if right {
        Stance::SwordLunge
    } else {
        Stance::ShieldBash
    };
    let reason = (stance == Stance::Down).then_some(DownReason::NoWeapons);
    (stance, reason)
}

/// Slow, and every connecting hit rots. Arms slam. No arms, the head bites.
fn stance_zombie(attached: [bool; PARTS]) -> (Stance, Option<DownReason>) {
    if let Some(reason) = alive(attached) {
        return (Stance::Down, Some(reason));
    }
    let (arms, legs, _) = counts(attached);
    let stance = if legs == 0 {
        Stance::ZombieCrawl
    } else if legs == 1 {
        Stance::ZombieHop
    } else if arms == 2 {
        Stance::ZombieSlam
    } else if arms == 1 {
        Stance::ZombieSlap
    } else {
        Stance::ZombieBite
    };
    (stance, None)
}

/// The bow needs both arms. One arm rushes in melee. No arms, a kick. No kick, they are done.
fn stance_archer(attached: [bool; PARTS]) -> (Stance, Option<DownReason>) {
    if let Some(reason) = alive(attached) {
        return (Stance::Down, Some(reason));
    }
    let (arms, legs, _) = counts(attached);
    let stance = if legs == 0 && arms == 0 {
        Stance::Down
    } else if legs == 0 {
        Stance::RogueCrawl
    } else if arms == 0 {
        Stance::RogueKick
    } else if arms == 2 && legs == 1 {
        Stance::ArcherHobble
    } else if arms == 2 {
        Stance::ArrowVolley
    } else {
        Stance::DaggerRush
    };
    let reason = (stance == Stance::Down).then_some(DownReason::NoWeapons);
    (stance, reason)
}

#[derive(Clone, Copy, Debug)]
pub struct FightPlan {
    pub velocity: Vec3,
    pub yaw: f32,
    /// Melee damage dealt this frame. Zero when the attack is an arrow instead.
    pub strike_damage: f32,
    /// Arrow damage to loose this frame. The bolt travels; it is not a hitscan.
    pub shot: f32,
    pub winding_up: bool,
    /// The recovery ran out this frame; an attack token held for it can go back.
    pub attack_finished: bool,
}

impl FightPlan {
    fn idle(yaw: f32) -> Self {
        Self { velocity: Vec3::ZERO, yaw, strike_damage: 0.0, shot: 0.0, winding_up: false, attack_finished: false }
    }
}

/// What the group allows this body this frame.
#[derive(Clone, Copy, Debug)]
pub struct Engage {
    /// Holds an attack token. Without one the body keeps its distance and circles.
    pub may_attack: bool,
    /// Which way to circle: +1 or -1, stable per body so a group fans out.
    pub side: f32,
    /// Backed into something. An archer stops trying to open the gap and shoots point-blank.
    pub cornered: bool,
    /// Multiplies windup and recovery; below 1 the body attacks faster (and moves a little faster).
    pub haste: f32,
}

impl Default for Engage {
    fn default() -> Self {
        Self { may_attack: true, side: 1.0, cornered: false, haste: 1.0 }
    }
}

/// An archer closer than this fraction of its stop distance won't draw; it backs off first.
const KITE_FRACTION: f32 = 0.75;
/// Backing off is a scramble, faster than an archer's walk.
const KITE_SPEED_SCALE: f32 = 1.6;

pub fn yaw_toward(from: Vec3, to: Vec3) -> f32 {
    let d = to - from;
    f32::atan2(-d.x, -d.z)
}

pub fn forward_from_yaw(yaw: f32) -> Vec3 {
    Vec3::new(-yaw.sin(), 0.0, -yaw.cos())
}

/// A body fighting alone: it always has the attack.
#[cfg(test)]
pub fn plan_fight(fighter: &mut Fighter, dt: f32, from: Vec3, to: Vec3) -> FightPlan {
    plan_fight_engaged(fighter, dt, from, to, Engage::default())
}

/// Advance the committed attack. Facing locks when the windup starts, so leaving that arc is a dodge.
pub fn plan_fight_engaged(fighter: &mut Fighter, dt: f32, from: Vec3, to: Vec3, engage: Engage) -> FightPlan {
    let stance = fighter.stance;
    let mut profile = profile(stance);
    let flat = Vec3::new(to.x - from.x, 0.0, to.z - from.z);
    let dist = flat.length();
    let dir = flat.normalize_or_zero();
    let face_yaw = if dist > 0.001 { yaw_toward(from, to) } else { 0.0 };

    if stance == Stance::Down {
        fighter.attack = AttackClock::default();
        return FightPlan::idle(face_yaw);
    }
    fighter.tick(dt);
    if fighter.is_staggered() {
        return FightPlan::idle(fighter.attack.locked_yaw.unwrap_or(face_yaw));
    }

    let mut strike_damage = 0.0;
    let mut shot = 0.0;
    let mut attack_finished = false;
    let ranged = shoots(stance);
    let haste = engage.haste.clamp(0.25, 2.0);
    profile.windup *= haste;
    profile.recover *= haste;
    profile.speed /= haste.sqrt();
    let idle_clock = fighter.attack.timer == 0.0;
    if idle_clock && !engage.may_attack {
        // Waiting a turn: hold a wider ring than the body's own and circle it.
        if !ranged {
            profile.preferred += WAIT_RING;
            profile.stop += WAIT_RING;
        }
        profile.strafe = true;
    }
    if fighter.attack.timer > 0.0 {
        let committed = forward_from_yaw(fighter.attack.locked_yaw.unwrap_or(face_yaw));
        fighter.attack.timer -= dt;
        if fighter.attack.timer <= 0.0 {
            let in_arc = dist <= profile.range && dist > 0.05 && dir.dot(committed) > ARC_DOT;
            if in_arc {
                if ranged {
                    shot = profile.damage;
                } else {
                    strike_damage = profile.damage;
                }
            }
            fighter.attack.timer = -profile.recover;
            fighter.attack.recover_len = profile.recover;
            fighter.attack.locked_yaw = None;
        }
    } else if fighter.attack.timer < 0.0 {
        fighter.attack.timer = (fighter.attack.timer + dt).min(0.0);
        attack_finished = fighter.attack.timer == 0.0;
    } else if engage.may_attack
        && dist <= profile.range
        && dist > 0.05
        && !(ranged && !engage.cornered && dist < profile.stop * KITE_FRACTION)
    {
        fighter.attack.timer = profile.windup;
        fighter.attack.windup_len = profile.windup;
        fighter.attack.locked_yaw = Some(face_yaw);
    }

    let winding = fighter.attack.timer > 0.0;
    let recovering = fighter.attack.timer < 0.0;
    let yaw = fighter.attack.locked_yaw.unwrap_or(face_yaw);
    let committed = forward_from_yaw(yaw);
    let velocity = if winding && ranged {
        Vec3::ZERO
    } else if winding {
        // Hold through the tell, then step. Stop short while the target is still on the blade;
        // once they leave that line, the step runs on so the dodge has somewhere to miss.
        let along = flat.dot(committed);
        let late = fighter.attack.timer <= profile.windup * 0.38;
        let want = if late { profile.lunge.max(profile.speed) } else { 0.0 };
        let speed = if along > 0.45 {
            want.min((along - 0.45) / dt.max(1.0e-3))
        } else {
            want
        };
        committed * speed
    } else if recovering && !ranged && profile.speed > 0.0 && dist < profile.preferred {
        // Open the gap so the next committed step reads, instead of circling in range.
        -dir * profile.speed
    } else if dist < profile.stop && profile.speed > 0.0 {
        -dir * profile.speed * if ranged { KITE_SPEED_SCALE } else { 1.0 }
    } else if dist > profile.preferred {
        dir * profile.speed
    } else if profile.strafe {
        let side = Vec3::new(-dir.z, 0.0, dir.x) * engage.side;
        let error = (dist - profile.preferred).clamp(-0.6, 0.6);
        // Waiting bodies drift slower than they fight, so the ring reads as pacing, not orbiting.
        let pace = if engage.may_attack { 1.0 } else { 0.55 };
        side * profile.speed * pace + dir * error * profile.speed
    } else {
        Vec3::ZERO
    };

    FightPlan { velocity, yaw, strike_damage, shot, winding_up: winding, attack_finished }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn part_boxes_match_the_demon_silhouette() {
        let brute = Fighter::claw_brute();
        let arm = brute.center(Part::LeftArm).unwrap();
        let head = brute.center(Part::Head).unwrap();
        let leg = brute.center(Part::LeftLeg).unwrap();
        assert!(arm.x < -0.2, "{arm}");
        assert!(head.y > 1.1, "{head}");
        assert!(leg.y < 0.4, "{leg}");
        assert!(brute.center(Part::RightArm).unwrap().x > 0.2);
    }

    #[test]
    fn one_cut_removes_only_the_part_the_swing_meets() {
        let mut brute = Fighter::claw_brute();
        let center = brute.center(Part::LeftArm).unwrap();
        let report = brute.strike_along(center + Vec3::Z, center - Vec3::Z, MELEE_CUT, None, 0.5);
        assert_eq!(report.hit, Some(Part::LeftArm));
        assert_eq!(report.severed, vec![Part::LeftArm]);
        assert!(brute.attached(Part::RightArm));
        assert!(brute.attached(Part::Head));
        assert_eq!(brute.stance(), Stance::LungeSlash);
        assert!(!brute.is_down());
    }

    #[test]
    fn damage_to_a_part_accumulates_until_it_comes_off() {
        let mut brute = Fighter::claw_brute();
        let first = brute.strike_part(Part::LeftArm, 20.0);
        assert!(first.severed.is_empty());
        assert_eq!(brute.stance(), Stance::CircleSlash);
        let second = brute.strike_part(Part::LeftArm, 20.0);
        assert_eq!(second.severed, vec![Part::LeftArm]);
        assert_eq!(brute.stance(), Stance::LungeSlash);
    }

    #[test]
    fn claw_brute_keeps_fighting_until_it_cannot_reach() {
        let mut brute = Fighter::claw_brute();
        brute.strike_part(Part::LeftArm, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::LungeSlash);
        brute.strike_part(Part::RightArm, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::GoreCharge);
        brute.strike_part(Part::Horns, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::TailWhip);
        let report = brute.strike_part(Part::Tail, MELEE_CUT);
        assert!(report.became_down);
        assert_eq!(brute.stance(), Stance::Down);
        assert_eq!(brute.down_reason(), Some(DownReason::NoWeapons));
        assert!(brute.attached(Part::LeftLeg) && brute.attached(Part::Head));
    }

    #[test]
    fn losing_legs_changes_how_they_move_without_ending_the_fight() {
        let mut brute = Fighter::claw_brute();
        brute.strike_part(Part::RightLeg, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::HopSwing);
        let hopping = plan_fight(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0));
        assert!(hopping.velocity.length() < 1.3, "{}", hopping.velocity.length());

        brute.strike_part(Part::LeftLeg, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::CrawlSwipe);
        let crawling = plan_fight(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0));
        assert!(crawling.velocity.length() < 0.7, "{}", crawling.velocity.length());
        assert!(!brute.is_down());
    }

    #[test]
    fn without_a_token_a_body_in_reach_does_not_wind_up() {
        let mut brute = Fighter::claw_brute();
        let waiting = Engage { may_attack: false, ..Engage::default() };
        let plan = plan_fight_engaged(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), waiting);
        assert!(!brute.is_winding());
        // Inside its waiting ring, it backs off instead of crowding in.
        assert!(plan.velocity.z > 0.0, "{}", plan.velocity);
        plan_fight(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert!(brute.is_winding());
        assert!(brute.telegraph().is_some());
    }

    #[test]
    fn the_token_comes_back_when_recovery_ends() {
        let mut brute = Fighter::claw_brute();
        let to = Vec3::new(0.0, 0.0, -1.0);
        let mut finished = false;
        for _ in 0..200 {
            finished |= plan_fight(&mut brute, 0.01, Vec3::ZERO, to).attack_finished;
            if finished {
                break;
            }
        }
        assert!(finished);
        assert!(!brute.mid_attack());
    }

    #[test]
    fn an_archer_backs_off_before_drawing_unless_cornered() {
        let mut archer = Fighter::new(Species::Archer);
        let close = Vec3::new(0.0, 0.0, -2.0);
        let plan = plan_fight(&mut archer, 0.016, Vec3::ZERO, close);
        assert!(!archer.is_winding());
        assert!(plan.velocity.z > 2.0, "scrambles away: {}", plan.velocity);
        let cornered = Engage { cornered: true, ..Engage::default() };
        plan_fight_engaged(&mut archer, 0.016, Vec3::ZERO, close, cornered);
        assert!(archer.is_winding());
    }

    #[test]
    fn haste_shortens_the_windup_and_the_telegraph_still_fills_from_zero() {
        let mut brute = Fighter::claw_brute();
        let to = Vec3::new(0.0, 0.0, -1.0);
        let hasted = Engage { haste: 0.5, ..Engage::default() };
        plan_fight_engaged(&mut brute, 0.001, Vec3::ZERO, to, hasted);
        assert!(brute.telegraph().unwrap().fill < 0.05);
        let mut landed_after = 0.0;
        for step in 1..200 {
            let plan = plan_fight_engaged(&mut brute, 0.01, Vec3::ZERO, to, hasted);
            if plan.strike_damage > 0.0 {
                landed_after = step as f32 * 0.01;
                break;
            }
        }
        let normal = profile(Stance::CircleSlash).windup;
        assert!(landed_after > 0.0 && landed_after < normal * 0.6, "{landed_after} vs {normal}");
    }

    #[test]
    fn breaking_poise_cancels_the_windup() {
        let mut brute = Fighter::claw_brute();
        plan_fight(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert!(brute.is_winding());
        // A torso chip takes no part but drains more than a fallen's poise.
        let report = brute.strike_part(Part::Torso, MELEE_CUT);
        assert!(report.severed.is_empty());
        assert!(report.staggered);
        assert!(!brute.is_winding());
        let plan = plan_fight(&mut brute, 0.016, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(plan.velocity, Vec3::ZERO);
        assert!(!brute.is_winding());
    }

    #[test]
    fn a_fresh_stagger_guards_against_stunlock() {
        let mut brute = Fighter::claw_brute();
        assert!(brute.strike_part(Part::Torso, 30.0).severed.is_empty());
        assert!(brute.strike_part(Part::Torso, 30.0).staggered);
        // Reel out the stagger; the guard window starts.
        for _ in 0..70 {
            plan_fight(&mut brute, 0.01, Vec3::ZERO, Vec3::new(0.0, 0.0, -5.0));
        }
        assert!(!brute.is_staggered());
        let report = brute.strike_part(Part::Torso, 10.0);
        assert!(!report.staggered);
    }

    #[test]
    fn a_zombie_shrugs_off_one_cut() {
        let mut zombie = Fighter::new(Species::Zombie);
        let report = zombie.strike_part(Part::Torso, MELEE_CUT);
        assert!(!report.staggered);
        assert!(zombie.strike_part(Part::Torso, MELEE_CUT).staggered);
    }

    #[test]
    fn a_blast_on_an_arm_does_not_core_the_torso() {
        let mut brute = Fighter::claw_brute();
        let center = brute.center(Part::LeftArm).unwrap();
        let report = brute.strike_area(center, 0.3, 120.0);
        assert!(report.severed.contains(&Part::LeftArm));
        assert!(brute.attached(Part::Head));
        assert!(brute.attached(Part::Torso));
        assert_eq!(brute.stance(), Stance::LungeSlash);
    }

    #[test]
    fn destroying_the_head_ends_the_fight_with_the_limbs_still_attached() {
        let mut brute = Fighter::claw_brute();
        let report = brute.strike_part(Part::Head, MELEE_CUT);
        assert!(report.became_down);
        assert_eq!(brute.down_reason(), Some(DownReason::Head));
        assert!(brute.attached(Part::LeftArm));
        let plan = plan_fight(&mut brute, 0.5, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0));
        assert_eq!(plan.strike_damage, 0.0);
    }

    #[test]
    fn a_committed_swing_misses_when_the_target_leaves_the_arc() {
        let mut brute = Fighter::claw_brute();
        let from = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, -1.2);
        let start = plan_fight(&mut brute, 0.05, from, ahead);
        assert!(start.winding_up);
        assert_eq!(start.strike_damage, 0.0);

        let windup = profile(Stance::CircleSlash).windup;
        let landed = plan_fight(&mut brute, windup + 0.02, from, ahead);
        assert!(landed.strike_damage > 0.0);

        let mut dodged = Fighter::claw_brute();
        plan_fight(&mut dodged, 0.05, from, ahead);
        let whiff = plan_fight(&mut dodged, windup + 0.02, from, Vec3::new(1.2, 0.0, 0.0));
        assert_eq!(whiff.strike_damage, 0.0);

        // After the blow they step back off the blade instead of orbiting.
        let reset = plan_fight(&mut brute, 0.016, from, ahead);
        assert_eq!(reset.strike_damage, 0.0);
        assert!(reset.velocity.z > 0.5, "recover should open space, got {}", reset.velocity);
        assert!(reset.velocity.x.abs() < 0.05, "recover should not strafe, got {}", reset.velocity);
    }

    #[test]
    fn a_swing_above_the_body_does_not_hit() {
        let mut brute = Fighter::claw_brute();
        let report = brute.strike_along(Vec3::new(0.0, 4.0, -1.0), Vec3::new(0.0, 4.0, 1.0), MELEE_CUT, None, 0.5);
        assert!(report.hit.is_none());
        assert_eq!(brute.stance(), Stance::CircleSlash);
    }

    #[test]
    fn swing_pose_follows_the_committed_attack() {
        let mut brute = Fighter::claw_brute();
        let from = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, -1.2);
        assert_eq!(brute.swing_pose(), SwingPose::Guard);

        plan_fight(&mut brute, 0.05, from, ahead);
        assert_eq!(brute.swing_pose(), SwingPose::Windup);

        let windup = profile(Stance::CircleSlash).windup;
        plan_fight(&mut brute, windup * 0.7, from, ahead);
        assert_eq!(brute.swing_pose(), SwingPose::Strike);

        plan_fight(&mut brute, windup, from, ahead);
        assert_eq!(brute.swing_pose(), SwingPose::Recover);
        let landed = brute.attack_progress().unwrap();
        assert!((landed - 0.36).abs() < 0.02, "the blow should be at the forward pose, progress {landed}");
    }

    #[test]
    fn a_skeleton_shield_catches_a_frontal_cut_and_not_a_side_cut() {
        let mut skeleton = Fighter::new(Species::Skeleton);
        assert_eq!(skeleton.stance(), Stance::SwordShield);
        let head = skeleton.center(Part::Head).unwrap();
        let from = head + Vec3::new(0.0, 0.0, -1.4);
        let to = head + Vec3::new(0.0, 0.0, 0.4);
        let report = skeleton.strike_along(from, to, MELEE_CUT, Some(Part::Head), 0.55);
        assert_eq!(report.hit, Some(Part::LeftArm));
        assert!(skeleton.attached(Part::Head));
        assert_eq!(skeleton.stance(), Stance::SwordLunge);

        let mut side = Fighter::new(Species::Skeleton);
        let arm = side.center(Part::RightArm).unwrap();
        let report = side.strike_along(arm + Vec3::new(1.1, 0.0, 0.3), arm, MELEE_CUT, Some(Part::RightArm), 0.55);
        assert_eq!(report.hit, Some(Part::RightArm));
        assert_eq!(side.stance(), Stance::ShieldBash);

        side.strike_part(Part::LeftArm, MELEE_CUT);
        assert_eq!(side.stance(), Stance::BoneKick);
        assert!(!side.is_down());
        side.strike_part(Part::LeftLeg, MELEE_CUT);
        side.strike_part(Part::RightLeg, MELEE_CUT);
        assert!(side.is_down());
        assert_eq!(side.down_reason(), Some(DownReason::NoWeapons));
        assert!(side.attached(Part::Head));
    }

    #[test]
    fn a_zombie_keeps_biting_after_both_arms_come_off() {
        let mut zombie = Fighter::new(Species::Zombie);
        assert_eq!(zombie.stance(), Stance::ZombieSlam);
        assert!(profile(Stance::ZombieSlam).speed < 1.0);
        assert!(profile(Stance::ZombieSlam).windup > profile(Stance::CircleSlash).windup);
        zombie.strike_part(Part::LeftArm, MELEE_CUT);
        assert_eq!(zombie.stance(), Stance::ZombieSlap);
        zombie.strike_part(Part::RightArm, MELEE_CUT);
        assert_eq!(zombie.stance(), Stance::ZombieBite);
        assert!(profile(Stance::ZombieBite).range < profile(Stance::ZombieSlam).range);
        assert!(!zombie.is_down());
        let report = zombie.strike_part(Part::Head, MELEE_CUT);
        assert!(report.became_down);
        assert_eq!(zombie.down_reason(), Some(DownReason::Head));
    }

    #[test]
    fn an_archer_looses_an_arrow_until_one_arm_is_gone() {
        let mut archer = Fighter::new(Species::Archer);
        assert_eq!(archer.stance(), Stance::ArrowVolley);
        assert!(profile(Stance::ArrowVolley).range > 8.0);
        let from = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, -8.0);
        let start = plan_fight(&mut archer, 0.05, from, ahead);
        assert!(start.winding_up);
        assert_eq!(start.strike_damage, 0.0);
        assert_eq!(start.shot, 0.0);
        assert_eq!(start.velocity, Vec3::ZERO);

        let windup = profile(Stance::ArrowVolley).windup;
        let landed = plan_fight(&mut archer, windup + 0.02, from, ahead);
        assert!(landed.shot > 0.0);
        assert_eq!(landed.strike_damage, 0.0);

        let mut dodged = Fighter::new(Species::Archer);
        plan_fight(&mut dodged, 0.05, from, ahead);
        let whiff = plan_fight(&mut dodged, windup + 0.02, from, Vec3::new(8.0, 0.0, 0.0));
        assert_eq!(whiff.shot, 0.0);

        // A shot does not turn the archer into a backpedaling melee fighter.
        let kite = plan_fight(&mut archer, 0.016, from, ahead);
        assert!(kite.velocity.z < -0.5, "archer should keep distance, got {}", kite.velocity);

        archer.strike_part(Part::LeftArm, MELEE_CUT);
        assert_eq!(archer.stance(), Stance::DaggerRush);
        assert!(profile(Stance::DaggerRush).range < 2.0);
        archer.strike_part(Part::RightArm, MELEE_CUT);
        assert_eq!(archer.stance(), Stance::RogueKick);
        archer.strike_part(Part::LeftLeg, MELEE_CUT);
        archer.strike_part(Part::RightLeg, MELEE_CUT);
        assert!(archer.is_down());
        assert_eq!(archer.down_reason(), Some(DownReason::NoWeapons));
    }

    #[test]
    fn losing_a_part_breaks_the_swing_they_had_started() {
        let mut brute = Fighter::claw_brute();
        let from = Vec3::ZERO;
        let ahead = Vec3::new(0.0, 0.0, -1.2);
        plan_fight(&mut brute, 0.05, from, ahead);
        assert!(brute.is_winding());
        brute.strike_part(Part::LeftArm, MELEE_CUT);
        assert_eq!(brute.stance(), Stance::LungeSlash);
        let windup = profile(Stance::LungeSlash).windup;
        let plan = plan_fight(&mut brute, windup + 0.02, from, ahead);
        assert_eq!(plan.strike_damage, 0.0);
    }

    #[test]
    fn the_cursor_grabs_a_near_miss_without_lengthening_a_sword_swing() {
        let fighter = Fighter::claw_brute();
        // Right-arm box ends at x = 0.55. This ray passes 0.60 m beside it.
        let beside = Vec3::new(1.15, 1.0, 4.0);
        let beside_end = Vec3::new(1.15, 1.0, -4.0);
        let grab = fighter.pick_part(beside, beside_end, None, CURSOR_PICK_RADIUS).unwrap();
        assert!(grab.distance > 0.50 && grab.distance < 0.75, "{}", grab.distance);
        let mut melee = Fighter::claw_brute();
        let report = melee.strike_along(beside, beside_end, MELEE_CUT, None, 0.55);
        assert_eq!(report.hit, None);
        assert!(melee.attached(Part::RightArm));

        // 17 samples on an 80 m ray sit 5 m apart, so a centred body can fall between them.
        let through = Vec3::new(0.0, 1.0, -42.0);
        let through_end = Vec3::new(0.0, 1.0, 38.0);
        let torso_min = Vec3::new(-0.25, 0.60, -0.20);
        let torso_max = Vec3::new(0.25, 1.20, 0.20);
        let coarse = segment_approach(through, through_end, torso_min, torso_max);
        assert!(coarse.distance > 1.0, "sampled sweep missed the torso by {}", coarse.distance);
        let centred = fighter.pick_part(through, through_end, None, CURSOR_PICK_RADIUS).unwrap();
        assert!(centred.distance < 0.05, "{}", centred.distance);

        // A downed body still has standing boxes. The cursor drops it before ranking.
        let mut corpse = Fighter::claw_brute();
        corpse.strike_part(Part::Head, MELEE_CUT);
        assert!(corpse.is_down());
        assert!(corpse.pick_part(through, through_end, None, CURSOR_PICK_RADIUS).is_some());

        let samples = [
            CursorSample { down: false, along: 10.0, miss: 0.1 },
            CursorSample { down: false, along: 6.0, miss: 0.8 },
            CursorSample { down: true, along: 5.0, miss: 0.0 },
        ];
        // Ground at 8 m. The close graze is 2 m behind that, past the slack, so the nearer body wins.
        assert_eq!(choose_cursor_body(&samples, Some(8.0), CURSOR_DEPTH_SLACK), Some(1));

        // Feet sit on the ground the ray hits. 5 cm of separation used to lose the old 15 cm bias.
        let feet = [CursorSample { down: false, along: 8.05, miss: 0.2 }];
        assert_eq!(choose_cursor_body(&feet, Some(8.10), CURSOR_DEPTH_SLACK), Some(0));

        let hidden = [CursorSample { down: false, along: 12.0, miss: 0.0 }];
        assert_eq!(choose_cursor_body(&hidden, Some(8.0), CURSOR_DEPTH_SLACK), None);
        assert_eq!(choose_cursor_body(&hidden, None, CURSOR_DEPTH_SLACK), Some(0));

        let tie = [
            CursorSample { down: false, along: 9.0, miss: 0.4 },
            CursorSample { down: false, along: 7.0, miss: 0.4 },
        ];
        assert_eq!(choose_cursor_body(&tie, Some(20.0), CURSOR_DEPTH_SLACK), Some(1));
    }
}
