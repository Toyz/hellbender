//! `system/hellbend.ini`, and the `[Control]` section in particular.
//!
//! The engine reads its settings with `GetPrivateProfileInt` out of
//! `.\system\hellbend.ini` (`0x502ab0`), passing the setting's current value
//! as the default - so the defaults are the globals' initial values, and a
//! machine with no `.INI` plays with exactly those. The file is written at
//! install and is not on the disc, so the table below is where the defaults
//! come from.
//!
//! Every binding is a set 1 keyboard scan code, which is what `.DMO` files
//! record too - see [`crate::demo`].

/// Where the key bindings live.
pub const CONTROL: &str = "Control";

/// Every binding in `[Control]`, with the value the engine starts it at
/// (`0x42d274` on, reading each global at `0x512758` and up).
///
/// The run from ` to 0 is the weapon keys in table order, which is how the
/// player-facing weapon list was worked out in the first place.
pub const CONTROL_DEFAULTS: [(&str, u8); 64] = [
    ("weaponKey", 33),
    ("fireKey", 57),
    ("leftKey", 75),
    ("rightKey", 77),
    ("upKey", 72),
    ("downKey", 80),
    ("rollLeftKey", 71),
    ("rollRightKey", 73),
    ("throttleUpKey", 45),
    ("throttleDownKey", 44),
    ("keyCloak", 46),
    ("keyMissileLock", 47),
    ("keyBeacon", 48),
    ("keyHeadlight", 38),
    ("keyNaviComp", 49),
    ("keyNavChoose", 15),
    ("keyMap", 50),
    ("keyMapZoomIn", 26),
    ("keyMapZoomOut", 27),
    ("keyTransferWeapon", 51),
    ("keyTransferShields", 52),
    ("keyCrosshair", 20),
    ("keyVulcanCannon", 41),
    ("keyDispersionCannon", 2),
    ("keySKL", 3),
    ("keyRFL20", 4),
    ("keyDOM", 5),
    ("keyCruiseMissile", 6),
    ("keyViperMissile", 7),
    ("keyClusterMissile", 8),
    ("keyMIRVMissile", 9),
    ("keyGuidedMIRV", 10),
    ("keyMine", 11),
    ("keySelectPrevWeapon", 12),
    ("keySelectNextWeapon", 13),
    ("keyChangeViews", 24),
    ("keyInstrument", 23),
    ("keyMultiTalk", 63),
    ("keyMultiTaunt1", 64),
    ("keyMultiTaunt2", 65),
    ("keyMultiTaunt3", 66),
    ("keyMultiTaunt4", 67),
    ("keyMultiTaunt5", 87),
    ("keyMultiTaunt6", 88),
    ("keyMultiTaunt7", 68),
    ("keyEndGame", 1),
    ("keyViewLeft", 82),
    ("keyViewRight", 83),
    ("keyViewForward", 74),
    ("keyViewBack", 78),
    ("keyCockpitLabel", 53),
    ("buttonFire", 1),
    ("buttonWeapon", 2),
    ("buttonThrottleUp", 8),
    ("buttonThrottleDown", 4),
    ("buttonFunction0", 1),
    ("buttonFunction1", 0),
    ("buttonFunction2", 8),
    ("buttonFunction3", 9),
    ("buttonFunction4", 33),
    ("buttonFunction5", 34),
    ("buttonFunction6", 20),
    ("buttonFunction7", 19),
    ("joystickActive", 0),
];

/// What a setting is set to, from the file if it says and from
/// [`CONTROL_DEFAULTS`] if it does not.
#[derive(Debug, Clone, Default)]
pub struct Ini {
    entries: Vec<(String, String, String)>,
}

impl Ini {
    /// Parse the whole file. Unreadable lines are skipped rather than
    /// refused: the engine's own reader takes what it understands.
    pub fn parse(text: &str) -> Ini {
        let mut entries = Vec::new();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = name.trim().to_ascii_lowercase();
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                entries.push((
                    section.clone(),
                    key.trim().to_ascii_lowercase(),
                    value.trim().to_string(),
                ));
            }
        }
        Ini { entries }
    }

    pub fn read(path: &std::path::Path) -> Option<Ini> {
        std::fs::read_to_string(path).ok().map(|text| Ini::parse(&text))
    }

    pub fn get(&self, section: &str, key: &str) -> Option<&str> {
        let (section, key) = (section.to_ascii_lowercase(), key.to_ascii_lowercase());
        self.entries
            .iter()
            .find(|(s, k, _)| *s == section && *k == key)
            .map(|(_, _, v)| v.as_str())
    }

    /// A whole number, the way `GetPrivateProfileInt` reads one.
    pub fn int(&self, section: &str, key: &str) -> Option<i64> {
        self.get(section, key)?.trim().parse().ok()
    }

    /// One binding: the file's value if it has one and it fits in a scan
    /// code, the engine's default otherwise.
    pub fn binding(&self, key: &str) -> Option<u8> {
        let default = CONTROL_DEFAULTS.iter().find(|(n, _)| *n == key).map(|(_, v)| *v);
        match self.int(CONTROL, key) {
            Some(v) if (0..=255).contains(&v) => Some(v as u8),
            _ => default,
        }
    }
}

/// The engine's default for a binding, for a game with no `.INI` at all.
pub fn default_binding(key: &str) -> Option<u8> {
    CONTROL_DEFAULTS.iter().find(|(n, _)| *n == key).map(|(_, v)| *v)
}
