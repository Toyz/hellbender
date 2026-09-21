//! What the mixer does when several things sound at once.
//!
//! The port used to clamp each voice into the buffer as it was added, so
//! three explosions at once were three squared-off waves rather than one loud
//! one. That is what a busy moment rasping sounds like.

use hb_audio::voices::{Voices, HEADROOM};
use hb_audio::wav::Wav;
use std::sync::Arc;

/// A loud tone at the rate the game's effects are recorded at.
fn tone(level: i16) -> Arc<Wav> {
    let rate = 11_025;
    let samples: Vec<i16> =
        (0..rate).map(|i| if (i / 16) % 2 == 0 { level } else { -level }).collect();
    Arc::new(Wav { rate: rate as u32, channels: 1, samples })
}

fn render(voices: &mut Voices, frames: usize) -> Vec<i16> {
    let mut out = vec![0i16; frames * 2];
    voices.mix_into(&mut out);
    out
}

#[test]
fn one_voice_comes_out_at_its_own_level() {
    let mut voices = Voices::new(44_100);
    voices.play(tone(20_000), 1.0);
    let out = render(&mut voices, 2_000);
    let peak = out.iter().map(|&s| (s as i32).abs()).max().unwrap();
    let want = (20_000.0 * HEADROOM) as i32;
    assert!((peak - want).abs() < 600, "peak {peak}, wanted about {want}");
}

/// Many at once stay inside the range and still get louder as more pile on -
/// where a clamp would have pinned four and eight to the same ceiling and
/// flattened both.
#[test]
fn more_voices_lean_on_the_ceiling_rather_than_hitting_it() {
    let peak_of = |count: usize| {
        let mut voices = Voices::new(44_100);
        for _ in 0..count {
            voices.play(tone(20_000), 1.0);
        }
        render(&mut voices, 2_000).iter().map(|&s| (s as i32).abs()).max().unwrap()
    };
    let (four, eight, twelve) = (peak_of(4), peak_of(8), peak_of(12));
    for peak in [four, eight, twelve] {
        assert!(peak < 32_767, "peak {peak} is the ceiling itself");
    }
    assert!(eight > four, "four {four}, eight {eight}");
    assert!(twelve > eight, "eight {eight}, twelve {twelve}");
}

#[test]
fn the_master_volume_turns_everything_down() {
    let peak_at = |master: f32| {
        let mut voices = Voices::new(44_100);
        voices.set_master(master);
        voices.play(tone(20_000), 1.0);
        render(&mut voices, 2_000).iter().map(|&s| (s as i32).abs()).max().unwrap()
    };
    let full = peak_at(1.0);
    let half = peak_at(0.5);
    assert!((half as f32 / full as f32 - 0.5).abs() < 0.05, "full {full}, half {half}");
    assert_eq!(peak_at(0.0), 0);
}

/// A 11,025 Hz effect played out at 44,100 used to hold each sample for four
/// output samples, which is a staircase and sounds like grit. Sliding
/// between them leaves fewer runs of identical samples.
#[test]
fn an_upsampled_effect_is_not_a_staircase() {
    let mut voices = Voices::new(44_100);
    // A slope rather than a square, so every input sample differs.
    let samples: Vec<i16> = (0..2_000).map(|i| ((i % 200) * 100 - 10_000) as i16).collect();
    voices.play(Arc::new(Wav { rate: 11_025, channels: 1, samples }), 1.0);
    let out = render(&mut voices, 4_000);
    let left: Vec<i16> = out.iter().step_by(2).copied().collect();
    let repeats = left.windows(2).filter(|w| w[0] == w[1]).count();
    assert!(repeats * 2 < left.len(), "{repeats} of {} samples repeat", left.len());
}
