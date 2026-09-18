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
}

pub struct Voices {
    output_rate: u32,
    playing: Vec<Voice>,
}

impl Voices {
    pub fn new(output_rate: u32) -> Voices {
        Voices { output_rate, playing: Vec::with_capacity(VOICES) }
    }

    pub fn play(&mut self, sound: Arc<Wav>, volume: f32) {
        if sound.samples.is_empty() || sound.rate == 0 {
            return;
        }
        if self.playing.len() >= VOICES {
            self.playing.remove(0);
        }
        let step = sound.rate as f32 / self.output_rate as f32;
        self.playing.push(Voice { sound, position: 0.0, step, volume });
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
                let at = voice.position as usize;
                if at >= total {
                    break;
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
        self.playing.retain(|v| (v.position as usize) < v.sound.samples.len() / v.sound.channels.max(1) as usize);
    }
}
