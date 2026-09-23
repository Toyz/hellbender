//! The mission: a level's `.NAV` list, worked through in order.
//!
//! One point is current at a time (`0x6283bc`). Each frame the engine finds
//! where it is, points the HUD arrow at it, and checks whether it is done
//! (`0x471df0`, called from the game loop at `0x4823d4`). A finished point
//! is marked and the next one chosen (`0x4719d0`, `0x471770`). The level ends
//! when the player flies into a jump zone, or when every required point
//! before the end marker is done; it fails when a timed point runs out or a
//! third friendly is lost.
//!
//! The loader's own additions - an end marker, sync points around tunnels,
//! the start - are made here too (`0x470bd0`).

use hb_formats::fixed::to_units;
use hb_formats::vector::{angles_of, flat_length, flat_within, offset};
use hb_formats::nav::{Data, Kind, Nav};
use hb_formats::text::Placement;

use crate::turret::Rng;

/// How many beacons the player can have out; the eleventh replaces the oldest.
pub const MAX_BEACONS: usize = 10;

/// A line from the engine's phrase table at `0x505c20`: a sound and the
/// subtitle shown with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voice {
    pub id: u16,
    pub sound: &'static str,
    pub text: &'static str,
}

pub const OBJECTIVE_COMPLETE: Voice =
    Voice { id: 0x3c, sound: "objcomp.wav", text: "Objective complete" };
pub const MISSION_ACCOMPLISHED: Voice = Voice {
    id: 0x57,
    sound: "accpZone.wav",
    text: "Mission accomplished...\nProceed to jump zone.",
};
pub const MISSION_COMPLETE: Voice = Voice {
    id: 0x58,
    sound: "compZone.wav",
    text: "Mission complete...\nProceed to jump zone.",
};
pub const BEACON_LAUNCHED: Voice =
    Voice { id: 0x5e, sound: "beaklaun.wav", text: "Beacon launched." };
/// Ten seconds left, then nine... one: phrases `0xa1`-`0xaa`.
pub const COUNTDOWN: [Voice; 10] = [
    Voice { id: 0xa1, sound: "10-sec.wav", text: "10 seconds..." },
    Voice { id: 0xa2, sound: "9.wav", text: "9..." },
    Voice { id: 0xa3, sound: "8.wav", text: "8..." },
    Voice { id: 0xa4, sound: "7.wav", text: "7..." },
    Voice { id: 0xa5, sound: "6.wav", text: "6..." },
    Voice { id: 0xa6, sound: "5.wav", text: "5..." },
    Voice { id: 0xa7, sound: "4.wav", text: "4..." },
    Voice { id: 0xa8, sound: "3.wav", text: "3..." },
    Voice { id: 0xa9, sound: "2.wav", text: "2..." },
    Voice { id: 0xaa, sound: "1.wav", text: "1..." },
];

/// Something the frame wants heard or shown.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A sound named by the mission file, or one of the engine's own.
    Sound(String),
    Voice(Voice),
    /// A line flashed on the HUD (`0x480ee0`).
    Message(&'static str),
    /// A guardian's own music in place of the level's (`0x451840`).
    Music(String),
    /// Back to the level's music when the guardian falls.
    MusicBack,
    /// The player is moved here (a [`Kind::Warp`] point).
    Warp([f32; 3]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The level was left (`0x5125c8`): the player flew into a jump zone,
    /// or a kill point's target fell.
    Jumped,
    /// Every required point is done (`0x5125cc`).
    Complete,
    /// A timed point ran out, three friendlies were destroyed, or the
    /// escorted shuttle was (`0x512720`).
    Failed,
}

/// What the mission needs to know about a placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Actor {
    pub position: [f32; 3],
    pub hit_points: f32,
    /// What it started with (the actor's `+0x24`).
    pub max: f32,
}

impl Actor {
    /// The engine's test: hit points above zero (and not in the flyers' dying
    /// phase `0x12d`, which ends with them at zero anyway).
    pub fn alive(&self) -> bool {
        self.hit_points > 0.0
    }
}

pub trait World {
    fn actor(&self, index: usize) -> Option<Actor>;
    /// Put a placement's hit points back to where they started.
    fn restore(&mut self, index: usize);
    /// The player's hull (`0x5b39ec`), 1.0 full.
    fn hull(&self) -> f32 {
        1.0
    }
}

/// Where the player starts: the first point, if it is a [`Kind::Start`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Start {
    pub position: [f32; 3],
    /// Pitch, roll and heading, in the 16-bit circle.
    pub angles: [u16; 3],
}

#[derive(Debug, Clone)]
struct Point {
    nav: Nav,
    /// Where it is, in units. Kinds that follow a placement look it up.
    position: [f32; 3],
    done: bool,
    /// Seconds left, 0 for none (`+0x64`).
    timer: f32,
    /// Cleared once played (`+0xa8`).
    proximity: Option<String>,
    /// When a beacon was dropped (`+0xbc`).
    dropped: f32,
}

#[derive(Debug, Clone)]
pub struct Mission {
    points: Vec<Point>,
    pub current: usize,
    pub outcome: Option<Outcome>,
    pub start: Option<Start>,
    /// The HUD's line for the current point (`0x625110`). Kinds without one
    /// leave the last line up, as the engine does.
    ///
    /// This is not what the objective box shows: that is [`Mission::code`],
    /// three letters. Where the long line is drawn, if anywhere, has not
    /// been found - `0x625110` is written twice in the image and read
    /// nowhere.
    pub label: String,
    /// The three letters the objective box shows (`0x44e5fe`), which the
    /// current point's kind picks. Kinds without one leave the last up.
    pub code: &'static str,
    /// Horizontal distance to the current point, units (`0x6283c0`).
    pub distance: f32,
    /// The HUD arrow (`0x59d118`): the direction from the point to the
    /// player, less the player's heading.
    pub arrow: u16,
    /// Within 60 units (`0x59d100` is 0x1f rather than 0x10).
    pub near: bool,
    friendlies_lost: u32,
    boss_music: bool,
    clock: f32,
}

fn units(v: [i32; 3]) -> [f32; 3] {
    v.map(to_units)
}

fn sync_point() -> Nav {
    // The loader clears the whole record, so an added sync point is required.
    Nav {
        kind: Kind::Sync,
        position: [0; 3],
        priority: 0,
        time: 0,
        completion_sound: None,
        proximity_sound: None,
        text: "Sync point: auto added".to_string(),
        data: Data::None,
    }
}

impl Mission {
    /// The loader's work after reading the file. `floor` is the height of
    /// the surface under a point (`0x41c300`); `rng` decides which optional
    /// points are kept.
    pub fn new(
        mut navs: Vec<Nav>,
        placements: &[Placement],
        floor: impl Fn([f32; 3]) -> f32,
        rng: &mut Rng,
    ) -> Mission {
        // Every point sits on or above the surface under it; a destroy point
        // then takes its first target's place, unclamped.
        let mut positions: Vec<[f32; 3]> = navs
            .iter()
            .map(|n| {
                let mut p = units(n.position);
                p[1] = p[1].max(floor(p));
                if let Data::Targets(t) = &n.data {
                    if let Some(at) = placements.get(t.first().copied().unwrap_or(0)) {
                        p = units([at.x, at.y, at.z]);
                    }
                }
                p
            })
            .collect();

        if !navs.iter().any(|n| n.kind == Kind::End) {
            navs.push(Nav {
                kind: Kind::End,
                position: [0; 3],
                priority: 1,
                time: 0,
                completion_sound: None,
                proximity_sound: None,
                text: String::new(),
                data: Data::None,
            });
            positions.push([0.0; 3]);
        }
        // A sync point after every tunnel exit...
        let mut i = 0;
        while i + 1 < navs.len() {
            if navs[i].kind == Kind::ExitTunnel && navs[i + 1].kind != Kind::Sync {
                navs.insert(i + 1, sync_point());
                positions.insert(i + 1, [0.0; 3]);
            }
            i += 1;
        }
        // ...and before every tunnel and jump zone.
        let mut i = 1;
        while i < navs.len() {
            let gated = matches!(navs[i].kind, Kind::EnterTunnel | Kind::JumpZone | Kind::ExitTunnel);
            if gated && navs[i - 1].kind != Kind::Sync {
                navs.insert(i, sync_point());
                positions.insert(i, [0.0; 3]);
            }
            i += 1;
        }

        let mut points: Vec<Point> = navs
            .into_iter()
            .zip(positions)
            .map(|(nav, position)| Point {
                position,
                done: false,
                timer: to_units(nav.time),
                proximity: nav.proximity_sound.clone(),
                dropped: 0.0,
                nav,
            })
            .collect();
        // Two in three optional points are dropped (`0x4712f6`). Only the end
        // markers are optional in the shipped files.
        for p in &mut points {
            if !p.nav.required() && p.nav.kind.code() <= 9 {
                let r = rng.next() as f32 * (1.0 / 32767.0) * 100.0;
                if r > 33.0 {
                    p.done = true;
                }
            }
        }

        let mut mission = Mission {
            points,
            current: 0,
            outcome: None,
            start: None,
            label: String::new(),
            code: "",
            distance: 0.0,
            arrow: 0,
            near: false,
            friendlies_lost: 0,
            boss_music: false,
            clock: 0.0,
        };
        // Start on the ground plus 16 units, facing as told, which is
        // `0x471333`: the engine writes the point's own position into the
        // ship, then overwrites the height with the ground under it
        // (`0x41c300`) plus `0x100000`, and takes pitch, roll and heading from
        // `+0xb0` of the record. The start point is done and the next is
        // current.
        if let Some(first) = mission.points.first_mut() {
            if let (Kind::Start, Data::Start { angles }) = (first.nav.kind, &first.nav.data) {
                let mut position = first.position;
                position[1] = floor(position) + 16.0;
                mission.start = Some(Start { position, angles: angles.map(|a| a as u16) });
                first.done = true;
                mission.current = 1.min(mission.points.len() - 1);
            }
        }
        mission
    }

    pub fn len(&self) -> usize {
        self.points.len()
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    pub fn nav(&self, i: usize) -> &Nav {
        &self.points[i].nav
    }

    pub fn done(&self, i: usize) -> bool {
        self.points[i].done
    }

    /// The current point's objective text.
    pub fn objective(&self) -> &str {
        self.points.get(self.current).map_or("", |p| p.nav.text.as_str())
    }

    /// Seconds left on the current point, if it is timed.
    pub fn time_left(&self) -> Option<f32> {
        self.points.get(self.current).map(|p| p.timer).filter(|&t| t > 0.0)
    }

    /// A friendly placement was destroyed (`0x40d412`). The third fails the
    /// mission.
    pub fn friendly_lost(&mut self) {
        self.friendlies_lost += 1;
        if self.friendlies_lost >= 3 && self.outcome.is_none() {
            self.outcome = Some(Outcome::Failed);
        }
    }

    /// Where the level's powerups lie, for the message-pod points, which
    /// point at theirs (`0x47241a` reads the powerup array).
    pub fn place_pods(&mut self, pods: &[[f32; 3]]) {
        for p in &mut self.points {
            if let (Kind::MessagePod, Data::Pod(i)) = (p.nav.kind, &p.nav.data) {
                if let Some(&at) = pods.get(*i) {
                    p.position = at;
                }
            }
        }
    }

    /// Powerup `index`, a message pod, was picked up: the first message-pod
    /// point naming it is done (`0x426deb` sets its `+0x68`), and moves on
    /// when it is current.
    pub fn pod_taken(&mut self, index: usize) {
        if let Some(p) = self
            .points
            .iter_mut()
            .find(|p| p.nav.kind == Kind::MessagePod && p.nav.data == Data::Pod(index))
        {
            p.done = true;
        }
    }

    /// The level is lost (`0x512720`): the escorted shuttle - the friendly
    /// class-50 actor `0x424fc0` finds - was destroyed (`0x40d7ca`), or a
    /// class-52 transport reached the sky (`0x422ec7`).
    pub fn fail(&mut self) {
        if self.outcome.is_none() {
            self.outcome = Some(Outcome::Failed);
        }
    }

    /// The beacon key (`0x471eb1`): a beacon where the player is, as a point
    /// of its own at the end of the list.
    pub fn drop_beacon(&mut self, player: [f32; 3]) -> Vec<Event> {
        let beacons = self.points.iter().filter(|p| p.nav.kind == Kind::Beacon).count();
        let mut slot = self.points.len();
        if beacons >= MAX_BEACONS {
            let mut oldest = f32::MAX;
            for (i, p) in self.points.iter().enumerate() {
                if p.nav.kind == Kind::Beacon && p.dropped < oldest {
                    oldest = p.dropped;
                    slot = i;
                }
            }
        }
        let point = Point {
            nav: Nav {
                kind: Kind::Beacon,
                position: player.map(hb_formats::fixed::from_units),
                priority: 1,
                time: 0,
                completion_sound: None,
                proximity_sound: None,
                text: "Beacon".to_string(),
                data: Data::None,
            },
            position: player,
            done: false,
            timer: 0.0,
            proximity: None,
            dropped: self.clock,
        };
        if slot == self.points.len() {
            self.points.push(point);
        } else {
            self.points[slot] = point;
        }
        if self.points[self.current].nav.kind == Kind::DropBeacon {
            vec![Event::Message("Beacon launched")]
        } else {
            vec![Event::Voice(BEACON_LAUNCHED)]
        }
    }

    /// One frame. `heading` is the player's, in the 16-bit circle.
    pub fn step(
        &mut self,
        world: &mut impl World,
        player: [f32; 3],
        heading: u16,
        dt: f32,
        floor: impl Fn([f32; 3]) -> f32,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        if self.outcome.is_some() || self.points.is_empty() {
            return events;
        }
        self.clock += dt;
        let cur = self.current;
        let kind = self.points[cur].nav.kind;
        let here = self.points[cur].position;

        // Where the point is, or that it is done. A point finished here
        // skips the arrow for the frame; the engine carries on with stale
        // registers.
        let target = match (kind, self.points[cur].nav.data.clone()) {
            (Kind::Destroy, Data::Targets(targets)) => {
                let alive = targets
                    .iter()
                    .find_map(|&i| world.actor(i).filter(Actor::alive).map(|a| a.position));
                if alive.is_none() {
                    self.complete(&mut events);
                }
                alive
            }
            (Kind::Guardian, Data::Guardian { actor, shields, .. }) => {
                self.guardian(world, actor, &shields, &mut events)
            }
            (Kind::Sync, _) => {
                self.advance(&mut events);
                None
            }
            (Kind::Escort | Kind::Kill, Data::Actor(i)) => world.actor(i).map(|a| a.position),
            (Kind::DropBeacon, _) => {
                let layer = |y: f32| y >= 0.0;
                let dropped = self.points.iter().any(|p| {
                    p.nav.kind == Kind::Beacon
                        && layer(p.position[1]) == layer(here[1])
                        && flat_within(here, p.position, 8.0)
                });
                if dropped {
                    if let Some(s) = self.points[cur].nav.completion_sound.clone() {
                        events.push(Event::Sound(s));
                    }
                    self.advance(&mut events);
                    None
                } else {
                    Some(here)
                }
            }
            _ => Some(here),
        };

        if let Some(t) = target {
            let d = offset(t, player);
            let distance = flat_length(d);
            let bearing = angles_of(d).0 as i32;
            self.arrow = (bearing - heading as i32) as u16;
            self.near = distance < 60.0;
            self.distance = distance;

            if kind == Kind::JumpZone
                && distance < 15.0
                && player[1] > here[1]
                && player[1] < here[1] + 20.0
            {
                self.outcome = Some(Outcome::Jumped);
            }
            if distance < 80.0 {
                if let Some(s) = self.points[cur].proximity.take() {
                    events.push(Event::Sound(s));
                }
            }

            let data = self.points[cur].nav.data.clone();
            let dead = |i: &usize| world.actor(*i).is_some_and(|a| !a.alive());
            match (kind, &data) {
                (Kind::EnterTunnel, _) if distance < 40.0 && player[1] < 0.0 => {
                    events.push(Event::Message("Enter Tunnel"));
                    self.complete(&mut events);
                }
                (Kind::Checkpoint, _) if distance < 40.0 => {
                    events.push(Event::Message("Checkpoint"));
                    self.complete(&mut events);
                }
                (Kind::ExitTunnel, _) if distance < 40.0 && player[1] > 0.0 => {
                    events.push(Event::Message("Exit Tunnel"));
                    self.complete(&mut events);
                }
                (Kind::Warp, Data::Warp { to, .. }) if distance < 30.0 => {
                    // Not advanced: the player is simply somewhere else.
                    if let Some(s) = self.points[cur].nav.completion_sound.clone() {
                        events.push(Event::Sound(s));
                    }
                    let mut at = units(*to);
                    at[1] = floor(at) + 16.0;
                    events.push(Event::Warp(at));
                }
                (Kind::Beacon, _) if distance < 40.0 && d[1].abs() < 20.0 => {
                    events.push(Event::Voice(OBJECTIVE_COMPLETE));
                    self.advance(&mut events);
                }
                (Kind::Escort, Data::Actor(i)) if dead(i) => {
                    self.complete(&mut events);
                }
                // A kill point is the way out, not a step on: its sound, and
                // the level ends for a player still in one piece
                // (`0x4727ef`). It is never advanced past.
                (Kind::Kill, Data::Actor(i)) if dead(i) => {
                    if let Some(s) = self.points[cur].nav.completion_sound.clone() {
                        events.push(Event::Sound(s));
                    }
                    if world.hull() > 0.0 && self.outcome.is_none() {
                        self.outcome = Some(Outcome::Jumped);
                    }
                }
                // No completion sound: the pod's own line has played.
                (Kind::MessagePod, _) if self.points[cur].done => self.advance(&mut events),
                _ => {}
            }
        }

        let label = match kind {
            Kind::Destroy | Kind::Kill => Some("Destroy Target"),
            Kind::EnterTunnel => Some("Enter Tunnel"),
            Kind::Checkpoint => Some("Fly to Checkpoint"),
            Kind::JumpZone => Some("Fly to Jump Zone"),
            Kind::ExitTunnel => Some("Exit Tunnel"),
            Kind::Escort => Some("Escort Ship"),
            Kind::MessagePod => Some("Pick up Message Pod"),
            _ => None,
        };
        if let Some(label) = label {
            self.label = label.to_string();
        }
        if let Some(code) = kind.abbreviation() {
            self.code = code;
        }

        // Every required point up to the end marker done: the level is won.
        let finished = self
            .points
            .iter()
            .take_while(|p| p.nav.kind != Kind::End)
            .all(|p| p.done || !p.nav.required());
        if finished && self.outcome.is_none() {
            self.outcome = Some(Outcome::Complete);
        }

        // The clock on the current point.
        let timer = &mut self.points[self.current].timer;
        if *timer > 0.0 {
            let was = *timer;
            *timer -= dt;
            let now = *timer;
            let crossed = |at: f32| was > at && now <= at;
            if crossed(20.0) {
                events.push(Event::Sound("20-sec.wav".to_string()));
            } else if let Some(n) = (1..=10).rev().find(|&n| crossed(n as f32)) {
                events.push(Event::Voice(COUNTDOWN[10 - n]));
            }
            if *timer <= 0.0 {
                *timer = 0.0;
                if self.outcome.is_none() {
                    self.outcome = Some(Outcome::Failed);
                }
            }
        }
        events
    }

    /// The guardian point (`0x4720d9`): while any of its shields stands the
    /// guardian is kept whole and the arrow points at a shield; when it falls
    /// the point is done.
    fn guardian(
        &mut self,
        world: &mut impl World,
        actor: usize,
        shields: &[usize],
        events: &mut Vec<Event>,
    ) -> Option<[f32; 3]> {
        let standing = shields.iter().rev().find(|&&s| world.actor(s).is_some_and(|a| a.alive()));
        let watched = match standing {
            Some(&s) => {
                world.restore(actor);
                s
            }
            None => actor,
        };
        if watched == actor && !world.actor(actor).is_some_and(|a| a.alive()) {
            events.push(Event::Message("Guardian Destroyed"));
            if self.boss_music {
                self.boss_music = false;
                events.push(Event::MusicBack);
            }
            self.complete(events);
            return None;
        }
        let a = world.actor(watched)?;
        let percent = if a.max > 0.0 { (a.hit_points * 100.0 / a.max) as i32 } else { 0 };
        self.label = format!("Guardian: {percent}%");
        Some(a.position)
    }

    /// The completion sound, then on to the next point.
    fn complete(&mut self, events: &mut Vec<Event>) {
        if let Some(s) = self.points[self.current].nav.completion_sound.clone() {
            events.push(Event::Sound(s));
        }
        self.advance(events);
    }

    /// `0x4719d0`: the current point is done; the first sync point nothing
    /// required still holds up is done too; then the next point is chosen.
    fn advance(&mut self, events: &mut Vec<Event>) {
        let cur = self.current;
        if self.points[cur].nav.kind != Kind::Beacon {
            self.points[cur].done = true;
        }
        self.points[cur].timer = 0.0;
        for p in &mut self.points {
            if p.done {
                continue;
            }
            if p.nav.kind == Kind::Sync {
                p.done = true;
                break;
            }
            if p.nav.required() {
                break;
            }
        }
        self.select_next();

        let next = &self.points[self.current];
        match (next.nav.kind, &next.nav.data) {
            (Kind::Guardian, Data::Guardian { music, .. }) => {
                self.boss_music = true;
                events.push(Event::Music(music.clone()));
                events.push(Event::Sound("warning.wav".to_string()));
                events.push(Event::Message("Mission Goal Ahead!"));
            }
            (Kind::JumpZone, _) => events.push(Event::Voice(if self.current % 2 == 1 {
                MISSION_ACCOMPLISHED
            } else {
                MISSION_COMPLETE
            })),
            _ => {}
        }
    }

    /// `0x471770`: the next point that is not done, where an unreached sync
    /// point sends the choice back to the first required point still open.
    /// A timed point cannot be left.
    fn select_next(&mut self) {
        let n = self.points.len();
        if self.points[self.current].timer > 0.0 {
            return;
        }
        let mut i = (self.current + 1) % n;
        let mut tries = 0;
        loop {
            if self.points[i].done {
                i = (i + 1) % n;
            }
            match self.points[i].nav.kind {
                Kind::Sync if !self.points[i].done => {
                    if let Some(end) = self.points.iter().position(|p| p.nav.kind == Kind::End) {
                        i = end + 1;
                    }
                    if i >= n {
                        i = 0;
                    }
                    let after = self.points[i].nav.kind;
                    if after != Kind::Beacon && after != Kind::Warp && i > 0 {
                        if let Some(open) =
                            (0..i).find(|&j| self.points[j].nav.required() && !self.points[j].done)
                        {
                            i = open;
                        }
                    }
                }
                Kind::End => i = (i + 1) % n,
                _ => {}
            }
            tries += 1;
            if !(self.points[i].done && n > tries) {
                break;
            }
        }
        self.current = if tries == n { 0 } else { i };
    }
}
