//! Playing a cutscene in the window.
//!
//! `hb-formats`'s `smk` does the decoding; this is the clock, the letterbox
//! and the handoff of a whole soundtrack to the mixer. A movie holds
//! everything behind it, the way the briefing screen does, and any key skips
//! it - which is what the engine's own player does.

use std::path::Path;
use std::time::Instant;

use hb_audio::Wav;
use hb_formats::smk::Player;

/// Where the movies live on the disc.
pub const STORY: &str = "system/Story";

/// The four the engine plays before a game, in the order its three routines
/// name them (`0x45bbc9`, `0x45bbf9`, `0x45bc19`): Terminal Reality, the
/// title, Microsoft, and then the story.
pub const OPENING: [&str; 4] = ["tri.smk", "hell.smk", "mslogo.smk", "intro.smk"];

pub struct Show {
    data: Vec<u8>,
    player: Player,
    started: Instant,
    pub name: String,
}

impl Show {
    /// Opens one by name, case as the disc has it or not.
    pub fn open(game: &Path, name: &str) -> Option<Show> {
        let dir = game.join(STORY);
        let data = std::fs::read(dir.join(name)).ok().or_else(|| {
            // The `.LVL` names them in lower case and the disc is mixed.
            let wanted = name.to_lowercase();
            std::fs::read_dir(&dir).ok()?.flatten().find_map(|e| {
                (e.file_name().to_string_lossy().to_lowercase() == wanted)
                    .then(|| std::fs::read(e.path()).ok())
                    .flatten()
            })
        })?;
        let player = Player::new(&data).ok()?;
        Some(Show { data, player, started: Instant::now(), name: name.to_string() })
    }

    /// The whole soundtrack, decoded ahead of the picture so it can be handed
    /// to the mixer in one piece. This walks its own player, so the picture's
    /// is untouched.
    pub fn sound(&self) -> Option<Wav> {
        let mut player = Player::new(&self.data).ok()?;
        let rate = player.movie.audio_rate[0] & 0xff_ffff;
        if rate == 0 {
            return None;
        }
        let mut samples = Vec::new();
        while player.step(&self.data) {
            samples.extend_from_slice(&player.sound);
        }
        (!samples.is_empty()).then(|| Wav { rate, channels: 1, samples })
    }

    /// Start the clock. Call once, beside starting the sound.
    pub fn begin(&mut self) {
        self.started = Instant::now();
    }

    /// Decode up to where the clock is. False when the movie is over.
    pub fn step(&mut self) -> bool {
        let want = (self.started.elapsed().as_secs_f32() * self.player.movie.fps()) as usize + 1;
        while self.player.frame() < want.min(self.player.movie.frames) {
            if !self.player.step(&self.data) {
                return false;
            }
        }
        self.player.frame() < self.player.movie.frames
    }

    /// The picture across the whole frame.
    ///
    /// No letterbox, on purpose. A movie is 320x240 and the game's own screen
    /// is 320x200, and both were shown on the same 4:3 monitor - which is what
    /// `hb-fly` presents its window as - so filling the frame is what puts the
    /// picture back in the shape it was made in. Letterboxing it inside a
    /// window that is already 4:3 would squash it twice.
    pub fn draw(&self, buffer: &mut [u32], width: usize, height: usize) {
        let (mw, mh) = (self.player.movie.width, self.player.movie.height);
        if mw == 0 || mh == 0 || width == 0 || height == 0 {
            buffer.fill(0xff00_0000);
            return;
        }
        for y in 0..height {
            let sy = y * mh / height;
            for x in 0..width {
                let sx = x * mw / width;
                let [r, g, b] = self.player.palette[self.player.picture[sy * mw + sx] as usize];
                buffer[y * width + x] =
                    0xff00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | b as u32;
            }
        }
    }
}
