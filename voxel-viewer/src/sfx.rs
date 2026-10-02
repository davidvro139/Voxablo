//! Short combat cues, synthesized at startup. They are original: a chunky weapon hit,
//! a falling hurt bark, and a separate voice for each spell. Nothing here is sampled
//! from another game.

use bevy::audio::{AudioSourceBundle, Decodable, PlaybackSettings, Source, Volume};
use bevy::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const RATE: u32 = 22_050;
const BEAM_GAP: f32 = 0.17;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Cue {
    Swing = 0,
    Hit = 1,
    Clang = 2,
    Sever = 3,
    Hurt = 4,
    Death = 5,
    Arrow = 6,
    ArrowHit = 7,
    Bolt = 8,
    BoltHit = 9,
    FireCast = 10,
    FireBurst = 11,
    Meteor = 12,
    Frost = 13,
    Beam = 14,
}

impl Cue {
    const COUNT: usize = 15;

    fn gain(self) -> f32 {
        match self {
            Cue::Swing => 0.46,
            Cue::Hit => 0.68,
            Cue::Clang => 0.58,
            Cue::Sever => 0.78,
            Cue::Hurt => 0.62,
            Cue::Death => 0.84,
            Cue::Arrow => 0.42,
            Cue::ArrowHit => 0.5,
            Cue::Bolt => 0.52,
            Cue::BoltHit => 0.5,
            Cue::FireCast => 0.48,
            Cue::FireBurst => 0.74,
            Cue::Meteor => 0.88,
            Cue::Frost => 0.66,
            Cue::Beam => 0.28,
        }
    }

    fn all() -> [Cue; Self::COUNT] {
        [
            Cue::Swing,
            Cue::Hit,
            Cue::Clang,
            Cue::Sever,
            Cue::Hurt,
            Cue::Death,
            Cue::Arrow,
            Cue::ArrowHit,
            Cue::Bolt,
            Cue::BoltHit,
            Cue::FireCast,
            Cue::FireBurst,
            Cue::Meteor,
            Cue::Frost,
            Cue::Beam,
        ]
    }
}

#[derive(Asset, TypePath, Clone)]
pub struct SfxClip {
    samples: Arc<[f32]>,
    rate: u32,
}

pub struct Pcm {
    samples: Arc<[f32]>,
    index: usize,
    rate: u32,
}

impl Iterator for Pcm {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = *self.samples.get(self.index)?;
        self.index += 1;
        Some(sample)
    }
}

impl Source for Pcm {
    fn current_frame_len(&self) -> Option<usize> {
        Some(self.samples.len().saturating_sub(self.index))
    }

    fn channels(&self) -> u16 {
        1
    }

    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(self.samples.len() as f64 / self.rate as f64))
    }
}

impl Decodable for SfxClip {
    type DecoderItem = f32;
    type Decoder = Pcm;

    fn decoder(&self) -> Self::Decoder {
        Pcm { samples: Arc::clone(&self.samples), index: 0, rate: self.rate }
    }
}

#[derive(Resource)]
pub struct SfxBank {
    clips: [Handle<SfxClip>; Cue::COUNT],
    hurt_alt: Handle<SfxClip>,
    salt: u32,
    beam_ready: f32,
}

impl SfxBank {
    pub fn play(&mut self, commands: &mut Commands, cue: Cue) {
        self.play_scaled(commands, cue, 1.0);
    }

    pub fn play_scaled(&mut self, commands: &mut Commands, cue: Cue, scale: f32) {
        self.salt = self.salt.wrapping_add(1);
        let steps = (self.salt % 9) as f32;
        let speed = 0.96 + steps * 0.01;
        let handle = if cue == Cue::Hurt && self.salt % 2 == 0 {
            self.hurt_alt.clone()
        } else {
            self.clips[cue as usize].clone()
        };
        commands.spawn(AudioSourceBundle {
            source: handle,
            settings: PlaybackSettings::DESPAWN
                .with_volume(Volume::new(cue.gain() * scale))
                .with_speed(speed),
        });
    }

    /// A channelled beam crackles, but not on every damage tick.
    pub fn tick_beam(&mut self, commands: &mut Commands, now: f32) {
        if let Some(next) = next_beam(self.beam_ready, now) {
            self.play(commands, Cue::Beam);
            self.beam_ready = next;
        }
    }

    pub fn rest_beam(&mut self) {
        self.beam_ready = 0.0;
    }
}

fn next_beam(ready: f32, now: f32) -> Option<f32> {
    if ready > 0.0 && now < ready {
        None
    } else {
        Some(now + BEAM_GAP)
    }
}

pub fn load(mut commands: Commands, mut assets: ResMut<Assets<SfxClip>>) {
    let mut clip = |cue: Cue| assets.add(SfxClip { samples: Arc::from(render(cue)), rate: RATE });
    let clips = Cue::all().map(&mut clip);
    let hurt_alt = assets.add(SfxClip { samples: Arc::from(hurt(142.0)), rate: RATE });
    commands.insert_resource(SfxBank { clips, hurt_alt, salt: 1, beam_ready: 0.0 });
}

fn render(cue: Cue) -> Vec<f32> {
    match cue {
        Cue::Swing => swing(),
        Cue::Hit => hit(),
        Cue::Clang => clang(),
        Cue::Sever => sever(),
        Cue::Hurt => hurt(168.0),
        Cue::Death => death(),
        Cue::Arrow => arrow(),
        Cue::ArrowHit => arrow_hit(),
        Cue::Bolt => bolt(),
        Cue::BoltHit => bolt_hit(),
        Cue::FireCast => fire_cast(),
        Cue::FireBurst => boom(0.32, 58.0, 1.0),
        Cue::Meteor => boom(0.48, 42.0, 1.35),
        Cue::Frost => frost(),
        Cue::Beam => beam(),
    }
}

struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / 16_777_216.0 * 2.0 - 1.0
    }
}

fn samples(seconds: f32) -> usize {
    (RATE as f32 * seconds) as usize
}

fn finish(mut buffer: Vec<f32>) -> Vec<f32> {
    let peak = buffer.iter().fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    if peak > 0.0001 {
        let gain = 0.82 / peak;
        for sample in &mut buffer {
            *sample = (*sample * gain).clamp(-1.0, 1.0);
        }
    }
    let fade = (RATE as usize) / 400;
    let len = buffer.len();
    for i in 0..fade.min(len / 2) {
        let weight = i as f32 / fade as f32;
        buffer[i] *= weight;
        buffer[len - 1 - i] *= weight;
    }
    buffer
}

fn swing() -> Vec<f32> {
    let n = samples(0.16);
    let mut noise = Noise::new(3);
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let u = t / 0.16;
        let cutoff = 2400.0 - 1800.0 * u;
        let alpha = alpha(cutoff);
        let white = noise.next();
        low += alpha * (white - low);
        let env = (u * 10.0).min(1.0) * (1.0 - u).powf(1.3);
        buffer.push(low * env * 1.6);
    }
    finish(buffer)
}

fn hit() -> Vec<f32> {
    let n = samples(0.12);
    let mut noise = Noise::new(11);
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let thump = (t * 86.0 * std::f32::consts::TAU).sin() * (-t * 26.0).exp();
        let body = (t * 196.0 * std::f32::consts::TAU).sin() * (-t * 42.0).exp() * 0.4;
        let white = noise.next();
        low += 0.22 * (white - low);
        let crack = if t < 0.007 { white * 0.9 } else { low * 0.75 };
        buffer.push((thump + body + crack * (-t * 48.0).exp()).tanh());
    }
    finish(buffer)
}

fn clang() -> Vec<f32> {
    let n = samples(0.15);
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let ring = (t * 720.0 * std::f32::consts::TAU).sin() * (-t * 16.0).exp()
            + (t * 1140.0 * std::f32::consts::TAU).sin() * (-t * 20.0).exp() * 0.45
            + (t * 188.0 * std::f32::consts::TAU).sin() * (-t * 14.0).exp() * 0.35;
        buffer.push(ring.tanh());
    }
    finish(buffer)
}

fn sever() -> Vec<f32> {
    let n = samples(0.22);
    let mut noise = Noise::new(19);
    let mut low = 0.0;
    let mut phase = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let freq = 78.0 - 36.0 * (t / 0.22);
        phase = (phase + freq / RATE as f32).fract();
        let body = (phase * std::f32::consts::TAU).sin() * (-t * 7.0).exp();
        let white = noise.next();
        low += 0.16 * (white - low);
        let crack = low * (-t * 14.0).exp();
        let second = if t > 0.045 { noise.next() * (-(t - 0.045) * 30.0).exp() * 0.45 } else { 0.0 };
        buffer.push((body * 0.8 + crack + second).tanh());
    }
    finish(buffer)
}

fn hurt(start: f32) -> Vec<f32> {
    let n = samples(0.22);
    let mut noise = Noise::new(start.to_bits());
    let mut phase = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let freq = start + (88.0 - start) * (t / 0.22);
        phase = (phase + freq / RATE as f32).fract();
        let voice = (phase * std::f32::consts::TAU).sin() + 0.28 * (phase * std::f32::consts::TAU * 2.0).sin();
        let env = (t * 50.0).min(1.0) * (-t * 9.0).exp();
        let breath = noise.next() * (-t * 28.0).exp() * 0.22;
        buffer.push((voice * env + breath).tanh());
    }
    finish(buffer)
}

fn death() -> Vec<f32> {
    let n = samples(0.46);
    let mut noise = Noise::new(29);
    let mut phase = 0.0;
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let freq = 146.0 + (52.0 - 146.0) * (t / 0.46);
        phase = (phase + freq / RATE as f32).fract();
        let voice = (phase * std::f32::consts::TAU).sin() + 0.22 * (phase * std::f32::consts::TAU * 2.0).sin();
        let env = (t * 30.0).min(1.0) * (-t * 5.5).exp();
        let white = noise.next();
        low += 0.12 * (white - low);
        let thud = (t * 58.0 * std::f32::consts::TAU).sin() * (-t * 12.0).exp() * 0.7;
        buffer.push((voice * env * 0.85 + low * (-t * 8.0).exp() * 0.35 + thud).tanh());
    }
    finish(buffer)
}

fn arrow() -> Vec<f32> {
    let freq = 740.0;
    let period = (RATE as f32 / freq).max(2.0) as usize;
    let mut delay = vec![0.0; period];
    let mut noise = Noise::new(41);
    for sample in &mut delay {
        *sample = noise.next();
    }
    let n = samples(0.1);
    let mut buffer = Vec::with_capacity(n);
    let mut index = 0;
    for k in 0..n {
        let averaged = (delay[index] + delay[(index + 1) % period]) * 0.5 * 0.94;
        delay[index] = averaged;
        let u = k as f32 / n as f32;
        let zip = if u < 0.04 { noise.next() * (1.0 - u / 0.04) * 0.35 } else { 0.0 };
        buffer.push(averaged * (1.0 - u) + zip);
        index = (index + 1) % period;
    }
    finish(buffer)
}

fn arrow_hit() -> Vec<f32> {
    let n = samples(0.08);
    let mut noise = Noise::new(43);
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let thud = (t * 140.0 * std::f32::consts::TAU).sin() * (-t * 40.0).exp();
        let tick = if t < 0.006 { noise.next() } else { 0.0 };
        buffer.push((thud * 0.8 + tick * 0.5).tanh());
    }
    finish(buffer)
}

fn bolt() -> Vec<f32> {
    let n = samples(0.14);
    let mut phase = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let u = (t / 0.09).min(1.0);
        let freq = 460.0 * 3.1_f32.powf(u);
        phase = (phase + freq / RATE as f32).fract();
        let chirp = (phase * std::f32::consts::TAU).sin();
        let env = if t < 0.09 { (t * 40.0).min(1.0) } else { (-(t - 0.09) * 28.0).exp() };
        let ping = if t > 0.07 {
            (t * 1680.0 * std::f32::consts::TAU).sin() * (-(t - 0.07) * 24.0).exp() * 0.45
        } else {
            0.0
        };
        buffer.push((chirp * env * 0.75 + ping).tanh());
    }
    finish(buffer)
}

fn bolt_hit() -> Vec<f32> {
    let n = samples(0.09);
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let ping = (t * 1320.0 * std::f32::consts::TAU).sin() * (-t * 30.0).exp()
            + (t * 1960.0 * std::f32::consts::TAU).sin() * (-t * 36.0).exp() * 0.4;
        buffer.push(ping.tanh());
    }
    finish(buffer)
}

fn fire_cast() -> Vec<f32> {
    let n = samples(0.2);
    let mut noise = Noise::new(53);
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let u = t / 0.2;
        low += alpha(420.0) * (noise.next() - low);
        let env = (u * 6.0).min(1.0) * (1.0 - u).powf(1.2);
        let crackle = if noise.next() > 0.82 { noise.next() * 0.5 } else { 0.0 };
        buffer.push(low * env * 1.8 + crackle * env);
    }
    finish(buffer)
}

fn boom(seconds: f32, sub: f32, body: f32) -> Vec<f32> {
    let n = samples(seconds);
    let mut noise = Noise::new(sub.to_bits());
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let bass = (t * sub * std::f32::consts::TAU).sin() * (-t * 8.0).exp();
        low += alpha(700.0) * (noise.next() - low);
        let crack = low * (-t * 6.5).exp();
        let spike = if noise.next() > 0.9 { noise.next() * (-t * 10.0).exp() } else { 0.0 };
        buffer.push((bass * body + crack * 0.85 + spike * 0.35).tanh());
    }
    finish(buffer)
}

fn frost() -> Vec<f32> {
    let n = samples(0.3);
    let mut noise = Noise::new(67);
    let mut low = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let white = noise.next();
        low += alpha(500.0) * (white - low);
        let air = (white - low) * (-t * 9.0).exp();
        let chime = (t * 1860.0 * std::f32::consts::TAU).sin() * (-t * 14.0).exp() * 0.35
            + (t * 2480.0 * std::f32::consts::TAU).sin() * (-t * 16.0).exp() * 0.22
            + (t * 920.0 * std::f32::consts::TAU).sin() * (-t * 10.0).exp() * 0.18;
        buffer.push((air * 0.8 + chime).tanh());
    }
    finish(buffer)
}

fn beam() -> Vec<f32> {
    let n = samples(0.11);
    let mut noise = Noise::new(71);
    let mut phase = 0.0;
    let mut buffer = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        phase = (phase + 78.0 / RATE as f32).fract();
        let saw = phase * 2.0 - 1.0;
        let buzz = saw * 0.45 + noise.next() * 0.55;
        let env = (t * 80.0).min(1.0) * (1.0 - t / 0.11);
        buffer.push((buzz * env * 1.4).tanh());
    }
    finish(buffer)
}

fn alpha(cutoff: f32) -> f32 {
    let dt = 1.0 / RATE as f32;
    let rc = 1.0 / (std::f32::consts::TAU * cutoff);
    dt / (rc + dt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(samples: &[f32]) -> f32 {
        let energy: f32 = samples.iter().map(|sample| sample * sample).sum();
        (energy / samples.len() as f32).sqrt()
    }

    #[test]
    fn every_cue_is_short_audible_and_distinct() {
        let rendered: Vec<Vec<f32>> = Cue::all().iter().copied().map(render).collect();
        for samples in &rendered {
            assert!(samples.len() > RATE as usize / 20, "shorter than 50 ms");
            assert!(samples.len() < RATE as usize, "longer than a second");
            assert!(samples.iter().all(|sample| sample.is_finite() && sample.abs() <= 1.0));
            assert!(rms(samples) > 0.04, "too quiet: {}", rms(samples));
        }
        let hit = &rendered[Cue::Hit as usize];
        let clang = &rendered[Cue::Clang as usize];
        let n = hit.len().min(clang.len());
        let gap: f32 = hit.iter().zip(clang.iter()).take(n).map(|(a, b)| (a - b).abs()).sum::<f32>() / n as f32;
        assert!(gap > 0.08, "hit and clang collapsed together: {gap}");
        assert!(hurt(168.0).len() != 0);
        assert!(next_beam(0.0, 1.0) == Some(1.0 + BEAM_GAP));
        assert!(next_beam(2.0, 1.5).is_none());
        assert!(next_beam(1.2, 1.2) == Some(1.2 + BEAM_GAP));
    }
}
