#![allow(
    unused_crate_dependencies,
    reason = "standalone example inherits the package-wide dev dependency graph"
)]
//! Render one Unicode scalar with the compact API.

use std::error::Error;
use std::io;

use fontdone::Font;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: render_mask FONT"))?;
    let data = std::fs::read(path)?;
    let font = Font::truetype(&data, 16.0)?;
    let mask = font.getmask("A")?;
    println!(
        "{}x{} coverage bytes={}, origin=({}, {}), advance={}px",
        mask.width,
        mask.height,
        mask.pixels.len(),
        mask.xmin,
        mask.ymin,
        mask.advance_width
    );
    Ok(())
}
