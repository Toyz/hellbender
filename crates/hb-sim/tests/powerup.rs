//! Powerups: their effects, drops, pickup, and the shipped models.

use hb_formats::nav::{Data, Kind, Nav};
use hb_formats::text::EnemyDef;
use hb_sim::combat::Pilot;
use hb_sim::mission::{Actor, Mission, World};
use hb_sim::powerup::{self, collect, drop_for, Event, Field, Stores, KINDS};
use hb_sim::turret::Rng;

fn voices(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Voice(v) => Some(v.text),
            _ => None,
        })
        .collect()
}

#[test]
fn hull_repairs_refuse_a_ship_that_needs_none() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    let mut said = Vec::new();
    // Full: every repair says so and stays where it is.
    for kind in [11, 16, 17, 18] {
        assert!(!collect(kind, &mut pilot, &mut stores, &mut said), "kind {kind}");
    }
    assert_eq!(voices(&said).len(), 4);
    assert!(voices(&said).iter().all(|v| v.starts_with("Hull undamaged")));

    // 16 to 18 refuse above 0xea60, about 91.6%; 11 only when all but full.
    pilot.health = 0.92;
    let mut said = Vec::new();
    assert!(!collect(16, &mut pilot, &mut stores, &mut said));
    assert!(collect(11, &mut pilot, &mut stores, &mut said));
    assert_eq!(pilot.health, 1.0);
    assert_eq!(voices(&said), ["Hull undamaged.  Repairs not required.", "Hull fully repaired"]);

    pilot.health = 0.25;
    let mut said = Vec::new();
    assert!(collect(16, &mut pilot, &mut stores, &mut said));
    assert!((pilot.health - 0.5).abs() < 1e-4);
    assert!(collect(17, &mut pilot, &mut stores, &mut said));
    assert!((pilot.health - 1.0).abs() < 1e-4);
    assert_eq!(
        said,
        [
            Event::Voice(powerup::HULL_PARTLY),
            Event::Voice(powerup::HULL_RESTORED),
            Event::Message("50% Damage Repair"),
            Event::Sound("power-1.wav"),
        ]
    );
}

#[test]
fn energy_tops_up_to_full_and_no_further() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    assert_eq!(stores.energy, 0.5);
    let mut said = Vec::new();
    assert!(collect(20, &mut pilot, &mut stores, &mut said));
    assert_eq!(stores.energy, 1.0);
    // Half full is not under half, so no boost line; and landing on full
    // exactly is not going over it, so no line for that either.
    assert!(said.is_empty(), "{said:?}");
    let mut said = Vec::new();
    assert!(!collect(19, &mut pilot, &mut stores, &mut said));
    assert_eq!(voices(&said), ["Energy not required."]);

    stores.energy = 0.25;
    let mut said = Vec::new();
    assert!(collect(19, &mut pilot, &mut stores, &mut said));
    assert_eq!(stores.energy, 0.5);
    assert_eq!(said, [Event::Message("25% Energy Boost"), Event::Voice(powerup::ENERGY_BOOST)]);

    // Over the top: capped, with the line for it.
    stores.energy = 0.875;
    let mut said = Vec::new();
    assert!(collect(19, &mut pilot, &mut stores, &mut said));
    assert_eq!(stores.energy, 1.0);
    assert_eq!(voices(&said), ["Energy boost secured.", "Main energy at maximum capacity."]);
}

#[test]
fn eight_bion_pieces_make_the_super_weapon() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    assert_eq!(stores.weapon, 23);
    let mut said = Vec::new();
    for kind in 23..30 {
        assert!(collect(kind, &mut pilot, &mut stores, &mut said));
    }
    assert_eq!(stores.weapon, 23);
    // A piece twice over counts once.
    assert!(collect(23, &mut pilot, &mut stores, &mut said));
    assert_eq!(stores.pieces, 0x7f);
    let mut said = Vec::new();
    assert!(collect(30, &mut pilot, &mut stores, &mut said));
    assert_eq!(stores.pieces, 0xff);
    assert_eq!(stores.weapon, 30);
    assert_eq!(stores.ammo[30], -1);
    assert_eq!(voices(&said), ["Bion technology captured.", "Weapon complete..."]);
}

#[test]
fn weapon_powerups_fill_their_slots() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    let mut said = Vec::new();
    for kind in [0, 2, 3, 4, 5, 9, 12, 13, 14, 15] {
        assert!(collect(kind, &mut pilot, &mut stores, &mut said));
    }
    let a = stores.ammo;
    assert_eq!([a[3], a[2], a[18], a[19], a[27], a[24], a[25], a[26], a[28]], [100, 100, 40, 45, 1, 7, 10, 1, 5]);
    assert_eq!(said.len(), 10);
}

fn dropper(chance: i32, kind: i32) -> EnemyDef {
    EnemyDef { drop_chance: chance, drop_kind: kind, ..EnemyDef::default() }
}

#[test]
fn a_destroyed_type_drops_at_its_chance() {
    let mut rng = Rng::new(3);
    let count = |def: &EnemyDef, rng: &mut Rng| (0..10_000).filter(|_| drop_for(def, rng).is_some()).count();
    assert_eq!(count(&dropper(0, 16), &mut rng), 0);
    assert_eq!(count(&dropper(100, 16), &mut rng), 10_000);
    assert_eq!(drop_for(&dropper(100, 9), &mut rng), Some(9));
    // A roll of 0 to 10 of 0 to 100 inclusive: about 11 in 100.
    let ten = count(&dropper(10, 16), &mut rng);
    assert!((1_000..1_200).contains(&ten), "{ten}");
    // -1 picks any of the 31. (A rand() of exactly 32,767 gives 31, one
    // past the table; the port drops nothing then.)
    let mut seen = [false; 31];
    for _ in 0..2_000 {
        if let Some(k) = drop_for(&dropper(100, -1), &mut rng) {
            seen[k] = true;
        }
    }
    assert!(seen.iter().all(|&s| s));
}

#[test]
fn a_powerup_is_picked_up_inside_its_size_on_every_axis() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    let mut field = Field::default();
    field.place([10.0, 0.0, 10.0], 0, 2.0, 5.0);
    // Put down twice its size over the floor.
    assert_eq!(field.items[0].position, [10.0, 9.0, 10.0]);
    let (mut touching, mut said) = (Vec::new(), Vec::new());
    assert!(field.step([12.5, 9.0, 10.0], &mut pilot, &mut stores, &mut touching, &mut said).is_empty());
    assert!(field.step([11.5, 10.5, 8.5], &mut pilot, &mut stores, &mut touching, &mut said) == [0]);
    assert!(field.items[0].taken);
    assert_eq!(voices(&said), ["RFL secured"]);
    // Taken is gone.
    let mut said = Vec::new();
    assert!(field.step([10.0, 9.0, 10.0], &mut pilot, &mut stores, &mut touching, &mut said).is_empty());
    assert!(said.is_empty());
}

#[test]
fn a_refused_powerup_speaks_once_while_it_is_touched() {
    let (mut pilot, mut stores) = (Pilot::default(), Stores::default());
    let mut field = Field::default();
    field.place([0.0, 5.0, 0.0], 18, 2.0, 0.0);
    let (mut touching, mut said) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        field.step([0.0, 5.0, 0.0], &mut pilot, &mut stores, &mut touching, &mut said);
    }
    assert_eq!(said.len(), 1);
    field.step([50.0, 5.0, 0.0], &mut pilot, &mut stores, &mut touching, &mut said);
    field.step([0.0, 5.0, 0.0], &mut pilot, &mut stores, &mut touching, &mut said);
    assert_eq!(said.len(), 2);
    assert!(!field.items[0].taken);
}

struct Nobody;

impl World for Nobody {
    fn actor(&self, _: usize) -> Option<Actor> {
        None
    }
    fn restore(&mut self, _: usize) {}
}

#[test]
fn picking_up_a_message_pod_finishes_its_mission_point() {
    let at = |kind, data| Nav {
        kind,
        position: [0; 3],
        priority: 0,
        time: 0,
        completion_sound: Some("unheard.wav".into()),
        proximity_sound: None,
        text: String::new(),
        data,
    };
    let navs = vec![
        at(Kind::Start, Data::Start { angles: [0; 3] }),
        at(Kind::MessagePod, Data::Pod(1)),
        at(Kind::Checkpoint, Data::None),
    ];
    let mut m = Mission::new(navs, &[], |_| 0.0, &mut Rng::new(1));
    m.place_pods(&[[0.0; 3], [100.0, 0.0, 0.0]]);
    let mut world = Nobody;
    m.step(&mut world, [0.0, 0.0, 500.0], 0, 0.1, |_| 0.0);
    assert_eq!(m.label, "Pick up Message Pod");
    assert!((m.distance - (100.0f32 * 100.0 + 500.0 * 500.0).sqrt()).abs() < 1e-2);
    m.pod_taken(0);
    m.step(&mut world, [0.0, 0.0, 500.0], 0, 0.1, |_| 0.0);
    assert_eq!(m.nav(m.current).kind, Kind::MessagePod);
    m.pod_taken(1);
    let events = m.step(&mut world, [0.0, 0.0, 500.0], 0, 0.1, |_| 0.0);
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(m.nav(m.current).kind, Kind::Checkpoint);
}

fn game_dir() -> std::path::PathBuf {
    std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"))
}

/// Every kind's model is in `STARTUP.POD`, carries its unit, and comes out
/// a couple of units across.
#[test]
fn every_powerup_model_is_shipped_and_small() {
    let path = game_dir().join("system/STARTUP.POD");
    if !path.exists() {
        eprintln!("skipping: {} is not there", path.display());
        return;
    }
    let pod = hb_pod::Pod::open(&path).unwrap();
    let mut sizes = Vec::new();
    for (name, model) in KINDS {
        let bytes = pod.read("models", model).unwrap_or_else(|e| panic!("{name}: {model}: {e}"));
        let m = hb_formats::mrgl::Model::parse(bytes).unwrap();
        assert!(m.unit.is_some_and(|u| u > 0), "{model}");
        sizes.push(powerup::size_of(&m));
    }
    // The f6 family is one size.
    assert_eq!(KINDS[16].1, "f6dam1.bin");
    assert!((sizes[16] - 2.60).abs() < 0.01, "{}", sizes[16]);
    for s in &sizes {
        assert!((1.0..6.0).contains(s), "{sizes:?}");
    }
}
