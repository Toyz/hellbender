//! Playing a recorded demo back the way `0x44d180` does.

use hb_formats::demo::{Demo, Player};
use hb_formats::fixed::to_units;

/// Two poses a second apart, the heading crossing zero, the fire button
/// held on the second, and a key pressed between them.
fn recording() -> Demo {
    let text = "4\r\ntest.lvl\r\n\
                0,0\r\n0,0,0\r\n0,0,65000,0\r\n\
                1,32768\r\n2\r\n\
                0,65536\r\n65536,0,-65536\r\n0,0,1000,1\r\n\
                0,131072\r\n131072,0,0\r\n0,0,1000,0\r\n";
    Demo::parse(text.as_bytes()).unwrap()
}

#[test]
fn angles_go_the_short_way_round() {
    let demo = recording();
    // Halfway between 65,000 and 1,000 the short way is past zero, not back
    // through half a turn.
    let half = demo.pose_at(0.5).unwrap();
    assert_eq!(half.angles[2], (65000 + (1000 + 65536 - 65000) / 2) & 0xffff);
    assert_eq!(half.x, 32768);
    assert_eq!(half.z, -32768);
}

#[test]
fn the_trigger_is_the_next_poses_and_the_keys_come_a_pose_late() {
    let demo = recording();
    let mut player = Player::default();
    let mut frames = Vec::new();
    while let Some(frame) = demo.play(&mut player) {
        frames.push((player.clock, frame.fire, frame.keys.clone()));
        player.advance(1.0 / 4.0);
    }
    // Played toward the second pose, whose fourth value is 1, the trigger is
    // held; toward the third it is let go.
    assert!(frames.iter().filter(|f| to_units(f.0) < 1.0).all(|f| f.1));
    assert!(frames.iter().filter(|f| to_units(f.0) >= 1.0).all(|f| !f.1));
    // The key at half a second is pressed once, when the pose played toward
    // moves on past the one after it.
    let pressed: Vec<f32> = frames.iter().filter(|f| f.2 == [2]).map(|f| to_units(f.0)).collect();
    assert_eq!(pressed, [1.0]);
    // And after the last pose there is nothing: "Demo Play Done".
    assert_eq!(to_units(player.clock), 2.0);
}

#[test]
fn a_shipped_demo_plays_smoothly() {
    let Some(pod) = hb_pod::game_pod("STARTUP.POD") else { return };
    let demo = Demo::parse(pod.read("demo", "demo1.dmo").unwrap()).unwrap();
    let mut player = Player::default();
    let mut last: Option<hb_formats::demo::Pose> = None;
    let (mut frames, mut worst_turn) = (0, 0);
    while let Some(frame) = demo.play(&mut player) {
        if let Some(was) = last {
            for k in 0..3 {
                let d = ((frame.pose.angles[k] - was.angles[k]) as i16).unsigned_abs();
                worst_turn = worst_turn.max(d);
            }
        }
        last = Some(frame.pose);
        frames += 1;
        player.advance(1.0 / 60.0);
    }
    println!("{frames} frames, worst turn in one frame {worst_turn}");
    assert!(frames as f32 > demo.seconds() * 59.0);
    // At sixty frames a second nothing turns more than a sixteenth of a
    // circle between frames.
    assert!(worst_turn < 4096, "{worst_turn}");
}
