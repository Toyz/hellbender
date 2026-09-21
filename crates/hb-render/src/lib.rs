//! A software rasteriser for Hellbender's world, in 8-bit indexed colour.
//!
//! This is the port's renderer, not a transcription of the original's. The
//! original's inner loops have not been read; what is reproduced here is
//! everything the data dictates - the palette, the shade and fog ramps, the
//! cell geometry, the texture indexing - so that the output is in the same
//! colour space and the same world units as the game's. Where a choice is the
//! renderer's own rather than the engine's, the comment says so.

pub mod camera;
pub mod level;
pub mod raster;
pub mod hud;
pub mod scene;

pub use camera::Camera;
pub use level::Level;
pub use raster::Target;
pub use scene::draw_world;
