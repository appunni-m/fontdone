#![allow(
    unused_crate_dependencies,
    reason = "standalone example inherits the package-wide dev dependency graph"
)]
//! Handle malformed input without panicking.

use std::error::Error;

use fontdone::{Font, FontError};

fn main() -> Result<(), Box<dyn Error>> {
    match Font::truetype(b"not a font", 16.0) {
        Err(FontError::InvalidFont(message)) => {
            println!("font rejected: {message}");
            Ok(())
        }
        Err(other) => Err(other.into()),
        Ok(_) => Err("invalid bytes were unexpectedly accepted".into()),
    }
}
