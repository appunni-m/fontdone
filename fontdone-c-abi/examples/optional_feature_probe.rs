//! Emits one disabled optional-feature contract for a Rust-backed lane.

use std::fs;
use std::process::ExitCode;
use std::ptr::NonNull;

use fontdone::ffi::{
    FT_Byte, FT_Color, FT_Err_Unimplemented_Feature, FT_Error, FT_Init_FreeType, FT_LcdFilter,
    FT_Library_LcdWeights, FT_Library_SetLcdFilter, FT_Library_SetLcdFilterWeights,
    FT_Library_SetLcdGeometry, FT_MemoryRec, FT_New_Memory_Face, FT_Palette_Data,
    FT_Palette_Data_Get, FT_Palette_Select, FT_Palette_Set_Foreground_Color, FT_Stream_OpenBzip2,
    FT_Stream_OpenLZW, FT_StreamRec, FT_ULong, FT_Vector,
};

const UNIMPLEMENTED_FEATURE: FT_Error = FT_Err_Unimplemented_Feature as FT_Error;

#[derive(Clone, Copy)]
enum Backend {
    Rust,
    CAbi,
    Wasm,
}

#[derive(Clone, Copy)]
enum OptionalFeature {
    Bzip2,
    ColorLayers,
    LcdFilteringDisabled,
    Lzw,
    SubpixelRendering,
}

fn sentinel_stream() -> FT_StreamRec {
    FT_StreamRec {
        base: NonNull::<u8>::dangling().as_ptr(),
        size: FT_ULong::MAX,
        pos: FT_ULong::MAX,
        read: NonNull::<u8>::dangling().as_ptr().cast(),
        close: NonNull::<u8>::dangling().as_ptr().cast(),
        memory: NonNull::<FT_MemoryRec>::dangling().as_ptr(),
        ..FT_StreamRec::default()
    }
}

fn pointer_class<T>(pointer: *const T) -> &'static str {
    if pointer.is_null() { "null" } else { "nonnull" }
}

fn stream_fields(stream: &FT_StreamRec) -> String {
    format!(
        concat!(
            "{{\"size\":{},\"pos\":{},\"base_class\":\"{}\",",
            "\"read_class\":\"{}\",\"close_class\":\"{}\",",
            "\"memory_class\":\"{}\"}}"
        ),
        stream.size,
        stream.pos,
        pointer_class(stream.base.cast_const()),
        pointer_class(stream.read.cast_const()),
        pointer_class(stream.close.cast_const()),
        pointer_class(stream.memory.cast_const()),
    )
}

fn call_lzw(
    backend: Backend,
    stream: Option<&mut FT_StreamRec>,
    source: Option<&mut FT_StreamRec>,
    source_bytes: Option<&[FT_Byte]>,
) -> FT_Error {
    match backend {
        Backend::Rust => FT_Stream_OpenLZW(stream, source.as_deref(), source_bytes),
        Backend::CAbi => fontdone_c_abi::FT_Stream_OpenLZW(
            stream.map_or(std::ptr::null_mut(), |value| value),
            source.map_or(std::ptr::null_mut(), |value| value),
        ),
        Backend::Wasm => fontdone_wasm::fontdone_wasm_stream_open_lzw(
            stream.map_or(std::ptr::null_mut(), |value| value),
            source.map_or(std::ptr::null(), |value| value),
        ),
    }
}

fn lzw_output(backend: Backend) -> String {
    let null_status = call_lzw(backend, None, None, None);
    let bytes = [0x1F, 0x9D, 0x90, 0x01];
    let mut source = FT_StreamRec {
        base: bytes.as_ptr().cast_mut(),
        size: FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX),
        ..FT_StreamRec::default()
    };
    let mut target = sentinel_stream();
    let before = stream_fields(&target);
    let valid_status = call_lzw(backend, Some(&mut target), Some(&mut source), Some(&bytes));
    let after = stream_fields(&target);
    let kind = if null_status == 0 { "ok" } else { "error" };
    format!(
        concat!(
            "{{\"status\":{{\"kind\":\"{}\",\"error_code\":{}}},",
            "\"output\":{{\"rows\":[",
            "{{\"variant\":\"null_arguments\",\"status\":{},",
            "\"argument_effect\":\"unobserved\",",
            "\"target_stream_before\":null,\"target_stream_after\":null}},",
            "{{\"variant\":\"valid_arguments\",\"status\":{},",
            "\"argument_effect\":\"unobserved\",",
            "\"target_stream_before\":{},\"target_stream_after\":{}}}",
            "]}}}}"
        ),
        kind, null_status, null_status, valid_status, before, after,
    )
}

fn call_bzip2(backend: Backend, stream: &mut FT_StreamRec, source: &mut FT_StreamRec) -> FT_Error {
    match backend {
        Backend::Rust => FT_Stream_OpenBzip2(Some(stream), Some(source), None),
        Backend::CAbi => fontdone_c_abi::FT_Stream_OpenBzip2(stream, source),
        Backend::Wasm => fontdone_wasm::fontdone_wasm_stream_open_bzip2(stream, source),
    }
}

fn bzip2_output(backend: Backend) -> String {
    let mut source = FT_StreamRec::default();
    let mut stream = FT_StreamRec::default();
    let status = call_bzip2(backend, &mut stream, &mut source);
    let kind = if status == 0 { "ok" } else { "error" };
    format!(
        concat!(
            "{{\"status\":{{\"kind\":\"{}\",\"error_code\":{}}},",
            "\"output\":{{\"build_features\":{{\"bzip2\":false}},",
            "\"error\":{},\"stream\":{{\"base_class\":\"{}\",",
            "\"read_class\":\"{}\",\"close_class\":\"{}\"}}}}}}"
        ),
        kind,
        status,
        status,
        pointer_class(stream.base.cast_const()),
        pointer_class(stream.read.cast_const()),
        pointer_class(stream.close.cast_const()),
    )
}

fn palette_data_fields(
    num_palettes: u16,
    palette_name_ids: *const u16,
    palette_flags: *const u16,
    num_palette_entries: u16,
    palette_entry_name_ids: *const u16,
) -> String {
    format!(
        concat!(
            "{{\"num_palettes\":{},\"num_palette_entries\":{},",
            "\"pointer_nullness\":{{\"palette_name_ids\":{},",
            "\"palette_flags\":{},\"palette_entry_name_ids\":{}}}}}"
        ),
        num_palettes,
        num_palette_entries,
        palette_name_ids.is_null(),
        palette_flags.is_null(),
        palette_entry_name_ids.is_null(),
    )
}

fn color_layers_output(backend: Backend, path: &str, face_index: i64) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|error| format!("read {path}: {error}"))?;
    let foreground = FT_Color {
        blue: 1,
        green: 2,
        red: 3,
        alpha: 4,
    };
    let (data_error, data_snapshot, select_error, palette_is_null, foreground_error) = match backend
    {
        Backend::Rust => {
            let library = FT_Init_FreeType();
            let face = FT_New_Memory_Face(&library, &bytes, face_index, 20.0)
                .map_err(|error| format!("open Rust face: {error}"))?;
            let mut data = FT_Palette_Data {
                num_palettes: 999,
                palette_name_ids: NonNull::<u16>::dangling().as_ptr(),
                palette_flags: NonNull::<u16>::dangling().as_ptr(),
                num_palette_entries: 999,
                palette_entry_name_ids: NonNull::<u16>::dangling().as_ptr(),
            };
            let mut palette: *const FT_Color = NonNull::<FT_Color>::dangling().as_ptr();
            let data_error = FT_Palette_Data_Get(Some(&face), Some(&mut data));
            let select_error = FT_Palette_Select(Some(&face), 0, Some(&mut palette));
            let foreground_error = FT_Palette_Set_Foreground_Color(Some(&face), foreground);
            (
                data_error,
                palette_data_fields(
                    data.num_palettes,
                    data.palette_name_ids,
                    data.palette_flags,
                    data.num_palette_entries,
                    data.palette_entry_name_ids,
                ),
                select_error,
                palette.is_null(),
                foreground_error,
            )
        }
        Backend::CAbi => {
            let mut library = std::ptr::null_mut();
            let init_error = fontdone_c_abi::FT_Init_FreeType(&mut library);
            if init_error != 0 {
                return Err(format!("initialize C ABI library: {init_error}"));
            }
            let mut face = std::ptr::null_mut();
            let file_size = i64::try_from(bytes.len()).map_err(|error| error.to_string())?;
            let open_error = fontdone_c_abi::FT_New_Memory_Face(
                library,
                bytes.as_ptr(),
                file_size,
                face_index,
                &mut face,
            );
            if open_error != 0 {
                let _ = fontdone_c_abi::FT_Done_FreeType(library);
                return Err(format!("open C ABI face: {open_error}"));
            }
            let mut data = fontdone_c_abi::FT_Palette_Data {
                num_palettes: 999,
                palette_name_ids: NonNull::<u16>::dangling().as_ptr(),
                palette_flags: NonNull::<u16>::dangling().as_ptr(),
                num_palette_entries: 999,
                palette_entry_name_ids: NonNull::<u16>::dangling().as_ptr(),
            };
            let mut palette = NonNull::<fontdone_c_abi::FT_Color>::dangling().as_ptr();
            let data_error = fontdone_c_abi::FT_Palette_Data_Get(face, &mut data);
            let select_error = fontdone_c_abi::FT_Palette_Select(face, 0, &mut palette);
            let foreground_error = fontdone_c_abi::FT_Palette_Set_Foreground_Color(
                face,
                fontdone_c_abi::FT_Color {
                    blue: foreground.blue,
                    green: foreground.green,
                    red: foreground.red,
                    alpha: foreground.alpha,
                },
            );
            let snapshot = palette_data_fields(
                data.num_palettes,
                data.palette_name_ids,
                data.palette_flags,
                data.num_palette_entries,
                data.palette_entry_name_ids,
            );
            let _ = fontdone_c_abi::FT_Done_Face(face);
            let _ = fontdone_c_abi::FT_Done_FreeType(library);
            (
                data_error,
                snapshot,
                select_error,
                palette.is_null(),
                foreground_error,
            )
        }
        Backend::Wasm => {
            let opened = fontdone_wasm::fontdone_wasm_open_face(
                bytes.as_ptr(),
                bytes.len(),
                face_index,
                20.0,
            );
            if opened.error != 0 {
                return Err(format!("open WASM-host face: {}", opened.error));
            }
            let mut data = fontdone_wasm::FontdoneWasmPaletteData {
                num_palettes: 999,
                palette_name_ids: NonNull::<u16>::dangling().as_ptr(),
                palette_flags: NonNull::<u16>::dangling().as_ptr(),
                num_palette_entries: 999,
                palette_entry_name_ids: NonNull::<u16>::dangling().as_ptr(),
            };
            let mut palette = NonNull::<fontdone_wasm::FontdoneWasmColor>::dangling().as_ptr();
            let data_error =
                fontdone_wasm::fontdone_wasm_palette_data_get(opened.handle, &mut data);
            let select_error =
                fontdone_wasm::fontdone_wasm_palette_select(opened.handle, 0, &mut palette);
            let foreground_error = fontdone_wasm::fontdone_wasm_palette_set_foreground_color(
                opened.handle,
                fontdone_wasm::FontdoneWasmColor {
                    blue: foreground.blue,
                    green: foreground.green,
                    red: foreground.red,
                    alpha: foreground.alpha,
                },
            );
            let snapshot = palette_data_fields(
                data.num_palettes,
                data.palette_name_ids,
                data.palette_flags,
                data.num_palette_entries,
                data.palette_entry_name_ids,
            );
            let _ = fontdone_wasm::fontdone_wasm_done_face(opened.handle);
            (
                data_error,
                snapshot,
                select_error,
                palette.is_null(),
                foreground_error,
            )
        }
    };
    let status = [data_error, select_error, foreground_error]
        .into_iter()
        .find(|error| *error != 0)
        .unwrap_or(0);
    let kind = if status == 0 { "ok" } else { "error" };
    Ok(format!(
        concat!(
            "{{\"status\":{{\"kind\":\"{}\",\"error_code\":{}}},",
            "\"output\":{{\"build_features\":{{\"color_layers\":false}},",
            "\"data_get\":{{\"error\":{},\"palette_data_snapshot\":{}}},",
            "\"select\":{{\"error\":{},\"apalette_snapshot\":\"{}\"}},",
            "\"set_foreground_color\":{{\"error\":{}}}}}}}"
        ),
        kind,
        status,
        data_error,
        data_snapshot,
        select_error,
        if palette_is_null { "null" } else { "non_null" },
        foreground_error,
    ))
}

fn parse_present(value: &str) -> Result<bool, String> {
    match value {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err(format!("library presence must be 0 or 1, got {value}")),
    }
}

fn weights_json(weights: Option<[FT_Byte; 5]>) -> String {
    weights.map_or_else(
        || "null".to_string(),
        |weights| {
            format!(
                "[{},{},{},{},{}]",
                weights[0], weights[1], weights[2], weights[3], weights[4]
            )
        },
    )
}

fn status_json(status: FT_Error, output: String) -> String {
    format!(
        "{{\"status\":{{\"kind\":\"{}\",\"error_code\":{status}}},\"output\":{output}}}",
        if status == 0 { "ok" } else { "error" },
    )
}

fn parse_filters(value: &str) -> Result<Vec<FT_LcdFilter>, String> {
    value
        .split(',')
        .map(|value| {
            value
                .parse::<FT_LcdFilter>()
                .map_err(|error| format!("invalid LCD filter {value}: {error}"))
        })
        .collect()
}

#[cfg(feature = "subpixel-rendering")]
fn c_abi_lcd_weights(library: fontdone_c_abi::FT_Library) -> Option<[FT_Byte; 5]> {
    if library.is_null() {
        None
    } else {
        // SAFETY: the probe owns the live library returned by FT_Init_FreeType.
        Some(unsafe { (*library).lcd_weights })
    }
}

#[cfg(not(feature = "subpixel-rendering"))]
fn c_abi_lcd_weights(_library: fontdone_c_abi::FT_Library) -> Option<[FT_Byte; 5]> {
    None
}

fn subpixel_filter_output(
    backend: Backend,
    library_present: bool,
    filters: &[FT_LcdFilter],
) -> Result<String, String> {
    let mut rust_library = match backend {
        Backend::Rust if library_present => Some(FT_Init_FreeType()),
        _ => None,
    };
    let mut c_library = std::ptr::null_mut();
    if matches!(backend, Backend::CAbi) && library_present {
        let error = fontdone_c_abi::FT_Init_FreeType(&mut c_library);
        if error != 0 {
            return Ok(status_json(error, "null".to_string()));
        }
    }
    let mut first_error = 0;
    let mut rows = Vec::new();
    for filter in filters {
        let (error, weights) = match backend {
            Backend::Rust => {
                let error = FT_Library_SetLcdFilter(rust_library.as_mut(), *filter);
                let weights = FT_Library_LcdWeights(rust_library.as_ref());
                (error, weights)
            }
            Backend::CAbi => {
                let error = fontdone_c_abi::FT_Library_SetLcdFilter(c_library, *filter);
                (error, c_abi_lcd_weights(c_library))
            }
            Backend::Wasm => {
                fontdone_wasm::abi_support_subpixel_lcd_filter(library_present, *filter)
            }
        };
        if first_error == 0 && error != 0 {
            first_error = error;
        }
        rows.push(format!(
            "{{\"filter\":{filter},\"error\":{error},\"library_weights\":{}}}",
            weights_json(weights)
        ));
    }
    if !c_library.is_null() {
        let _ = fontdone_c_abi::FT_Done_FreeType(c_library);
    }
    Ok(status_json(
        first_error,
        format!("{{\"outputs\":[{}]}}", rows.join(",")),
    ))
}

fn parse_weights(value: &str) -> Result<Option<Vec<FT_Byte>>, String> {
    if value == "-" {
        return Ok(None);
    }
    let weights = value
        .split(',')
        .map(|value| {
            value
                .parse::<FT_Byte>()
                .map_err(|error| format!("invalid LCD weight {value}: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if weights.len() < 5 {
        return Err("LCD filter weights require at least five bytes".to_string());
    }
    Ok(Some(weights))
}

fn copied_weights(weights: Option<&[FT_Byte]>) -> Option<[FT_Byte; 5]> {
    weights.map(|weights| {
        let mut copied = [0; 5];
        copied.copy_from_slice(&weights[..5]);
        copied
    })
}

fn byte_array_json(bytes: &[FT_Byte]) -> String {
    format!(
        "[{}]",
        bytes
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn subpixel_weights_output(
    backend: Backend,
    library_present: bool,
    mut weights: Option<Vec<FT_Byte>>,
) -> Result<String, String> {
    let copied = copied_weights(weights.as_deref());
    let ignored_tail = weights
        .as_deref()
        .and_then(|weights| weights.get(5..))
        .unwrap_or_default()
        .to_vec();
    let (error, snapshot) = match backend {
        Backend::Rust => {
            let mut library = library_present.then(FT_Init_FreeType);
            let error = FT_Library_SetLcdFilterWeights(library.as_mut(), copied);
            let snapshot = FT_Library_LcdWeights(library.as_ref());
            (error, snapshot)
        }
        Backend::CAbi => {
            let mut library = std::ptr::null_mut();
            if library_present {
                let init_error = fontdone_c_abi::FT_Init_FreeType(&mut library);
                if init_error != 0 {
                    return Ok(status_json(init_error, "null".to_string()));
                }
            }
            let pointer = weights
                .as_mut()
                .map_or(std::ptr::null_mut(), |weights| weights.as_mut_ptr());
            let error = fontdone_c_abi::FT_Library_SetLcdFilterWeights(library, pointer);
            let snapshot = c_abi_lcd_weights(library);
            if !library.is_null() {
                let _ = fontdone_c_abi::FT_Done_FreeType(library);
            }
            (error, snapshot)
        }
        Backend::Wasm => {
            fontdone_wasm::abi_support_subpixel_lcd_filter_weights(library_present, copied)
        }
    };
    Ok(status_json(
        error,
        format!(
            concat!(
                "{{\"error\":{},\"library_weights\":{},",
                "\"ignored_tail_bytes\":{}}}"
            ),
            error,
            weights_json(snapshot),
            byte_array_json(&ignored_tail),
        ),
    ))
}

fn subpixel_geometry_output(
    backend: Backend,
    library_present: bool,
    geometry_present: bool,
) -> String {
    let geometry = geometry_present.then_some([
        FT_Vector { x: -21, y: 0 },
        FT_Vector { x: 0, y: 0 },
        FT_Vector { x: 21, y: 0 },
    ]);
    let error = match backend {
        Backend::Rust => {
            let mut library = library_present.then(FT_Init_FreeType);
            FT_Library_SetLcdGeometry(library.as_mut(), geometry)
        }
        Backend::CAbi => {
            let mut library = std::ptr::null_mut();
            if library_present {
                let init_error = fontdone_c_abi::FT_Init_FreeType(&mut library);
                if init_error != 0 {
                    return status_json(init_error, "null".to_string());
                }
            }
            let vectors = geometry.map(|geometry| {
                geometry.map(|vector| fontdone_c_abi::FT_Vector {
                    x: vector.x,
                    y: vector.y,
                })
            });
            let error = fontdone_c_abi::FT_Library_SetLcdGeometry(
                library,
                vectors
                    .as_ref()
                    .map_or(std::ptr::null(), |vectors| vectors.as_ptr()),
            );
            if !library.is_null() {
                let _ = fontdone_c_abi::FT_Done_FreeType(library);
            }
            error
        }
        Backend::Wasm => {
            let vectors = geometry.map(|geometry| {
                geometry.map(|vector| fontdone_wasm::FontdoneWasmVector {
                    x: vector.x,
                    y: vector.y,
                })
            });
            fontdone_wasm::fontdone_wasm_library_set_lcd_geometry(
                i32::from(library_present),
                vectors
                    .as_ref()
                    .map_or(std::ptr::null(), |vectors| vectors.as_ptr()),
            )
        }
    };
    status_json(
        error,
        format!("{{\"error\":{error},\"feature_branch\":\"subpixel_rendering\"}}"),
    )
}

fn subpixel_rendering_output(backend: Backend, args: &[String]) -> Result<String, String> {
    let [operation, library_present, value] = args else {
        return Err(
            "subpixel-rendering requires an oracle operation, library flag, and value".to_string(),
        );
    };
    let library_present = parse_present(library_present)?;
    match operation.as_str() {
        "--set-lcd-filter" => {
            subpixel_filter_output(backend, library_present, &parse_filters(value)?)
        }
        "--set-lcd-filter-weights" => {
            subpixel_weights_output(backend, library_present, parse_weights(value)?)
        }
        "--set-lcd-geometry" => Ok(subpixel_geometry_output(
            backend,
            library_present,
            value != "-",
        )),
        _ => Err(format!(
            "unsupported subpixel-rendering operation {operation}"
        )),
    }
}

fn lcd_filtering_disabled_output(backend: Backend, args: &[String]) -> Result<String, String> {
    let [operation, library_present, value] = args else {
        return Err(
            "lcd-filtering-disabled requires an oracle operation, library flag, and value"
                .to_string(),
        );
    };
    let library_present = parse_present(library_present)?;
    match operation.as_str() {
        "--set-lcd-filter" => {
            let filters = parse_filters(value)?;
            let mut rust_library = match backend {
                Backend::Rust if library_present => Some(FT_Init_FreeType()),
                _ => None,
            };
            let mut c_library = std::ptr::null_mut();
            if matches!(backend, Backend::CAbi) && library_present {
                let error = fontdone_c_abi::FT_Init_FreeType(&mut c_library);
                if error != 0 {
                    return Ok(status_json(error, "null".to_string()));
                }
            }
            let mut first_error = 0;
            let mut rows = Vec::new();
            for filter in filters {
                let error = match backend {
                    Backend::Rust => FT_Library_SetLcdFilter(rust_library.as_mut(), filter),
                    Backend::CAbi => fontdone_c_abi::FT_Library_SetLcdFilter(c_library, filter),
                    Backend::Wasm => fontdone_wasm::fontdone_wasm_library_set_lcd_filter(filter),
                };
                if first_error == 0 && error != 0 {
                    first_error = error;
                }
                rows.push(format!("{{\"filter\":{filter},\"error\":{error}}}"));
            }
            if !c_library.is_null() {
                let _ = fontdone_c_abi::FT_Done_FreeType(c_library);
            }
            Ok(status_json(
                first_error,
                format!("{{\"outputs\":[{}]}}", rows.join(",")),
            ))
        }
        "--set-lcd-filter-weights" => {
            let mut weights = parse_weights(value)?;
            let copied = copied_weights(weights.as_deref());
            let error = match backend {
                Backend::Rust => {
                    let mut library = library_present.then(FT_Init_FreeType);
                    FT_Library_SetLcdFilterWeights(library.as_mut(), copied)
                }
                Backend::CAbi => {
                    let mut library = std::ptr::null_mut();
                    if library_present {
                        let init_error = fontdone_c_abi::FT_Init_FreeType(&mut library);
                        if init_error != 0 {
                            return Ok(status_json(init_error, "null".to_string()));
                        }
                    }
                    let pointer = weights
                        .as_mut()
                        .map_or(std::ptr::null_mut(), |weights| weights.as_mut_ptr());
                    let error = fontdone_c_abi::FT_Library_SetLcdFilterWeights(library, pointer);
                    if !library.is_null() {
                        let _ = fontdone_c_abi::FT_Done_FreeType(library);
                    }
                    error
                }
                Backend::Wasm => {
                    let pointer = weights
                        .as_mut()
                        .map_or(std::ptr::null_mut(), |weights| weights.as_mut_ptr());
                    fontdone_wasm::fontdone_wasm_library_set_lcd_filter_weights(pointer)
                }
            };
            Ok(status_json(error, format!("{{\"error\":{error}}}")))
        }
        _ => Err(format!(
            "unsupported lcd-filtering-disabled operation {operation}"
        )),
    }
}

fn main() -> ExitCode {
    let feature = match std::env::args().nth(1).as_deref() {
        Some("bzip2") => OptionalFeature::Bzip2,
        Some("color-layers") => OptionalFeature::ColorLayers,
        Some("lcd-filtering-disabled") => OptionalFeature::LcdFilteringDisabled,
        Some("lzw") => OptionalFeature::Lzw,
        Some("subpixel-rendering") => OptionalFeature::SubpixelRendering,
        _ => {
            eprintln!(
                "usage: optional_feature_probe \
                 <bzip2|color-layers|lcd-filtering-disabled|lzw|subpixel-rendering> \
                 <rust|c-abi|wasm> [font-path face-index]"
            );
            return ExitCode::from(2);
        }
    };
    let backend = match std::env::args().nth(2).as_deref() {
        Some("rust") => Backend::Rust,
        Some("c-abi") => Backend::CAbi,
        Some("wasm") => Backend::Wasm,
        _ => {
            eprintln!(
                "usage: optional_feature_probe \
                 <bzip2|color-layers|lcd-filtering-disabled|lzw|subpixel-rendering> \
                 <rust|c-abi|wasm> [font-path face-index]"
            );
            return ExitCode::from(2);
        }
    };
    let result = match feature {
        OptionalFeature::Bzip2 => Ok(bzip2_output(backend)),
        OptionalFeature::ColorLayers => {
            let Some(path) = std::env::args().nth(3) else {
                eprintln!("color-layers probe requires font-path and face-index");
                return ExitCode::from(2);
            };
            let face_index = match std::env::args().nth(4).and_then(|value| value.parse().ok()) {
                Some(value) => value,
                None => {
                    eprintln!("color-layers probe requires a numeric face-index");
                    return ExitCode::from(2);
                }
            };
            color_layers_output(backend, &path, face_index)
        }
        OptionalFeature::LcdFilteringDisabled => {
            let args = std::env::args().skip(3).collect::<Vec<_>>();
            lcd_filtering_disabled_output(backend, &args)
        }
        OptionalFeature::Lzw => Ok(lzw_output(backend)),
        OptionalFeature::SubpixelRendering => {
            let args = std::env::args().skip(3).collect::<Vec<_>>();
            subpixel_rendering_output(backend, &args)
        }
    };
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!("{result}");
    if matches!(feature, OptionalFeature::SubpixelRendering)
        || result.contains(&format!("\"error_code\":{UNIMPLEMENTED_FEATURE}"))
    {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
