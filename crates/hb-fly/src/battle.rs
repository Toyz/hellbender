//! The fight: the player's shots, the turrets', the SAM sites' missiles, and
//! what they do to each other. The rules are `hb_sim`'s; this holds the state
//! for one level and reports what should be heard.

use hb_formats::text::Placement;
use hb_render::Level;
use hb_sim::combat::{self, Health, HitVolume, Pilot, Shot, Side, Stop};
use hb_sim::flyer::{Flyer, Target};
use hb_sim::explosion::Blasts;
use hb_sim::powerup::{self, Field, Stores};
use hb_sim::weapons::{Guns, Pose, Volley};
use hb_sim::turret::{Aim, Launch, Missile, Rng, Struck, Turret};

/// The behaviour classes that fly. 7 and 53 run the dogfight routine
/// `0x4967b0`; 56, 59 and 60 have routines of their own (`0x497050`,
/// `0x497be0`, `0x4999a0`) that start the same way and are not read yet, so
/// they borrow it.
pub const FLYING: [i64; 5] = [7, 53, 56, 59, 60];

/// The classes that aim and shoot: the turret and the four guns that differ
/// from it only in the aim.
pub const GUNS: [i64; 5] = [1, 3, 10, 14, 35];

/// Something the frame wants played.
pub enum Noise {
    /// A placement of this type was destroyed.
    Destroyed(usize),
    /// The player was hit: one of `exp1.wav`-`exp5.wav`, as `0x476ef0` picks.
    PlayerHit(usize),
    /// An enemy shot of this weapon kind came within 16 units, closing.
    NearMiss(i32),
    /// The player was killed.
    Died,
}

/// A shot in flight, with what its near-miss test remembers.
pub struct Flying {
    pub shot: Shot,
    /// Whether its sound has played (the shot's `0x54`).
    whizzed: bool,
    /// Its distance from the player last frame (the shot's `0x50`).
    last_distance: f32,
}

impl Flying {
    fn new(shot: Shot) -> Flying {
        Flying { shot, whizzed: false, last_distance: f32::MAX }
    }
}

pub struct Battle {
    pub health: Vec<Health>,
    volumes: Vec<HitVolume>,
    turrets: Vec<(usize, Turret)>,
    flyers: Vec<(usize, Flyer)>,
    pub shots: Vec<Flying>,
    pub missiles: Vec<Missile>,
    pub pilot: Pilot,
    /// The powerups lying about, and what the player has picked up.
    pub field: Field,
    pub stores: Stores,
    pub guns: Guns,
    /// The explosions burning (`hb_sim::explosion`).
    pub blasts: Blasts,
    /// How long the afterburner's tank has been empty.
    empty_for: f32,
    /// The powerups the player was inside last frame.
    touching: Vec<usize>,
    /// Seconds since the level started, for the animated models' poses.
    clock: f32,
    /// Where the player's shots and missiles hit the world this frame. The
    /// engine hands each of these to the quake code, which is what opens a
    /// door (`0x410b80`).
    pub ground_hits: Vec<[f32; 3]>,
    pub destroyed: usize,
    pub deaths: usize,
    rng: Rng,
}

impl Battle {
    pub fn new(level: &Level) -> Battle {
        let volumes = level
            .kinds
            .iter()
            .enumerate()
            .map(|(i, k)| HitVolume::for_type(k, level.meshes.get(i).and_then(Option::as_ref)))
            .collect();
        let turrets = level
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                level.kinds.get(p.kind).is_some_and(|k| GUNS.contains(&k.class()))
            })
            .map(|(i, p)| {
                let turret = match level.kinds[p.kind].class() {
                    3 => Turret::aiming(p, Aim::Direct),
                    10 => Turret::new(p),
                    _ => Turret::aiming(p, Aim::Led),
                };
                (i, turret)
            })
            .collect();
        let flyers = level
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| level.kinds.get(p.kind).is_some_and(|k| FLYING.contains(&k.class())))
            .map(|(i, p)| (i, Flyer::new(p)))
            .collect();
        Battle {
            health: level.placements.iter().map(Health::for_placement).collect(),
            volumes,
            turrets,
            flyers,
            shots: Vec::new(),
            missiles: Vec::new(),
            pilot: Pilot::default(),
            ground_hits: Vec::new(),
            clock: 0.0,
            field: Field::default(),
            stores: Stores::default(),
            guns: Guns::default(),
            blasts: Blasts::default(),
            empty_for: 0.0,
            touching: Vec::new(),
            destroyed: 0,
            deaths: 0,
            rng: Rng::new(0x1996),
        }
    }

    pub fn turret_count(&self) -> usize {
        self.turrets.len()
    }

    pub fn flyer_count(&self) -> usize {
        self.flyers.len()
    }

    /// One frame. `live` is what the renderer draws; a destroyed placement's
    /// kind is swapped for its wreck, or pushed out of range when it leaves
    /// nothing.
    pub fn step(
        &mut self,
        level: &Level,
        live: &mut [Placement],
        player: [f32; 3],
        velocity: [f32; 3],
        ship: Option<(&[[f32; 3]; 3], f32)>,
        dt: f32,
        solid: &impl Fn([f32; 3]) -> bool,
        ground: &impl Fn(f32, f32) -> f32,
    ) -> Vec<Noise> {
        let mut noises = Vec::new();
        self.clock += dt;
        self.pilot.since_hit += dt;
        self.blasts.step(dt);

        // Turrets think whether or not anyone is near.
        let target = self.pilot.alive().then_some(player);
        for (i, turret) in &mut self.turrets {
            if self.health[*i].destroyed {
                continue;
            }
            let Some(player) = target else { break };
            let p = &live[*i];
            if !combat::in_range(player, combat::position_of(p)) {
                continue;
            }
            let def = &level.kinds[p.kind];
            let mesh = level.meshes.get(p.kind).and_then(Option::as_ref);
            // A class 14 tower aims from the part of its model its gun sits
            // on - the first of the type's muzzle entries, read as a part
            // rather than a vertex (`0x406ac0`).
            turret.aim_from = [14, 35].contains(&def.class())
                .then(|| {
                    let part = *def.muzzles.first()? as usize;
                    let at = combat::position_of(p);
                    let off = level.part_origin(p.kind, part, self.clock)?;
                    Some([
                        at[0] + off[0] as f32 / 65536.0,
                        at[1] + off[1] as f32 / 65536.0,
                        at[2] + off[2] as f32 / 65536.0,
                    ])
                })
                .flatten();
            match turret.step(def, mesh, p, player, velocity, dt, &mut self.rng) {
                Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                Some(Launch::Missile(m)) => self.missiles.push(m),
                None => {}
            }
            // The turret's heading is what the renderer shows, and a class
            // 1 gun's pitch with it. A class 14 tower keeps its own angles
            // for the gun alone (`0x408ee0` eases a second set), so its body
            // does not turn.
            if ![14, 35].contains(&def.class()) {
                live[*i].heading = turret.heading as u16;
                if turret.aim != Aim::Flat {
                    live[*i].pitch = turret.pitch as i32;
                }
            } else if def.class() == 35 {
                // A class 35 tower turns on the spot while its gun aims
                // (`0x40c4a0` before it runs class 14's routine).
                let turn = hb_sim::behaviour::RATE * dt;
                live[*i].heading = (live[*i].heading as f32 + turn) as u16;
            }
        }

        // The flyers, within the same 80 units.
        if let (Some(player), Some((axes, speed))) = (target, ship) {
            let seen = Target { position: player, right: axes[0], up: axes[1], forward: axes[2], speed };
            for (i, flyer) in &mut self.flyers {
                if self.health[*i].destroyed || !combat::in_range(player, flyer.position) {
                    continue;
                }
                let def = &level.kinds[level.placements[*i].kind];
                let mesh = level.meshes.get(level.placements[*i].kind).and_then(Option::as_ref);
                match flyer.step(def, mesh, &seen, dt, ground, &mut self.rng) {
                    Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                    Some(Launch::Missile(m)) => self.missiles.push(m),
                    None => {}
                }
                let fixed = |v: f32| (v * 65536.0) as i32;
                let placed = &mut live[*i];
                placed.x = fixed(flyer.position[0]);
                placed.y = fixed(flyer.position[1]);
                placed.z = fixed(flyer.position[2]);
                placed.heading = flyer.heading as u16;
                placed.pitch = flyer.pitch as i32;
                placed.roll = flyer.roll as i32;
            }
        }

        let health = &self.health;
        let mut hits: Vec<(usize, f32, i32)> = Vec::new();
        let mut player_damage = 0.0f32;
        self.ground_hits.clear();
        let mut ground_hits: Vec<[f32; 3]> = Vec::new();
        for flying in &mut self.shots {
            let shot = &mut flying.shot;
            let stop = combat::step_shot(
                shot,
                dt,
                live,
                &self.volumes,
                |i| !health[i].destroyed,
                player,
                solid,
            );
            match stop {
                Some(Stop::Object(i)) => hits.push((i, shot.damage, shot.kind)),
                Some(Stop::Player) if self.pilot.alive() => player_damage += shot.damage,
                Some(Stop::Ground) if shot.side == Side::Player => {
                    ground_hits.push(shot.position)
                }
                _ => {}
            }
            if stop.is_some() {
                shot.age = f32::MAX;
                continue;
            }
            // `0x4768a0`: an enemy shot that comes within 16 units while
            // closing plays its sound once.
            if shot.side == Side::Enemy && !flying.whizzed {
                let d = [
                    combat::wrapped(shot.position[0] - player[0]),
                    shot.position[1] - player[1],
                    combat::wrapped(shot.position[2] - player[2]),
                ];
                let distance = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                if distance < 16.0 && distance < flying.last_distance {
                    flying.whizzed = true;
                    noises.push(Noise::NearMiss(shot.kind));
                }
                flying.last_distance = distance;
            }
        }
        self.shots.retain(|f| f.shot.alive());

        let health = &self.health;
        let volumes = &self.volumes;
        let mut children: Vec<(Missile, Vec<usize>)> = Vec::new();
        let mut scorched: Vec<([f32; 3], f32, i32)> = Vec::new();
        for m in &mut self.missiles {
            match m.side {
                Side::Enemy => match m.step(dt, target, solid) {
                    Some(true) => {
                        player_damage += m.damage;
                        m.age = f32::MAX;
                    }
                    Some(false) => m.age = f32::MAX,
                    None => {}
                },
                // A MIRV breaks up a second in, into ten of its own and a
                // blast of 32 units (`0x477b90`).
                Side::Player if m.splitting() => {
                    let targets: Vec<usize> = live
                        .iter()
                        .enumerate()
                        .filter(|&(i, _)| !health[i].destroyed)
                        .filter(|(_, p)| combat::in_range(m.position, combat::position_of(p)))
                        .map(|(i, _)| i)
                        .collect();
                    children.push((*m, targets));
                    m.age = f32::MAX;
                }
                // The super weapon scorches what it passes: everything
                // within 16 units takes its damage as it goes (`0x477a19`
                // does a quarter of it each of the four sub-steps).
                Side::Player if m.kind == hb_sim::turret::SUPER => {
                    scorched.push((m.position, m.damage, m.kind));
                    let aim = |i: usize| {
                        let p = live.get(i)?;
                        (!health.get(i)?.destroyed).then(|| combat::position_of(p))
                    };
                    if m.step_at(dt, aim, |_| None, solid).is_some() {
                        m.age = f32::MAX;
                    }
                }
                // The player's: home on the target while it stands, strike
                // whatever they fly into.
                Side::Player => {
                    let aim = |i: usize| {
                        let p = live.get(i)?;
                        (!health.get(i)?.destroyed).then(|| combat::position_of(p))
                    };
                    let hit = |at: [f32; 3]| combat::object_at(at, live, volumes, |i| !health[i].destroyed);
                    match m.step_at(dt, aim, hit, solid) {
                        Some(Struck::Object(i)) => {
                            hits.push((i, m.damage, m.kind));
                            m.age = f32::MAX;
                        }
                        Some(Struck::Ground) => {
                            ground_hits.push(m.position);
                            m.age = f32::MAX;
                        }
                        Some(_) => m.age = f32::MAX,
                        None => {}
                    }
                }
            }
        }
        self.ground_hits = ground_hits;
        for (at, damage, kind) in scorched {
            for i in combat::splash(at, hb_sim::turret::SUPER_REACH, live, |i| !self.health[i].destroyed) {
                hits.push((i, damage, kind));
            }
        }
        for (parent, targets) in children {
            self.missiles.extend(parent.split(&mut self.rng, &targets));
            for i in combat::splash(
                parent.position,
                hb_sim::turret::SPLIT_REACH,
                live,
                |i| !self.health[i].destroyed,
            ) {
                hits.push((i, hb_sim::turret::SPLIT_DAMAGE, parent.kind));
            }
            self.blasts.burst(parent.position, 4.0, &mut self.rng);
        }
        self.missiles.retain(Missile::alive);

        for (i, damage, kind) in hits {
            let original = level.placements[i].kind;
            let scaled = damage * combat::multiplier(&level.kinds[original], kind);
            if self.health[i].take(scaled) {
                self.destroyed += 1;
                // The port's own: an explosion the size of what was
                // destroyed. The engine spawns an actor of its own for this
                // (`0x40c7d0`), which is not read; the puffs are the
                // engine's (`0x47f3f0`).
                let radius = level.kinds[original].radius() as f32 / 65536.0;
                let at = combat::position_of(&live[i]);
                self.blasts.burst(at, radius.clamp(0.5, 8.0), &mut self.rng);
                // What it leaves behind (`0x40cc3c`), before it becomes its
                // wreck.
                if let Some(k) = powerup::drop_for(&level.kinds[original], &mut self.rng) {
                    let at = combat::position_of(&live[i]);
                    let size = level.powerup_size.get(k).copied().unwrap_or(0.0);
                    self.field.place(at, k, size, ground(at[0], at[2]));
                }
                live[i].kind = level.wreck_mesh[original].unwrap_or(usize::MAX);
                noises.push(Noise::Destroyed(original));
            }
        }

        if player_damage > 0.0 && self.pilot.alive() {
            noises.push(Noise::PlayerHit(self.rng.below(5)));
            if self.pilot.take(player_damage) {
                self.deaths += 1;
                noises.push(Noise::Died);
            }
        }
        noises
    }
}

impl Battle {
    /// The trigger and the afterburner for one frame: the volleys fired
    /// join the shots in flight.
    pub fn trigger(&mut self, fire: bool, burn: bool, dt: f32, pose: &Pose) -> (Vec<Volley>, Vec<hb_sim::mission::Voice>) {
        let (volleys, voices, _) = self.guns.step(fire, burn, dt, pose, &mut self.stores, &mut self.rng);
        for v in &volleys {
            for s in &v.shots {
                self.shots.push(Flying::new(*s));
            }
            self.missiles.extend(v.missiles.iter().copied());
        }
        (volleys, voices)
    }

    /// An explosion this wide, and nothing else.
    pub fn burst(&mut self, at: [f32; 3], size: f32) {
        self.blasts.burst(at, size, &mut self.rng);
    }

    /// The wreck hitting the ground: the engine's explosion two units
    /// across, and the loadout back to what it started with (`0x464998`).
    pub fn blow_up(&mut self, at: [f32; 3]) {
        self.blasts.burst(at, 2.0, &mut self.rng);
        self.stores = Stores::default();
        self.guns = Guns::default();
    }

    /// Energy and hull creeping back, and the afterburner's tank refilling.
    pub fn tick(&mut self, dt: f32) -> Vec<hb_sim::mission::Voice> {
        hb_sim::weapons::regenerate(&mut self.stores, &mut self.pilot.health, dt, &mut self.empty_for)
    }

    /// Offer the player every powerup they are inside. Returns what to say
    /// and the indices taken.
    pub fn pick_up(&mut self, player: [f32; 3]) -> (Vec<powerup::Event>, Vec<usize>) {
        let mut events = Vec::new();
        if !self.pilot.alive() {
            return (events, Vec::new());
        }
        let taken =
            self.field.step(player, &mut self.pilot, &mut self.stores, &mut self.touching, &mut events);
        (events, taken)
    }
}

/// The placements as the mission sees them: where each stands now and what
/// it has left.
pub struct Standing<'a> {
    pub health: &'a mut [Health],
    pub hull: f32,
    pub live: &'a [Placement],
    pub placed: &'a [Placement],
}

impl hb_sim::mission::World for Standing<'_> {
    fn actor(&self, index: usize) -> Option<hb_sim::mission::Actor> {
        let p = self.live.get(index)?;
        let h = self.health.get(index)?;
        Some(hb_sim::mission::Actor {
            position: combat::position_of(p),
            hit_points: if h.destroyed { 0.0 } else { h.hit_points },
            max: self.placed.get(index)?.hit_points as f32 / 65536.0,
        })
    }

    fn hull(&self) -> f32 {
        self.hull
    }

    fn restore(&mut self, index: usize) {
        let max = self.placed.get(index).map(|p| p.hit_points as f32 / 65536.0);
        if let (Some(h), Some(max)) = (self.health.get_mut(index), max) {
            if !h.destroyed {
                h.hit_points = max;
            }
        }
    }
}
