//! A held sound loops until it is let go.

use hb_audio::voices::Voices;
use hb_audio::wav::Wav;
use std::sync::Arc;

/// A tenth of a second of a square wave at the engine's usual rate.
fn tone() -> Arc<Wav> {
    let rate = 11_025;
    let samples: Vec<i16> = (0..rate / 10).map(|i| if (i / 32) % 2 == 0 { 8000 } else { -8000 }).collect();
    Arc::new(Wav { rate: rate as u32, channels: 1, samples })
}

fn loudness(buffer: &[i16]) -> i64 {
    buffer.iter().map(|&s| (s as i64).abs()).sum()
}

#[test]
fn a_held_sound_is_still_sounding_after_its_end() {
    let mut voices = Voices::new(22_050);
    let handle = voices.hold(tone(), 1.0).expect("it should start");

    // Half a second of output, which is five times the sample's length.
    let mut out = vec![0i16; 22_050];
    voices.mix_into(&mut out);
    assert!(loudness(&out) > 0, "nothing came out");
    // The second half is as loud as the first: it went round again.
    let (first, second) = out.split_at(out.len() / 2);
    let (a, b) = (loudness(first), loudness(second));
    assert!(b * 4 > a * 3, "it faded out: {a} then {b}");

    voices.stop(handle);
    let mut after = vec![0i16; 4_410];
    voices.mix_into(&mut after);
    assert_eq!(loudness(&after), 0, "it did not stop");
}

#[test]
fn a_one_shot_still_stops_on_its_own() {
    let mut voices = Voices::new(22_050);
    voices.play(tone(), 1.0);
    let mut out = vec![0i16; 22_050];
    voices.mix_into(&mut out);
    assert_eq!(voices.active(), 0, "a one-shot should have ended");
    let (first, second) = out.split_at(out.len() / 2);
    assert!(loudness(first) > 0 && loudness(second) == 0, "it should play once");
}
