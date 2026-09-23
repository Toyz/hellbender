//! The player's weapons: the table, the trigger, the guns' patterns, and the
//! energy that feeds them.
//!
//! The trigger (`0x47db11`) runs a rate accumulator: while fire is held it
//! gains the weapon's volleys a second times the frame time, and each time it
//! passes 1.0 the fire routine (`0x47d520`) looses one volley; let go, it
//! waits at 1.0 so the next press fires at once. The volley's shape depends on
//! the weapon and on how much weapon energy is left - the guns fire from one,
//! two or four barrels, and the more barrels the more each volley costs.

use hb_formats::fixed::to_units;
use hb_formats::vector::direction;

use crate::combat::{Shot, Side};
use crate::mission::Voice;
use crate::powerup::{Stores, ENERGY_MAX};
use crate::turret::{Missile, Rng};

/// One row of the weapon table at `0x50e7a0`, 68 bytes: the model name 28
/// bytes before the speed and the HUD code 8 bytes before it, then speed and
/// damage (16.16), whether the next-weapon key stops on it, volleys a second,
/// and the sound it fires with. The names are the engine's (`0x50e700`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub code: &'static str,
    pub name: &'static str,
    /// Units a second, 16.16. The ship's own speed is added when fired.
    pub speed: i32,
    /// A fraction of full health, 16.16.
    pub damage: i32,
    /// The next-weapon key stops here (`+8`, `0x479ccc`).
    pub cycles: bool,
    /// Volleys a second (`+0xc`).
    pub rate: i32,
    pub sound: &'static str,
    pub model: &'static str,
    /// The row's first word: 0 draws the shot as a laser turned by its own
    /// angles or held facing the eye, 1 as a missile, 2 not at all
    /// (`0x476473` only loads a model for 0 and 1).
    pub draw: i32,
}

pub const ROWS: [Row; 31] = [
    Row { code: "", name: "Purple Laser", speed: 2097152, damage: 4096, cycles: false, rate: 0, sound: "", model: "laser.bin", draw: 0 },
    Row { code: "SKL", name: "Servo Kinetic Laser", speed: 2097152, damage: 4096, cycles: true, rate: 6, sound: "laser4.wav", model: "laser3.bin", draw: 0 },
    Row { code: "DIS", name: "Dispersion Cannon 14", speed: 1572864, damage: 8192, cycles: true, rate: 2, sound: "laser3.wav", model: "laser4.bin", draw: 0 },
    Row { code: "RFL", name: "Rapid Fire Laser", speed: 4194304, damage: 4096, cycles: true, rate: 6, sound: "laser5.wav", model: "laser5.bin", draw: 0 },
    Row { code: "", name: "Fire Ball", speed: 2097152, damage: 16384, cycles: false, rate: 0, sound: "", model: "laser6.bin", draw: 0 },
    Row { code: "", name: "Green Laser", speed: 2097152, damage: 16384, cycles: false, rate: 0, sound: "", model: "laser7.bin", draw: 0 },
    Row { code: "", name: "Red Laser", speed: 2097152, damage: 16384, cycles: false, rate: 0, sound: "", model: "laser8.bin", draw: 0 },
    Row { code: "", name: "Blue Laser", speed: 2097152, damage: 16384, cycles: false, rate: 0, sound: "", model: "laser9.bin", draw: 0 },
    Row { code: "", name: "Bullet", speed: 2097152, damage: 16384, cycles: false, rate: 0, sound: "", model: "bullet.bin", draw: 0 },
    Row { code: "Boss1", name: "Purple Ball for Mine", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss1.bin", draw: 0 },
    Row { code: "Boss2", name: "Blue Fireball for Artic", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss2.bin", draw: 0 },
    Row { code: "Boss3", name: "Gold ball for Canyon", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss3.bin", draw: 0 },
    Row { code: "Boss4", name: "Atom Weapon for asteroid", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss4.bin", draw: 0 },
    Row { code: "Boss5", name: "Purple Ring", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss5.bin", draw: 0 },
    Row { code: "Boss6", name: "Boss W6", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss6.bin", draw: 0 },
    Row { code: "Boss7", name: "Boss W7", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss7.bin", draw: 0 },
    Row { code: "Boss8", name: "Boss W8", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "wboss8.bin", draw: 0 },
    Row { code: "", name: "Enemy missile", speed: 0, damage: 16384, cycles: false, rate: 0, sound: "", model: "missile.bin", draw: 1 },
    Row { code: "HAM", name: "Dead On Missile", speed: 4194304, damage: 65536, cycles: true, rate: 1, sound: "missile.wav", model: "rocket4.bin", draw: 1 },
    Row { code: "VIP", name: "Viper Missile", speed: 4194304, damage: 65536, cycles: true, rate: 1, sound: "missl-2.wav", model: "viper6.bin", draw: 1 },
    Row { code: " ", name: "Super missile", speed: 0, damage: 0, cycles: false, rate: 0, sound: "", model: "", draw: 2 },
    Row { code: " ", name: "Smart bomb", speed: 0, damage: 0, cycles: false, rate: 0, sound: "", model: "", draw: 2 },
    Row { code: "FLY", name: "Afterburner", speed: 0, damage: 8192, cycles: false, rate: 6, sound: "", model: "", draw: 2 },
    Row { code: "VAL", name: "Valkyrie Cannon", speed: 8388608, damage: 8192, cycles: true, rate: 6, sound: "m-gun-r.wav", model: "", draw: 2 },
    Row { code: "SCR", name: "Cruise Missile", speed: 4194304, damage: 65536, cycles: true, rate: 1, sound: "missl-1.wav", model: "cruise5.bin", draw: 1 },
    Row { code: "LGN", name: "Cluster Missile", speed: 4194304, damage: 65536, cycles: true, rate: 1, sound: "missl-3.wav", model: "cluster7.bin", draw: 1 },
    Row { code: "TIM", name: "MIRV Missile", speed: 4194304, damage: 65536, cycles: false, rate: 1, sound: "missl-4.wav", model: "mirv8.bin", draw: 1 },
    Row { code: "HEL", name: "Guided MIRV Missile", speed: 4194304, damage: 65536, cycles: false, rate: 1, sound: "missl-4.wav", model: "gmirv9.bin", draw: 1 },
    Row { code: "DDM", name: "Floating Mine", speed: 4194304, damage: 65536, cycles: false, rate: 1, sound: "mines-x8.wav", model: "mine.bin", draw: 2 },
    Row { code: "CLOK", name: "Cloak", speed: 0, damage: 0, cycles: false, rate: 1, sound: "cloak-b.wav", model: "", draw: 2 },
    Row { code: "TEX", name: "Super Weapon", speed: 3145728, damage: 65536, cycles: false, rate: 1, sound: "missile.wav", model: "mine.bin", draw: 1 },
];

/// What the player starts with selected (`0x426ebf`): the Valkyrie Cannon.
pub const START: usize = 23;
pub const SERVO_KINETIC: usize = 1;
pub const DISPERSION: usize = 2;
pub const RAPID_FIRE: usize = 3;
/// "Sledgehammer Rockets" to the voice, unguided.
pub const DEAD_ON: usize = 18;
/// Locks on things that fly.
pub const VIPER: usize = 19;
pub const AFTERBURNER: usize = 22;
pub const VALKYRIE: usize = 23;
/// "Scorcher Missiles": locks on things on the ground, flies ten seconds.
pub const CRUISE: usize = 24;
/// "Legion": two missiles at once, one each side (`0x47cec0`).
pub const CLUSTER: usize = 25;

/// The MIRV and the guided MIRV, which break up a second into their flight.
pub const MIRV: usize = 26;
pub const GUIDED_MIRV: usize = 27;
/// The floating mine, laid behind the ship rather than fired
/// ([`crate::mine`]).
pub const MINE: usize = 28;
/// The Bion super weapon: the eight pieces make it and select it.
pub const SUPER: usize = 30;

/// The weapons this port can fire. The cluster missile's spread
/// (`0x47cec0`) is not ported yet.
pub const PORTED: [usize; 12] = [
    SERVO_KINETIC,
    DISPERSION,
    RAPID_FIRE,
    VALKYRIE,
    DEAD_ON,
    CRUISE,
    VIPER,
    MIRV,
    GUIDED_MIRV,
    CLUSTER,
    MINE,
    SUPER,
];

/// The weapon keys, as `HELLBEND.INI` binds them and the trigger routine
/// reads them (`0x47dcc4` on): the backquote for the Valkyrie
/// (`keyVulcanCannon`), 1 the dispersion cannon, 2 the servo-kinetic laser,
/// 3 the rapid-fire laser, 4 Dead-On, 5 cruise, 6 Viper missiles.
pub const KEYS: [(char, usize); 11] = [
    ('0', MINE),
    ('`', VALKYRIE),
    ('1', DISPERSION),
    ('2', SERVO_KINETIC),
    ('3', RAPID_FIRE),
    ('4', DEAD_ON),
    ('5', CRUISE),
    ('6', VIPER),
    ('7', CLUSTER),
    ('8', MIRV),
    ('9', GUIDED_MIRV),
];

pub const WEAPON_ENERGY_LOW: Voice =
    Voice { id: 0x4f, sound: "warn-wel.wav", text: "Warning!  Weapon energy low!" };
pub const SHIELD_ENERGY_LOW: Voice =
    Voice { id: 0x50, sound: "warn-sel.wav", text: "Warning!  Shield energy low!" };
pub const NO_DISPERSION: Voice =
    Voice { id: 0x1e, sound: "pause.wav", text: "Dispersion Cannon not in arsenal" };
pub const NO_RAPID_FIRE: Voice =
    Voice { id: 0x1f, sound: "pause.wav", text: "Rapid Fire Laser not in arsenal" };
pub const NO_DEAD_ON: Voice =
    Voice { id: 0x20, sound: "pause.wav", text: "Sledgehammer Rockets not in arsenal" };
pub const NO_CRUISE: Voice =
    Voice { id: 0x21, sound: "pause.wav", text: "Scorcher Missiles not in arsenal" };
pub const NO_VIPER: Voice =
    Voice { id: 0x22, sound: "pause.wav", text: "Viper Missiles not in arsenal" };
/// Not a voice line but a line on the HUD: the mine's own refusal, printed
/// by `0x480ee0` from the string at `0x50f410`.
pub const TOO_SLOW_FOR_MINE: Voice =
    Voice { id: 0, sound: "", text: "Go Faster to Deploy Mine" };

/// The behaviour classes that fly, as `0x40dca0` sorts them for the Viper's
/// lock; the cruise missile's (`0x40dc00`) takes every other class up to 62.
pub const AIRBORNE: [i64; 34] = [
    2, 4, 7, 8, 16, 17, 18, 25, 26, 28, 29, 32, 35, 38, 39, 40, 43, 44, 46, 48, 49, 50, 51, 52, 53, 54, 55, 56,
    57, 58, 59, 60, 63, 64,
];

pub fn airborne(class: i64) -> bool {
    AIRBORNE.contains(&class)
}

/// The three models the Valkyrie Cannon's shots cycle through, one a draw
/// (`0x476a44`, `0x61ad70`).
pub const MUZZLE: [&str; 3] = ["muzzle.bin", "muzzle2.bin", "muzzle3.bin"];

/// The shot kinds drawn facing the eye rather than along their own flight
/// (`0x476a09` in the table at `0x476a7c`): the fireballs, the balls and the
/// bosses' weapons.
pub const FACES_THE_EYE: [i32; 10] = [2, 4, 9, 10, 11, 12, 13, 14, 15, 16];

/// The line below which weapon or shield energy is "low", 0x199a.
const LOW: f32 = to_units(0x199a);

/// The ship as the guns see it.
#[derive(Debug, Clone, Copy)]
pub struct Pose {
    pub position: [f32; 3],
    pub right: [f32; 3],
    pub up: [f32; 3],
    pub forward: [f32; 3],
    /// Pitch and heading in the 16-bit circle, for the shots' direction.
    pub pitch: f32,
    pub heading: f32,
    /// Units a second.
    pub speed: f32,
}

/// One volley.
#[derive(Debug, Clone)]
pub struct Volley {
    pub shots: Vec<Shot>,
    pub missiles: Vec<Missile>,
    /// A mine was laid this shot: the caller puts it in the field, since
    /// the mines outlive the volley.
    pub mine: bool,
    /// The weapon's sound, played at the ship.
    pub sound: &'static str,
}

#[derive(Debug, Clone)]
pub struct Guns {
    pub selected: usize,
    /// The rate accumulator (`0x613c84`).
    accumulator: f32,
    /// Which side a single barrel fires from next (`0x50f034`).
    side: bool,
    /// Where the dispersion cannon's pattern is (`0x50f038`).
    pattern: usize,
    /// Which side a missile leaves from next (`0x50f088`).
    rail: bool,
    /// The locked target (`0x50e6fc`).
    pub lock: Option<usize>,
}

impl Default for Guns {
    fn default() -> Guns {
        Guns { selected: START, accumulator: 1.0, side: false, pattern: 0, rail: false, lock: None }
    }
}

/// What a placement looks like to the missile lock.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    pub class: i64,
    pub friendly: bool,
    /// Hit points above zero.
    pub alive: bool,
    /// Within the 80-unit box this frame, so the actor loop ran it
    /// (`+0x84`).
    pub near: bool,
    /// Its position in view space: x right, y up, z ahead.
    pub view: [f32; 3],
}

impl Candidate {
    /// `0x47bbd0` for one weapon: alive and near, not friendly, not class 33,
    /// on screen - `0x42f710` with no radius: in front and inside the 90
    /// degrees each way - and of the kind the weapon locks on. The Viper
    /// takes things that fly, the cruise missile things that do not, and
    /// the cluster missile and super weapon anything.
    pub fn lockable(&self, weapon: usize) -> bool {
        let kind = match weapon {
            VIPER => airborne(self.class),
            CRUISE => !airborne(self.class) && self.class <= 62,
            25 | 30 => true,
            _ => return false,
        };
        let [x, y, z] = self.view;
        kind && self.alive
            && self.near
            && !self.friendly
            && self.class != 33
            && z >= 0.0
            && x.abs() <= z
            && y.abs() <= z
    }
}

fn add(a: [f32; 3], b: [f32; 3], k: f32) -> [f32; 3] {
    [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k]
}

/// Weapon energy up or down by `amount` (`0x465910`), warning on the way
/// below the low line. What does not fit is returned.
fn weapon_energy(stores: &mut Stores, amount: f32, voices: &mut Vec<Voice>) -> f32 {
    if stores.weapon_energy + amount < LOW && stores.weapon_energy > LOW {
        voices.push(WEAPON_ENERGY_LOW);
    }
    stores.weapon_energy += amount;
    let mut over = 0.0;
    if stores.weapon_energy > 1.0 {
        over = stores.weapon_energy - 1.0;
        stores.weapon_energy = 1.0;
    }
    if stores.weapon_energy < 0.0 {
        over = stores.weapon_energy;
        stores.weapon_energy = 0.0;
    }
    over
}

impl Guns {
    /// How many barrels the guns fire from: one with no weapon energy, two up
    /// to half, four above (`0x479ee3`, `0x47a7c3`, `0x47ab63`).
    pub fn barrels(stores: &Stores) -> usize {
        let e = stores.weapon_energy;
        if e <= 0.0 {
            1
        } else if e <= 0.5 {
            2
        } else {
            4
        }
    }

    /// A weapon key: select it if there is anything to fire, or say it is
    /// not there. The port only offers the weapons it fires.
    pub fn select(&mut self, weapon: usize, stores: &Stores) -> Option<Voice> {
        if !PORTED.contains(&weapon) {
            return None;
        }
        if stores.ammo[weapon] != 0 {
            self.selected = weapon;
            return None;
        }
        match weapon {
            DISPERSION => Some(NO_DISPERSION),
            RAPID_FIRE => Some(NO_RAPID_FIRE),
            DEAD_ON => Some(NO_DEAD_ON),
            CRUISE => Some(NO_CRUISE),
            VIPER => Some(NO_VIPER),
            _ => None,
        }
    }

    /// The next-weapon key (`0x479ca0`): on to the next weapon with a stock
    /// that the key stops on.
    pub fn next(&mut self, stores: &Stores) {
        let mut w = self.selected;
        for _ in 0..32 {
            w = (w + 1) % 32;
            if w < ROWS.len() && stores.ammo[w] != 0 && ROWS[w].cycles && PORTED.contains(&w) {
                self.selected = w;
                return;
            }
        }
    }

    /// The ring the previous-weapon key walks (`0x47e254`), in the order it
    /// walks it: each weapon's predecessor is the one after it here, and the
    /// twelfth wraps to the first.
    ///
    /// It is a hand-written ring, not the mirror of [`Guns::next`], which
    /// searches forward through the whole table of 32. That is why the two
    /// keys are different code in the engine and different code here.
    pub const RING: [usize; 12] = [
        SERVO_KINETIC,
        DISPERSION,
        VALKYRIE,
        SUPER,
        MINE,
        GUIDED_MIRV,
        MIRV,
        CLUSTER,
        VIPER,
        CRUISE,
        DEAD_ON,
        RAPID_FIRE,
    ];

    /// The previous-weapon key (`0x47e0f6`): round [`Guns::RING`], skipping
    /// anything with no stock. A weapon that is not on the ring at all -
    /// which is most of the table - leaves the selection where it is.
    pub fn previous(&mut self, stores: &Stores) {
        let Some(mut at) = Self::RING.iter().position(|&w| w == self.selected) else { return };
        for _ in 0..Self::RING.len() {
            at = (at + 1) % Self::RING.len();
            let w = Self::RING[at];
            if stores.ammo[w] != 0 && PORTED.contains(&w) {
                self.selected = w;
                return;
            }
        }
    }

    /// The lock, each frame (`0x47b700`): the lock key moves it to the next
    /// placement the selected weapon can lock, round the list; with nothing
    /// locked the first that can be is taken; a lock that can no longer be
    /// held is dropped.
    pub fn track(&mut self, pressed: bool, count: usize, can: impl Fn(usize) -> bool) {
        let search = |from: Option<usize>| -> Option<usize> {
            let mut i = from.map_or(0, |f| f + 1);
            for _ in 0..count {
                if i >= count {
                    i = 0;
                }
                if can(i) {
                    return Some(i);
                }
                i += 1;
            }
            None
        };
        if pressed && count > 0 {
            self.lock = search(self.lock);
        }
        if self.lock.is_none() && count > 0 {
            self.lock = search(None);
        }
        if let Some(i) = self.lock {
            if !can(i) {
                self.lock = None;
            }
        }
    }

    /// One frame of the trigger and the afterburner. Returns the volleys,
    /// anything to say, and whether the afterburner is lit.
    ///
    /// The engine runs the afterburner through the same accumulator as
    /// weapon 22, which burns fuel instead of firing: `0x1000 / rate` a
    /// volley at its six a second (`0x47d9de`), a sixteenth of the tank a
    /// second, so a full tank lasts sixteen seconds.
    pub fn step(
        &mut self,
        held: bool,
        burn: bool,
        dt: f32,
        pose: &Pose,
        stores: &mut Stores,
        rng: &mut Rng,
    ) -> (Vec<Volley>, Vec<Voice>, bool) {
        let (mut volleys, mut voices) = (Vec::new(), Vec::new());
        let lit = burn && stores.fuel > 0.0;
        if lit {
            stores.fuel = (stores.fuel - to_units(0x1000) * dt).max(0.0);
        }
        if !held {
            self.accumulator = 1.0;
            return (volleys, voices, lit);
        }
        self.accumulator += ROWS[self.selected].rate as f32 * dt;
        while self.accumulator > 1.0 {
            self.accumulator -= 1.0;
            if let Some(v) = self.fire(pose, dt, stores, rng, &mut voices) {
                volleys.push(v);
            }
        }
        (volleys, voices, lit)
    }

    /// `0x47d520`: one volley of the selected weapon, then its stock.
    fn fire(&mut self, pose: &Pose, dt: f32, stores: &mut Stores, rng: &mut Rng, voices: &mut Vec<Voice>) -> Option<Volley> {
        if stores.ammo[self.selected] == 0 {
            self.next(stores);
        }
        let w = self.selected;
        let row = ROWS[w];
        let speed = to_units(row.speed) + pose.speed;
        let damage = to_units(row.damage);
        let (mut shots, mut missiles) = (Vec::new(), Vec::new());
        let mut mine = false;
        match w {
            SERVO_KINETIC | RAPID_FIRE | VALKYRIE => shots = self.guns(w, pose, speed, damage, dt, stores, voices),
            DISPERSION => shots = self.dispersion(pose, speed, damage, stores, rng, voices),
            DEAD_ON | MIRV => missiles.push(self.missile(w, pose, None)),
            VIPER | CRUISE | GUIDED_MIRV | SUPER => missiles.push(self.missile(w, pose, self.lock)),
            // Two at once (`0x47cec0` calls the launcher twice): half a unit
            // forward and half up, one out to each side. The engine's first
            // leaves at `+(right + up) / 2` and the second a full right
            // vector along from it, which is `+(up - right) / 2`.
            CLUSTER => {
                for side in [1.0, -1.0] {
                    let at = add(
                        add(add(pose.position, pose.forward, 0.5), pose.up, 0.5),
                        pose.right,
                        0.5 * side,
                    );
                    missiles.push(Missile {
                        position: at,
                        heading: pose.heading,
                        pitch: pose.pitch,
                        speed: pose.speed,
                        age: 0.0,
                        damage,
                        kind: w as i32,
                        side: Side::Player,
                        target: self.lock,
                        life: Missile::LIFE,
                        trail: crate::smoke::Trail::new(at),
                    });
                }
            }
            // The mine is laid rather than fired, and refused outright below
            // 6.1 units a second - the engine does not spend one for a
            // refusal (`0x47d861`).
            MINE => {
                if !crate::mine::Field::fast_enough(pose.speed) {
                    voices.push(TOO_SLOW_FOR_MINE);
                    return None;
                }
                mine = true;
            }
            _ => return None,
        }
        let stock = &mut stores.ammo[w];
        if *stock != -1 {
            *stock -= 1;
            if *stock <= 0 {
                *stock = 0;
                self.next(stores);
            }
        }
        Some(Volley { shots, missiles, mine, sound: row.sound })
    }

    /// `0x47cd70` into `0x477890`: a missile from under one wing or the
    /// other in turn - half a unit ahead, a unit to the side and a unit down
    /// - along the nose at the ship's speed, into the guided missiles'
    /// pool. The Dead-On is launched with no target and flies straight.
    fn missile(&mut self, w: usize, pose: &Pose, target: Option<usize>) -> Missile {
        self.rail = !self.rail;
        let side = if self.rail { 1.0 } else { -1.0 };
        let at = add(add(add(pose.position, pose.forward, 0.5), pose.right, side), pose.up, -1.0);
        Missile {
            position: at,
            heading: pose.heading,
            pitch: pose.pitch,
            speed: pose.speed,
            age: 0.0,
            damage: to_units(ROWS[w].damage),
            kind: w as i32,
            side: Side::Player,
            target,
            life: if w == CRUISE { 10.0 } else { Missile::LIFE },
            trail: crate::smoke::Trail::new(at),
        }
    }

    /// The lasers and the cannon (`0x47a5d0`, `0x479ce0`): shots along the
    /// nose from one, two or four barrels. Each starts a frame's flight
    /// behind the ship, so its first update brings it level; the lasers'
    /// barrels are also half a unit forward, the cannon's not.
    #[allow(clippy::too_many_arguments)]
    fn guns(&mut self, w: usize, pose: &Pose, speed: f32, damage: f32, dt: f32, stores: &mut Stores, voices: &mut Vec<Voice>) -> Vec<Shot> {
        let dir = direction(pose.heading, pose.pitch);
        let mut base = add(pose.position, pose.forward, -speed * dt);
        if w != VALKYRIE {
            base = add(base, pose.forward, 0.5);
        }
        let (r, u) = (pose.right, pose.up);
        let at: Vec<[f32; 3]> = match Guns::barrels(stores) {
            1 => {
                // One barrel, three quarters out to either side in turn.
                self.side = !self.side;
                let s = if self.side { 0.75 } else { -0.75 };
                vec![add(add(base, r, s), u, -0.5)]
            }
            2 => {
                weapon_energy(stores, -(0x100 as f32) / 65536.0, voices);
                let low = add(base, u, -0.5);
                vec![add(low, r, 0.5), add(low, r, -0.5)]
            }
            _ => {
                weapon_energy(stores, -(0x200 as f32) / 65536.0, voices);
                let c = |a: f32, b: f32| add(add(base, r, a), u, b);
                vec![c(0.5, 0.5), c(-0.5, 0.5), c(-0.5, -0.5), c(0.5, -0.5)]
            }
        };
        at.into_iter().map(|p| Shot::fire(p, dir, speed, damage, w as i32, Side::Player)).collect()
    }

    /// The dispersion cannon (`0x47ab60`): shots from the ship's centre in a
    /// plus-shaped pattern of five, 1,024 of the 16-bit circle out
    /// (`0x50f040`, `0x50f058`), with shots fired straight back as well. One
    /// barrel fires one pair on one step of the pattern; two fire four ahead
    /// and two behind; four fire nine ahead scattered at random up to 2,048
    /// either way in pitch and heading, and two behind.
    fn dispersion(&mut self, pose: &Pose, speed: f32, damage: f32, stores: &mut Stores, rng: &mut Rng, voices: &mut Vec<Voice>) -> Vec<Shot> {
        const HEADING: [f32; 5] = [0.0, -1024.0, 0.0, 1024.0, 0.0];
        const PITCH: [f32; 5] = [-1024.0, 0.0, 0.0, 0.0, 1024.0];
        let shot = |pitch: f32, heading: f32| {
            Shot::fire(pose.position, direction(heading, pitch), speed, damage, DISPERSION as i32, Side::Player)
        };
        let ahead = |k: usize| shot(pose.pitch + PITCH[k], pose.heading + HEADING[k]);
        let behind = |k: usize| shot(-(pose.pitch + PITCH[k]), pose.heading + HEADING[k] + 32768.0);
        let mut shots = Vec::new();
        let step = |g: &mut Guns| {
            let k = g.pattern;
            g.pattern = (k + 1) % 5;
            k
        };
        match Guns::barrels(stores) {
            1 => {
                let k = step(self);
                shots.push(ahead(k));
                shots.push(behind(k));
            }
            2 => {
                for _ in 0..4 {
                    shots.push(ahead(step(self)));
                }
                for _ in 0..2 {
                    shots.push(behind(step(self)));
                }
                weapon_energy(stores, -(0x100 as f32) / 65536.0, voices);
            }
            _ => {
                for _ in 0..9 {
                    let dp = (rng.next() & 0xfff) as f32 - 2048.0;
                    let dh = (rng.next() & 0xfff) as f32 - 2048.0;
                    shots.push(shot(pose.pitch + dp, pose.heading + dh));
                    step(self);
                }
                for _ in 0..2 {
                    shots.push(behind(step(self)));
                }
                weapon_energy(stores, -(0x200 as f32) / 65536.0, voices);
            }
        }
        shots
    }
}

/// Main energy to weapon energy, an eighth at a press (`0x465ba0`,
/// `keyTransferWeapon`). What main energy lacks is taken off the transfer,
/// and what weapon energy cannot hold goes back.
pub fn transfer_to_weapons(stores: &mut Stores) -> Vec<Voice> {
    let mut voices = Vec::new();
    let short = take_main(stores);
    let back = weapon_energy(stores, 0.125 + short, &mut voices);
    give_main(stores, back, &mut voices);
    voices
}

/// Main energy to the shield, the same way (`0x465a70`,
/// `keyTransferShields`).
pub fn transfer_to_shields(stores: &mut Stores, shield: &mut f32) -> Vec<Voice> {
    let mut voices = Vec::new();
    let short = take_main(stores);
    let amount = 0.125 + short;
    if *shield + amount < LOW && *shield > LOW {
        voices.push(SHIELD_ENERGY_LOW);
    }
    *shield += amount;
    let mut back = 0.0;
    if *shield > 1.0 {
        back = *shield - 1.0;
        *shield = 1.0;
    }
    if *shield < 0.0 {
        back = *shield;
        *shield = 0.0;
    }
    give_main(stores, back, &mut voices);
    voices
}

/// An eighth off main energy; returns the shortfall, negative, when there
/// was less than that.
fn take_main(stores: &mut Stores) -> f32 {
    stores.energy -= 0.125;
    let mut short = 0.0;
    if stores.energy > 1.0 {
        stores.energy = 1.0;
    }
    if stores.energy < 0.0 {
        short = stores.energy;
        stores.energy = 0.0;
    }
    short
}

fn give_main(stores: &mut Stores, amount: f32, voices: &mut Vec<Voice>) {
    if stores.energy + amount > 1.0 && stores.energy < 1.0 {
        voices.push(ENERGY_MAX);
    }
    stores.energy = (stores.energy + amount).clamp(0.0, 1.0);
}

/// Every frame (`0x464d0f`): main energy and the hull creep back at 0x48 a
/// second, a fifteen-minute climb from empty. The afterburner's tank refills
/// from weapon energy, a thirty-second a second for 0xda of weapon energy;
/// an empty tank waits five seconds and is given a thirty-second.
pub fn regenerate(stores: &mut Stores, hull: &mut f32, dt: f32, empty_for: &mut f32) -> Vec<Voice> {
    let mut voices = Vec::new();
    let creep = to_units(0x48) * dt;
    give_main(stores, creep, &mut voices);
    if *hull > 0.0 {
        *hull = (*hull + creep).min(1.0);
    }
    if stores.fuel <= 0.0 {
        *empty_for += dt;
        if *empty_for > 5.0 {
            stores.fuel = to_units(0x800);
            *empty_for = 0.0;
        }
    } else if stores.fuel >= to_units(0xffff) {
        stores.fuel = to_units(0xffff);
    } else if stores.weapon_energy > 0.0 {
        stores.fuel += to_units(0x800) * dt;
        weapon_energy(stores, -(0xda as f32) / 65536.0 * dt, &mut voices);
    }
    voices
}
