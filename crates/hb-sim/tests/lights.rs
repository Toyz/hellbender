//! The lamps (`0x48bd60`, `0x48ae30`, `0x48c800`, `0x48b550`).

use hb_formats::terrain::Layer;
use hb_sim::lights::{light_at, Face, Kind, Lamp, Lamps, Light, Record, State};

fn record(numbers: [i64; 8]) -> Record {
    Record { lit: Some(10), unlit: Some(11), broken: Some(12), numbers }
}

/// The commonest shipped record: 262144,90000,131072,6,1 / 32768,6,2.
const COMMON: [i64; 8] = [262_144, 90_000, 131_072, 6, 1, 32_768, 6, 2];

fn lamps(r: Record) -> Lamps {
    let face = Face { layer: Layer::ChamberCeiling, cell: (5, 5), side: 0 };
    Lamps {
        records: vec![r],
        lamps: vec![Lamp {
            record: 0,
            face,
            kind: Kind::Round,
            at: [44.0, -20.0, 44.0],
            normal: [0.0, -1.0, 0.0],
            state: State::Lit,
            clock: 0.0,
            hits: r.hits(),
        }],
    }
}

#[test]
fn the_record_reads_as_a_reach_a_strength_and_hits() {
    let r = record(COMMON);
    assert_eq!(r.reach(), 32.0, "4.0 times eight");
    assert!((r.strength() - 1.373).abs() < 0.001);
    assert_eq!((r.hits(), r.shaky()), (6, 2));
    assert_eq!(r.on_for(), 2.0);
}

#[test]
fn a_round_light_falls_off_to_nothing_at_its_reach() {
    let l = Light { kind: Kind::Round, at: [0.0; 3], reach: 32.0, strength: 0.5, normal: [0.0; 3] };
    assert!((light_at([0.0; 3], &[l]) - 0.5).abs() < 1e-6);
    assert!((light_at([16.0, 0.0, 0.0], &[l]) - 0.25).abs() < 1e-6);
    assert_eq!(light_at([33.0, 0.0, 0.0], &[l]), 0.0, "outside its box");
    // Two of them add, and the total is clamped to full.
    let bright = Light { strength: 1.373, ..l };
    assert_eq!(light_at([0.0; 3], &[bright, bright]), 1.0);
    // In the corner of its box, past its reach, it takes light away.
    let corner = [30.0, 0.0, 30.0];
    assert!(light_at(corner, &[bright, l]) < light_at(corner, &[bright]) + 1e-6);
}

#[test]
fn a_lamp_blinks_after_enough_hits_and_goes_out_after_its_last() {
    let mut l = lamps(record(COMMON));
    let face = l.lamps[0].face;
    for shot in 1..=3 {
        assert_eq!(l.shot(face), (None, false), "shot {shot}");
        assert_eq!(l.lamps[0].state, State::Lit);
    }
    // Two hits left: the eighth number, so it starts to blink.
    l.shot(face);
    assert!(matches!(l.lamps[0].state, State::Blinking { .. }));
    l.shot(face);
    let (paint, broke) = l.shot(face);
    assert!(broke);
    assert_eq!(paint.unwrap().texture, 12, "the broken texture");
    assert_eq!(l.lamps[0].state, State::Broken);
    // Shots on another cell do nothing to it.
    let mut other = lamps(record(COMMON));
    assert_eq!(other.shot(Face { cell: (6, 5), ..face }), (None, false));
    assert_eq!(other.lamps[0].hits, 6);
}

#[test]
fn a_blinking_lamp_is_on_two_seconds_and_off_a_frame() {
    let mut l = lamps(record(COMMON));
    l.lamps[0].state = State::Blinking { on: false };
    let (lights, paints) = l.step(1.0 / 30.0, (5, 5));
    assert_eq!(lights.len(), 1, "on after the briefest off");
    assert_eq!(paints[0].texture, 10, "wearing the lit texture");
    let mut frames_on = 1;
    loop {
        let (lights, paints) = l.step(1.0 / 30.0, (5, 5));
        if lights.is_empty() {
            assert_eq!(paints[0].texture, 11, "and the unlit one while off");
            break;
        }
        frames_on += 1;
    }
    assert!((60..=62).contains(&frames_on), "{frames_on}");
}

#[test]
fn only_lamps_within_ten_cells_give_light() {
    let mut l = lamps(record(COMMON));
    assert_eq!(l.step(0.03, (15, 5)).0.len(), 1);
    assert_eq!(l.step(0.03, (16, 5)).0.len(), 0);
    // Across the world's edge.
    l.lamps[0].face.cell = (1, 5);
    assert_eq!(l.step(0.03, (120, 5)).0.len(), 1);
}
