//! The cutscenes, against the 32 files on the disc.

use hb_formats::smk::{Movie, Player, Trees};

fn story() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original")
        })
        .join("system/Story");
    if !dir.is_dir() {
        eprintln!("skipping: {} is not there", dir.display());
        return None;
    }
    Some(dir)
}

fn movies() -> Vec<(String, Vec<u8>)> {
    let Some(dir) = story() else { return Vec::new() };
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("smk")) {
            out.push((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&path).unwrap(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The header's arithmetic accounts for every byte of every file.
#[test]
fn the_frames_fill_the_file() {
    for (name, data) in movies() {
        let movie = Movie::parse(&data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let total: usize = movie.sizes.iter().sum();
        assert_eq!(movie.first + total, data.len(), "{name} does not add up");
        assert_eq!((movie.width, movie.height), (320, 240), "{name}");
        assert_eq!(movie.flags, 0, "{name} has flags this port does not read");
        assert!((movie.fps() - 15.0).abs() < 0.01, "{name} runs at {}", movie.fps());
    }
}

/// The header's size for a tree is `(leaves + 1) * 8`, and the four of them
/// fit inside the packed blob.
#[test]
fn the_trees_are_the_size_the_header_says() {
    for (name, data) in movies() {
        let movie = Movie::parse(&data).unwrap();
        let trees = Trees::parse(&movie.trees);
        let counts =
            [trees.map.leaves(), trees.colour.leaves(), trees.full.leaves(), trees.kind.leaves()];
        for (i, (&leaves, &size)) in counts.iter().zip(movie.tree_sizes.iter()).enumerate() {
            assert_eq!((leaves + 1) * 8, size, "{name}: tree {i} has {leaves} leaves for {size}");
        }
        assert!(trees.bits.div_ceil(8) <= movie.trees.len(), "{name}: the trees ran over");
        assert!(!trees.map.is_empty() && !trees.kind.is_empty(), "{name} is missing a tree");
    }
}

/// Every frame of every movie decodes, covers all 4,800 of its blocks, and
/// stops inside its own bytes.
///
/// The last two are what catch a decoder that is out of step with the stream
/// rather than wrong about a pixel. Carrying the trees' caches from one frame
/// to the next - which is what this decoder did at first - passes on 600
/// frames of `MORBBRF.SMK` and fails here on the fourth frame of
/// `MORBIN.SMK`, which is the sort of thing only an invariant finds.
#[test]
fn every_frame_decodes() {
    for (name, data) in movies() {
        let mut player = Player::new(&data).unwrap();
        let frames = player.movie.frames;
        let blocks = player.movie.width / 4 * (player.movie.height / 4);
        for i in 0..frames {
            assert!(player.step(&data), "{name}: frame {i} of {frames} stopped short");
            let (bytes, bits, covered) = player.spent;
            assert_eq!(covered, blocks, "{name}: frame {i} covered {covered} of {blocks} blocks");
            assert!(
                bits.div_ceil(8) <= bytes,
                "{name}: frame {i} read {} bytes of a {bytes}-byte chunk",
                bits.div_ceil(8)
            );
        }
        assert!(!player.step(&data), "{name}: a frame past the end");
    }
}

/// Every audio chunk unpacks to exactly the size it declares.
#[test]
fn the_sound_is_the_length_it_says() {
    for (name, data) in movies() {
        let mut player = Player::new(&data).unwrap();
        let frames = player.movie.frames;
        let (mut chunks, mut samples) = (0usize, 0usize);
        for _ in 0..frames {
            player.step(&data);
            if !player.sound.is_empty() {
                chunks += 1;
                samples += player.sound.len();
            }
        }
        let rate = (player.movie.audio_rate[0] & 0xff_ffff) as f32;
        let seconds = samples as f32 / rate;
        let runtime = frames as f32 / player.movie.fps();
        assert!(chunks > 0, "{name} has no sound");
        assert!(
            (seconds - runtime).abs() < 0.5,
            "{name}: {seconds:.2}s of sound against {runtime:.2}s of picture"
        );
    }
}
