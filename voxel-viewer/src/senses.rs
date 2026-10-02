//! What an enemy knows about the player: sight, hearing, memory, and nerve.
//!
//! Pure rules. The line-of-sight test is passed in, so these run without a voxel world.
//! A body is idle until it sees the player, hears something, or is called by a packmate.
//! Alert bodies remember where the player was and go there when they lose sight.
//! Fallen scatter from a death nearby (Diablo II retreat mode); a cornered one fights.

use bevy::prelude::*;

use crate::combat::Species;

pub const SIGHT_RANGE: f32 = 22.0;
/// Inside this a body senses the player through walls and behind its back.
pub const NEAR_SENSE: f32 = 3.0;
/// An idle body only sees ahead of it: cosine of the edge of that view.
const IDLE_VIEW_DOT: f32 = -0.2;
/// Seconds an alert body keeps hunting after it last sensed the player.
pub const ALERT_MEMORY: f32 = 8.0;
/// A body that spots the player shouts; packmates this close come running.
pub const PACK_CALL_RADIUS: f32 = 12.0;
/// Fallen this close to a death scatter.
pub const FEAR_RADIUS: f32 = 5.0;
pub const FEAR_SECONDS: f32 = 3.5;
/// A body left with no arms may limp away for this long.
pub const MAIMED_FEAR_SECONDS: f32 = 6.0;
/// A fleeing body that cannot get away for this long turns and fights.
const CORNERED_AFTER: f32 = 0.4;
/// A sword connecting is quieter than any spell.
pub const MELEE_NOISE_RADIUS: f32 = 7.0;
/// A death cry carries this far, and tells listeners where the killer stood.
pub const DEATH_NOISE_RADIUS: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoiseKind {
    /// Impacts, blasts, collapses. Listeners go to look at the spot.
    Sound,
    /// A body went down here. Also frightens fallen.
    Death,
    /// A packmate shouting where it saw the player.
    Call,
}

#[derive(Clone, Copy, Debug)]
pub struct Noise {
    pub at: Vec3,
    pub radius: f32,
    pub kind: NoiseKind,
    /// Where the player is, when the noise gives that away.
    pub reveals: Option<Vec3>,
}

impl Noise {
    pub fn sound(at: Vec3, radius: f32) -> Self {
        Self { at, radius, kind: NoiseKind::Sound, reveals: None }
    }

    /// A blast of `radius_m` metres. Bigger blasts carry further.
    pub fn blast(center: Vec3, radius_m: f32) -> Self {
        Self::sound(center, 10.0 + radius_m * 8.0)
    }
}

/// Noises made this frame, heard by every enemy next update.
#[derive(Resource, Default)]
pub struct Noises(Vec<Noise>);

impl Noises {
    pub fn emit(&mut self, noise: Noise) {
        self.0.push(noise);
    }

    pub fn drain(&mut self) -> Vec<Noise> {
        std::mem::take(&mut self.0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Awareness {
    /// Seconds of alertness left. Zero is idle.
    pub alert: f32,
    pub last_known: Option<Vec3>,
    /// Has an unbroken line to the player this frame.
    pub sees: bool,
    /// Seconds of fleeing left.
    pub fear: f32,
    pub flee_from: Vec3,
    stuck: f32,
    /// A maimed body only loses its nerve once.
    pub maimed_fled: bool,
}

fn flat(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

impl Awareness {
    pub fn is_alert(&self) -> bool {
        self.alert > 0.0
    }

    pub fn fleeing(&self) -> bool {
        self.fear > 0.0
    }

    /// Fights the player this frame: alert, holding its nerve, and able to sense them.
    pub fn engaged(&self, distance: f32) -> bool {
        !self.fleeing() && self.is_alert() && (self.sees || distance < NEAR_SENSE)
    }

    /// Update sight and memory. Returns true when this look turned an idle body alert.
    pub fn look(&mut self, dt: f32, pos: Vec3, facing: Vec3, player: Vec3, line_clear: impl FnOnce() -> bool) -> bool {
        let was_idle = !self.is_alert();
        let offset = flat(player - pos);
        let distance = offset.length();
        let ahead = flat(facing).normalize_or_zero().dot(offset.normalize_or_zero());
        let in_view = distance < SIGHT_RANGE && (!was_idle || ahead > IDLE_VIEW_DOT);
        self.sees = in_view && line_clear();
        if self.sees || distance < NEAR_SENSE {
            self.alert = ALERT_MEMORY;
            self.last_known = Some(player);
            return was_idle;
        }
        self.alert = (self.alert - dt).max(0.0);
        if !self.is_alert() {
            self.last_known = None;
        }
        false
    }

    /// Returns true when the noise turned an idle body alert.
    pub fn hear(&mut self, pos: Vec3, noise: &Noise) -> bool {
        if pos.distance(noise.at) > noise.radius {
            return false;
        }
        let was_idle = !self.is_alert();
        self.alert = self.alert.max(ALERT_MEMORY);
        // A call or a death cry says where the player is; a plain sound only says where to look.
        if !self.sees {
            self.last_known = Some(noise.reveals.unwrap_or(noise.at));
        }
        was_idle
    }

    /// Returns true when the body just started fleeing.
    pub fn frighten(&mut self, from: Vec3, seconds: f32) -> bool {
        let started = !self.fleeing();
        self.fear = self.fear.max(seconds);
        self.flee_from = from;
        self.stuck = 0.0;
        started
    }

    /// Count down fear. `wanted` and `moved` are m/s; a body that can't get away turns to fight.
    pub fn tick_fear(&mut self, dt: f32, wanted: f32, moved: f32) {
        if !self.fleeing() {
            return;
        }
        self.fear = (self.fear - dt).max(0.0);
        if wanted > 0.1 && moved < wanted * 0.3 {
            self.stuck += dt;
            if self.stuck > CORNERED_AFTER {
                self.fear = 0.0;
                self.stuck = 0.0;
            }
        } else {
            self.stuck = 0.0;
        }
    }

    /// Unit direction away from what scared it. Straight through it picks a stable side.
    pub fn flee_direction(&self, pos: Vec3, fallback: Vec3) -> Vec3 {
        let away = flat(pos - self.flee_from);
        if away.length_squared() > 1.0e-4 {
            away.normalize()
        } else {
            flat(fallback).normalize_or_zero()
        }
    }
}

/// Diablo II: fallen scatter from a death nearby. Other bodies hold their nerve.
pub fn scared_by_death(species: Species, pos: Vec3, death_at: Vec3) -> bool {
    species == Species::Fallen && flat(pos - death_at).length() <= FEAR_RADIUS
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORWARD: Vec3 = Vec3::NEG_Z;

    #[test]
    fn an_idle_body_does_not_see_behind_it() {
        let mut senses = Awareness::default();
        let behind = Vec3::new(0.0, 0.0, 10.0);
        assert!(!senses.look(0.016, Vec3::ZERO, FORWARD, behind, || true));
        assert!(!senses.is_alert());
        assert!(senses.look(0.016, Vec3::ZERO, FORWARD, Vec3::new(0.0, 0.0, -10.0), || true));
        assert!(senses.is_alert());
    }

    #[test]
    fn walls_block_sight_but_not_the_near_sense() {
        let mut senses = Awareness::default();
        assert!(!senses.look(0.016, Vec3::ZERO, FORWARD, Vec3::new(0.0, 0.0, -10.0), || false));
        assert!(!senses.is_alert());
        assert!(senses.look(0.016, Vec3::ZERO, FORWARD, Vec3::new(0.0, 0.0, 2.0), || false));
        assert!(senses.engaged(2.0));
    }

    #[test]
    fn losing_sight_hunts_the_last_known_spot_then_forgets() {
        let mut senses = Awareness::default();
        let seen_at = Vec3::new(0.0, 0.0, -10.0);
        senses.look(0.016, Vec3::ZERO, FORWARD, seen_at, || true);
        senses.look(0.016, Vec3::ZERO, FORWARD, Vec3::new(5.0, 0.0, -10.0), || false);
        assert!(senses.is_alert() && !senses.engaged(11.0));
        assert_eq!(senses.last_known, Some(seen_at));
        for _ in 0..600 {
            senses.look(0.016, Vec3::ZERO, FORWARD, Vec3::new(5.0, 0.0, -10.0), || false);
        }
        assert!(!senses.is_alert());
        assert_eq!(senses.last_known, None);
    }

    #[test]
    fn a_call_reveals_the_player_and_a_sound_only_its_spot() {
        let mut senses = Awareness::default();
        let player = Vec3::new(3.0, 0.0, 3.0);
        let call = Noise { at: Vec3::new(5.0, 0.0, 0.0), radius: PACK_CALL_RADIUS, kind: NoiseKind::Call, reveals: Some(player) };
        assert!(senses.hear(Vec3::ZERO, &call));
        assert_eq!(senses.last_known, Some(player));
        let far = Noise::sound(Vec3::new(50.0, 0.0, 0.0), 10.0);
        assert!(!senses.hear(Vec3::ZERO, &far));
        let near = Noise::sound(Vec3::new(4.0, 0.0, 0.0), 10.0);
        senses.hear(Vec3::ZERO, &near);
        assert_eq!(senses.last_known, Some(Vec3::new(4.0, 0.0, 0.0)));
    }

    #[test]
    fn only_fallen_near_a_death_lose_their_nerve() {
        assert!(scared_by_death(Species::Fallen, Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0)));
        assert!(!scared_by_death(Species::Fallen, Vec3::ZERO, Vec3::new(8.0, 0.0, 0.0)));
        assert!(!scared_by_death(Species::Skeleton, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn a_cornered_body_stops_fleeing_and_fights() {
        let mut senses = Awareness::default();
        senses.alert = ALERT_MEMORY;
        assert!(senses.frighten(Vec3::ZERO, FEAR_SECONDS));
        assert!(!senses.engaged(1.0));
        senses.tick_fear(0.1, 3.0, 2.5);
        assert!(senses.fleeing());
        for _ in 0..5 {
            senses.tick_fear(0.1, 3.0, 0.0);
        }
        assert!(!senses.fleeing());
        assert!(senses.engaged(1.0));
    }

    #[test]
    fn fear_runs_out() {
        let mut senses = Awareness::default();
        senses.frighten(Vec3::ZERO, 1.0);
        for _ in 0..11 {
            senses.tick_fear(0.1, 3.0, 3.0);
        }
        assert!(!senses.fleeing());
    }
}
