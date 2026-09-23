//! Powerups: what a level lays out in its `.PUP`, what destroyed things drop,
//! and what picking one up does.
//!
//! They live in an array at `0x66fd60`, 24 bytes each: the position, the kind
//! (bit 31 set once taken), the pickup size, and a timer the network game
//! respawns them with after 120 seconds. `0x426cf0` runs them each frame:
//! a powerup the player is within its size of on every axis is offered to
//! `0x426760`, which applies it or refuses it; the rest are drawn.

use hb_formats::text::EnemyDef;

use hb_formats::vector::within;

use crate::combat::Pilot;
use crate::mission::Voice;
use crate::turret::Rng;

/// The kinds, as the engine's names at `0x502308` give them, and the models
/// they are drawn with (`0x502288`, from `STARTUP.POD`). Several names are
/// older than the effects: kind 11 is labelled an energy can and repairs the
/// hull, kind 3 is "Dead-On Missiles" and is picked up as Sledgehammer
/// Rockets.
pub const KINDS: [(&str, &str); 31] = [
    ("Rapid-Fire Lasers", "f6rfl.bin"),
    ("ServoKinetic Lasers (Not used)", "powerpla.bin"),
    ("Dispersion Cannon 14", "f6dc.bin"),
    ("Dead-On Missiles", "f6dom.bin"),
    ("Vipers", "f6vip.bin"),
    ("Bion Fury Missiles (Not used)", "powersup.bin"),
    ("Shield Restore (not used)", "powershe.bin"),
    ("Cloak (Multi only)", "powervis.bin"),
    ("Invincibility", "powerinv.bin"),
    ("Guided MIRV", "f6gmv.bin"),
    ("Turbo Thrust (Not used)", "powerzap.bin"),
    ("Energy Can (Not used)", "powercan.bin"),
    ("Cruise", "f6cru.bin"),
    ("Cluster", "f6cls.bin"),
    ("MIRV", "f6mirv.bin"),
    ("MINE", "f6mine.bin"),
    ("Damage 25%", "f6dam1.bin"),
    ("Damage 50%", "f6dam2.bin"),
    ("Damage 100%", "f6dam3.bin"),
    ("Energy 25%", "f6eng1.bin"),
    ("Energy 50%", "f6eng2.bin"),
    ("Energy 100%", "f6eng3.bin"),
    ("Message Pod", "powercan.bin"),
    ("Super Weapon Piece 1", "f6swep.bin"),
    ("Super Weapon Piece 2", "f6swep.bin"),
    ("Super Weapon Piece 3", "f6swep.bin"),
    ("Super Weapon Piece 4", "f6swep.bin"),
    ("Super Weapon Piece 5", "f6swep.bin"),
    ("Super Weapon Piece 6", "f6swep.bin"),
    ("Super Weapon Piece 7", "f6swep.bin"),
    ("Super Weapon Piece 8", "f6swep.bin"),
];

pub const MESSAGE_POD: usize = 22;

/// The engine caps the array at 299 (`0x426f88`).
pub const MAX: usize = 299;

const fn voice(id: u16, sound: &'static str, text: &'static str) -> Voice {
    Voice { id, sound, text }
}

pub const RFL_SECURED: Voice = voice(0x10, "rfl-sec.wav", "RFL secured");
pub const DISPERSION_SECURED: Voice = voice(0x0e, "dc-sec.wav", "Dispersion Cannon secured");
pub const SLEDGEHAMMER_SECURED: Voice = voice(0x11, "hmmr-sec.wav", "Sledgehammer Rockets secured");
pub const VIPER_SECURED: Voice = voice(0x13, "vip-sec.wav", "Viper Missiles secured");
pub const HELLION_SECURED: Voice = voice(0x16, "hell-sed.wav", "Hellion Missiles secured");
pub const SCORCHER_SECURED: Voice = voice(0x12, "scor-sec.wav", "Scorcher Missiles secured");
pub const LEGION_SECURED: Voice = voice(0x14, "legn-sec.wav", "Legion Missiles secured");
pub const INDEPENDENCE_SECURED: Voice = voice(0x15, "indp-sec.wav", "Independence Missile secured");
pub const DOOMSDAY_SECURED: Voice = voice(0x17, "doom-sec.wav", "Doomsday Mines secured");
pub const HULL_NOT_NEEDED: Voice = voice(0x4e, "hull-rnr.wav", "Hull undamaged.  Repairs not required.");
pub const HULL_FULLY_REPAIRED: Voice = voice(0x42, "hullrpr.wav", "Hull fully repaired");
pub const HULL_REPAIRED: Voice = voice(0x29, "hullrpr.wav", "Hull Repaired");
pub const HULL_RESTORED: Voice = voice(0x4b, "hull-fr.wav", "Hull fully restored.");
pub const HULL_PARTLY: Voice = voice(0x4a, "hull-pr.wav", "Hull partially repaired.");
pub const ENERGY_NOT_NEEDED: Voice = voice(0x4d, "engy-nr.wav", "Energy not required.");
pub const ENERGY_BOOST: Voice = voice(0x48, "engy-sec.wav", "Energy boost secured.");
pub const ENERGY_FULL: Voice = voice(0x27, "engy100.wav", "Main energy at 100%");
pub const ENERGY_MAX: Voice = voice(0x4c, "engy-max.wav", "Main energy at maximum capacity.");
pub const POD_RETRIEVED: Voice = voice(0x55, "messpod.wav", "Message pod retrieved.");
pub const BION_CAPTURED: Voice = voice(0x43, "BtechCap.wav", "Bion technology captured.");
pub const WEAPON_COMPLETE: Voice = voice(0x44, "comptech.wav", "Weapon complete...");

/// What a pickup wants heard or shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Voice(Voice),
    Message(&'static str),
    Sound(&'static str),
}

/// What the player carries. The engine keeps weapon stocks in an array of
/// 32 eight-byte slots at `0x61bce0`; the slot each powerup fills is given.
#[derive(Debug, Clone, PartialEq)]
pub struct Stores {
    /// Rounds by weapon slot.
    pub ammo: [i32; 32],
    /// Main energy (`0x62d63c`), 1.0 full. Starts at half (`0x426f13`).
    pub energy: f32,
    /// Weapon energy (`0x62d678`), which the guns spend and main energy can be
    /// moved into; also half at the start (`0x426f56`).
    pub weapon_energy: f32,
    /// Afterburner fuel (slot 22, `0x61bd90`), 1.0 full.
    pub fuel: f32,
    /// Bits for the eight Bion super weapon pieces (`0x62d634`).
    pub pieces: u8,
    /// The weapon selected (`0x612220`), 23 at the start (`0x426ebf`).
    pub weapon: usize,
}

impl Default for Stores {
    /// The single-player loadout, `0x426e90`: slots 1 and 23 unlimited (-1),
    /// 20 in slot 18, 2 in 24, 5 in 19, a full tank.
    fn default() -> Stores {
        let mut ammo = [0; 32];
        ammo[1] = -1;
        ammo[23] = -1;
        ammo[18] = 20;
        ammo[24] = 2;
        ammo[19] = 5;
        Stores { ammo, energy: 0.5, weapon_energy: 0.5, fuel: 1.0, pieces: 0, weapon: 23 }
    }
}

impl Stores {
    /// `0x4659f0`: energy added, capped at full, with a line when it gets
    /// there. Returns what did not fit.
    fn add_energy(&mut self, amount: f32, events: &mut Vec<Event>) -> f32 {
        if self.energy + amount > 1.0 && self.energy < 1.0 {
            events.push(Event::Voice(ENERGY_MAX));
        }
        self.energy += amount;
        let mut over = 0.0;
        if self.energy > 1.0 {
            over = self.energy - 1.0;
            self.energy = 1.0;
        }
        if self.energy < 0.0 {
            over = self.energy;
            self.energy = 0.0;
        }
        over
    }
}

/// `0x426760`: apply a powerup of `kind`. False when it is refused - the
/// hull or the energy is already near full - and stays where it is.
pub fn collect(kind: usize, pilot: &mut Pilot, stores: &mut Stores, events: &mut Vec<Event>) -> bool {
    // The engine's hull is 16.16 with 0xffff full.
    let hull = (pilot.health * 65535.0).round() as i32;
    let set_hull = |pilot: &mut Pilot, h: i32| pilot.health = h as f32 / 65535.0;
    let energy = (stores.energy * 65536.0).round() as i32;
    let mut stock = |slot: usize, n: i32, v: Voice, events: &mut Vec<Event>| {
        stores.ammo[slot] += n;
        events.push(Event::Voice(v));
    };
    match kind {
        0 => stock(3, 100, RFL_SECURED, events),
        2 => stock(2, 100, DISPERSION_SECURED, events),
        3 => stock(18, 20, SLEDGEHAMMER_SECURED, events),
        4 | 5 => stock(19, 20, VIPER_SECURED, events),
        7 => events.push(Event::Message("Cloaking Device")),
        9 => stock(27, 1, HELLION_SECURED, events),
        10 => {
            stores.fuel = 1.0;
            events.push(Event::Message("Afterburner Fuel"));
            events.push(Event::Sound("power-1.wav"));
        }
        11 => {
            if hull >= 0xfffa {
                events.push(Event::Voice(HULL_NOT_NEEDED));
                return false;
            }
            let h = hull + 0x4000;
            if h > 0xffff {
                set_hull(pilot, 0xffff);
                events.push(Event::Voice(HULL_FULLY_REPAIRED));
            } else {
                set_hull(pilot, h);
                events.push(Event::Voice(HULL_REPAIRED));
            }
        }
        12 => stock(24, 5, SCORCHER_SECURED, events),
        13 => stock(25, 10, LEGION_SECURED, events),
        14 => stock(26, 1, INDEPENDENCE_SECURED, events),
        15 => stock(28, 5, DOOMSDAY_SECURED, events),
        16 | 17 => {
            if hull > 0xea60 {
                events.push(Event::Voice(HULL_NOT_NEEDED));
                return false;
            }
            let h = hull + if kind == 16 { 0x4000 } else { 0x8000 };
            if h > 0xffff {
                set_hull(pilot, 0xffff);
                events.push(Event::Voice(HULL_RESTORED));
            } else {
                set_hull(pilot, h);
                events.push(Event::Voice(HULL_PARTLY));
            }
            if kind == 17 {
                events.push(Event::Message("50% Damage Repair"));
                events.push(Event::Sound("power-1.wav"));
            }
        }
        18 => {
            if hull > 0xea60 {
                events.push(Event::Voice(HULL_NOT_NEEDED));
                return false;
            }
            set_hull(pilot, 0xffff);
            events.push(Event::Voice(HULL_RESTORED));
        }
        19 | 20 => {
            if energy >= 0xffff {
                events.push(Event::Voice(ENERGY_NOT_NEEDED));
                return false;
            }
            let (amount, below) = if kind == 19 { (0x4000, 0x10000) } else { (0x8000, 0x8000) };
            if energy < below {
                events.push(Event::Message(if kind == 19 {
                    "25% Energy Boost"
                } else {
                    "50% Energy Boost"
                }));
                events.push(Event::Voice(ENERGY_BOOST));
            }
            stores.add_energy(amount as f32 / 65536.0, events);
        }
        21 => {
            if energy >= 0xffff {
                events.push(Event::Voice(ENERGY_NOT_NEEDED));
                return false;
            }
            stores.energy = 0xffff as f32 / 65536.0;
            events.push(Event::Voice(ENERGY_FULL));
        }
        MESSAGE_POD => events.push(Event::Voice(POD_RETRIEVED)),
        23..=30 => {
            stores.pieces |= 1 << (kind - 23);
            events.push(Event::Voice(BION_CAPTURED));
            if stores.pieces == 0xff {
                // The super weapon: unlimited, and selected.
                stores.ammo[30] = -1;
                stores.weapon = 30;
                events.push(Event::Voice(WEAPON_COMPLETE));
            }
        }
        // 1, 6 and 8 are taken and do nothing.
        _ => {}
    }
    true
}

/// A powerup in the world.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Powerup {
    pub position: [f32; 3],
    pub kind: usize,
    /// How close the player must come on every axis, units: the model's
    /// bounding radius (`+0x10`, from the model bounds' `+0x24`).
    pub size: f32,
    pub taken: bool,
}

/// The size a powerup of a model is picked up at: `0x473ee0`'s radius,
/// the length of the largest absolute extent on each axis, in world units.
pub fn size_of(model: &hb_formats::mrgl::Model) -> f32 {
    let Some(unit) = model.unit.filter(|&u| u > 0) else { return 0.0 };
    let mut most = [0i32; 3];
    for v in &model.vertices {
        most[0] = most[0].max(v.x.abs());
        most[1] = most[1].max(v.y.abs());
        most[2] = most[2].max(v.z.abs());
    }
    let [x, y, z] = most.map(|m| m as f32 / unit as f32);
    (x * x + y * y + z * z).sqrt()
}

/// What a destroyed actor of this type drops, if anything (`0x40cc3c`).
pub fn drop_for(def: &EnemyDef, rng: &mut Rng) -> Option<usize> {
    if def.drop_chance == 0 {
        return None;
    }
    let roll = (rng.next() as i32 * 100) / 0x7fff;
    if def.drop_chance < roll {
        return None;
    }
    let kind = if def.drop_kind == -1 {
        (rng.next() as i32 * 31 / 0x7fff) as usize
    } else {
        def.drop_kind as usize
    };
    (kind < KINDS.len()).then_some(kind)
}

/// Every powerup in a level.
#[derive(Debug, Clone, Default)]
pub struct Field {
    pub items: Vec<Powerup>,
}

impl Field {
    /// Put one down (`0x426f80` for a drop, `0x426540` for the `.PUP`): at
    /// least twice its size over the floor under it. The engine also keeps a
    /// drop below the ceiling over it and under twice `0x5055d4`, neither of
    /// which the port models.
    pub fn place(&mut self, position: [f32; 3], kind: usize, size: f32, floor: f32) {
        if self.items.len() >= MAX {
            return;
        }
        let mut position = position;
        position[1] = position[1].max(floor + 2.0 * size);
        self.items.push(Powerup { position, kind, size, taken: false });
    }

    /// `0x426cf0`, the single-player half: offer each powerup the player is
    /// inside to [`collect`]. Returns the indices taken, so a message pod can
    /// finish its mission point. `touching` carries which were being touched
    /// last frame; a refused one speaks only when first touched, where the
    /// engine repeats the line every frame.
    pub fn step(
        &mut self,
        player: [f32; 3],
        pilot: &mut Pilot,
        stores: &mut Stores,
        touching: &mut Vec<usize>,
        events: &mut Vec<Event>,
    ) -> Vec<usize> {
        let mut taken = Vec::new();
        let mut now = Vec::new();
        for (i, p) in self.items.iter_mut().enumerate() {
            if p.taken {
                continue;
            }
            let inside = within(player, p.position, p.size);
            if !inside {
                continue;
            }
            now.push(i);
            let mut said = Vec::new();
            if collect(p.kind, pilot, stores, &mut said) {
                p.taken = true;
                taken.push(i);
                events.extend(said);
            } else if !touching.contains(&i) {
                events.extend(said);
            }
        }
        *touching = now;
        taken
    }
}
