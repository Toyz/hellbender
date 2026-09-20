//! The doors, lifts and moving ground of a level's `.QKE`.
//!
//! `processBoxQuake` (`0x410ec0`) runs each live box entry through a six-way
//! state machine: resting, a delay, moving out, a hold, moving back, a hold.
//! The cell's two altitudes step together so the box keeps its thickness, and
//! the step is the travel divided by the entry's duration - the motion line's
//! second and fourth numbers are seconds, not rates, so a door takes the same
//! time whatever its height (`0x410e40`).
//!
//! The two heights are where the box's edges end up: moving out raises the
//! top to the first, moving back lowers the bottom to the second. A box
//! parked with its top already at the first height starts open, and its first
//! move arrives at once - which is how a door that closes is written.
//!
//! A shot starts one. The impact calls `0x410b80` with the point it hit, and
//! any resting entry whose flags say "shot" and whose current span contains
//! the point begins its cycle (`0x4107a0`). While it moves it wakes the
//! entries that watch it, by id or by cell (`0x410da0`).
//!
//! The ground list is the same machine over a rectangle of cells rather than
//! one box (`0x4121d0`, `0x411b80`). Each entry names a layer - the ground,
//! a chamber's floor or its ceiling - and every cell in its rectangle steps
//! by the same amount, up to the first height and back to the second. None of
//! the shipped ones is shot open: 320 of the 739 start themselves and never
//! stop, and the rest wait for a box to move.
//!
//! See `docs/formats/scenery.md`.

use hb_formats::quake::Watches;

/// Altitudes here are the terrain's own words - a stored height shifted up
/// eight - so this many to a world unit.
pub const WORD: f32 = 256.0;

/// Which grid an entry moves. A box entry names one of the two box sets
/// (the third number of its where line); a ground entry names the ground or
/// one of a chamber's two surfaces (the fifth number of its where line, read
/// at `0x411ba3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    BoxA,
    BoxB,
    Ground,
    ChamberFloor,
    ChamberCeiling,
}

impl Layer {
    fn of_box(set: i64) -> Layer {
        if set == 1 { Layer::BoxA } else { Layer::BoxB }
    }

    fn of_ground(which: i64) -> Option<Layer> {
        match which {
            1 => Some(Layer::Ground),
            2 => Some(Layer::ChamberFloor),
            3 => Some(Layer::ChamberCeiling),
            _ => None,
        }
    }
}

/// What sets an entry going, from the flags line's last number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// 1: a shot landing inside it.
    Shot,
    /// A ground entry with its mode byte at 1 and its first bit set: it
    /// starts itself and so never stops (`0x411c20`).
    Always,
    /// 2: a call by id, which nothing in the shipped game makes.
    Called(i64),
    /// 3 and 4: another entry moving. Carries what it watches.
    Watching(Watches),
    /// Anything else: it never starts.
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Rest,
    /// Waiting out the watch line's fourth number before moving.
    About,
    Out,
    Hold,
    Back,
    Settle,
}

/// One box entry, with where its box is now.
#[derive(Debug, Clone)]
pub struct Door {
    /// The cell it moves, as (x, z): the file's second number is the column
    /// and its first the row (`0x410f90`).
    pub cell: (i32, i32),
    /// 1 for box set A, anything else for set B.
    pub set: i64,
    /// The altitude the top rises to going out, and the one the bottom
    /// comes back to.
    pub target: f32,
    pub rest: f32,
    /// Seconds out, the hold, seconds back, the rest.
    pub seconds: [f32; 4],
    /// Seconds after being started before it moves - 0 in every shipped
    /// entry.
    pub delay: f32,
    pub trigger: Trigger,
    /// The number after `!--Additional quake info--`, which a watcher names.
    pub id: i64,
    /// The sounds going out and coming back, where it has them.
    pub sounds: [Option<String>; 2],
    pub state: State,
    pub timer: f32,
    /// Where the box is now, in altitude words.
    pub bottom: f32,
    pub top: f32,
}

/// What one frame did to a cell, for the caller to write into the world.
/// A box layer moves both altitudes; the others have one height, and it
/// arrives in both fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Moved {
    pub layer: Layer,
    pub cell: (i32, i32),
    pub bottom: i16,
    pub top: i16,
}

/// A sound the caller should play, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Played {
    pub name: String,
    pub cell: (i32, i32),
}


/// One ground entry: a rectangle of cells on one layer, all stepping
/// together.
#[derive(Debug, Clone)]
pub struct Patch {
    /// The corner the rectangle starts at, as (x, z).
    pub from: (i32, i32),
    /// How many cells it covers in x and in z. The engine walks the span
    /// with `& 0x7f` at each step, so a rectangle may wrap the world
    /// (`0x4121e7`).
    pub span: (i32, i32),
    pub layer: Layer,
    /// The height the cells rise to, and the one they come back to.
    pub target: f32,
    pub rest: f32,
    pub seconds: [f32; 4],
    pub delay: f32,
    pub trigger: Trigger,
    pub id: i64,
    pub state: State,
    pub timer: f32,
    /// How far every cell has moved from where the level left it.
    pub offset: f32,
    /// Each cell's height at load, row by row over the rectangle.
    base: Vec<i16>,
}

impl Patch {
    /// Every cell of the rectangle, wrapped into the grid.
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let (x0, z0) = self.from;
        let (sx, sz) = self.span;
        (0..sz).flat_map(move |dz| (0..sx).map(move |dx| ((x0 + dx) & 127, (z0 + dz) & 127)))
    }

    /// The height the arrival test watches: the first cell of the rectangle,
    /// where the engine tests each cell in turn and lets the first one to
    /// arrive change the state.
    fn reference(&self) -> f32 {
        self.base.first().copied().unwrap_or(0) as f32 + self.offset
    }

    /// The travel is the two heights apart - a ground cell has one height,
    /// so there is no thickness to take off (`0x411c70`).
    fn speed(&self, out: bool) -> f32 {
        let seconds = if out { self.seconds[0] } else { self.seconds[2] };
        if seconds <= 0.0 {
            return f32::INFINITY;
        }
        (self.target - self.rest) / seconds
    }

    fn goes_anywhere(&self) -> bool {
        self.target != 0.0 || self.rest != 0.0
    }
}

#[derive(Debug, Clone, Default)]
pub struct Quakes {
    pub doors: Vec<Door>,
    pub patches: Vec<Patch>,
    pub sounds: Vec<Played>,
}

impl Door {
    /// The travel, in altitude words: the target less the resting bottom less
    /// the box's thickness (`0x410e7f`). It is constant, so the speed is too.
    fn travel(&self, thickness: f32) -> f32 {
        self.target - self.rest - thickness
    }

    fn speed(&self, out: bool, thickness: f32) -> f32 {
        let seconds = if out { self.seconds[0] } else { self.seconds[2] };
        if seconds <= 0.0 {
            return f32::INFINITY;
        }
        self.travel(thickness) / seconds
    }

    /// True while the entry has somewhere to go. An entry whose two heights
    /// are both zero arrives the moment it starts (`0x411010`), and 264 of
    /// the shipped entries are like that.
    fn goes_anywhere(&self) -> bool {
        self.target != 0.0 || self.rest != 0.0
    }
}

impl Quakes {
    /// Build from a parsed `.QKE`. `at` gives a cell's current bottom and
    /// top in altitude words - for the one-height layers both are that
    /// height. Entries whose kind is 0 are left out, as the engine's
    /// per-frame walk skips them (`0x4122e0`).
    pub fn new(
        quake: &hb_formats::quake::Quake,
        mut at: impl FnMut(Layer, (i32, i32)) -> (i16, i16),
    ) -> Quakes {
        let doors = quake
            .boxes
            .iter()
            .filter(|e| e.live() && e.where_.len() == 3)
            .map(|e| {
                let cell = (e.where_[1] as i32, e.where_[0] as i32);
                let set = e.where_[2];
                let (bottom, top) = at(Layer::of_box(set), cell);
                let sound = |n: usize| e.sounds.get(n).cloned().flatten();
                Door {
                    cell,
                    set,
                    target: e.heights[0] as f32,
                    rest: e.heights[1] as f32,
                    seconds: e.timing(),
                    delay: e.delay(),
                    trigger: match (e.flags.get(4), e.watches()) {
                        (Some(1), _) => Trigger::Shot,
                        (Some(2), _) => Trigger::Called(e.watch[0]),
                        (_, Some(what)) => Trigger::Watching(what),
                        _ => Trigger::Never,
                    },
                    id: e.extra,
                    sounds: [sound(0), sound(1)],
                    state: State::Rest,
                    timer: 0.0,
                    bottom: bottom as f32,
                    top: top as f32,
                }
            })
            .collect();

        // The ground list: kind 1 is the one the engine moves, and kind 3
        // keys off where the ship is instead - one entry in the shipped
        // levels, and not ported.
        let patches = quake
            .ground
            .iter()
            .filter(|e| e.kind == 1 && e.where_.len() == 5)
            .filter_map(|e| {
                let layer = Layer::of_ground(e.where_[4])?;
                let from = (e.where_[0] as i32, e.where_[1] as i32);
                let span = (
                    ((e.where_[2] - e.where_[0]) as i32 & 127) + 1,
                    ((e.where_[3] - e.where_[1]) as i32 & 127) + 1,
                );
                let mut patch = Patch {
                    from,
                    span,
                    layer,
                    target: e.heights[0] as f32,
                    rest: e.heights[1] as f32,
                    seconds: e.timing(),
                    delay: e.delay(),
                    // A ground entry's flags line has four numbers, so the
                    // watch kind is its last (`0x410dcd`), and the mode byte
                    // and first bit are its first two (`0x411c20`).
                    trigger: match (e.flags.first(), e.flags.get(1), e.flags.get(3)) {
                        (Some(1), Some(1), _) => Trigger::Always,
                        (_, _, Some(1)) => Trigger::Shot,
                        _ => match e.watches_ground() {
                            Some(what) => Trigger::Watching(what),
                            None => Trigger::Never,
                        },
                    },
                    id: e.extra,
                    state: State::Rest,
                    timer: 0.0,
                    offset: 0.0,
                    base: Vec::new(),
                };
                patch.base = patch.cells().map(|c| at(layer, c).0).collect();
                Some(patch)
            })
            .collect();

        Quakes { doors, patches, sounds: Vec::new() }
    }

    /// A shot landed at this point - cell (x, z) and altitude in words.
    /// Starts every resting door there whose flags say it is shot open, and
    /// returns how many started.
    pub fn shot(&mut self, cell: (i32, i32), altitude: f32) -> usize {
        let mut started = 0;
        for i in 0..self.doors.len() {
            let door = &self.doors[i];
            if door.state != State::Rest
                || door.trigger != Trigger::Shot
                || door.cell != cell
                || altitude > door.top
                || altitude < door.bottom
            {
                continue;
            }
            self.start(i);
            started += 1;
        }
        started
    }

    /// Put a door into the state before moving, with its timer cleared
    /// (`0x410b6b`).
    pub fn start(&mut self, door: usize) {
        let d = &mut self.doors[door];
        d.state = State::About;
        d.timer = 0.0;
    }

    /// One frame. Returns every cell that moved, and collects any sounds in
    /// [`Quakes::sounds`].
    pub fn step(&mut self, dt: f32) -> Vec<Moved> {
        let mut moved = Vec::new();
        let mut woken = Vec::new();
        for i in 0..self.doors.len() {
            let Some(change) = self.step_one(i, dt) else { continue };
            if change {
                moved.push(self.report(i));
                woken.push(i);
            }
        }
        // A moving box wakes whatever watches it (`0x410da0`). A moving
        // patch wakes nothing - the ground mover calls no one.
        for i in woken {
            self.wake_watchers(i);
        }
        for i in 0..self.patches.len() {
            if self.step_patch(i, dt) {
                let patch = &self.patches[i];
                let (layer, offset) = (patch.layer, patch.offset);
                for (cell, base) in patch.cells().zip(patch.base.iter()) {
                    let height = (*base as f32 + offset).round() as i16;
                    moved.push(Moved { layer, cell, bottom: height, top: height });
                }
            }
        }
        moved
    }

    /// One frame of one ground entry. True when its cells moved.
    fn step_patch(&mut self, i: usize, dt: f32) -> bool {
        let patch = &mut self.patches[i];
        match patch.state {
            // An entry that starts itself is never resting for long: the
            // engine puts it straight back into its cycle (`0x411c45`).
            State::Rest => {
                if patch.trigger == Trigger::Always {
                    patch.state = State::About;
                    patch.timer = 0.0;
                }
                false
            }
            State::About => {
                patch.timer += dt;
                if patch.timer < patch.delay {
                    return false;
                }
                patch.timer = 0.0;
                patch.state = State::Out;
                false
            }
            State::Hold | State::Settle => {
                patch.timer += dt;
                let wait =
                    if patch.state == State::Hold { patch.seconds[1] } else { patch.seconds[3] };
                if patch.timer < wait {
                    return false;
                }
                patch.timer = 0.0;
                patch.state = if patch.state == State::Hold { State::Back } else { State::Rest };
                false
            }
            State::Out | State::Back => {
                let out = patch.state == State::Out;
                let step = patch.speed(out) * dt * if out { 1.0 } else { -1.0 };
                let height = patch.reference();
                let arrived = !patch.goes_anywhere()
                    || !step.is_finite()
                    || if out { height + step >= patch.target } else { height + step <= patch.rest };
                if arrived {
                    let to = match (patch.goes_anywhere(), out) {
                        (false, _) => 0.0,
                        (true, true) => patch.target - height,
                        (true, false) => patch.rest - height,
                    };
                    patch.offset += to;
                    patch.timer = 0.0;
                    patch.state = if out { State::Hold } else { State::Settle };
                    return to != 0.0;
                }
                patch.offset += step;
                true
            }
        }
    }

    /// Returns `Some(true)` when the box moved this frame.
    fn step_one(&mut self, i: usize, dt: f32) -> Option<bool> {
        let thickness = self.doors[i].top - self.doors[i].bottom;
        let door = &mut self.doors[i];
        match door.state {
            State::Rest => None,
            State::About => {
                door.timer += dt;
                if door.timer < door.delay {
                    return None;
                }
                door.timer = 0.0;
                door.state = State::Out;
                if let Some(name) = door.sounds[0].clone() {
                    self.sounds.push(Played { name, cell: self.doors[i].cell });
                }
                Some(false)
            }
            State::Hold | State::Settle => {
                door.timer += dt;
                let wait = if door.state == State::Hold { door.seconds[1] } else { door.seconds[3] };
                if door.timer < wait {
                    return None;
                }
                door.timer = 0.0;
                if door.state == State::Hold {
                    door.state = State::Back;
                    if let Some(name) = door.sounds[1].clone() {
                        self.sounds.push(Played { name, cell: self.doors[i].cell });
                    }
                } else {
                    door.state = State::Rest;
                }
                Some(false)
            }
            State::Out | State::Back => {
                let out = door.state == State::Out;
                let step = door.speed(out, thickness) * dt * if out { 1.0 } else { -1.0 };
                let arrived = !door.goes_anywhere()
                    || !step.is_finite()
                    || if out { door.top + step >= door.target } else { door.bottom + step <= door.rest };
                if arrived {
                    // An entry with nowhere to go does not snap anywhere
                    // either: the engine's last step is zero (`0x411054`).
                    let to = match (door.goes_anywhere(), out) {
                        (false, _) => 0.0,
                        (true, true) => door.target - door.top,
                        (true, false) => door.rest - door.bottom,
                    };
                    door.top += to;
                    door.bottom += to;
                    door.timer = 0.0;
                    door.state = if out { State::Hold } else { State::Settle };
                    return Some(to != 0.0);
                }
                door.top += step;
                door.bottom += step;
                Some(true)
            }
        }
    }

    fn report(&self, i: usize) -> Moved {
        let d = &self.doors[i];
        Moved {
            layer: Layer::of_box(d.set),
            cell: d.cell,
            bottom: d.bottom.round() as i16,
            top: d.top.round() as i16,
        }
    }

    /// Start everything resting that watches the box which just moved -
    /// the other boxes (`0x410d00`) and the ground patches (`0x410da0`).
    fn wake_watchers(&mut self, moving: usize) {
        let (id, cell, set) = {
            let d = &self.doors[moving];
            (d.id, d.cell, d.set)
        };
        let watches = |trigger: Trigger| match trigger {
            Trigger::Watching(Watches::Link(link)) => link == id,
            Trigger::Watching(Watches::Cell { row, column, set: s }) => {
                (column as i32, row as i32) == cell && s == set
            }
            _ => false,
        };
        for i in 0..self.doors.len() {
            if self.doors[i].state == State::Rest && watches(self.doors[i].trigger) {
                self.start(i);
            }
        }
        for patch in &mut self.patches {
            if patch.state == State::Rest && watches(patch.trigger) {
                patch.state = State::About;
                patch.timer = 0.0;
            }
        }
    }

    /// The `0x410bc0` entry point: start everything waiting to be called by
    /// this id. Nothing in the shipped game calls it.
    pub fn call(&mut self, id: i64) -> usize {
        let mut started = 0;
        for i in 0..self.doors.len() {
            if self.doors[i].state == State::Rest && self.doors[i].trigger == Trigger::Called(id) {
                self.start(i);
                started += 1;
            }
        }
        started
    }

    /// Whether anything is moving, for a caller that wants to skip the work.
    pub fn busy(&self) -> bool {
        self.doors.iter().any(|d| d.state != State::Rest)
    }
}
