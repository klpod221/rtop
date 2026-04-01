//! rust-embed wrapper for the Vue 3 frontend dist assets.
//!
//! The `web/dist/` folder is produced by `npm run build` inside the `web/` directory.
//! Run `cd ../../web && npm run build` before compiling this crate for production.

use rust_embed::Embed;

/// Embeds everything from the built Vue 3 frontend at compile time.
/// Path is relative to this crate's Cargo.toml directory.
#[derive(Embed)]
#[folder = "../../web/dist"]
pub struct WebAssets;
