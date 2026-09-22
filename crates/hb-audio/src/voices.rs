//! One-shot sound effects, mixed alongside the music.
//!
//! The engine's effects are `.WAV` files at their own rates - 327 of the 332
//! at 11,025 Hz, the rest at 22,050 or 8,287 - so each voice steps through its
//! samples at `rate / output_rate` and the mixer adds the result in.

use crate::wav::Wav;
use std::sync::Arc;

/// How many effects can sound at once. A shot, its impact and an explosion or
/// two; beyond this the oldest is dropped, which is what a fixed channel count
/// on 1996 hardware would have done anyway.
pub const VOICES: usize = 12;

struct Voice {
    sound: Arc<Wav>,
    position: f32,
    step: f32,
    volume: f32,
    /// A held sound - the missile warning - starts again at its end and
    /// plays until the caller stops it by this handle.
    held: Option<u64>,
}

/// What a single effect is worth of full scale. The music is already
/// mixed down by twelve in [`crate::Mixer`] and every effect is added on top
/// of it, so a voice at 1.0 is full scale on its own and a second one over it
/// clips. The engine plays its own effects at around half - the missile
/// warning goes out at 0x8000 - and its driver mixes with headroom, which is
/// what this stands in for.
pub const HEADROOM: f32 = 0.4;

/// Where the sum stops growing linearly and starts bending. Below this it is
/// left alone; above, it is curved into the last of the range, so a busy
/// moment gets quieter rather than square.
const KNEE: f32 = 0.6 * 32767.0;

/// The engine's `soundVolume` (`0x5125c0`, 16.16, 1.0 by default), which
/// scales every effect.
pub const FULL: f32 = 1.0;

pub struct Voices {
    output_rate: u32,
    playing: Vec<Voice>,
    next_handle: u64,
    /// `soundVolume` from the `.INI`.
    master: f32,
}

/// Fold what is past the knee into what is left of the range, so a sum of
/// several voices leans on the ceiling instead of hitting it. A hard clamp
/// is what makes a loud moment rasp.
fn soft(value: f32) -> i16 {
    let ceiling = 32767.0;
    let magnitude = value.abs();
    if magnitude <= KNEE {
        return value as i16;
    }
    let over = (magnitude - KNEE) / (ceiling - KNEE);
    let folded = KNEE + (ceiling - KNEE) * (over / (1.0 + over));
    (folded.min(ceiling) * value.signum()) as i16
}

impl Voices {
    pub fn new(output_rate: u32) -> Voices {
        Voices {
            output_rate,
            playing: Vec::with_capacity(VOICES),
            next_handle: 1,
            master: FULL,
        }
    }

    /// `soundVolume`, 0 to 1. Anything outside that is clamped.
    pub fn set_master(&mut self, volume: f32) {
        self.master = volume.clamp(0.0, 1.0);
    }

    pub fn play(&mut self, sound: Arc<Wav>, volume: f32) {
        if sound.samples.is_empty() || sound.rate == 0 {
            return;
        }
        if self.playing.len() >= VOICES {
            self.playing.remove(0);
        }
        let step = sound.rate as f32 / self.output_rate as f32;
        self.playing.push(Voice { sound, position: 0.0, step, volume, held: None });
    }

    /// Start a sound that loops until [`Voices::stop`] is called with the
    /// handle this returns. The engine holds one this way - the missile
    /// warning at `0x44ea5b` - and stops it when nothing is chasing the
    /// player any more.
    pub fn hold(&mut self, sound: Arc<Wav>, volume: f32) -> Option<u64> {
        if sound.samples.is_empty() || sound.rate == 0 {
            return None;
        }
        if self.playing.len() >= VOICES {
            self.playing.remove(0);
        }
        let handle = self.next_handle;
        self.next_handle += 1;
        let step = sound.rate as f32 / self.output_rate as f32;
        self.playing.push(Voice { sound, position: 0.0, step, volume, held: Some(handle) });
        Some(handle)
    }

    /// Let a held sound go. Unknown handles are ignored, so a caller may stop
    /// one twice.
    /// Drop every voice at once.
    pub fn silence(&mut self) {
        self.playing.clear();
    }

    pub fn stop(&mut self, handle: u64) {
        self.playing.retain(|v| v.held != Some(handle));
    }

    pub fn active(&self) -> usize {
        self.playing.len()
    }

    /// Add the voices into a stereo buffer that already holds the music.
    ///
    /// Every voice is summed at [`HEADROOM`] of its own volume and the whole
    /// thing goes through a soft knee at the end, rather than each voice
    /// being clamped into the buffer as it is added. A clamp per voice is
    /// what made a busy moment rasp: three explosions at once were three
    /// squared-off waves, not one loud one.
    pub fn mix_into(&mut self, out: &mut [i16]) {
        let frames = out.len() / 2;
        // Mix into floats so the sum is not cut short on the way.
        let mut sum: Vec<f32> = out.iter().map(|&s| s as f32).collect();
        for voice in &mut self.playing {
            let channels = voice.sound.channels.max(1) as usize;
            let total = voice.sound.samples.len() / channels;
            let gain = voice.volume * self.master * HEADROOM;
            for frame in 0..frames {
                if voice.position as usize >= total {
                    if voice.held.is_none() || total == 0 {
                        break;
                    }
                    // Held: round again from the start.
                    voice.position -= total as f32;
                }
                // The game's effects are 11,025 Hz and the device is usually
                // 44,100 or 48,000, so every fourth sample would be a step
                // and the steps are what sound like grit. Slide between them
                // instead.
                let at = voice.position as usize;
                let next = if at + 1 < total { at + 1 } else { at };
                let blend = voice.position - at as f32;
                // Mono in the game's effects; a stereo file is averaged.
                let sample = |i: usize| {
                    let mut s = 0i32;
                    for c in 0..channels {
                        s += voice.sound.samples[i * channels + c] as i32;
                    }
                    (s / channels as i32) as f32
                };
                let value = (sample(at) * (1.0 - blend) + sample(next) * blend) * gain;
                sum[frame * 2] += value;
                sum[frame * 2 + 1] += value;
                voice.position += voice.step;
            }
        }
        for (slot, value) in out.iter_mut().zip(sum) {
            *slot = soft(value);
        }
        self.playing.retain(|v| {
            v.held.is_some()
                || (v.position as usize) < v.sound.samples.len() / v.sound.channels.max(1) as usize
        });
    }
}
