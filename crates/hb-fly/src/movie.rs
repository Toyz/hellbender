//! Playing a cutscene in the window.
//!
//! `hb-formats`'s `smk` does the decoding; this is the clock and the handoff
//! of a whole soundtrack to the mixer. A movie holds
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

    /// The movie's own size, which is the buffer to draw it into.
    pub fn size(&self) -> (usize, usize) {
        (self.player.movie.width, self.player.movie.height)
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

    /// The picture.
    ///
    /// A movie is drawn at its own size and the window stretches it, so no
    /// row is ever dropped. It matters: 240 rows into the game's 200 loses one
    /// in six, and one row is a whole stroke of the small text a briefing
    /// movie is full of - "DOWNLOAD COMPLETE." came out as "UUWNLUHU
    /// LUMPLETE.". Where the sizes do differ this averages over the source
    /// pixels each destination one covers rather than sampling at a point.
    pub fn draw(&self, buffer: &mut [u32], width: usize, height: usize) {
        let (mw, mh) = (self.player.movie.width, self.player.movie.height);
        if mw == 0 || mh == 0 || width == 0 || height == 0 {
            buffer.fill(0xff00_0000);
            return;
        }
        for y in 0..height {
            // The rows of the movie this row of the frame stands for.
            let (top, bottom) = (y * mh / height, ((y + 1) * mh).div_ceil(height).min(mh));
            let bottom = bottom.max(top + 1);
            for x in 0..width {
                let (left, right) = (x * mw / width, ((x + 1) * mw).div_ceil(width).min(mw));
                let right = right.max(left + 1);
                let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
                for sy in top..bottom {
                    for sx in left..right {
                        let rgb = self.player.palette[self.player.picture[sy * mw + sx] as usize];
                        r += rgb[0] as u32;
                        g += rgb[1] as u32;
                        b += rgb[2] as u32;
                        n += 1;
                    }
                }
                buffer[y * width + x] =
                    0xff00_0000 | (r / n) << 16 | (g / n) << 8 | (b / n);
            }
        }
    }
}
