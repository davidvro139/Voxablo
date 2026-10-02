//! What an engaged body wants to do, scored like Dave Mark's Infinite Axis Utility System:
//! each intent multiplies a few considerations, each a response curve over one input.
//! The stance table still decides what a body *can* do; this decides what it *wants*.
//!
//! - Press: has the attack token, goes in.
//! - Hold: waits its turn on the ring (a shielded skeleton screens an archer while holding).
//! - Flank: the player is looking at it and a packmate is in front, so it circles to the side.
//! - Regroup: badly hurt with nobody near, it falls back to an ally.

/// Re-score at most this often, so intents read as decisions instead of flicker.
pub const RETHINK_SECONDS: f32 = 0.35;
/// The current intent keeps this much bonus; a rival has to clearly beat it.
const STICKINESS: f32 = 0.15;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Intent {
    Press,
    #[default]
    Hold,
    Flank,
    Regroup,
}

impl Intent {
    const ALL: [Intent; 4] = [Intent::Press, Intent::Hold, Intent::Flank, Intent::Regroup];

    pub fn label(self) -> &'static str {
        match self {
            Intent::Press => "pressing",
            Intent::Hold => "holding",
            Intent::Flank => "flanking",
            Intent::Regroup => "falling back",
        }
    }
}

/// Inputs to the decision, all from the body's point of view.
#[derive(Clone, Copy, Debug)]
pub struct Situation {
    pub distance: f32,
    /// Parts still attached, as a fraction of a whole body.
    pub integrity: f32,
    /// Other alert bodies within a few metres.
    pub allies_near: u32,
    /// Some other alert body exists to fall back to.
    pub ally_anywhere: bool,
    pub has_token: bool,
    /// 1 when the player looks straight at this body, -1 when its back is turned.
    pub facing_me: f32,
    pub ranged: bool,
    /// Zombies don't plan: they press or wait.
    pub mindless: bool,
}

/// 0 below `lo`, 1 above `hi`, straight in between.
fn linear(x: f32, lo: f32, hi: f32) -> f32 {
    ((x - lo) / (hi - lo)).clamp(0.0, 1.0)
}

fn flag(on: bool) -> f32 {
    if on {
        1.0
    } else {
        0.0
    }
}

pub fn score(intent: Intent, s: &Situation) -> f32 {
    match intent {
        Intent::Press => {
            if s.ranged {
                return 1.0;
            }
            0.2 + 0.75 * flag(s.has_token)
        }
        Intent::Hold => {
            if s.ranged {
                return 0.0;
            }
            0.55 * flag(!s.has_token)
        }
        Intent::Flank => {
            if s.ranged || s.mindless || s.has_token {
                return 0.0;
            }
            let watched = linear(s.facing_me, 0.25, 0.85);
            let covered = if s.allies_near > 0 { 1.0 } else { 0.35 };
            // Worth it from the waiting ring out; not from across the map.
            let room = linear(s.distance, 1.0, 2.5) * (1.0 - linear(s.distance, 10.0, 16.0));
            0.9 * watched * covered * room
        }
        Intent::Regroup => {
            if s.mindless || !s.ally_anywhere {
                return 0.0;
            }
            let hurt = 1.0 - linear(s.integrity, 0.35, 0.7);
            let alone = if s.allies_near == 0 { 1.0 } else { 0.15 };
            0.95 * hurt * alone
        }
    }
}

/// Best intent, with a bonus for the current one so a near tie doesn't flip it.
pub fn choose(current: Intent, s: &Situation) -> Intent {
    Intent::ALL
        .into_iter()
        .map(|intent| (intent, score(intent, s) + if intent == current { STICKINESS } else { 0.0 }))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(intent, _)| intent)
        .unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Situation {
        Situation {
            distance: 3.0,
            integrity: 1.0,
            allies_near: 1,
            ally_anywhere: true,
            has_token: false,
            facing_me: 0.0,
            ranged: false,
            mindless: false,
        }
    }

    #[test]
    fn the_token_holder_presses_and_the_rest_hold() {
        assert_eq!(choose(Intent::Hold, &Situation { has_token: true, ..base() }), Intent::Press);
        assert_eq!(choose(Intent::Press, &base()), Intent::Hold);
    }

    #[test]
    fn a_watched_body_with_cover_flanks() {
        let watched = Situation { facing_me: 0.95, ..base() };
        assert_eq!(choose(Intent::Hold, &watched), Intent::Flank);
        // Alone, it is not worth the walk.
        let alone = Situation { allies_near: 0, ..watched };
        assert_eq!(choose(Intent::Hold, &alone), Intent::Hold);
        // Zombies never plan it.
        assert_eq!(choose(Intent::Hold, &Situation { mindless: true, ..watched }), Intent::Hold);
    }

    #[test]
    fn a_wrecked_body_alone_falls_back_if_someone_is_there() {
        let wrecked = Situation { integrity: 0.3, allies_near: 0, ..base() };
        assert_eq!(choose(Intent::Hold, &wrecked), Intent::Regroup);
        assert_eq!(choose(Intent::Hold, &Situation { ally_anywhere: false, ..wrecked }), Intent::Hold);
        // Back with the pack, it rejoins the fight.
        assert_ne!(choose(Intent::Regroup, &Situation { allies_near: 2, ..wrecked }), Intent::Regroup);
    }

    #[test]
    fn a_near_tie_keeps_the_current_intent() {
        let edge = Situation { facing_me: 0.62, ..base() };
        let flank = score(Intent::Flank, &edge);
        let hold = score(Intent::Hold, &edge);
        assert!((flank - hold).abs() < STICKINESS, "{flank} vs {hold}");
        assert_eq!(choose(Intent::Hold, &edge), Intent::Hold);
        assert_eq!(choose(Intent::Flank, &edge), Intent::Flank);
    }

    #[test]
    fn archers_always_press() {
        let archer = Situation { ranged: true, integrity: 0.2, allies_near: 0, ..base() };
        assert_eq!(choose(Intent::Hold, &archer), Intent::Press);
    }
}
