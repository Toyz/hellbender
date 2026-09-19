//! The simulation: things in the world that move.
//!
//! This starts with the one behaviour whose shape has been read out of
//! `HELLBEND.EXE` - an actor following a `.CRS` course - and grows from there.
//! What is transcribed and what is this crate's own choice is said at each
//! step, the same way `hb-render` does.

pub mod combat;
pub mod course;
pub mod flight;
pub mod flyer;
pub mod mission;
pub mod powerup;
pub mod turret;
pub mod weapons;

pub use course::{Follower, Phase};
