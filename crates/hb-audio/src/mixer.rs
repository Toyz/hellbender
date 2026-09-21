//! A ProTracker playback mixer.
//!
//! Enough of the format to play the fifteen modules in the archives: note
//! triggering, volume, the tempo and speed effects, position jump and pattern
//! break, volume slides, portamento, vibrato and sample offset. The effects it
//! does not implement are listed in `unimplemented_effects` so a listener can
//! tell what is missing rather than wonder.
//!
//! No dependency, and it produces samples rather than playing them.

use crate::modfile::{Module, Note, ROWS};

/// The PAL Amiga clock, which is what a period is measured against.
pub const PAL_CLOCK: f32 = 7_093_789.2;

/// Default ticks per row and beats per minute, as ProTracker starts.
pub const DEFAULT_SPEED: u32 = 6;
pub const DEFAULT_TEMPO: u32 = 125;

#[derive(Debug, Clone, Default)]
struct Channel {
    sample: usize,
    position: f32,
    step: f32,
    period: f32,
    target_period: f32,
    volume: u8,
    finetune: i8,
    playing: bool,
    porta_speed: u8,
    vibrato_depth: u8,
    vibrato_speed: u8,
    vibrato_phase: u8,
    /// Which side this channel is panned to, as ProTracker's hard LRRL.
    left: bool,
}

pub struct Mixer {
    module: Module,
    rate: u32,
    channels: Vec<Channel>,
    order_index: usize,
    row: usize,
    tick: u32,
    speed: u32,
    tempo: u32,
    /// Samples still to generate before the next tick.
    until_tick: f32,
    pub finished: bool,
    /// The engine's `musicVolume` (`0x5125bc`, 16.16, 1.0 by default).
    volume: f32,
    /// Effects seen in this module that the mixer ignores.
    pub unimplemented_effects: Vec<u8>,
}

/// ProTracker's sine table, used by vibrato.
const SINE: [i16; 32] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250,
    244, 235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24,
];

impl Mixer {
    pub fn new(module: Module, rate: u32) -> Mixer {
        let channels = (0..module.channels)
            .map(|i| Channel {
                volume: 64,
                // ProTracker pans hard left, right, right, left.
                left: matches!(i % 4, 0 | 3),
                ..Channel::default()
            })
            .collect();
        Mixer {
            module,
            rate,
            channels,
            order_index: 0,
            row: 0,
            tick: 0,
            speed: DEFAULT_SPEED,
            tempo: DEFAULT_TEMPO,
            until_tick: 0.0,
            finished: false,
            volume: 1.0,
            unimplemented_effects: Vec::new(),
        }
    }

    pub fn module(&self) -> &Module {
        &self.module
    }

    pub fn position(&self) -> (usize, usize) {
        (self.order_index, self.row)
    }

    fn samples_per_tick(&self) -> f32 {
        // A tick is 2.5 / tempo seconds, which is ProTracker's own formula.
        self.rate as f32 * 2.5 / self.tempo as f32
    }

    /// Fill a stereo buffer. Returns the number of frames written.
    /// `musicVolume`, 0 to 1. Anything outside that is clamped.
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn render(&mut self, out: &mut [i16]) -> usize {
        let frames = out.len() / 2;
        for frame in 0..frames {
            if self.until_tick <= 0.0 {
                self.advance_tick();
                self.until_tick += self.samples_per_tick();
            }
            self.until_tick -= 1.0;

            let (mut left, mut right) = (0i32, 0i32);
            for channel in &mut self.channels {
                if !channel.playing {
                    continue;
                }
                let sample = match self.module.samples.get(channel.sample) {
                    Some(s) if !s.data.is_empty() => s,
                    _ => {
                        channel.playing = false;
                        continue;
                    }
                };
                let at = channel.position as usize;
                if at >= sample.data.len() {
                    if sample.loops() {
                        channel.position = sample.repeat_from as f32;
                    } else {
                        channel.playing = false;
                        continue;
                    }
                }
                let value =
                    sample.data[(channel.position as usize).min(sample.data.len() - 1)] as i32;
                let scaled = value * channel.volume as i32;
                if channel.left {
                    left += scaled;
                } else {
                    right += scaled;
                }
                channel.position += channel.step;
                if sample.loops() {
                    let end = (sample.repeat_from + sample.repeat_len) as f32;
                    if channel.position >= end {
                        channel.position -= sample.repeat_len as f32;
                    }
                }
            }
            // Each channel contributes at most 127 * 64; scale so a full mix
            // of three channels a side does not clip.
            let scale = |v: i32| ((v / 12) as f32 * self.volume).clamp(-32768.0, 32767.0) as i16;
            out[frame * 2] = scale(left);
            out[frame * 2 + 1] = scale(right);
        }
        frames
    }

    fn advance_tick(&mut self) {
        if self.tick == 0 {
            self.play_row();
        } else {
            self.tick_effects();
        }
        self.tick += 1;
        if self.tick >= self.speed {
            self.tick = 0;
            self.next_row();
        }
    }

    fn next_row(&mut self) {
        self.row += 1;
        if self.row >= ROWS {
            self.row = 0;
            self.order_index += 1;
            if self.order_index >= self.module.order.len() {
                self.order_index = 0;
                self.finished = true;
            }
        }
    }

    fn current_row(&self) -> Option<&Vec<Note>> {
        let pattern = *self.module.order.get(self.order_index)? as usize;
        self.module.patterns.get(pattern)?.get(self.row)
    }

    fn play_row(&mut self) {
        let Some(row) = self.current_row().cloned() else {
            self.finished = true;
            return;
        };
        let mut jump: Option<usize> = None;
        let mut break_to: Option<usize> = None;

        for (i, note) in row.iter().enumerate() {
            let Some(channel) = self.channels.get_mut(i) else { continue };
            if note.sample > 0 {
                let index = note.sample as usize - 1;
                channel.sample = index;
                if let Some(sample) = self.module.samples.get(index) {
                    channel.volume = sample.volume;
                    channel.finetune = sample.finetune;
                }
            }
            // Effect 3 is tone portamento: it bends to the note rather than
            // restarting the sample on it.
            let porta = note.effect == 0x3 || note.effect == 0x5;
            if note.period > 0 {
                if porta {
                    channel.target_period = note.period as f32;
                } else {
                    channel.period = note.period as f32;
                    channel.target_period = note.period as f32;
                    channel.position = 0.0;
                    channel.playing = true;
                    channel.vibrato_phase = 0;
                }
            }

            let arg = note.argument;
            match note.effect {
                0x0 => {}
                0x3 => {
                    if arg > 0 {
                        channel.porta_speed = arg;
                    }
                }
                0x4 => {
                    if arg >> 4 > 0 {
                        channel.vibrato_speed = arg >> 4;
                    }
                    if arg & 0xf > 0 {
                        channel.vibrato_depth = arg & 0xf;
                    }
                }
                0x9 => {
                    // Sample offset, in units of 256 samples.
                    channel.position = (arg as f32) * 256.0;
                }
                0xb => jump = Some(arg as usize),
                0xc => channel.volume = arg.min(64),
                0xd => break_to = Some(((arg >> 4) * 10 + (arg & 0xf)) as usize),
                0xf => {
                    if arg < 32 {
                        self.speed = (arg as u32).max(1);
                    } else {
                        self.tempo = arg as u32;
                    }
                }
                other => {
                    if !self.unimplemented_effects.contains(&other) {
                        self.unimplemented_effects.push(other);
                    }
                }
            }
            Self::retune(channel, self.rate);
        }

        if let Some(to) = jump {
            self.order_index = to;
            self.row = ROWS - 1;
        } else if let Some(to) = break_to {
            self.row = to.min(ROWS - 1).wrapping_sub(1);
            self.order_index += 1;
            if self.order_index >= self.module.order.len() {
                self.order_index = 0;
                self.finished = true;
            }
        }
    }

    fn tick_effects(&mut self) {
        let Some(row) = self.current_row().cloned() else { return };
        for (i, note) in row.iter().enumerate() {
            let Some(channel) = self.channels.get_mut(i) else { continue };
            let arg = note.argument;
            match note.effect {
                0x1 => channel.period = (channel.period - arg as f32).max(113.0),
                0x2 => channel.period = (channel.period + arg as f32).min(856.0),
                0x3 | 0x5 => {
                    let speed = channel.porta_speed as f32;
                    if channel.period < channel.target_period {
                        channel.period = (channel.period + speed).min(channel.target_period);
                    } else if channel.period > channel.target_period {
                        channel.period = (channel.period - speed).max(channel.target_period);
                    }
                }
                0x4 | 0x6 => {
                    let depth = channel.vibrato_depth as i32;
                    let phase = (channel.vibrato_phase & 31) as usize;
                    let offset = SINE[phase] as i32 * depth / 128;
                    let signed = if channel.vibrato_phase < 32 { offset } else { -offset };
                    channel.period = (channel.period + signed as f32).clamp(113.0, 856.0);
                    channel.vibrato_phase = channel.vibrato_phase.wrapping_add(channel.vibrato_speed) & 63;
                }
                0xa => {
                    let up = arg >> 4;
                    let down = arg & 0xf;
                    if up > 0 {
                        channel.volume = (channel.volume + up).min(64);
                    } else {
                        channel.volume = channel.volume.saturating_sub(down);
                    }
                }
                _ => {}
            }
            Self::retune(channel, self.rate);
        }
    }

    fn retune(channel: &mut Channel, rate: u32) {
        if channel.period <= 0.0 {
            channel.step = 0.0;
            return;
        }
        // Finetune shifts the period by a sixteenth of a semitone per step.
        let tuned = channel.period * 2f32.powf(-(channel.finetune as f32) / (12.0 * 8.0));
        channel.step = (PAL_CLOCK / (tuned * 2.0)) / rate as f32;
    }
}
