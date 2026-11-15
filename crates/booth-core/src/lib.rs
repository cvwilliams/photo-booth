//! Browser-free image processing for the photo booth.
//!
//! Everything here works on tightly packed RGBA8 buffers (`width * height * 4`
//! bytes), so it can be unit-tested with plain `cargo test` and reused from Wasm.

pub mod animate;
pub mod filters;
pub mod layout;

pub use animate::{GifError, GifWriter, downscale};
pub use filters::Filter;
pub use layout::{StripLayout, compose_strip};
