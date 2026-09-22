//! `.SMK` - Smacker, RAD Game Tools, and the game's 32 cutscenes.
//!
//! `SMACKW32.DLL` ships beside them, so the engine never decodes one itself and
//! there is nothing in `HELLBEND.EXE` to read. What is written here was worked
//! out from the 32 files under `system/Story/` and holds for all of them: the
//! header's arithmetic, the four Huffman trees, and the block stream.
//!
//! Every shipped file is `SMK2`, 320 by 240, one audio track, and no flags.

use crate::{Error, Result};

/// The only signature in the shipped files. `SMK4` exists in the wild and has
/// a third block mode; nothing here reads one.
pub const SIGNATURE: &[u8; 4] = b"SMK2";

/// Bytes before the frame table.
pub const HEADER: usize = 104;

/// The palette is six bits a channel, spread over eight: `(v << 2) | (v >> 4)`.
pub fn channel(six: u8) -> u8 {
    let v = six & 0x3f;
    (v << 2) | (v >> 4)
}

/// How many 4x4 blocks a run of the given code covers. The first sixty are
/// themselves; the last five are the powers of two (`0x40` on in the type
/// code's run field).
pub const RUNS: [usize; 64] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26,
    27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 128, 256, 512, 1024, 2048,
];

/// The four block kinds, the low two bits of a type code.
pub const MONO: u32 = 0;
pub const FULL: u32 = 1;
pub const SKIP: u32 = 2;
pub const FILL: u32 = 3;

/// A movie's header, and where everything in it starts.
#[derive(Debug, Clone)]
pub struct Movie {
    pub width: usize,
    pub height: usize,
    /// Frame count, not counting the ring frame the flags may ask for.
    pub frames: usize,
    /// As stored. Positive is milliseconds a frame, negative is hundredths of
    /// a millisecond, zero means ten a second. Use [`Movie::fps`].
    pub rate: i32,
    /// Bit 0 a ring frame, bit 1 y-interlaced, bit 2 y-doubled. Zero in every
    /// shipped file.
    pub flags: u32,
    /// The largest unpacked size of each of the seven audio tracks.
    pub audio_size: [u32; 7],
    /// Each track's rate word: bit 31 compressed, bit 30 sixteen-bit, bit 29
    /// stereo, and the sample rate in the low 24.
    pub audio_rate: [u32; 7],
    /// The packed Huffman trees, all four of them.
    pub trees: Vec<u8>,
    /// What each tree unpacks to, in entries: map, colour, full, type.
    pub tree_sizes: [usize; 4],
    /// Each frame's byte length, with the two flag bits masked off.
    pub sizes: Vec<usize>,
    /// Bit 0 says the frame carries a palette; bits 1 to 7 say which audio
    /// tracks it carries.
    pub kinds: Vec<u8>,
    /// Where the first frame starts.
    pub first: usize,
}

impl Movie {
    /// Frames a second, as the header's three cases give it.
    pub fn fps(&self) -> f32 {
        match self.rate {
            0 => 10.0,
            r if r > 0 => 1000.0 / r as f32,
            r => 100_000.0 / -r as f32,
        }
    }

    /// Whether a frame carries a palette.
    pub fn has_palette(&self, frame: usize) -> bool {
        self.kinds.get(frame).is_some_and(|k| k & 1 != 0)
    }

    /// The frames' bytes, in order, as offsets into the file.
    pub fn frame_at(&self, frame: usize) -> Option<(usize, usize)> {
        let mut at = self.first;
        for (i, &size) in self.sizes.iter().enumerate() {
            if i == frame {
                return Some((at, size));
            }
            at += size;
        }
        None
    }

    pub fn parse(data: &[u8]) -> Result<Movie> {
        let need = |at: usize, n: usize| -> Result<()> {
            if data.len() < at + n {
                Err(Error::Truncated { what: "SMK", at, need: n, have: data.len().saturating_sub(at) })
            } else {
                Ok(())
            }
        };
        need(0, HEADER)?;
        if &data[0..4] != SIGNATURE {
            return Err(Error::BadTag {
                what: "SMK signature",
                tag: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
                at: 0,
            });
        }
        let word = |at: usize| u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
        let frames = word(12) as usize;
        let trees_size = word(52) as usize;
        let table = 4 * frames + frames;
        need(HEADER, table + trees_size)?;
        let sizes: Vec<usize> =
            (0..frames).map(|i| (word(HEADER + i * 4) & !3) as usize).collect();
        let kinds = data[HEADER + 4 * frames..HEADER + table].to_vec();
        let trees = data[HEADER + table..HEADER + table + trees_size].to_vec();
        Ok(Movie {
            width: word(4) as usize,
            height: word(8) as usize,
            frames,
            rate: word(16) as i32,
            flags: word(20),
            audio_size: std::array::from_fn(|i| word(24 + i * 4)),
            audio_rate: std::array::from_fn(|i| word(72 + i * 4)),
            tree_sizes: std::array::from_fn(|i| word(56 + i * 4) as usize),
            trees,
            sizes,
            kinds,
            first: HEADER + table + trees_size,
        })
    }
}

/// The bit order Smacker packs in: least significant first, within each byte
/// in turn.
pub struct Bits<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Bits<'a> {
    pub fn new(data: &'a [u8]) -> Bits<'a> {
        Bits { data, at: 0 }
    }

    pub fn read(&mut self) -> bool {
        let byte = self.data.get(self.at >> 3).copied().unwrap_or(0);
        let bit = byte >> (self.at & 7) & 1 != 0;
        self.at += 1;
        bit
    }

    pub fn take(&mut self, count: u32) -> u32 {
        let mut out = 0;
        for i in 0..count {
            out |= (self.read() as u32) << i;
        }
        out
    }

    /// How many bits have been read.
    pub fn position(&self) -> usize {
        self.at
    }

    pub fn past_end(&self) -> bool {
        self.at > self.data.len() * 8
    }
}

/// One tree node: either a value or the two children a bit chooses between.
#[derive(Debug, Clone, Copy)]
enum Node {
    Leaf(u32),
    Branch(usize, usize),
}

/// A byte tree. Each is written as a bit per node - 0 a leaf and its eight
/// bits, 1 a branch and then its two children - and is followed by one bit
/// this reader skips.
#[derive(Debug, Clone, Default)]
pub struct ByteTree {
    nodes: Vec<Node>,
    root: Option<usize>,
}

impl ByteTree {
    /// Reads one, or nothing at all when the flag bit that precedes it is
    /// clear, which is how a file says a tree is not there.
    pub fn parse(bits: &mut Bits) -> ByteTree {
        if !bits.read() {
            return ByteTree::default();
        }
        let mut tree = ByteTree::default();
        tree.root = Some(tree.node(bits, 0));
        bits.read();
        tree
    }

    fn node(&mut self, bits: &mut Bits, depth: u32) -> usize {
        // The shipped trees are at most eight deep for a byte alphabet; the
        // limit is only here so a corrupt file cannot recurse forever.
        if depth > 32 || bits.past_end() {
            self.nodes.push(Node::Leaf(0));
            return self.nodes.len() - 1;
        }
        if !bits.read() {
            let value = bits.take(8);
            self.nodes.push(Node::Leaf(value));
            return self.nodes.len() - 1;
        }
        let left = self.node(bits, depth + 1);
        let right = self.node(bits, depth + 1);
        self.nodes.push(Node::Branch(left, right));
        self.nodes.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn decode(&self, bits: &mut Bits) -> u32 {
        let Some(mut at) = self.root else { return 0 };
        loop {
            match self.nodes[at] {
                Node::Leaf(v) => return v,
                Node::Branch(left, right) => at = if bits.read() { right } else { left },
            }
        }
    }
}

/// A sixteen-bit tree: two byte trees for the halves of a value, three escape
/// values, and then the tree itself over the pairs.
///
/// The escapes are the format's cache. A leaf whose value is one of the three
/// is not a value at all - it stands for the first, second or third most
/// recently decoded value, and every decode moves the list along.
#[derive(Debug, Clone, Default)]
pub struct Tree {
    nodes: Vec<Node>,
    root: Option<usize>,
    /// Which leaf stands for each of the three cache slots.
    escape: [Option<usize>; 3],
    /// The three most recent values, most recent first.
    recent: [u32; 3],
}

impl Tree {
    /// Reads one. Like a byte tree it is preceded by a bit that says whether
    /// it is there at all, and followed by one this reader skips.
    pub fn parse(bits: &mut Bits) -> Tree {
        if !bits.read() {
            return Tree::default();
        }
        let low = ByteTree::parse(bits);
        let high = ByteTree::parse(bits);
        let escapes = [bits.take(16), bits.take(16), bits.take(16)];
        let mut tree = Tree::default();
        tree.root = Some(tree.node(bits, &low, &high, &escapes, 0));
        bits.read();
        tree
    }

    fn node(
        &mut self,
        bits: &mut Bits,
        low: &ByteTree,
        high: &ByteTree,
        escapes: &[u32; 3],
        depth: u32,
    ) -> usize {
        if depth > 64 || bits.past_end() {
            self.nodes.push(Node::Leaf(0));
            return self.nodes.len() - 1;
        }
        if !bits.read() {
            let value = low.decode(bits) | high.decode(bits) << 8;
            self.nodes.push(Node::Leaf(value));
            let at = self.nodes.len() - 1;
            for (slot, &escape) in escapes.iter().enumerate() {
                if value == escape {
                    self.escape[slot] = Some(at);
                    self.nodes[at] = Node::Leaf(0);
                    break;
                }
            }
            return at;
        }
        let left = self.node(bits, low, high, escapes, depth + 1);
        let right = self.node(bits, low, high, escapes, depth + 1);
        self.nodes.push(Node::Branch(left, right));
        self.nodes.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// How many values the tree holds. The header's size for a tree is
    /// `(leaves + 1) * 8`, which holds for all four trees of all 32 movies.
    pub fn leaves(&self) -> usize {
        self.nodes.iter().filter(|n| matches!(n, Node::Leaf(_))).count()
    }

    /// One value, and the cache moved along.
    pub fn decode(&mut self, bits: &mut Bits) -> u32 {
        let Some(mut at) = self.root else { return 0 };
        loop {
            match self.nodes[at] {
                Node::Branch(left, right) => at = if bits.read() { right } else { left },
                Node::Leaf(v) => {
                    let value = match self.escape.iter().position(|&e| e == Some(at)) {
                        Some(slot) => self.recent[slot],
                        None => v,
                    };
                    if value != self.recent[0] {
                        self.recent[2] = self.recent[1];
                        self.recent[1] = self.recent[0];
                        self.recent[0] = value;
                    }
                    return value;
                }
            }
        }
    }
}

/// The four trees, in the order the header sizes them: the block map, the
/// two-colour pairs, the full-colour pairs, and the type codes.
#[derive(Debug, Clone, Default)]
pub struct Trees {
    pub map: Tree,
    pub colour: Tree,
    pub full: Tree,
    pub kind: Tree,
    /// How many bits of the packed blob the four took.
    pub bits: usize,
}

impl Trees {
    pub fn parse(packed: &[u8]) -> Trees {
        let mut bits = Bits::new(packed);
        let map = Tree::parse(&mut bits);
        let colour = Tree::parse(&mut bits);
        let full = Tree::parse(&mut bits);
        let kind = Tree::parse(&mut bits);
        Trees { map, colour, full, kind, bits: bits.position() }
    }
}

/// One audio chunk, decoded to 16-bit samples.
///
/// The chunk is an unpacked size, then a bit that says whether what follows is
/// packed at all, then two bits for stereo and sixteen-bit. Then one byte tree
/// a channel for eight-bit sound, or two - low half and high - for sixteen.
/// The samples are differences: the first is stored outright and every one
/// after it is added to the last, wrapping.
pub fn sound(chunk: &[u8]) -> Vec<i16> {
    if chunk.len() < 4 {
        return Vec::new();
    }
    let unpacked = u32::from_le_bytes(chunk[0..4].try_into().unwrap()) as usize;
    let mut bits = Bits::new(&chunk[4..]);
    if !bits.read() {
        return Vec::new();
    }
    let stereo = bits.read() as usize;
    let wide = bits.read() as usize;
    let trees: Vec<ByteTree> =
        (0..1 << (wide + stereo)).map(|_| ByteTree::parse(&mut bits)).collect();
    let channels = stereo + 1;
    let mut out = Vec::with_capacity(if wide == 1 { unpacked / 2 } else { unpacked });
    if wide == 1 {
        // The first sample of each channel is stored whole, high byte first.
        let mut last = [0i32; 2];
        for c in (0..channels).rev() {
            last[c] = (bits.take(16) as u16).swap_bytes() as i16 as i32;
        }
        for c in 0..channels {
            out.push(last[c] as i16);
        }
        while out.len() < unpacked / 2 && !bits.past_end() {
            let c = out.len() & stereo;
            let low = trees[c * 2].decode(&mut bits);
            let high = trees[c * 2 + 1].decode(&mut bits);
            let step = (low | high << 8) as u16 as i16 as i32;
            last[c] = (last[c] + step) as i16 as i32;
            out.push(last[c] as i16);
        }
    } else {
        let mut last = [0i32; 2];
        for c in (0..channels).rev() {
            last[c] = bits.take(8) as i32;
        }
        for c in 0..channels {
            out.push(eight(last[c]));
        }
        while out.len() < unpacked && !bits.past_end() {
            let c = out.len() & stereo;
            let step = trees[c].decode(&mut bits) as u8 as i8 as i32;
            last[c] = (last[c] + step) as u8 as i32;
            out.push(eight(last[c]));
        }
    }
    out
}

/// An unsigned eight-bit sample as a signed sixteen-bit one.
fn eight(value: i32) -> i16 {
    ((value as u8 as i32 - 128) * 256) as i16
}

/// A movie being played: the trees, the palette and the picture, all of which
/// carry from frame to frame.
pub struct Player {
    pub movie: Movie,
    pub trees: Trees,
    /// 256 entries of red, green, blue.
    pub palette: Vec<[u8; 3]>,
    /// One byte a pixel, `width * height`.
    pub picture: Vec<u8>,
    /// Track 0's samples for the frame just decoded.
    pub sound: Vec<i16>,
    next: usize,
}

impl Player {
    pub fn new(data: &[u8]) -> Result<Player> {
        let movie = Movie::parse(data)?;
        let trees = Trees::parse(&movie.trees);
        let picture = vec![0; movie.width * movie.height];
        Ok(Player { movie, trees, palette: vec![[0; 3]; 256], picture, sound: Vec::new(), next: 0 })
    }

    /// Which frame comes next.
    pub fn frame(&self) -> usize {
        self.next
    }

    /// Decode the next frame into [`Player::picture`], from the whole file.
    /// Returns false at the end.
    pub fn step(&mut self, data: &[u8]) -> bool {
        let Some((at, size)) = self.movie.frame_at(self.next) else { return false };
        if at + size > data.len() {
            return false;
        }
        let kind = self.movie.kinds.get(self.next).copied().unwrap_or(0);
        self.sound.clear();
        let mut chunk = &data[at..at + size];
        if kind & 1 != 0 {
            let length = chunk.first().map_or(0, |&n| n as usize * 4);
            let length = length.min(chunk.len());
            self.repalette(&chunk[1..length.max(1)]);
            chunk = &chunk[length..];
        }
        // The seven audio tracks, each behind its own four-byte length.
        for track in 0..7 {
            if kind >> (track + 1) & 1 == 0 {
                continue;
            }
            if chunk.len() < 4 {
                return false;
            }
            let length =
                u32::from_le_bytes(chunk[0..4].try_into().unwrap()) as usize;
            let length = length.clamp(4, chunk.len());
            if track == 0 {
                self.sound = sound(&chunk[4..length]);
            }
            chunk = &chunk[length..];
        }
        self.video(chunk);
        self.next += 1;
        true
    }

    /// The palette chunk: runs that keep what is there, runs that copy from
    /// somewhere else in the old palette, and three-byte entries.
    fn repalette(&mut self, chunk: &[u8]) {
        let old = self.palette.clone();
        let mut out = 0usize;
        let mut at = 0usize;
        while out < 256 && at < chunk.len() {
            let flag = chunk[at];
            if flag & 0x80 != 0 {
                let count = (flag & 0x7f) as usize + 1;
                at += 1;
                for _ in 0..count {
                    if out >= 256 {
                        break;
                    }
                    self.palette[out] = old[out];
                    out += 1;
                }
            } else if flag & 0x40 != 0 {
                let count = (flag & 0x3f) as usize + 1;
                let from = chunk.get(at + 1).copied().unwrap_or(0) as usize;
                at += 2;
                for i in 0..count {
                    if out >= 256 {
                        break;
                    }
                    self.palette[out] = old[(from + i) & 0xff];
                    out += 1;
                }
            } else {
                let r = channel(flag);
                let g = channel(chunk.get(at + 1).copied().unwrap_or(0));
                let b = channel(chunk.get(at + 2).copied().unwrap_or(0));
                at += 3;
                self.palette[out] = [r, g, b];
                out += 1;
            }
        }
    }

    /// The block stream. The picture is 4x4 blocks in rows, and a type code
    /// carries a kind, a run of blocks and, for a fill, the colour.
    fn video(&mut self, chunk: &[u8]) {
        let across = self.movie.width / 4;
        let down = self.movie.height / 4;
        let stride = self.movie.width;
        let mut bits = Bits::new(chunk);
        let mut block = 0usize;
        let blocks = across * down;
        while block < blocks && !bits.past_end() {
            let code = self.trees.kind.decode(&mut bits);
            let kind = code & 3;
            let mut run = RUNS[(code >> 2 & 0x3f) as usize];
            let mode = code >> 8;
            match kind {
                MONO => {
                    while run > 0 && block < blocks {
                        let colours = self.trees.colour.decode(&mut bits);
                        let mut map = self.trees.map.decode(&mut bits);
                        let (lo, hi) = ((colours & 0xff) as u8, (colours >> 8) as u8);
                        let (bx, by) = (block % across * 4, block / across * 4);
                        for y in 0..4 {
                            for x in 0..4 {
                                let at = (by + y) * stride + bx + x;
                                self.picture[at] = if map & 1 != 0 { hi } else { lo };
                                map >>= 1;
                            }
                        }
                        block += 1;
                        run -= 1;
                    }
                }
                FULL => {
                    while run > 0 && block < blocks {
                        let (bx, by) = (block % across * 4, block / across * 4);
                        for y in 0..4 {
                            // Two codes a row, the right pair before the left.
                            let right = self.trees.full.decode(&mut bits);
                            let left = self.trees.full.decode(&mut bits);
                            let at = (by + y) * stride + bx;
                            self.picture[at] = (left & 0xff) as u8;
                            self.picture[at + 1] = (left >> 8) as u8;
                            self.picture[at + 2] = (right & 0xff) as u8;
                            self.picture[at + 3] = (right >> 8) as u8;
                        }
                        block += 1;
                        run -= 1;
                    }
                }
                SKIP => {
                    block += run.min(blocks - block);
                }
                _ => {
                    let colour = mode as u8;
                    while run > 0 && block < blocks {
                        let (bx, by) = (block % across * 4, block / across * 4);
                        for y in 0..4 {
                            let at = (by + y) * stride + bx;
                            self.picture[at..at + 4].fill(colour);
                        }
                        block += 1;
                        run -= 1;
                    }
                }
            }
        }
    }
}
