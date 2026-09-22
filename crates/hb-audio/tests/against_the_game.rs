//! The audio formats, checked against the archives.


use hb_audio::{Mixer, Module, Wav};

macro_rules! archive {
    ($name:expr) => {
        match hb_pod::game_pod($name) {
            Some(pod) => pod,
            None => return,
        }
    };
}

#[test]
fn every_effect_is_mono_eight_bit_and_almost_all_are_eleven_kilohertz() {
    let mut rates: Vec<(u32, usize)> = Vec::new();
    let (mut count, mut seconds) = (0usize, 0.0f32);
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries().iter().filter(|e| e.ext() == "wav") {
            let wav = Wav::parse(pod.bytes(e)).unwrap_or_else(|why| panic!("{}: {why}", e.name));
            assert_eq!(wav.channels, 1, "{}", e.name);
            assert!(!wav.samples.is_empty(), "{}", e.name);
            match rates.iter_mut().find(|(r, _)| *r == wav.rate) {
                Some((_, n)) => *n += 1,
                None => rates.push((wav.rate, 1)),
            }
            seconds += wav.seconds();
            count += 1;
        }
    }
    rates.sort();
    assert_eq!(count, 332, "sound effects across both archives");
    // mixSpeed=11025 in HELLBEND.INI is what nearly all of them are, but not
    // all: five are not, and one of those is an odd 8,287.
    assert_eq!(rates, [(8_287, 1), (11_025, 327), (22_050, 4)]);
    assert!(seconds > 100.0, "only {seconds} seconds of audio");
}

#[test]
fn every_module_is_six_channel_protracker() {
    let mut count = 0usize;
    for name in ["STARTUP.POD", "GAME.POD"] {
        let pod = archive!(name);
        for e in pod.entries().iter().filter(|e| e.ext() == "mod") {
            let module =
                Module::parse(pod.bytes(e)).unwrap_or_else(|why| panic!("{}: {why}", e.name));
            assert_eq!(module.channels, 6, "{}", e.name);
            assert!(!module.order.is_empty(), "{}", e.name);
            assert!(!module.patterns.is_empty(), "{}", e.name);
            // Every position names a pattern that was read.
            for &position in &module.order {
                assert!(
                    (position as usize) < module.patterns.len(),
                    "{}: position {position} of {}",
                    e.name,
                    module.patterns.len()
                );
            }
            // Every note names a sample that exists, or none at all.
            for pattern in &module.patterns {
                for row in pattern {
                    assert_eq!(row.len(), 6, "{}", e.name);
                    for note in row {
                        assert!(note.sample as usize <= 31, "{}", e.name);
                    }
                }
            }
            assert!(module.used_samples() >= 10, "{}", e.name);
            count += 1;
        }
    }
    assert_eq!(count, 15, "modules across both archives");
}

#[test]
fn a_module_renders_audible_audio() {
    let pod = archive!("GAME.POD");
    let module = Module::parse(pod.read("music", "float8.mod").unwrap()).unwrap();
    assert_eq!(module.channels, 6);
    let mut mixer = Mixer::new(module, 22_050);
    // Two seconds of stereo.
    let mut out = vec![0i16; 22_050 * 2 * 2];
    let frames = mixer.render(&mut out);
    assert_eq!(frames, 22_050 * 2);

    // It has to actually make a sound, and not be a constant.
    let peak = out.iter().map(|s| s.unsigned_abs()).max().unwrap();
    assert!(peak > 1_000, "peak was only {peak}");
    let distinct = out.iter().step_by(64).collect::<std::collections::HashSet<_>>();
    assert!(distinct.len() > 50, "only {} distinct samples", distinct.len());

    // And it should have moved through the pattern rather than stalling.
    let (order, row) = mixer.position();
    assert!(order > 0 || row > 0, "the mixer did not advance");
}

#[test]
fn the_mixer_says_which_effects_it_ignores() {
    let pod = archive!("GAME.POD");
    let mut ignored: Vec<u8> = Vec::new();
    for e in pod.entries().iter().filter(|e| e.ext() == "mod") {
        let module = Module::parse(pod.bytes(e)).unwrap();
        let mut mixer = Mixer::new(module, 11_025);
        let mut out = vec![0i16; 11_025 * 2];
        for _ in 0..8 {
            mixer.render(&mut out);
        }
        for effect in &mixer.unimplemented_effects {
            if !ignored.contains(effect) {
                ignored.push(*effect);
            }
        }
    }
    ignored.sort();
    // Nothing. The fifteen modules between them use only four effects -
    // none at all, Bxx position jump, Cxx set volume and Fxx speed or tempo -
    // and the mixer implements all of them. No portamento, no vibrato, no
    // volume slide anywhere in the soundtrack.
    assert!(ignored.is_empty(), "the mixer ignores {ignored:?}");
}

#[test]
fn the_soundtrack_uses_four_effects() {
    let pod = archive!("GAME.POD");
    let mut used: Vec<u8> = Vec::new();
    for e in pod.entries().iter().filter(|e| e.ext() == "mod") {
        let module = Module::parse(pod.bytes(e)).unwrap();
        for pattern in &module.patterns {
            for row in pattern {
                for note in row {
                    if !used.contains(&note.effect) {
                        used.push(note.effect);
                    }
                }
            }
        }
    }
    used.sort();
    assert_eq!(used, [0x0, 0xb, 0xc, 0xf]);
}
