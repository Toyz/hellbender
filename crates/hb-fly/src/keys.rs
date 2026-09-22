//! The game's own key bindings, from `system/hellbend.ini`.
//!
//! The engine keeps a scan code per action in `[Control]` and reads them with
//! `GetPrivateProfileInt`, passing each setting's current value as the
//! default - so a machine with no `.INI` plays on
//! [`hb_formats::ini::CONTROL_DEFAULTS`], and this does the same.
//!
//! Scan codes are set 1 make codes, the same numbers a `.DMO` records.
//! [`key_of`] turns one into the key minifb reports, which is where this
//! stops being about the game and starts being about the window.

use hb_formats::ini::Ini;
use minifb::Key;

/// A set 1 make code, as far as one maps onto a key a window can report.
/// The extended codes (the arrow cluster, the right-hand modifiers) share
/// numbers with the keypad in this table, which is what the engine's own
/// defaults use them as - 75 is the left arrow to the game.
pub fn key_of(scan: u8) -> Option<Key> {
    Some(match scan {
        1 => Key::Escape,
        2 => Key::Key1,
        3 => Key::Key2,
        4 => Key::Key3,
        5 => Key::Key4,
        6 => Key::Key5,
        7 => Key::Key6,
        8 => Key::Key7,
        9 => Key::Key8,
        10 => Key::Key9,
        11 => Key::Key0,
        12 => Key::Minus,
        13 => Key::Equal,
        14 => Key::Backspace,
        15 => Key::Tab,
        16 => Key::Q,
        17 => Key::W,
        18 => Key::E,
        19 => Key::R,
        20 => Key::T,
        21 => Key::Y,
        22 => Key::U,
        23 => Key::I,
        24 => Key::O,
        25 => Key::P,
        26 => Key::LeftBracket,
        27 => Key::RightBracket,
        28 => Key::Enter,
        29 => Key::LeftCtrl,
        30 => Key::A,
        31 => Key::S,
        32 => Key::D,
        33 => Key::F,
        34 => Key::G,
        35 => Key::H,
        36 => Key::J,
        37 => Key::K,
        38 => Key::L,
        39 => Key::Semicolon,
        40 => Key::Apostrophe,
        41 => Key::Backquote,
        42 => Key::LeftShift,
        43 => Key::Backslash,
        44 => Key::Z,
        45 => Key::X,
        46 => Key::C,
        47 => Key::V,
        48 => Key::B,
        49 => Key::N,
        50 => Key::M,
        51 => Key::Comma,
        52 => Key::Period,
        53 => Key::Slash,
        54 => Key::RightShift,
        56 => Key::LeftAlt,
        57 => Key::Space,
        59 => Key::F1,
        60 => Key::F2,
        61 => Key::F3,
        62 => Key::F4,
        63 => Key::F5,
        64 => Key::F6,
        65 => Key::F7,
        66 => Key::F8,
        67 => Key::F9,
        68 => Key::F10,
        71 => Key::Home,
        72 => Key::Up,
        73 => Key::PageUp,
        74 => Key::NumPadMinus,
        75 => Key::Left,
        77 => Key::Right,
        78 => Key::NumPadPlus,
        79 => Key::End,
        80 => Key::Down,
        81 => Key::PageDown,
        82 => Key::Insert,
        83 => Key::Delete,
        87 => Key::F11,
        88 => Key::F12,
        _ => return None,
    })
}

/// Every binding this port listens to, resolved once.
///
/// The five at the end are the port's own - the game has no such switches -
/// and sit on codes `[Control]` does not claim, so nothing here collides with
/// anything the game binds.
#[derive(Debug, Clone)]
pub struct Bindings {
    pub up: Option<Key>,
    pub down: Option<Key>,
    pub left: Option<Key>,
    pub right: Option<Key>,
    pub roll_left: Option<Key>,
    pub roll_right: Option<Key>,
    pub throttle_up: Option<Key>,
    pub throttle_down: Option<Key>,
    pub fire: Option<Key>,
    /// `weaponKey`, the engine's second trigger - what the cockpit hand
    /// reaches for (`0x41fde0`).
    pub weapon: Option<Key>,
    pub missile_lock: Option<Key>,
    pub beacon: Option<Key>,
    pub transfer_weapon: Option<Key>,
    pub transfer_shields: Option<Key>,
    pub next_weapon: Option<Key>,
    pub previous_weapon: Option<Key>,
    pub crosshair: Option<Key>,
    pub cockpit_label: Option<Key>,
    pub end_game: Option<Key>,
    /// The eleven weapon keys, in the order `[Control]` lists them, which is
    /// the order the weapon table holds them.
    pub weapons: [Option<Key>; 11],
    /// The port's own, on codes the game leaves alone: H, K, Y, G and P.
    pub hud: Key,
    pub cockpit: Key,
    pub music: Key,
    pub collide: Key,
    pub next_level: Key,
}

/// What each weapon key selects, as a row of the weapon table, in the order
/// `[Control]` lists the keys.
pub const WEAPON_ROWS: [usize; 11] = {
    use hb_sim::weapons::*;
    [VALKYRIE, DISPERSION, SERVO_KINETIC, RAPID_FIRE, DEAD_ON, CRUISE, VIPER, CLUSTER, MIRV,
     GUIDED_MIRV, MINE]
};

const WEAPON_KEYS: [&str; 11] = [
    "keyVulcanCannon",
    "keyDispersionCannon",
    "keySKL",
    "keyRFL20",
    "keyDOM",
    "keyCruiseMissile",
    "keyViperMissile",
    "keyClusterMissile",
    "keyMIRVMissile",
    "keyGuidedMIRV",
    "keyMine",
];

impl Bindings {
    /// Read them out of the game's `.INI` if it is there, and fall back to
    /// the engine's defaults setting by setting if it is not.
    pub fn load(game: &std::path::Path) -> (Bindings, bool) {
        let ini = Ini::read(&game.join("system").join("hellbend.ini"))
            .or_else(|| Ini::read(&game.join("HELLBEND.INI")));
        let found = ini.is_some();
        let ini = ini.unwrap_or_default();
        let key = |name: &str| ini.binding(name).and_then(key_of);
        let mut weapons = [None; 11];
        for (slot, name) in weapons.iter_mut().zip(WEAPON_KEYS) {
            *slot = key(name);
        }
        let bindings = Bindings {
            up: key("upKey"),
            down: key("downKey"),
            left: key("leftKey"),
            right: key("rightKey"),
            roll_left: key("rollLeftKey"),
            roll_right: key("rollRightKey"),
            throttle_up: key("throttleUpKey"),
            throttle_down: key("throttleDownKey"),
            fire: key("fireKey"),
            weapon: key("weaponKey"),
            missile_lock: key("keyMissileLock"),
            beacon: key("keyBeacon"),
            transfer_weapon: key("keyTransferWeapon"),
            transfer_shields: key("keyTransferShields"),
            next_weapon: key("keySelectNextWeapon"),
            previous_weapon: key("keySelectPrevWeapon"),
            crosshair: key("keyCrosshair"),
            cockpit_label: key("keyCockpitLabel"),
            end_game: key("keyEndGame"),
            weapons,
            hud: Key::H,
            cockpit: Key::K,
            music: Key::Y,
            collide: Key::G,
            next_level: Key::P,
        };
        (bindings, found)
    }
}
