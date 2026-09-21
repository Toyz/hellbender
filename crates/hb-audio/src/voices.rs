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

pub struct Voices {
    output_rate: u32,
    playing: Vec<Voice>,
    next_handle: u64,
}

impl Voices {
    pub fn new(output_rate: u32) -> Voices {
        Voices { output_rate, playing: Vec::with_capacity(VOICES), next_handle: 1 }
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
    pub fn stop(&mut self, handle: u64) {
        self.playing.retain(|v| v.held != Some(handle));
    }

    pub fn active(&self) -> usize {
        self.playing.len()
    }

    /// Add the voices into a stereo buffer that already holds the music.
    pub fn mix_into(&mut self, out: &mut [i16]) {
        let frames = out.len() / 2;
        for voice in &mut self.playing {
            let channels = voice.sound.channels.max(1) as usize;
            let total = voice.sound.samples.len() / channels;
            for frame in 0..frames {
                let mut at = voice.position as usize;
                if at >= total {
                    if voice.held.is_none() || total == 0 {
                        break;
                    }
                    // Held: round again from the start.
                    voice.position -= total as f32;
                    at = voice.position as usize;
                }
                // Mono in the game's effects; a stereo file is averaged.
                let mut sum = 0i32;
                for c in 0..channels {
                    sum += voice.sound.samples[at * channels + c] as i32;
                }
                let value = (sum / channels as i32) as f32 * voice.volume;
                for side in 0..2 {
                    let slot = &mut out[frame * 2 + side];
                    *slot = (*slot as f32 + value).clamp(-32768.0, 32767.0) as i16;
                }
                voice.position += voice.step;
            }
        }
        self.playing.retain(|v| {
            v.held.is_some()
                || (v.position as usize) < v.sound.samples.len() / v.sound.channels.max(1) as usize
        });
    }
}
