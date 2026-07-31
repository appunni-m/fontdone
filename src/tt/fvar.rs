//! Minimal `fvar` parsing for named-instance face selection.

use crate::error::FontError;

/// Parsed variation-axis and named-instance metadata.
#[derive(Debug, Clone)]
pub struct FvarTable {
    pub axis_count: u16,
    pub instance_count: u16,
    pub axes: Vec<FvarAxis>,
    pub instances: Vec<FvarInstance>,
}

/// One axis record from the `fvar` table.
#[derive(Debug, Clone, Copy)]
pub struct FvarAxis {
    pub tag: u32,
    pub min_value: i32,
    pub default_value: i32,
    pub max_value: i32,
    pub flags: u16,
    pub name_id: u16,
}

/// One named instance from the `fvar` table.
#[derive(Debug, Clone)]
pub struct FvarInstance {
    pub subfamily_name_id: u16,
    pub postscript_name_id: Option<u16>,
    pub coords: Vec<i32>,
}

pub fn parse_fvar(data: &[u8]) -> Result<FvarTable, FontError> {
    if data.len() < 20 {
        return Err(FontError::InvalidFont(
            "fvar table too short (need 20 bytes)".into(),
        ));
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if version != 0x0001_0000 {
        return Err(FontError::InvalidFont("unsupported fvar version".into()));
    }
    let axes_offset = u16::from_be_bytes([data[4], data[5]]) as usize;
    let axis_count = u16::from_be_bytes([data[8], data[9]]);
    let axis_size = u16::from_be_bytes([data[10], data[11]]) as usize;
    let instance_count = u16::from_be_bytes([data[12], data[13]]);
    let instance_size = u16::from_be_bytes([data[14], data[15]]) as usize;
    let axis_count_usize = usize::from(axis_count);
    let instance_count_usize = usize::from(instance_count);

    // `sfnt_init_face` validates these limits before exposing GX variation
    // support.  They also bound every offset below to 32-bit arithmetic, as
    // relied on by `TT_Get_MM_Var` in `ttgxvar.c`.
    if axis_count == 0 || axis_count > 0x3FFE {
        return Err(FontError::InvalidFont("invalid fvar axis count".into()));
    }
    if axis_size != 20 {
        return Err(FontError::InvalidFont("invalid fvar axis size".into()));
    }
    let min_instance_size = 4 + axis_count_usize * 4;
    if instance_size != min_instance_size && instance_size != min_instance_size + 2 {
        return Err(FontError::InvalidFont("invalid fvar instance size".into()));
    }
    if instance_count > 0x7EFF {
        return Err(FontError::InvalidFont("invalid fvar instance count".into()));
    }

    let instances_offset = axes_offset + axis_count_usize * axis_size;
    let instances_end = instances_offset + instance_count_usize * instance_size;
    if instances_end > data.len() {
        return Err(FontError::InvalidFont(
            "fvar instance array too short".into(),
        ));
    }

    let mut axes = Vec::with_capacity(axis_count_usize);
    for index in 0..axis_count_usize {
        let off = axes_offset + index * axis_size;
        axes.push(FvarAxis {
            tag: u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]),
            min_value: i32::from_be_bytes([
                data[off + 4],
                data[off + 5],
                data[off + 6],
                data[off + 7],
            ]),
            default_value: i32::from_be_bytes([
                data[off + 8],
                data[off + 9],
                data[off + 10],
                data[off + 11],
            ]),
            max_value: i32::from_be_bytes([
                data[off + 12],
                data[off + 13],
                data[off + 14],
                data[off + 15],
            ]),
            flags: u16::from_be_bytes([data[off + 16], data[off + 17]]),
            name_id: u16::from_be_bytes([data[off + 18], data[off + 19]]),
        });
    }

    let mut instances = Vec::with_capacity(instance_count_usize);
    for index in 0..instance_count_usize {
        let off = instances_offset + index * instance_size;
        let subfamily_name_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let coords = (0..axis_count_usize)
            .map(|axis| {
                let coord_off = off + 4 + axis * 4;
                i32::from_be_bytes([
                    data[coord_off],
                    data[coord_off + 1],
                    data[coord_off + 2],
                    data[coord_off + 3],
                ])
            })
            .collect();
        let postscript_name_id = if instance_size >= min_instance_size + 2 {
            let id = u16::from_be_bytes([
                data[off + min_instance_size],
                data[off + min_instance_size + 1],
            ]);
            (id != 0xFFFF).then_some(id)
        } else {
            None
        };
        instances.push(FvarInstance {
            subfamily_name_id,
            postscript_name_id,
            coords,
        });
    }

    Ok(FvarTable {
        axis_count,
        instance_count,
        axes,
        instances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis_record(tag: u32, min: i32, default: i32, max: i32) -> Vec<u8> {
        let mut bytes = tag.to_be_bytes().to_vec();
        bytes.extend_from_slice(&min.to_be_bytes());
        bytes.extend_from_slice(&default.to_be_bytes());
        bytes.extend_from_slice(&max.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes()); // flags
        bytes.extend_from_slice(&1u16.to_be_bytes()); // name_id
        bytes
    }

    fn fvar_with(instances: &[(u16, Option<u16>, &[i32])]) -> Vec<u8> {
        let axis_count = 2u16;
        let axes_offset = 16usize;
        let axes = [
            axis_record(0x7767_6874, 100, 400, 900), // 'wght'
            axis_record(0x7764_7468, 50, 100, 200),  // 'wdth'
        ];
        let has_ps = instances.iter().any(|(_, ps, _)| ps.is_some());
        let instance_size = 4 + axis_count as usize * 4 + usize::from(has_ps) * 2;
        let instances_offset = axes_offset + axis_count as usize * 20;

        let mut data = Vec::new();
        data.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        data.extend_from_slice(&(axes_offset as u16).to_be_bytes());
        data.extend_from_slice(&0u16.to_be_bytes()); // reserved
        data.extend_from_slice(&axis_count.to_be_bytes());
        data.extend_from_slice(&20u16.to_be_bytes()); // axis size
        data.extend_from_slice(&(instances.len() as u16).to_be_bytes());
        data.extend_from_slice(&(instance_size as u16).to_be_bytes());
        data.extend_from_slice(&axes.concat());
        for (subfamily_id, ps_id, coords) in instances {
            data.extend_from_slice(&subfamily_id.to_be_bytes());
            data.extend_from_slice(&0u16.to_be_bytes()); // padding before coords
            for coord in *coords {
                data.extend_from_slice(&coord.to_be_bytes());
            }
            if let Some(ps_id) = ps_id {
                data.extend_from_slice(&ps_id.to_be_bytes());
            }
        }
        let _ = instances_offset;
        data
    }

    #[test]
    fn parses_axes_and_instances() -> Result<(), FontError> {
        let data = fvar_with(&[(2, Some(3), &[400, 100]), (4, Some(5), &[900, 200])]);
        let table = parse_fvar(&data)?;
        assert_eq!(table.axis_count, 2);
        assert_eq!(table.instance_count, 2);
        assert_eq!(table.axes[0].tag, 0x7767_6874);
        assert_eq!(table.axes[0].default_value, 400);
        assert_eq!(table.axes[1].max_value, 200);
        assert_eq!(table.instances[0].subfamily_name_id, 2);
        assert_eq!(table.instances[0].postscript_name_id, Some(3));
        assert_eq!(table.instances[0].coords, vec![400, 100]);
        assert_eq!(table.instances[1].postscript_name_id, Some(5));
        assert_eq!(table.instances[1].coords, vec![900, 200]);
        Ok(())
    }

    #[test]
    fn rejects_bad_tables() {
        assert!(parse_fvar(&[0u8; 19]).is_err());
        let mut data = fvar_with(&[]);
        data[0..4].copy_from_slice(&0x0002_0000u32.to_be_bytes());
        assert!(parse_fvar(&data).is_err());

        let mut data = fvar_with(&[]);
        data[8..10].copy_from_slice(&0u16.to_be_bytes()); // zero axes
        assert!(parse_fvar(&data).is_err());

        let mut data = fvar_with(&[]);
        data[10..12].copy_from_slice(&21u16.to_be_bytes()); // bad axis size
        assert!(parse_fvar(&data).is_err());
    }
}
