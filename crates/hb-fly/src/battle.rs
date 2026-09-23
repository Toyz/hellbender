//! The fight: the player's shots, the turrets', the SAM sites' missiles, and
//! what they do to each other. The rules are `hb_sim`'s; this holds the state
//! for one level and reports what should be heard.

use hb_formats::fixed::to_units;
use hb_formats::text::Placement;
use hb_render::Level;
use hb_sim::combat::{self, Health, HitVolume, Pilot, Shot, Side, Stop};
use hb_sim::flyer::{Flyer, Hover, Target};
use hb_sim::steer::{Body, Surfaces};
use hb_sim::explosion::Blasts;
use hb_sim::powerup::{self, Field, Stores};
use hb_sim::weapons::{Guns, Pose, Volley};
use hb_sim::turret::{Aim, Launch, Missile, Rng, Struck, Turret};

/// The behaviour classes that fly. 7 and 53 run the dogfight routine
/// `0x4967b0`; 56, 59 and 60 have routines of their own (`0x497050`,
/// `0x497be0`, `0x4999a0`) that start the same way and are not read yet, so
/// they borrow it.
pub const FLYING: [i64; 6] = [7, 53, 55, 56, 59, 60];

/// The hover craft, which is its own routine (`0x4976d0`).
pub const HOVERING: i64 = 58;

/// What a shot leaves where it hits the world, in units: `0x476e14` passes
/// 60,000 in 16.16 to the explosion.
pub const SHOT_BURST: f32 = to_units(60_000);

/// And a missile, which is bigger: `0x478b07` passes 4.0.
pub const MISSILE_BURST: f32 = 4.0;

/// The classes that aim and shoot: the turret and the four guns that differ
/// from it only in the aim.
pub const GUNS: [i64; 5] = [1, 3, 10, 14, 35];

/// Something the frame wants played.
pub enum Noise {
    /// A placement of this type was destroyed, and where it was.
    Destroyed(usize, [f32; 3]),
    /// The player was hit: one of `exp1.wav`-`exp5.wav`, as `0x476ef0` picks.
    PlayerHit(usize),
    /// An enemy shot of this weapon kind came within 16 units, closing, and
    /// where the shot was when it did.
    NearMiss(i32, [f32; 3]),
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
    /// The course followers that shoot. Only the trigger of a `Turret` is
    /// used: the follower's own heading is where the gun points.
    drivers: Vec<(usize, Turret)>,
    flyers: Vec<(usize, Flyer)>,
    hovers: Vec<(usize, Hover)>,
    pub shots: Vec<Flying>,
    pub missiles: Vec<Missile>,
    /// The trails the missiles leave (`hb_sim::smoke`).
    pub smoke: hb_sim::smoke::Smoke,
    pub pilot: Pilot,
    /// The powerups lying about, and what the player has picked up.
    pub field: Field,
    pub stores: Stores,
    pub guns: Guns,
    /// The explosions burning (`hb_sim::explosion`).
    pub blasts: Blasts,
    /// The mines laid, a hundred slots as the engine has.
    pub mines: hb_sim::mine::Field,
    /// And the enemy's own pool, which the mine layers fill
    /// (`0x5c00e0`).
    pub laid: hb_sim::mine::laid::Field,
    /// How long the afterburner's tank has been empty.
    empty_for: f32,
    /// Whether the afterburner is lit, which the engine gives a kick and a
    /// held engine note (`0x47d6a3`).
    pub burning: bool,
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
        let mut rng = Rng::new(0x1996);
        let volumes = level
            .kinds
            .iter()
            .enumerate()
            .map(|(i, k)| HitVolume::for_type(k, level.meshes.get(i).and_then(Option::as_ref)))
            .collect();
        let mut turrets: Vec<(usize, Turret)> = level
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
        let mut drivers: Vec<(usize, Turret)> = level
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                level.kinds.get(p.kind).is_some_and(|k| hb_sim::course::SHOOTING.contains(&k.class()))
            })
            .map(|(i, p)| (i, Turret::new(p)))
            .collect();
        let mut flyers: Vec<(usize, Flyer)> = level
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| level.kinds.get(p.kind).is_some_and(|k| FLYING.contains(&k.class())))
            .map(|(i, p)| {
                let flyer = match level.kinds[p.kind].class() {
                    55 => Flyer::layer(p),
                    _ => Flyer::new(p),
                };
                (i, flyer)
            })
            .collect();
        let mut hovers: Vec<(usize, Hover)> = level
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| level.kinds.get(p.kind).is_some_and(|k| k.class() == HOVERING))
            .map(|(i, p)| (i, Hover::new(p)))
            .collect();
        for gun in turrets
            .iter_mut()
            .chain(drivers.iter_mut())
            .map(|(_, t)| t)
            .chain(flyers.iter_mut().map(|(_, f)| &mut f.gun))
            .chain(hovers.iter_mut().map(|(_, h)| &mut h.gun))
        {
            gun.waited = hb_sim::turret::first_wait(&mut rng);
        }
        Battle {
            health: level.placements.iter().map(Health::for_placement).collect(),
            hovers,
            volumes,
            turrets,
            drivers,
            flyers,
            shots: Vec::new(),
            missiles: Vec::new(),
            smoke: hb_sim::smoke::Smoke::default(),
            pilot: Pilot::default(),
            ground_hits: Vec::new(),
            mines: hb_sim::mine::Field::new(),
            laid: hb_sim::mine::laid::Field::new(),
            clock: 0.0,
            field: Field::default(),
            stores: Stores::default(),
            guns: Guns::default(),
            blasts: Blasts::default(),
            empty_for: 0.0,
            burning: false,
            touching: Vec::new(),
            destroyed: 0,
            deaths: 0,
            rng,
        }
    }

    pub fn turret_count(&self) -> usize {
        self.turrets.len()
    }

    pub fn flyer_count(&self) -> usize {
        self.flyers.len() + self.hovers.len()
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
        world: &impl Surfaces,
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
                        at[0] + to_units(off[0]),
                        at[1] + to_units(off[1]),
                        at[2] + to_units(off[2]),
                    ])
                })
                .flatten();
            match turret.step(def, mesh, p, player, velocity, dt, &mut self.rng) {
                Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                Some(Launch::Missile(m)) => self.missiles.push(m),
                // A turret has no mines to lay.
                Some(Launch::Mine { .. }) | None => {}
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

        // The course followers that shoot: the same trigger as a turret's
        // (`0x407770`), along whatever heading the course has them on.
        for (i, gun) in &mut self.drivers {
            let Some(player) = target else { break };
            let p = &live[*i];
            if self.health[*i].destroyed || !combat::in_range(player, combat::position_of(p)) {
                continue;
            }
            let def = &level.kinds[p.kind];
            let mesh = level.meshes.get(p.kind).and_then(Option::as_ref);
            gun.heading = p.heading as f32;
            gun.pitch = p.pitch as f32;
            let at = combat::position_of(p);
            let speed = to_units(def.shot_speed());
            match gun.trigger(def, mesh, at, player, dt, speed, &mut self.rng) {
                Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                Some(Launch::Missile(m)) => self.missiles.push(m),
                Some(Launch::Mine { .. }) | None => {}
            }
        }

        // The flyers, within the same 80 units.
        if let (Some(player), Some((axes, speed))) = (target, ship) {
            let seen = Target {
                position: player,
                right: axes[0],
                up: axes[1],
                forward: axes[2],
                speed,
                velocity,
                alive: self.pilot.alive(),
            };
            for (i, flyer) in &mut self.flyers {
                if self.health[*i].destroyed || !combat::in_range(player, flyer.body.position) {
                    continue;
                }
                let def = &level.kinds[level.placements[*i].kind];
                let mesh = level.meshes.get(level.placements[*i].kind).and_then(Option::as_ref);
                match flyer.step(def, mesh, &seen, dt, world, &mut self.rng) {
                    Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                    Some(Launch::Missile(m)) => self.missiles.push(m),
                    Some(Launch::Mine { at, radius, damage }) => {
                        self.laid.lay(at, radius, damage);
                    }
                    None => {}
                }
                place(&mut live[*i], &flyer.body);
            }
            // And the hover craft, which sit on their posts until he comes.
            for (i, hover) in &mut self.hovers {
                if self.health[*i].destroyed || !combat::in_range(player, hover.body.position) {
                    continue;
                }
                let def = &level.kinds[level.placements[*i].kind];
                let mesh = level.meshes.get(level.placements[*i].kind).and_then(Option::as_ref);
                match hover.step(def, mesh, &seen, dt, world, &mut self.rng) {
                    Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                    Some(Launch::Missile(m)) => self.missiles.push(m),
                    Some(Launch::Mine { at, radius, damage }) => {
                        self.laid.lay(at, radius, damage);
                    }
                    None => {}
                }
                place(&mut live[*i], &hover.body);
            }
        }

        let health = &self.health;
        let mut hits: Vec<(usize, f32, i32)> = Vec::new();
        let mut player_damage = 0.0f32;
        self.ground_hits.clear();
        let mut ground_hits: Vec<[f32; 3]> = Vec::new();
        // What a shot or a missile leaves where it meets the world: the
        // engine explodes at the point of impact before it tells the quake
        // code about it - 0.9155 units for a shot (`0x476e14`) and 4.0 for a
        // missile (`0x478b07`).
        let mut bursts: Vec<([f32; 3], f32)> = Vec::new();
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
                Some(Stop::Ground) => {
                    bursts.push((shot.position, SHOT_BURST));
                    if shot.side == Side::Player {
                        ground_hits.push(shot.position);
                    }
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
                let distance = hb_formats::vector::distance(player, shot.position);
                if distance < 16.0 && distance < flying.last_distance {
                    flying.whizzed = true;
                    noises.push(Noise::NearMiss(shot.kind, shot.position));
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
                    Some(false) => {
                        bursts.push((m.position, MISSILE_BURST));
                        m.age = f32::MAX;
                    }
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
                            bursts.push((m.position, MISSILE_BURST));
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
        // A shot's mark and a missile's are single puffs too: `0x476e22`
        // and `0x4784f2` call the spawner straight.
        for (at, size) in bursts {
            self.blasts.puff(at, size);
        }
        // The mines, which go off on the ship and splash everything near
        // them (`0x479670`).
        for blast in self.mines.step(dt, player) {
            self.blasts.puff(blast.at, hb_sim::mine::BURST);
            for i in combat::splash(blast.at, hb_sim::mine::BLAST, live, |i| !self.health[i].destroyed) {
                hits.push((i, blast.damage, hb_sim::mine::KIND));
            }
            // The ship is what set it off, so the ship is inside the blast.
            if self.pilot.alive() {
                player_damage += blast.damage;
            }
        }
        // Flying into something (`0x464f2f`): the ship's own position
        // against every live actor's volume. Nothing is pushed anywhere -
        // both just take damage while it lasts.
        if self.pilot.alive() {
            let health = &self.health;
            let volumes = &self.volumes;
            let rammed = combat::object_at(player, live, volumes, |i| {
                !health[i].destroyed
                    && combat::rammable(level.kinds[live[i].kind].class())
            });
            if let Some(i) = rammed {
                hits.push((i, combat::RAM_ACTOR * dt, 0));
                player_damage += combat::RAM_PLAYER;
            }
        }

        // And the enemy's, which only ever test the player (`0x495de0`) and
        // scale what they take off by how far in he was.
        if self.pilot.alive() {
            for blast in self.laid.step(player) {
                self.blasts.puff(blast.at, hb_sim::mine::laid::BURST);
                player_damage += blast.damage;
            }
        }
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
        // Their smoke (`0x478d90`), after they have moved, and the old
        // smoke aged (`0x479480`).
        self.smoke.step(dt);
        for m in &mut self.missiles {
            if let Some((from, to)) = m.trail.step(m.kind, m.position, dt) {
                self.smoke.lay(from, to);
            }
        }

        for (i, damage, kind) in hits {
            let original = level.placements[i].kind;
            let scaled = damage * combat::multiplier(&level.kinds[original], kind);
            if self.health[i].take(scaled) {
                self.destroyed += 1;
                // The port's own: an explosion the size of what was
                // destroyed. The engine spawns an actor of its own for this
                // (`0x40c7d0`), which is not read; the puffs are the
                // engine's (`0x47f3f0`).
                let radius = to_units(level.kinds[original].radius());
                let at = combat::position_of(&live[i]);
                // One puff of the type's own radius, which is what
                // `0x407c20` spawns - not a burst. A burst of eleven at a
                // building's radius filled the screen.
                self.blasts.puff(at, radius);
                // What it leaves behind (`0x40cc3c`), before it becomes its
                // wreck.
                if let Some(k) = powerup::drop_for(&level.kinds[original], &mut self.rng) {
                    let at = combat::position_of(&live[i]);
                    let size = level.powerup_size.get(k).copied().unwrap_or(0.0);
                    self.field.place(at, k, size, ground(at[0], at[2]));
                }
                live[i].kind = level.wreck_mesh[original].unwrap_or(usize::MAX);
                noises.push(Noise::Destroyed(original, at));
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
        let (volleys, voices, lit) = self.guns.step(fire, burn, dt, pose, &mut self.stores, &mut self.rng);
        self.burning = lit;
        for v in &volleys {
            for s in &v.shots {
                self.shots.push(Flying::new(*s));
            }
            self.missiles.extend(v.missiles.iter().copied());
            if v.mine {
                let damage = to_units(hb_sim::weapons::ROWS[hb_sim::weapons::MINE].damage);
                self.mines.lay(pose.position, pose.forward, pose.speed, damage);
            }
        }
        (volleys, voices)
    }

    /// One puff this wide, and nothing else.
    pub fn burst(&mut self, at: [f32; 3], size: f32) {
        self.blasts.puff(at, size);
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
            max: to_units(self.placed.get(index)?.hit_points),
        })
    }

    fn hull(&self) -> f32 {
        self.hull
    }

    fn restore(&mut self, index: usize) {
        let max = self.placed.get(index).map(|p| to_units(p.hit_points));
        if let (Some(h), Some(max)) = (self.health.get_mut(index), max) {
            if !h.destroyed {
                h.hit_points = max;
            }
        }
    }
}

/// Where a flying actor is, for drawing and for shots to find it.
fn place(placed: &mut Placement, body: &Body) {
    let [x, y, z] = body.position.map(hb_formats::fixed::from_units);
    (placed.x, placed.y, placed.z) = (x, y, z);
    placed.heading = body.heading as u16;
    placed.pitch = body.pitch as i32;
    placed.roll = body.roll as i32;
}
