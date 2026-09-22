//! The mission, on hand-made lists and on the shipped ones.

use hb_formats::nav::{self, Data, Kind, Nav};
use hb_formats::text::Placement;
use hb_sim::mission::{self, Actor, Event, Mission, Outcome, World};
use hb_sim::turret::Rng;

fn point(kind: Kind, at: [f32; 3], data: Data) -> Nav {
    Nav {
        kind,
        position: at.map(hb_formats::fixed::from_units),
        priority: 0,
        time: 0,
        completion_sound: Some(format!("{kind:?}.wav").to_lowercase()),
        proximity_sound: None,
        text: format!("{kind:?}"),
        data,
    }
}

fn placed(x: f32, z: f32) -> Placement {
    Placement { kind: 0, hit_points: 65536, x: hb_formats::fixed::from_units(x), y: 0, z: hb_formats::fixed::from_units(z), pitch: 0, roll: 0, heading: 0 }
}

struct Actors {
    actors: Vec<Actor>,
    restored: Vec<usize>,
}

impl Actors {
    fn at(positions: &[[f32; 3]]) -> Actors {
        Actors {
            actors: positions.iter().map(|&position| Actor { position, hit_points: 1.0, max: 1.0 }).collect(),
            restored: Vec::new(),
        }
    }
}

impl World for Actors {
    fn actor(&self, index: usize) -> Option<Actor> {
        self.actors.get(index).copied()
    }
    fn restore(&mut self, index: usize) {
        self.actors[index].hit_points = self.actors[index].max;
        self.restored.push(index);
    }
}

fn flat(_: [f32; 3]) -> f32 {
    0.0
}

fn start() -> Nav {
    point(Kind::Start, [0.0, 0.0, 0.0], Data::Start { angles: [0, 0, 0x8000] })
}

fn kinds(m: &Mission) -> Vec<Kind> {
    (0..m.len()).map(|i| m.nav(i).kind).collect()
}

#[test]
fn the_loader_adds_an_end_and_fences_tunnels_and_jump_zones_with_sync_points() {
    let navs = vec![
        start(),
        point(Kind::Destroy, [0.0; 3], Data::Targets(vec![0, 1])),
        point(Kind::Checkpoint, [0.0; 3], Data::None),
        point(Kind::ExitTunnel, [0.0; 3], Data::None),
        point(Kind::JumpZone, [0.0; 3], Data::None),
    ];
    let m = Mission::new(navs, &[placed(10.0, 0.0), placed(20.0, 0.0)], flat, &mut Rng::new(1));
    use Kind::*;
    assert_eq!(kinds(&m), [Start, Destroy, Checkpoint, Sync, ExitTunnel, Sync, JumpZone, End]);
    // The added sync points are required; the added end is not.
    assert!(m.nav(3).required() && m.nav(5).required());
    assert!(!m.nav(7).required());
}

#[test]
fn the_player_starts_sixteen_units_over_the_start_facing_its_heading() {
    let navs = vec![
        point(Kind::Start, [100.0, 3.0, -50.0], Data::Start { angles: [0, 0, 0x8000] }),
        point(Kind::Checkpoint, [0.0; 3], Data::None),
    ];
    let m = Mission::new(navs, &[], |_| 7.5, &mut Rng::new(1));
    let s = m.start.unwrap();
    assert_eq!(s.position, [100.0, 23.5, -50.0]);
    assert_eq!(s.angles, [0, 0, 0x8000]);
    assert_eq!(m.current, 1);
    assert!(m.done(0));
}

#[test]
fn a_level_is_played_through_to_its_jump_zone() {
    let navs = vec![
        start(),
        point(Kind::Destroy, [0.0; 3], Data::Targets(vec![0, 1])),
        point(Kind::Checkpoint, [0.0, 0.0, 200.0], Data::None),
        point(Kind::ExitTunnel, [0.0, 0.0, 300.0], Data::None),
        point(Kind::JumpZone, [0.0, 10.0, 400.0], Data::None),
    ];
    let mut world = Actors::at(&[[10.0, 0.0, 0.0], [20.0, 0.0, 0.0]]);
    let mut m = Mission::new(navs, &[placed(10.0, 0.0), placed(20.0, 0.0)], flat, &mut Rng::new(1));
    let dt = 1.0 / 30.0;

    // The arrow follows the first target still standing.
    let events = m.step(&mut world, [0.0, 5.0, 0.0], 0, dt, flat);
    assert!(events.is_empty());
    assert_eq!(m.label, "Destroy Target");
    assert_eq!(m.distance, 10.0);
    world.actors[0].hit_points = 0.0;
    m.step(&mut world, [0.0, 5.0, 0.0], 0, dt, flat);
    assert_eq!(m.distance, 20.0);
    world.actors[1].hit_points = 0.0;
    let events = m.step(&mut world, [0.0, 5.0, 0.0], 0, dt, flat);
    assert_eq!(events, [Event::Sound("destroy.wav".into())]);
    assert_eq!(m.nav(m.current).kind, Kind::Checkpoint);

    // Forty units out is not close enough; thirty-nine is.
    m.step(&mut world, [0.0, 5.0, 160.0], 0, dt, flat);
    assert_eq!(m.nav(m.current).kind, Kind::Checkpoint);
    let events = m.step(&mut world, [0.0, 5.0, 161.0], 0, dt, flat);
    assert_eq!(events, [Event::Message("Checkpoint"), Event::Sound("checkpoint.wav".into())]);
    // The sync point before the tunnel exit was passed on the way.
    assert_eq!(m.nav(m.current).kind, Kind::ExitTunnel);

    // A tunnel exit wants the player above ground.
    m.step(&mut world, [0.0, -5.0, 300.0], 0, dt, flat);
    assert_eq!(m.nav(m.current).kind, Kind::ExitTunnel);
    let events = m.step(&mut world, [0.0, 5.0, 300.0], 0, dt, flat);
    assert_eq!(
        events,
        [
            Event::Message("Exit Tunnel"),
            Event::Sound("exittunnel.wav".into()),
            Event::Voice(mission::MISSION_COMPLETE)
        ]
    );
    assert_eq!(m.nav(m.current).kind, Kind::JumpZone);
    assert_eq!(m.label, "Exit Tunnel");

    // The jump zone: within 15 units across, and between its height and 20
    // units over it.
    m.step(&mut world, [0.0, 5.0, 400.0], 0, dt, flat);
    assert_eq!(m.outcome, None);
    assert_eq!(m.label, "Fly to Jump Zone");
    m.step(&mut world, [0.0, 31.0, 400.0], 0, dt, flat);
    assert_eq!(m.outcome, None);
    m.step(&mut world, [10.0, 20.0, 390.0], 0, dt, flat);
    assert_eq!(m.outcome, Some(Outcome::Jumped));
}

#[test]
fn the_arrow_is_the_bearing_from_the_point_to_the_player_less_the_heading() {
    let navs = vec![start(), point(Kind::Checkpoint, [0.0, 0.0, 100.0], Data::None)];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[]);
    // The point is due +z of the player, so the player is due -z of it.
    m.step(&mut world, [0.0, 0.0, 0.0], 0, 0.0, flat);
    assert_eq!(m.arrow, 0x8000);
    m.step(&mut world, [0.0, 0.0, 0.0], 0x4000, 0.0, flat);
    assert_eq!(m.arrow, 0x4000);
    assert!(!m.near);
    m.step(&mut world, [0.0, 0.0, 41.0], 0, 0.0, flat);
    assert!(m.near);
    // Distances wrap with the world.
    m.step(&mut world, [0.0, 0.0, -1000.0], 0, 0.0, flat);
    assert!((m.distance - 76.0).abs() < 1e-3, "{}", m.distance);
}

#[test]
fn every_required_point_done_completes_a_level_without_a_jump_zone() {
    let navs = vec![start(), point(Kind::Checkpoint, [0.0; 3], Data::None)];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[]);
    m.step(&mut world, [0.0, 0.0, 100.0], 0, 0.1, flat);
    assert_eq!(m.outcome, None);
    m.step(&mut world, [0.0, 0.0, 0.0], 0, 0.1, flat);
    assert_eq!(m.outcome, Some(Outcome::Complete));
}

#[test]
fn a_timed_point_counts_down_aloud_and_fails_at_zero() {
    let mut timed = point(Kind::Checkpoint, [0.0, 0.0, 500.0], Data::None);
    timed.time = 21 << 16;
    let navs = vec![start(), timed];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[]);
    let mut heard = Vec::new();
    for _ in 0..(22 * 30) {
        for e in m.step(&mut world, [0.0; 3], 0, 1.0 / 30.0, flat) {
            heard.push(e);
        }
    }
    let mut want = vec![Event::Sound("20-sec.wav".into())];
    want.extend(mission::COUNTDOWN.iter().map(|&v| Event::Voice(v)));
    assert_eq!(heard, want);
    assert_eq!(m.outcome, Some(Outcome::Failed));
}

#[test]
fn a_guardian_is_kept_whole_while_its_shields_stand() {
    let guardian = Data::Guardian { actor: 0, music: "boss.mod".into(), shields: vec![1, 2] };
    let navs = vec![
        start(),
        point(Kind::Checkpoint, [0.0; 3], Data::None),
        point(Kind::Guardian, [0.0; 3], guardian),
    ];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[[0.0, 0.0, 50.0], [10.0, 0.0, 0.0], [20.0, 0.0, 0.0]]);
    let events = m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(
        events,
        [
            Event::Message("Checkpoint"),
            Event::Sound("checkpoint.wav".into()),
            Event::Music("boss.mod".into()),
            Event::Sound("warning.wav".into()),
            Event::Message("Mission Goal Ahead!")
        ]
    );

    // The arrow is on the last shield standing, and so is the readout.
    world.actors[0].hit_points = 0.25;
    world.actors[2].hit_points = 0.5;
    m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(m.distance, 20.0);
    assert_eq!(m.label, "Guardian: 50%");
    assert_eq!(world.actors[0].hit_points, 1.0);
    assert_eq!(world.restored, [0]);

    world.actors[1].hit_points = 0.0;
    world.actors[2].hit_points = 0.0;
    world.actors[0].hit_points = 0.75;
    m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(m.distance, 50.0);
    assert_eq!(m.label, "Guardian: 75%");

    world.actors[0].hit_points = 0.0;
    let events = m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(
        events,
        [Event::Message("Guardian Destroyed"), Event::MusicBack, Event::Sound("guardian.wav".into())]
    );
    assert_eq!(m.outcome, Some(Outcome::Complete));
}

#[test]
fn a_rescue_beacon_counts_within_eight_units_on_the_same_side_of_the_ground() {
    let navs = vec![start(), point(Kind::DropBeacon, [0.0, -20.0, 0.0], Data::None)];
    let mut m = Mission::new(navs, &[], |p| p[1].min(-20.0), &mut Rng::new(1));
    let mut world = Actors::at(&[]);
    // Above ground, over the prison: the wrong layer.
    assert_eq!(m.drop_beacon([0.0, 10.0, 0.0]), [Event::Message("Beacon launched")]);
    m.step(&mut world, [0.0, 10.0, 0.0], 0, 0.1, flat);
    assert!(!m.done(1));
    // Underground, but nine units off.
    m.drop_beacon([9.0, -10.0, 0.0]);
    m.step(&mut world, [0.0, 10.0, 0.0], 0, 0.1, flat);
    assert!(!m.done(1));
    m.drop_beacon([7.0, -10.0, -7.0]);
    let events = m.step(&mut world, [0.0, 10.0, 0.0], 0, 0.1, flat);
    assert_eq!(events, [Event::Sound("dropbeacon.wav".into())]);
    assert!(m.done(1));
}

#[test]
fn the_eleventh_beacon_replaces_the_oldest() {
    let navs = vec![start(), point(Kind::Checkpoint, [0.0, 0.0, 500.0], Data::None)];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[]);
    let before = m.len();
    for i in 0..11 {
        assert_eq!(m.drop_beacon([i as f32, 0.0, 0.0]), [Event::Voice(mission::BEACON_LAUNCHED)]);
        m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    }
    assert_eq!(m.len(), before + 10);
    let xs: Vec<i32> = (before..m.len()).map(|i| m.nav(i).position[0] >> 16).collect();
    assert_eq!(xs, [10, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn three_friendlies_lost_fail_the_mission() {
    let mut m = Mission::new(vec![start(), point(Kind::Checkpoint, [0.0; 3], Data::None)], &[], flat, &mut Rng::new(1));
    m.friendly_lost();
    m.friendly_lost();
    assert_eq!(m.outcome, None);
    m.friendly_lost();
    assert_eq!(m.outcome, Some(Outcome::Failed));
}

/// Every shipped mission loads within the engine's 50 records, starts at its
/// start, and every sync point the loader adds lands where the files already
/// put one - the editor wrote "Sync point: auto added" into them.
#[test]
fn every_shipped_mission_already_carries_its_sync_points() {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return };
    let mut levels = 0;
    for e in pod.entries().iter().filter(|e| e.ext() == "lvl") {
        let level = hb_formats::lvl::Level::parse(pod.bytes(e)).unwrap();
        let (dir, name) = level.slot("navigation").unwrap();
        let navs = nav::navs(pod.read(dir, name).unwrap()).unwrap();
        let def = pod.read("data", &format!("{}.def", level.stem())).unwrap();
        let placements = hb_formats::text::placements(def).unwrap();
        let count = navs.len();
        let m = Mission::new(navs, &placements, flat, &mut Rng::new(1));
        assert_eq!(m.len(), count, "{name}");
        assert!(m.start.is_some(), "{name}");
        levels += 1;
    }
    assert_eq!(levels, 26);
}

#[test]
fn a_kill_point_ends_the_level_when_its_target_falls() {
    let navs = vec![
        start(),
        point(Kind::Kill, [0.0; 3], Data::Actor(0)),
        point(Kind::JumpZone, [0.0, 0.0, 300.0], Data::None),
    ];
    let mut m = Mission::new(navs, &[], flat, &mut Rng::new(1));
    let mut world = Actors::at(&[[0.0, 0.0, 50.0]]);
    m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(m.label, "Destroy Target");
    assert_eq!(m.distance, 50.0);
    world.actors[0].hit_points = 0.0;
    let events = m.step(&mut world, [0.0; 3], 0, 0.1, flat);
    assert_eq!(events, [Event::Sound("kill.wav".into())]);
    assert_eq!(m.outcome, Some(Outcome::Jumped));
    // Left, not advanced: the jump zone after it was never current.
    assert_eq!(m.nav(m.current).kind, Kind::Kill);
}

/// Every line in the engine's phrase table names a sound that ships, and the
/// table is the size the image says.
#[test]
fn the_phrase_table_is_three_hundred_lines_that_all_have_a_sound() {
    use hb_sim::phrases::{phrase, PHRASES};
    assert_eq!(PHRASES.len(), 300);
    let missing: Vec<&str> =
        PHRASES.iter().filter(|p| p.sound.is_empty()).map(|p| p.text).collect();
    // Two of the three hundred are empty slots at the top of the table.
    assert_eq!(missing.len(), 2, "{missing:?}");
    for p in PHRASES.iter().filter(|p| !p.sound.is_empty()) {
        assert!(p.sound.to_ascii_lowercase().ends_with(".wav"), "{}", p.sound);
        // The second sound, where there is one, is a sound too - usually
        // `pause.wav`, but not always.
        assert!(p.also.is_empty() || p.also.ends_with(".wav"), "{}", p.also);
        // Two, four or five seconds, in 16.16.
        assert!([131072, 262144, 327680].contains(&p.seconds), "{}", p.seconds);
    }
    // The ones the port already had by hand line up with the table.
    assert_eq!(phrase(0x3c).unwrap().sound, "objcomp.wav");
    // And the line Toyz went looking for.
    assert_eq!(phrase(47).unwrap().text, "Troop transport destroyed.");
}

/// Every sound the phrase table names is in the archive.
#[test]
fn every_phrase_names_a_sound_that_ships() {
    let Some(pod) = hb_pod::game_pod("STARTUP.POD") else { return };
    let mut missing: Vec<String> = Vec::new();
    for p in hb_sim::phrases::PHRASES {
        for name in [p.sound, p.also] {
            if name.is_empty() {
                continue;
            }
            let file = name.to_ascii_lowercase();
            if pod.find("sound", &file).is_none() {
                missing.push(file);
            }
        }
    }
    missing.sort();
    missing.dedup();
    assert!(missing.is_empty(), "the table names sounds that did not ship: {missing:?}");
}

/// The words that go with a sound the level data names are in the phrase
/// table, found by that file name - which is how a `.NAV` point and a `.DEF`
/// type say anything at all.
#[test]
fn the_level_datas_sounds_have_words_in_the_table() {
    use hb_sim::phrases::by_sound;
    assert_eq!(by_sound("comm-des.wav").unwrap().text, "Bion commando transports destroyed.");
    assert_eq!(by_sound("trp-des.wav").unwrap().text, "Troop transport destroyed.");
    assert_eq!(by_sound("COMM-DES.WAV").unwrap().text, "Bion commando transports destroyed.");
    assert!(by_sound("not-a-sound.wav").is_none());
}

/// And every sound the shipped missions name is in there - all 432 of them.
#[test]
fn everything_the_missions_name_has_words() {
    let Some(pod) = hb_pod::game_pod("GAME.POD") else { return };
    let (mut named, mut spoken) = (0, 0);
    for e in pod.entries().iter().filter(|e| e.ext() == "nav") {
        for n in nav::navs(pod.bytes(e)).unwrap() {
            for sound in [&n.completion_sound, &n.proximity_sound].into_iter().flatten() {
                named += 1;
                if hb_sim::phrases::by_sound(sound).is_some() {
                    spoken += 1;
                }
            }
        }
    }
    assert!(named > 400, "only {named} sounds named");
    // Every one of them, in all twenty three levels.
    assert_eq!(spoken, named, "{spoken} of {named} have words");
}
