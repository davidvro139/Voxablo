//! Pacing, after Left 4 Dead's AI Director: build up, peak, relax. It never adds enemies, since the
//! world's population is finite. It only decides when an unaware group gets a nudge toward the
//! player, so fights arrive in waves with breathing room, and it eases the pressure when the
//! player is nearly dead.

use bevy::prelude::*;

/// Intensity added per point of damage the player takes.
const DAMAGE_INTENSITY: f32 = 1.0 / 45.0;
/// Intensity added per second for each body fighting the player.
const ENGAGED_INTENSITY: f32 = 0.04;
const DECAY_QUIET: f32 = 0.08;
const DECAY_FIGHTING: f32 = 0.02;
const PEAK_AT: f32 = 0.85;
/// A peak lasts at least this long, and ends at the latest after this long.
const PEAK_MIN: f32 = 3.0;
const PEAK_MAX: f32 = 20.0;
const RELAX_SECONDS: f32 = 10.0;
/// Quiet this long during a build-up and the director wakes a group.
const QUIET_BEFORE_NUDGE: f32 = 6.0;
const NUDGE_COOLDOWN: f32 = 12.0;
/// Below this health only one melee body attacks at a time.
const MERCY_HEALTH: f32 = 35.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    BuildUp,
    Peak,
    Relax,
}

#[derive(Resource, Debug, Default)]
pub struct Director {
    pub phase: Phase,
    pub intensity: f32,
    in_phase: f32,
    quiet: f32,
    /// Seconds until another nudge is allowed. Zero at the start, so the first wave can come.
    nudge_wait: f32,
    last_health: Option<f32>,
    /// Ease the pressure: fewer melee attackers at once.
    pub mercy: bool,
}

/// What the director wants done this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    None,
    /// Wake the unaware group nearest the player.
    Nudge,
}

impl Director {
    /// Seconds spent in the current phase.
    pub fn phase_time(&self) -> f32 {
        self.in_phase
    }

    /// Seconds until a wave may be woken, counting the quiet it still needs.
    pub fn next_wave_in(&self) -> f32 {
        if self.phase != Phase::BuildUp {
            return f32::INFINITY;
        }
        self.nudge_wait.max(QUIET_BEFORE_NUDGE - self.quiet)
    }

    /// Advance pacing. `engaged` is how many bodies are fighting the player right now;
    /// `sleepers` whether any unaware body is left to wake.
    pub fn tick(&mut self, dt: f32, health: f32, engaged: u32, sleepers: bool) -> Order {
        let taken = self.last_health.map_or(0.0, |last| (last - health).max(0.0));
        self.last_health = Some(health);
        self.mercy = health < MERCY_HEALTH;

        let fighting = engaged > 0;
        self.intensity += taken * DAMAGE_INTENSITY + engaged as f32 * ENGAGED_INTENSITY * dt;
        self.intensity -= if fighting { DECAY_FIGHTING } else { DECAY_QUIET } * dt;
        self.intensity = self.intensity.clamp(0.0, 1.0);
        self.in_phase += dt;
        self.quiet = if fighting { 0.0 } else { self.quiet + dt };
        self.nudge_wait = (self.nudge_wait - dt).max(0.0);

        let next = match self.phase {
            Phase::BuildUp if self.intensity >= PEAK_AT => Some(Phase::Peak),
            Phase::Peak if self.in_phase >= PEAK_MAX || (self.in_phase >= PEAK_MIN && !fighting) => Some(Phase::Relax),
            Phase::Relax if self.in_phase >= RELAX_SECONDS => Some(Phase::BuildUp),
            _ => None,
        };
        if let Some(phase) = next {
            self.phase = phase;
            self.in_phase = 0.0;
        }

        let ready = self.phase == Phase::BuildUp && self.quiet >= QUIET_BEFORE_NUDGE && self.nudge_wait <= 0.0;
        if ready && sleepers {
            self.nudge_wait = NUDGE_COOLDOWN;
            Order::Nudge
        } else {
            Order::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(director: &mut Director, seconds: f32, health: f32, engaged: u32) -> Vec<Order> {
        let mut orders = Vec::new();
        for _ in 0..(seconds / 0.1) as usize {
            orders.push(director.tick(0.1, health, engaged, true));
        }
        orders
    }

    #[test]
    fn a_quiet_build_up_wakes_a_group_then_waits() {
        let mut director = Director::default();
        let orders = run(&mut director, 7.0, 100.0, 0);
        assert_eq!(orders.iter().filter(|o| **o == Order::Nudge).count(), 1);
        let orders = run(&mut director, 6.0, 100.0, 0);
        assert!(orders.iter().all(|o| *o == Order::None), "cooldown");
    }

    #[test]
    fn heavy_damage_peaks_then_relaxes_without_nudges() {
        let mut director = Director::default();
        director.tick(0.1, 100.0, 3, true);
        director.tick(0.1, 55.0, 3, true);
        assert_eq!(director.phase, Phase::Peak);
        run(&mut director, 4.0, 55.0, 0);
        assert_eq!(director.phase, Phase::Relax);
        let orders = run(&mut director, 9.0, 55.0, 0);
        assert!(orders.iter().all(|o| *o == Order::None));
        run(&mut director, 2.0, 55.0, 0);
        assert_eq!(director.phase, Phase::BuildUp);
    }

    #[test]
    fn a_peak_holds_while_the_fight_goes_on() {
        let mut director = Director::default();
        director.tick(0.1, 100.0, 2, true);
        director.tick(0.1, 50.0, 2, true);
        run(&mut director, 10.0, 50.0, 2);
        assert_eq!(director.phase, Phase::Peak);
        run(&mut director, 11.0, 50.0, 2);
        assert_eq!(director.phase, Phase::Relax, "a peak can't last forever");
    }

    #[test]
    fn low_health_asks_for_mercy_and_respawn_is_not_damage() {
        let mut director = Director::default();
        director.tick(0.1, 30.0, 1, true);
        assert!(director.mercy);
        let before = director.intensity;
        director.tick(0.1, 100.0, 0, true);
        assert!(!director.mercy);
        assert!(director.intensity <= before);
    }

    #[test]
    fn nothing_to_wake_means_no_order() {
        let mut director = Director::default();
        let mut orders = Vec::new();
        for _ in 0..100 {
            orders.push(director.tick(0.1, 100.0, 0, false));
        }
        assert!(orders.iter().all(|o| *o == Order::None));
    }
}
