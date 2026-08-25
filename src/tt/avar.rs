//! OpenType `avar` axis-normalization maps.
//!
//! FreeType first normalizes design coordinates against the `fvar` axis
//! limits, then applies the per-axis piecewise-linear maps in `avar`. The
//! parsed values are kept in 16.16 form while interpolating so the arithmetic
//! follows the `FT_MulDiv` path used by the pinned C runtime.

use crate::error::FontError;

#[derive(Debug, Clone)]
pub struct AvarTable {
    axis_maps: Vec<Vec<AvarPair>>,
}

#[derive(Debug, Clone, Copy)]
struct AvarPair {
    from: i32,
    to: i32,
}

impl AvarTable {
    /// Apply the table's maps to normalized 2.14 coordinates.
    pub fn map_normalized(&self, normalized: &mut [i16]) {
        for (coordinate, pairs) in normalized.iter_mut().zip(&self.axis_maps) {
            let value = i32::from(*coordinate) << 2;
            let mapped = map_coordinate(value, pairs);
            *coordinate = (mapped >> 2).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16;
        }
    }

    /// Apply the same maps while retaining FreeType's normalized 16.16
    /// precision for gvar tuple scalar calculations.
    pub fn map_normalized_fixed(&self, normalized: &mut [i32]) {
        for (coordinate, pairs) in normalized.iter_mut().zip(&self.axis_maps) {
            *coordinate = map_coordinate(*coordinate, pairs).clamp(-0x1_0000, 0x1_0000);
        }
    }
}

/// Parse an OpenType version 1 `avar` table.
///
/// The table is optional in FreeType. Callers should discard an error and
/// continue without the map, matching the optional-table loading path.
pub fn parse_avar(data: &[u8], axis_count: usize) -> Result<AvarTable, FontError> {
    if data.len() < 8 {
        return Err(FontError::InvalidFont("avar table too short".into()));
    }
    let version = read_u32(data, 0)?;
    if version != 0x0001_0000 {
        return Err(FontError::InvalidFont("unsupported avar version".into()));
    }
    let table_axis_count = usize::try_from(read_u32(data, 4)?)
        .map_err(|_| FontError::InvalidFont("avar axis count overflow".into()))?;
    if table_axis_count != axis_count {
        return Err(FontError::InvalidFont(
            "avar and fvar axis counts differ".into(),
        ));
    }

    let mut offset = 8usize;
    let mut axis_maps = Vec::with_capacity(axis_count);
    for _ in 0..axis_count {
        let pair_count = usize::from(read_u16(data, offset)?);
        offset = offset
            .checked_add(2)
            .ok_or_else(|| FontError::InvalidFont("avar offset overflow".into()))?;
        let pair_bytes = pair_count
            .checked_mul(4)
            .ok_or_else(|| FontError::InvalidFont("avar pair count overflow".into()))?;
        let end = offset
            .checked_add(pair_bytes)
            .ok_or_else(|| FontError::InvalidFont("avar table length overflow".into()))?;
        if end > data.len() {
            return Err(FontError::InvalidFont("avar segment map truncated".into()));
        }

        let mut pairs = Vec::with_capacity(pair_count);
        for _ in 0..pair_count {
            let from = i32::from(read_i16(data, offset)?) << 2;
            let to = i32::from(read_i16(data, offset + 2)?) << 2;
            pairs.push(AvarPair { from, to });
            offset += 4;
        }
        axis_maps.push(pairs);
    }

    Ok(AvarTable { axis_maps })
}

fn map_coordinate(value: i32, pairs: &[AvarPair]) -> i32 {
    for pair_index in 1..pairs.len() {
        let upper = pairs[pair_index];
        if value < upper.from {
            let lower = pairs[pair_index - 1];
            return crate::fixed::ft_mul_div(
                value - lower.from,
                upper.to - lower.to,
                upper.from - lower.from,
            )
            .wrapping_add(lower.to);
        }
    }
    value
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("avar u16 out of range".into()))?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_i16(data: &[u8], offset: usize) -> Result<i16, FontError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| FontError::InvalidFont("avar i16 out of range".into()))?;
    Ok(i16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| FontError::InvalidFont("avar u32 out of range".into()))?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
