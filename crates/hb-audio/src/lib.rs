//! Hellbender's audio: RIFF `.WAV` effects and 6-channel ProTracker `.MOD`
//! music, decoded and mixed without a dependency.
//!
//! Playback to a device is somebody else's problem - this produces samples.

pub mod mixer;
pub mod modfile;
pub mod wav;

pub use mixer::Mixer;
pub use modfile::Module;
pub use wav::Wav;
