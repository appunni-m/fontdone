//! Pure-Rust font loading, hinting, metrics, outlines, and rasterization.
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use fontdone::Font;
//! let data = std::fs::read("font.ttf")?;
//! let font = Font::truetype(&data, 12.0)?;
//! let mask = font.getmask("A")?;
//! # Ok(())
//! # }
//! ```
//!
//! The FreeType-style facade exposes the measured compatibility surface:
//!
//! ```rust,no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use fontdone::ffi::*;
//! let data = std::fs::read("font.ttf")?;
//! let library = FT_Init_FreeType();
//! let face = FT_New_Memory_Face(&library, &data, 0, 20.0).expect("font face should open");
//! let left = FT_Get_Char_Index(&face, 'A' as FT_ULong);
//! let right = FT_Get_Char_Index(&face, 'V' as FT_ULong);
//! let slot = FT_Load_Glyph(&face, left, FT_LOAD_RENDER | FT_LOAD_TARGET_NORMAL)
//!     .expect("glyph should render");
//! let mut kerning = FT_Vector::default();
//! let error = FT_Get_Kerning(
//!     Some(&face),
//!     left,
//!     right,
//!     FT_KERNING_DEFAULT as FT_UInt,
//!     Some(&mut kerning),
//! );
//! assert_eq!(error, FT_Err_Ok as FT_Error);
//! assert!(slot.bitmap.is_some());
//! assert_eq!(FT_Done_Face(Some(face)), FT_Err_Ok as FT_Error);
//! assert_eq!(FT_Done_FreeType(Some(library)), FT_Err_Ok as FT_Error);
//! # Ok(())
//! # }
//! ```
//!
//! The runtime is implemented in Rust. Pinned FreeType C is used only by
//! offline repository tooling for comparison and never by this crate.
//!
//! # Architecture
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`font`] | Compact font, mask, bbox, and metric API |
//! | [`api`] | Safe face/library/glyph API |
//! | [`ffi`] | FreeType-shaped safe Rust compatibility facade |
//! | [`tt`] | TrueType parsing and bytecode execution |
//! | [`tables`] | SFNT and format-specific table support |
//! | [`autohint`] | Script classification and auto-hinting infrastructure |
//! | [`scaler`] | Outline scaling and geometry |
//! | [`render`] and [`grays`] | Render modes, bitmap metadata, and rasterization |
//! | [`fixed`] | FreeType-compatible fixed-point arithmetic |

#![deny(unsafe_code)]
#![deny(missing_docs)]
// 26.6 fixed-point arithmetic uses infallible cast wrappers from casts.rs.
// The single remaining allow (arithmetic_side_effects) covers 579 sites
// of i32 +/×/- operations inherent to the 26.6 domain. See casts.rs for why
// wrapping_add/saturating_add are incorrect alternatives.
#![allow(
    clippy::arithmetic_side_effects,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::if_same_then_else
)]
// Repository integration tests use the package-wide development dependencies.
#![cfg_attr(test, allow(unused_crate_dependencies))]
// Parity instrumentation reaches internal helpers through integration-only
// routes that the library target cannot observe during dead-code analysis.
#![allow(dead_code)]

pub mod api;
#[allow(
    missing_docs,
    reason = "internal-public parity engine; downstream surface is re-exported from api/font/render"
)]
pub mod autohint;
#[allow(
    missing_docs,
    reason = "internal-public checked-cast helpers, not a downstream integration surface"
)]
pub mod casts;
pub mod error;
pub mod ffi;
#[allow(
    missing_docs,
    reason = "internal-public fixed-point parity helpers with FreeType-compatible names"
)]
pub mod fixed;
pub mod font;
#[allow(
    missing_docs,
    reason = "internal-public rasterizer implementation, not a downstream integration surface"
)]
pub mod grays;
#[allow(
    missing_docs,
    reason = "internal-public outline implementation, not a downstream integration surface"
)]
pub mod outline;
mod pfr;
pub mod render;
#[allow(
    missing_docs,
    reason = "internal-public scaler implementation, not a downstream integration surface"
)]
pub mod scaler;
#[allow(
    missing_docs,
    reason = "internal-public parsed-table implementation, not a downstream integration surface"
)]
pub mod tables;
#[allow(
    missing_docs,
    reason = "internal-public TrueType implementation, not a downstream integration surface"
)]
pub mod tt;

pub use api::{Face, GlyphFormat, GlyphSlot, Library, LoadFlags, Vector};
pub use error::FontError;
pub use font::{
    BBox, CharmapInfo, FaceInfo, Font, GlyphMask, LoadMode, SfntTableInfo, SizeMetrics,
};
pub use render::{PixelMode, RenderMode, RenderedBitmap};
