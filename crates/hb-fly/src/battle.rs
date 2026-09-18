//! The fight: the player's shots, the turrets', the SAM sites' missiles, and
//! what they do to each other. The rules are `hb_sim`'s; this holds the state
//! for one level and reports what should be heard.

use hb_formats::text::Placement;
use hb_render::Level;
use hb_sim::combat::{self, Health, HitVolume, Pilot, Shot, Side, Stop};
use hb_sim::turret::{Launch, Missile, Rng, Turret};

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
    pub shots: Vec<Flying>,
    pub missiles: Vec<Missile>,
    pub pilot: Pilot,
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
            .filter(|(_, p)| level.kinds.get(p.kind).is_some_and(|k| k.class() == 10))
            .map(|(i, p)| (i, Turret::new(p)))
            .collect();
        Battle {
            health: level.placements.iter().map(Health::for_placement).collect(),
            volumes,
            turrets,
            shots: Vec::new(),
            missiles: Vec::new(),
            pilot: Pilot::default(),
            destroyed: 0,
            deaths: 0,
            rng: Rng::new(0x1996),
        }
    }

    pub fn turret_count(&self) -> usize {
        self.turrets.len()
    }

    pub fn fire(&mut self, shot: Shot) {
        self.shots.push(Flying::new(shot));
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
        dt: f32,
        solid: &impl Fn([f32; 3]) -> bool,
    ) -> Vec<Noise> {
        let mut noises = Vec::new();
        self.pilot.since_hit += dt;

        // Turrets think whether or not anyone is near.
        let target = self.pilot.alive().then_some(player);
        for (i, turret) in &mut self.turrets {
            if self.health[*i].destroyed {
                continue;
            }
            let Some(player) = target else { break };
            let p = &live[*i];
            let def = &level.kinds[p.kind];
            let mesh = level.meshes.get(p.kind).and_then(Option::as_ref);
            match turret.step(def, mesh, p, player, velocity, dt, &mut self.rng) {
                Some(Launch::Shot(s)) => self.shots.push(Flying::new(s)),
                Some(Launch::Missile(m)) => self.missiles.push(m),
                None => {}
            }
            // The turret's heading is what the renderer shows.
            live[*i].heading = turret.heading as u16;
        }

        let health = &self.health;
        let mut hits: Vec<(usize, f32, i32)> = Vec::new();
        let mut player_damage = 0.0f32;
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

        for m in &mut self.missiles {
            match m.step(dt, target, solid) {
                Some(true) => {
                    player_damage += Missile::DAMAGE;
                    m.age = f32::MAX;
                }
                Some(false) => m.age = f32::MAX,
                None => {}
            }
        }
        self.missiles.retain(Missile::alive);

        for (i, damage, kind) in hits {
            let original = level.placements[i].kind;
            let scaled = damage * combat::multiplier(&level.kinds[original], kind);
            if self.health[i].take(scaled) {
                self.destroyed += 1;
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
