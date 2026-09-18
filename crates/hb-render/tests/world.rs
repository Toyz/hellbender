//! Whole frames of a real level. These need the game and skip without it.

use hb_formats::Angle;
use hb_render::{Camera, Level, Target};

fn level(stem: &str) -> Option<Level> {
    let dir = std::env::var_os("HB_GAME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../original"));
    let game = dir.join("system/GAME.POD");
    if !game.exists() {
        eprintln!("skipping: {} is not there", game.display());
        return None;
    }
    let game = hb_pod::Pod::open(game).unwrap();
    let startup = hb_pod::Pod::open(dir.join("system/STARTUP.POD")).ok();
    Some(Level::load(&game, startup.as_ref(), stem).unwrap())
}

fn frame(level: &Level, x: i32, y: i32, z: i32) -> (Vec<u8>, hb_render::scene::Drawn) {
    let (w, h) = Target::MODE_200;
    let mut target = Target::new(w, h);
    target.clear(0);
    let mut camera = Camera::looking_at(x, y, z, Angle(0));
    camera.pitch = Angle(3000);
    let drawn = hb_render::draw_world(&mut target, &level.scene(), &camera);
    (target.colour, drawn)
}

/// The same place in the wrapping world, reached from either side, is the
/// same picture - and the ground is in it. Until this held, a camera at a
/// negative coordinate drew the terrain 1,024 units away and saw nothing.
#[test]
fn a_view_is_the_same_from_either_side_of_the_wrap() {
    let Some(level) = level("hoth") else { return };
    // The HOTH radar base's spike gun, from 40 units south and 13 above:
    // negative in both x and z.
    let (x, y, z) = (-172 << 16, 113 << 16, -260 << 16);
    let (signed, drawn) = frame(&level, x, y, z);
    let (unwrapped, _) = frame(&level, x + (1024 << 16), y, z + (1024 << 16));
    let differ = signed.iter().zip(&unwrapped).filter(|(a, b)| a != b).count();
    assert_eq!(differ, 0, "the two copies of the place disagree on {differ} pixels");
    assert!(drawn.ground > 1000 && drawn.models > 50, "{drawn:?}");
    // Below the horizon something was drawn nearly everywhere.
    let (w, h) = Target::MODE_200;
    let bottom = &signed[w * h * 3 / 4..];
    let empty = bottom.iter().filter(|&&c| c == 0).count();
    assert!(empty * 20 < bottom.len(), "{empty} of {} pixels empty", bottom.len());
}

/// The eight ways a square texture can lie on a cell, as corner coordinates
/// for a (x, z), b (x+1, z), c (x+1, z+1), d (x, z+1): four turns of the
/// engine's base, then the same four mirrored in u.
fn symmetries(lo: f32, hi: f32) -> Vec<[(f32, f32); 4]> {
    let base = [(lo, hi), (hi, hi), (hi, lo), (lo, lo)];
    (0..8)
        .map(|k| {
            let r = k % 4;
            let mut uv = [base[r], base[(r + 1) & 3], base[(r + 2) & 3], base[(r + 3) & 3]];
            if k >= 4 {
                uv.swap(0, 1);
                uv.swap(3, 2);
            }
            uv
        })
        .collect()
}

/// Mean colour difference, texel for texel, across the shared edges of
/// neighbouring ground cells that both have orientation code 0 and different
/// textures - every cell laid the same one of the eight ways.
fn untouched_seams(level: &Level, uv: [(f32, f32); 4]) -> f64 {
    use hb_formats::terrain::{TextureRef, SIDE};
    let colour = &level.terrain.colour;
    let texel = |word: TextureRef, p: (f32, f32)| -> Option<[i32; 3]> {
        let img = level.textures.get(word.index() as usize)?.as_ref()?;
        let (w, h) = (img.shape.width, img.shape.height);
        let u = ((p.0 * w as f32 / 256.0) as usize).min(w - 1);
        let v = ((p.1 * h as f32 / 256.0) as usize).min(h - 1);
        let [r, g, b] = level.palette.rgb(img.pixels[v * w + u]);
        Some([r as i32, g as i32, b as i32])
    };
    let lerp = |p: (f32, f32), q: (f32, f32), t: f32| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
    let [a, b, c, d] = uv;
    let (mut total, mut n) = (0f64, 0usize);
    for z in 0..SIDE as i32 {
        for x in 0..SIDE as i32 {
            let here = TextureRef(colour.at(x, z, 0));
            for (dx, dz) in [(1, 0), (0, 1)] {
                let there = TextureRef(colour.at(x + dx, z + dz, 0));
                if here.orientation() != 0 || there.orientation() != 0 || here == there {
                    continue;
                }
                // Here's far edge against there's near edge, point for point.
                let (mine, theirs) = if dx == 1 { ((b, c), (a, d)) } else { ((d, c), (a, b)) };
                for i in 0..16 {
                    let t = (i as f32 + 0.5) / 16.0;
                    if let (Some(p), Some(q)) =
                        (texel(here, lerp(mine.0, mine.1, t)), texel(there, lerp(theirs.0, theirs.1, t)))
                    {
                        total += ((p[0] - q[0]).abs() + (p[1] - q[1]).abs() + (p[2] - q[2]).abs()) as f64;
                        n += 1;
                    }
                }
            }
        }
    }
    total / n.max(1) as f64
}

/// The engine lays a texture with u along x and v **against** z (`0x413c20`
/// gives corner (x, z) the coordinates (lo, hi)). Neighbouring tiles meet
/// best that way, by a wide margin, against every other way a square can lie.
#[test]
fn untouched_tiles_meet_best_the_engines_way_round() {
    for stem in ["hoth", "kreash", "jurasic"] {
        let Some(level) = level(stem) else { return };
        let scores: Vec<f64> =
            symmetries(0.5, 255.5).into_iter().map(|uv| untouched_seams(&level, uv)).collect();
        let others = scores[1..].iter().copied().fold(f64::MAX, f64::min);
        println!("{stem}: engine's way {:.1}, best other {:.1}", scores[0], others);
        assert!(scores[0] * 1.15 < others, "{stem}: {scores:?}");
    }
}
