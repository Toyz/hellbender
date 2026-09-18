//! The world as the engine sees it: a wrapping 128 x 128 grid of cells, each
//! split into two triangles whose diagonal alternates with the cell's parity.
//!
//! Everything here is transcribed from `HELLBEND.EXE`, not invented:
//! `groundTriangleMidpoint` at `0x428900`, `groundTriangleInt` at `0x428ac0`
//! and `heightAtGrid` at `0x428c40`.

pub mod grid;

pub use grid::{Cell, Corner, Diagonal, Grid, Half, Triangle};
pub use hb_formats::terrain::{BoxFace, Layer};
