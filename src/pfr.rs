//! Compact PFR metadata parser used by the public PFR metrics service.
//!
//! The runtime only retains fields required by `FT_Get_PFR_Metrics`,
//! `FT_Get_PFR_Advance`, and `FT_Get_PFR_Kerning`.  Pinned FreeType remains
//! the offline oracle for the format and public behavior.

use crate::font::BBox;

const PFR_HEADER_SIZE: usize = 58;
const PFR_LOG_STROKE: u8 = 0x04;
const PFR_LOG_2BYTE_STROKE: u8 = 0x08;
const PFR_LOG_BOLD: u8 = 0x10;
const PFR_LOG_2BYTE_BOLD: u8 = 0x20;
const PFR_LOG_EXTRA_ITEMS: u8 = 0x40;
const PFR_LINE_JOIN_MASK: u8 = 0x03;

const PFR_PHY_VERTICAL: u8 = 0x01;
const PFR_PHY_2BYTE_CHARCODE: u8 = 0x02;
const PFR_PHY_PROPORTIONAL: u8 = 0x04;
const PFR_PHY_ASCII_CODE: u8 = 0x08;
const PFR_PHY_2BYTE_GPS_SIZE: u8 = 0x10;
const PFR_PHY_3BYTE_GPS_OFFSET: u8 = 0x20;
const PFR_PHY_EXTRA_ITEMS: u8 = 0x80;

const PFR_KERN_2BYTE_CHAR: u8 = 0x01;
const PFR_KERN_2BYTE_ADJ: u8 = 0x02;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PfrFont {
    pub(crate) outline_resolution: u16,
    pub(crate) metrics_resolution: u16,
    pub(crate) bbox: BBox,
    pub(crate) proportional: bool,
    pub(crate) vertical: bool,
    pub(crate) advances: Vec<i32>,
    char_codes: Vec<u16>,
    kerning: Vec<PfrKerningPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PfrKerningPair {
    left: u16,
    right: u16,
    adjustment: i32,
}

impl PfrFont {
    pub(crate) fn parse(data: &[u8], face_index: usize) -> Result<Self, &'static str> {
        if data.len() < PFR_HEADER_SIZE
            || data.get(..4) != Some(b"PFR0")
            || be_u16(data, 4).is_none_or(|version| version > 4)
            || be_u16(data, 6) != Some(0x0D0A)
            || be_u16(data, 8).is_none_or(|size| usize::from(size) < PFR_HEADER_SIZE)
        {
            return Err("invalid PFR header");
        }

        let logical_directory =
            usize::from(be_u16(data, 12).ok_or("missing PFR logical directory")?);
        let face_count =
            usize::from(be_u16(data, logical_directory).ok_or("truncated PFR logical directory")?);
        if face_index >= face_count {
            return Err("PFR face index out of range");
        }

        let directory_entry = logical_directory
            .checked_add(2)
            .and_then(|offset| offset.checked_add(face_index.checked_mul(5)?))
            .ok_or("PFR logical directory overflow")?;
        let logical_size = usize::from(
            be_u16(data, directory_entry).ok_or("truncated PFR logical directory entry")?,
        );
        let logical_offset = be_u24(data, directory_entry + 2)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("invalid PFR logical font offset")?;
        let logical =
            range(data, logical_offset, logical_size).ok_or("PFR logical font outside stream")?;
        if logical.len() < 18 {
            return Err("truncated PFR logical font");
        }

        let logical_flags = logical[12];
        let mut cursor = 13usize;
        if logical_flags & PFR_LOG_STROKE != 0 {
            cursor = checked_skip(cursor, 1)?;
            if logical_flags & PFR_LOG_2BYTE_STROKE != 0 {
                cursor = checked_skip(cursor, 1)?;
            }
            if logical_flags & PFR_LINE_JOIN_MASK == 0 {
                cursor = checked_skip(cursor, 3)?;
            }
        }
        if logical_flags & PFR_LOG_BOLD != 0 {
            cursor = checked_skip(cursor, 1)?;
            if logical_flags & PFR_LOG_2BYTE_BOLD != 0 {
                cursor = checked_skip(cursor, 1)?;
            }
        }
        if logical_flags & PFR_LOG_EXTRA_ITEMS != 0 {
            cursor = skip_extra_items(logical, cursor)?;
        }
        let mut physical_size =
            usize::from(be_u16(logical, cursor).ok_or("truncated PFR physical font size")?);
        let physical_offset = be_u24(logical, cursor + 2)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("invalid PFR physical font offset")?;
        if data[41] != 0 {
            physical_size = physical_size
                .checked_add(
                    usize::from(
                        *logical
                            .get(cursor + 5)
                            .ok_or("truncated high PFR physical font size")?,
                    ) << 16,
                )
                .ok_or("PFR physical font size overflow")?;
        }

        let physical = range(data, physical_offset, physical_size)
            .ok_or("PFR physical font outside stream")?;
        Self::parse_physical(physical)
    }

    fn parse_physical(physical: &[u8]) -> Result<Self, &'static str> {
        if physical.len() < 15 {
            return Err("truncated PFR physical font");
        }
        let outline_resolution = be_u16(physical, 2).ok_or("missing PFR outline resolution")?;
        let metrics_resolution = be_u16(physical, 4).ok_or("missing PFR metrics resolution")?;
        if outline_resolution == 0 || metrics_resolution == 0 {
            return Err("invalid zero PFR resolution");
        }
        let bbox = BBox {
            x_min: i32::from(be_i16(physical, 6).ok_or("missing PFR bbox")?),
            y_min: i32::from(be_i16(physical, 8).ok_or("missing PFR bbox")?),
            x_max: i32::from(be_i16(physical, 10).ok_or("missing PFR bbox")?),
            y_max: i32::from(be_i16(physical, 12).ok_or("missing PFR bbox")?),
        };
        let flags = physical[14];
        let proportional = flags & PFR_PHY_PROPORTIONAL != 0;
        let vertical = flags & PFR_PHY_VERTICAL != 0;
        let mut cursor = 15usize;
        let standard_advance = if proportional {
            0
        } else {
            let value = i32::from(be_i16(physical, cursor).ok_or("missing PFR standard advance")?);
            cursor = checked_skip(cursor, 2)?;
            value
        };

        let (next, kerning) = if flags & PFR_PHY_EXTRA_ITEMS != 0 {
            parse_physical_extra_items(physical, cursor)?
        } else {
            (cursor, Vec::new())
        };
        cursor = next;

        let auxiliary_size = be_u24(physical, cursor)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or("missing PFR auxiliary data size")?;
        cursor = checked_skip(cursor, 3)?;
        cursor = checked_skip(cursor, auxiliary_size)?;

        let blue_count = usize::from(*physical.get(cursor).ok_or("missing PFR blue-value count")?);
        cursor = checked_skip(cursor, 1)?;
        cursor = checked_skip(
            cursor,
            blue_count
                .checked_mul(2)
                .ok_or("PFR blue-value size overflow")?,
        )?;
        // blue fuzz, blue scale, vertical standard, horizontal standard
        cursor = checked_skip(cursor, 6)?;

        let char_count =
            usize::from(be_u16(physical, cursor).ok_or("missing PFR character count")?);
        cursor = checked_skip(cursor, 2)?;
        if char_count == 0 {
            return Err("PFR physical font has no characters");
        }

        let mut char_codes = Vec::with_capacity(char_count);
        let mut advances = Vec::with_capacity(char_count);
        for _ in 0..char_count {
            let char_code = if flags & PFR_PHY_2BYTE_CHARCODE != 0 {
                let value = be_u16(physical, cursor).ok_or("truncated PFR character code")?;
                cursor = checked_skip(cursor, 2)?;
                value
            } else {
                let value = u16::from(*physical.get(cursor).ok_or("truncated PFR character code")?);
                cursor = checked_skip(cursor, 1)?;
                value
            };
            let advance = if proportional {
                let value =
                    i32::from(be_i16(physical, cursor).ok_or("truncated PFR character advance")?);
                cursor = checked_skip(cursor, 2)?;
                value
            } else {
                standard_advance
            };
            if flags & PFR_PHY_ASCII_CODE != 0 {
                cursor = checked_skip(cursor, 1)?;
            }
            cursor = checked_skip(
                cursor,
                if flags & PFR_PHY_2BYTE_GPS_SIZE != 0 {
                    2
                } else {
                    1
                },
            )?;
            cursor = checked_skip(
                cursor,
                if flags & PFR_PHY_3BYTE_GPS_OFFSET != 0 {
                    3
                } else {
                    2
                },
            )?;
            if cursor > physical.len() {
                return Err("truncated PFR character descriptor");
            }
            char_codes.push(char_code);
            advances.push(advance);
        }

        Ok(Self {
            outline_resolution,
            metrics_resolution,
            bbox,
            proportional,
            vertical,
            advances,
            char_codes,
            kerning,
        })
    }

    pub(crate) fn advance(&self, glyph_index: u32) -> Option<i32> {
        let index = glyph_index.checked_sub(1)?;
        self.advances.get(usize::try_from(index).ok()?).copied()
    }

    pub(crate) fn kerning(&self, left_glyph: u32, right_glyph: u32) -> i32 {
        let Some(left_index) = left_glyph
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
        else {
            return 0;
        };
        let Some(right_index) = right_glyph
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
        else {
            return 0;
        };
        let Some((&left, &right)) = self
            .char_codes
            .get(left_index)
            .zip(self.char_codes.get(right_index))
        else {
            return 0;
        };
        self.kerning
            .iter()
            .find(|pair| pair.left == left && pair.right == right)
            .map_or(0, |pair| pair.adjustment)
    }

    pub(crate) fn has_kerning(&self) -> bool {
        !self.kerning.is_empty()
    }

    pub(crate) fn max_advance(&self) -> i32 {
        self.advances.iter().copied().max().unwrap_or(0)
    }
}

fn parse_physical_extra_items(
    data: &[u8],
    mut cursor: usize,
) -> Result<(usize, Vec<PfrKerningPair>), &'static str> {
    let count = usize::from(
        *data
            .get(cursor)
            .ok_or("missing PFR physical extra-item count")?,
    );
    cursor = checked_skip(cursor, 1)?;
    let mut kerning = Vec::new();
    for _ in 0..count {
        let item_size = usize::from(
            *data
                .get(cursor)
                .ok_or("truncated PFR physical extra item")?,
        );
        let item_type = *data
            .get(cursor + 1)
            .ok_or("truncated PFR physical extra item")?;
        cursor = checked_skip(cursor, 2)?;
        let item = range(data, cursor, item_size).ok_or("PFR extra item outside physical font")?;
        if item_type == 4 {
            parse_kerning_item(item, &mut kerning)?;
        }
        cursor = checked_skip(cursor, item_size)?;
    }
    Ok((cursor, kerning))
}

fn parse_kerning_item(item: &[u8], kerning: &mut Vec<PfrKerningPair>) -> Result<(), &'static str> {
    if item.len() < 4 {
        return Err("truncated PFR kerning item");
    }
    let pair_count = usize::from(item[0]);
    let base_adjustment = i32::from(be_i16(item, 1).ok_or("missing PFR kerning base")?);
    let flags = item[3];
    let mut cursor = 4usize;
    for _ in 0..pair_count {
        let left = if flags & PFR_KERN_2BYTE_CHAR != 0 {
            let value = be_u16(item, cursor).ok_or("truncated PFR kerning pair")?;
            cursor = checked_skip(cursor, 2)?;
            value
        } else {
            let value = u16::from(*item.get(cursor).ok_or("truncated PFR kerning pair")?);
            cursor = checked_skip(cursor, 1)?;
            value
        };
        let right = if flags & PFR_KERN_2BYTE_CHAR != 0 {
            let value = be_u16(item, cursor).ok_or("truncated PFR kerning pair")?;
            cursor = checked_skip(cursor, 2)?;
            value
        } else {
            let value = u16::from(*item.get(cursor).ok_or("truncated PFR kerning pair")?);
            cursor = checked_skip(cursor, 1)?;
            value
        };
        let delta = if flags & PFR_KERN_2BYTE_ADJ != 0 {
            let value = i32::from(be_i16(item, cursor).ok_or("truncated PFR kerning adjustment")?);
            cursor = checked_skip(cursor, 2)?;
            value
        } else {
            let value = i32::from(*item.get(cursor).ok_or("truncated PFR kerning adjustment")?);
            cursor = checked_skip(cursor, 1)?;
            value
        };
        kerning.push(PfrKerningPair {
            left,
            right,
            adjustment: base_adjustment.saturating_add(delta),
        });
    }
    Ok(())
}

fn skip_extra_items(data: &[u8], mut cursor: usize) -> Result<usize, &'static str> {
    let count = usize::from(*data.get(cursor).ok_or("missing PFR extra-item count")?);
    cursor = checked_skip(cursor, 1)?;
    for _ in 0..count {
        let item_size = usize::from(*data.get(cursor).ok_or("truncated PFR extra item")?);
        cursor = checked_skip(cursor, 2)?;
        cursor = checked_skip(cursor, item_size)?;
        if cursor > data.len() {
            return Err("PFR extra item outside table");
        }
    }
    Ok(cursor)
}

fn range(data: &[u8], offset: usize, size: usize) -> Option<&[u8]> {
    data.get(offset..offset.checked_add(size)?)
}

fn checked_skip(cursor: usize, amount: usize) -> Result<usize, &'static str> {
    cursor.checked_add(amount).ok_or("PFR cursor overflow")
}

fn be_u16(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_be_bytes([
        *data.get(offset)?,
        *data.get(offset + 1)?,
    ]))
}

fn be_i16(data: &[u8], offset: usize) -> Option<i16> {
    Some(i16::from_be_bytes([
        *data.get(offset)?,
        *data.get(offset + 1)?,
    ]))
}

fn be_u24(data: &[u8], offset: usize) -> Option<u32> {
    Some(
        u32::from(*data.get(offset)?) << 16
            | u32::from(*data.get(offset + 1)?) << 8
            | u32::from(*data.get(offset + 2)?),
    )
}

#[cfg(test)]
#[path = "../tests/unit/pfr.rs"]
mod tests;
