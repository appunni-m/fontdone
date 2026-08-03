//! Low-level WebAssembly ABI for the pure-Rust `fontdone` engine.
//!
//! This crate produces a `wasm32-unknown-unknown` module for direct
//! `WebAssembly` hosts. It is not a `wasm-bindgen`, WASI, component-model, or
//! text-layout package. Rust applications should use the `fontdone` crate.
//!
//! # Linear-memory contract
//!
//! On wasm32, pointer and `usize` values are 32-bit byte offsets into the
//! instance's exported little-endian memory. Every pointer/length pair must
//! remain in bounds for the synchronous call. Callers allocate with
//! `fontdone_wasm_malloc`, free with the identical size, and reacquire
//! `memory.buffer` views after calls that can grow memory. Handles and borrowed
//! outputs are instance-local and become invalid at the lifecycle boundary
//! documented in `abi.json` and the package README.
//!
//! The generated `abi.json` file is authoritative for all exports and
//! `#[repr(C)]` field offsets. `fontdone_wasm.d.ts` describes the smaller direct
//! Node-host subset promoted for application use.

mod implementation;

pub use implementation::*;

macro_rules! document_wasm_entry_points {
    ($($name:ident),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Exports `",
                stringify!($name),
                "` through the wasm32 linear-memory ABI."
            )]
            ///
            /// Pointer-like arguments are offsets in this instance's memory.
            /// Exact scalar lowering, record layout, ownership, and invalidation
            /// rules are defined by `abi.json` and the crate-level contract.
            pub use implementation::$name;
        )+
    };
}

#[cfg(feature = "abi-test-support")]
macro_rules! document_wasm_test_support {
    ($($name:ident),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Exposes WebAssembly-facade verification data for `",
                stringify!($name),
                "`."
            )]
            ///
            /// This helper exists for the maintained cross-facade parity harness
            /// and is not part of the promoted direct-host subset.
            pub use implementation::$name;
        )+
    };
}

document_wasm_entry_points!(
    fontdone_wasm_list_add,
    fontdone_wasm_list_insert,
    fontdone_wasm_list_find,
    fontdone_wasm_list_remove,
    fontdone_wasm_list_up,
    fontdone_wasm_list_iterate,
    fontdone_wasm_list_finalize,
    fontdone_wasm_bitmap_init,
    fontdone_wasm_bitmap_new,
    fontdone_wasm_bitmap_copy,
    fontdone_wasm_bitmap_convert,
    fontdone_wasm_bitmap_done,
    fontdone_wasm_bitmap_embolden,
    fontdone_wasm_bitmap_blend,
    fontdone_wasm_palette_data_get,
    fontdone_wasm_palette_select,
    fontdone_wasm_palette_set_foreground_color,
    fontdone_wasm_get_color_glyph_layer,
    fontdone_wasm_get_color_glyph_clipbox,
    fontdone_wasm_get_color_glyph_paint,
    fontdone_wasm_get_paint,
    fontdone_wasm_get_paint_layers,
    fontdone_wasm_get_colorline_stops,
    fontdone_wasm_truetype_gx_free,
    fontdone_wasm_truetype_gx_validate,
    fontdone_wasm_classic_kern_free,
    fontdone_wasm_classic_kern_validate,
    fontdone_wasm_malloc,
    fontdone_wasm_free,
    fontdone_wasm_gzip_uncompress,
    fontdone_wasm_stream_open_gzip,
    fontdone_wasm_stream_open_bzip2,
    fontdone_wasm_stream_open_lzw,
    fontdone_wasm_node_unref,
    fontdone_wasm_open_face,
    fontdone_wasm_open_face_handle,
    fontdone_wasm_open_external_stream_face,
    fontdone_wasm_open_face_with_name_options,
    fontdone_wasm_interpreter_version_open,
    fontdone_wasm_ps_hinting_engine_open,
    fontdone_wasm_done_face,
    fontdone_wasm_new_size,
    fontdone_wasm_new_size_out,
    fontdone_wasm_activate_size,
    fontdone_wasm_done_size,
    fontdone_wasm_active_size,
    fontdone_wasm_done_freetype,
    fontdone_wasm_face_check_truetype_patents,
    fontdone_wasm_face_set_unpatented_hinting,
    fontdone_wasm_outline_get_cbox,
    fontdone_wasm_glyph_get_cbox,
    fontdone_wasm_get_glyph,
    fontdone_wasm_get_glyph_from_face,
    fontdone_wasm_glyph_copy,
    fontdone_wasm_done_glyph,
    fontdone_wasm_done_glyph_handle,
    fontdone_wasm_glyph_transform,
    fontdone_wasm_new_glyph,
    fontdone_wasm_glyph_to_bitmap,
    fontdone_wasm_glyph_to_bitmap_handle,
    fontdone_wasm_outline_get_bbox,
    fontdone_wasm_outline_get_bitmap,
    fontdone_wasm_outline_render,
    fontdone_wasm_outline_get_orientation,
    fontdone_wasm_outline_check,
    fontdone_wasm_outline_copy,
    fontdone_wasm_outline_embolden,
    fontdone_wasm_outline_embolden_xy,
    fontdone_wasm_outline_get_inside_border,
    fontdone_wasm_outline_get_outside_border,
    fontdone_wasm_outline_new,
    fontdone_wasm_outline_done,
    fontdone_wasm_outline_reverse,
    fontdone_wasm_outline_transform,
    fontdone_wasm_outline_translate,
    fontdone_wasm_library_set_lcd_filter,
    fontdone_wasm_library_set_lcd_filter_weights,
    fontdone_wasm_library_set_lcd_geometry,
    fontdone_wasm_get_truetype_engine_type,
    fontdone_wasm_property_get,
    fontdone_wasm_property_set_then_get,
    fontdone_wasm_property_increase_x_height_set_then_get,
    fontdone_wasm_property_glyph_to_script_map_invalid_face,
    fontdone_wasm_property_increase_x_height_invalid_face,
    fontdone_wasm_face_properties_one,
    fontdone_wasm_mul_div,
    fontdone_wasm_mul_fix,
    fontdone_wasm_div_fix,
    fontdone_wasm_round_fix,
    fontdone_wasm_ceil_fix,
    fontdone_wasm_floor_fix,
    fontdone_wasm_sin,
    fontdone_wasm_cos,
    fontdone_wasm_tan,
    fontdone_wasm_atan2,
    fontdone_wasm_angle_diff,
    fontdone_wasm_vector_unit,
    fontdone_wasm_vector_rotate,
    fontdone_wasm_vector_length,
    fontdone_wasm_vector_polarize,
    fontdone_wasm_vector_from_polar,
    fontdone_wasm_vector_transform,
    fontdone_wasm_matrix_multiply,
    fontdone_wasm_matrix_invert,
    fontdone_wasm_error_string,
    fontdone_wasm_open_type_validate,
    fontdone_wasm_open_type_free,
    fontdone_wasm_set_pixel_sizes,
    fontdone_wasm_set_transform,
    fontdone_wasm_set_char_size,
    fontdone_wasm_request_size,
    fontdone_wasm_select_size,
    fontdone_wasm_get_char_index,
    fontdone_wasm_get_char_variant_index,
    fontdone_wasm_get_char_variant_is_default,
    fontdone_wasm_get_variant_selectors,
    fontdone_wasm_get_variants_of_char,
    fontdone_wasm_get_chars_of_variant,
    fontdone_wasm_get_kerning,
    fontdone_wasm_get_pfr_kerning,
    fontdone_wasm_get_pfr_metrics,
    fontdone_wasm_get_pfr_advance,
    fontdone_wasm_select_charmap,
    fontdone_wasm_get_charmap_count,
    fontdone_wasm_get_active_charmap_index,
    fontdone_wasm_get_charmap,
    fontdone_wasm_get_cmap_format,
    fontdone_wasm_get_cmap_language_id,
    fontdone_wasm_set_charmap,
    fontdone_wasm_set_charmap_from_face,
    fontdone_wasm_set_var_design_coordinates,
    fontdone_wasm_get_var_design_coordinates,
    fontdone_wasm_get_var_blend_coordinates,
    fontdone_wasm_get_mm_blend_coordinates,
    fontdone_wasm_set_var_blend_coordinates,
    fontdone_wasm_set_mm_blend_coordinates,
    fontdone_wasm_get_fstype_flags,
    fontdone_wasm_attach_stream,
    fontdone_wasm_get_track_kerning,
    fontdone_wasm_get_gasp,
    fontdone_wasm_get_glyph_name,
    fontdone_wasm_get_name_index,
    fontdone_wasm_get_postscript_name,
    fontdone_wasm_get_font_format,
    fontdone_wasm_get_x11_font_format,
    fontdone_wasm_set_named_instance,
    fontdone_wasm_get_default_named_instance,
    fontdone_wasm_get_multi_master,
    fontdone_wasm_get_mm_var,
    fontdone_wasm_get_var_axis_flags,
    fontdone_wasm_set_mm_design_coordinates,
    fontdone_wasm_set_mm_weight_vector,
    fontdone_wasm_get_mm_weight_vector,
    fontdone_wasm_get_winfnt_header,
    fontdone_wasm_get_ps_font_info,
    fontdone_wasm_get_ps_font_private,
    fontdone_wasm_has_ps_glyph_names,
    fontdone_wasm_get_ps_font_value,
    fontdone_wasm_get_bdf_property,
    fontdone_wasm_get_bdf_charset_id,
    fontdone_wasm_get_cid_is_internally_cid_keyed,
    fontdone_wasm_get_cid_from_glyph_index,
    fontdone_wasm_get_cid_registry_ordering_supplement,
    fontdone_wasm_get_sfnt_name_count,
    fontdone_wasm_get_sfnt_name,
    fontdone_wasm_get_sfnt_os2,
    fontdone_wasm_get_sfnt_vhea,
    fontdone_wasm_get_sfnt_maxp,
    fontdone_wasm_load_sfnt_table,
    fontdone_wasm_sfnt_table_info,
    fontdone_wasm_get_first_char,
    fontdone_wasm_get_next_char,
    fontdone_wasm_library_version,
    fontdone_wasm_load_char,
    fontdone_wasm_load_glyph,
    fontdone_wasm_get_advance,
    fontdone_wasm_get_advances,
    fontdone_wasm_get_subglyph_info,
    fontdone_wasm_render_glyph,
    fontdone_wasm_bitmap_buffer,
    fontdone_wasm_bitmap_len,
    fontdone_wasm_bitmap_width,
    fontdone_wasm_bitmap_rows,
    fontdone_wasm_bitmap_pitch,
    fontdone_wasm_glyphslot_oblique,
    fontdone_wasm_glyphslot_embolden,
    fontdone_wasm_glyphslot_own_bitmap,
    fontdone_wasm_glyphslot_adjust_weight,
    fontdone_wasm_glyphslot_slant,
    fontdone_wasm_get_slot,
    fontdone_wasm_size_metrics,
);

#[cfg(feature = "abi-test-support")]
document_wasm_test_support!(
    abi_palette_data_snapshot,
    abi_palette_select_snapshot,
    abi_palette_select_without_output,
    abi_palette_mutate_entry,
    abi_support_colr_v1_paint_layer_iterator,
    abi_support_colr_v1_paint_colorline,
    abi_support_colr_v1_paint_linear_gradient,
    abi_support_colr_v1_paint_transform,
    abi_support_colr_v1_paint_graph,
    abi_support_colr_v1_public_paint_solid,
    abi_outline_glyph_snapshot,
    abi_support_corrupt_outline_glyph_for_render_failure,
    abi_support_corrupt_outline_glyph_record,
    abi_bitmap_glyph_snapshot,
    abi_svg_glyph_snapshot,
    abi_support_zero_length_svg_glyph,
    abi_support_new_glyph_allocation_failure,
    abi_face_info,
    abi_face_stream_info,
    abi_face_available_sizes,
    abi_face_names,
    abi_slot_snapshot,
    abi_glyphslot_set_own_bitmap,
    abi_uint32_list,
    abi_support_gzip_stream_bytes,
    abi_support_gzip_stream_close,
    abi_support_bzip2_stream_bytes,
    abi_support_bzip2_stream_close,
    abi_support_bzip2_stream_is_open,
    abi_support_lzw_stream_bytes,
    abi_support_lzw_stream_close,
    abi_support_glyph_stroke_outline_success,
    abi_support_glyph_stroke_destroy_option,
    abi_support_glyph_stroke_border_outside_success,
    abi_support_glyph_stroke_border_inside_success,
    abi_support_glyph_stroke_border_destroy_option,
    abi_support_outline_render_direct_spans,
    abi_support_outline_decompose_trace,
    abi_support_stroker_null_noop,
    abi_support_stroker_lifecycle,
    abi_support_stroker_zero_line,
    abi_support_stroker_simple_line_counts,
    abi_support_stroker_open_line_geometry,
    abi_support_stroker_closed_line_geometry,
    abi_support_stroker_first_segment,
    abi_support_stroker_closed_end_subpath,
    abi_support_stroker_conic_success,
    abi_support_stroker_conic_first_segment,
    abi_support_stroker_cubic_success,
    abi_support_stroker_cubic_first_segment,
    abi_support_stroker_parse_opened_outline,
    abi_support_stroker_finalized_counts,
    abi_support_stroker_reset_counts,
    abi_support_stroker_rewind_attributes,
    abi_support_stroker_set_miter_limit,
    abi_support_stroker_miter_join_geometry,
    abi_support_stroker_bevel_join_geometry,
    abi_support_stroker_parse_degenerate,
    abi_support_stroker_end_subpath_no_segment,
    abi_support_stroker_degenerate_curve,
    abi_support_subpixel_lcd_filter,
    abi_support_subpixel_lcd_filter_weights,
    abi_property_glyph_to_script_map_snapshot,
    abi_property_glyph_to_script_map_mutate,
    abi_support_set_default_properties,
    abi_face_properties_state,
    abi_support_truetype_engine_observation,
    abi_support_debug_hook_classes,
    abi_support_add_default_modules,
    abi_support_add_default_modules_observation,
    abi_support_add_minimal_module_observation,
    abi_support_add_synthetic_module_observation,
    abi_support_module_class_lifecycle_observation,
    abi_support_raster_lifecycle_observation,
    abi_support_raster_new_error_observation,
    abi_support_raster_class_probe,
    abi_support_raster_set_mode_observation,
    abi_support_module_remove_lifecycle_observation,
    abi_support_library_final_destroy_observation,
    abi_support_new_library_observation,
    abi_support_custom_memory_lifecycle,
    abi_support_custom_glyph_lifecycle,
    abi_support_glyph_copy_failure_cleanup,
    abi_support_incremental_opaque_handle,
    abi_support_incremental_glyph_lifecycle,
    abi_support_incremental_state_lifecycle,
    abi_support_incremental_callback_table_contract,
    abi_support_reference_library_observation,
    abi_support_reference_then_done_library_observation,
    abi_support_final_done_library_observation,
    abi_support_default_module_flags,
    abi_support_default_module_present,
    abi_support_module_interface_present,
    abi_support_module_requester_service_available,
    abi_support_default_renderer_class,
    abi_support_null_renderer_class,
    abi_support_set_default_outline_renderer,
    abi_support_init_free_type_created_library,
    abi_support_done_mm_var,
    abi_support_get_and_done_mm_var,
    abi_mm_var_namedstyles,
    abi_support_enable_open_type_validator,
    abi_support_enable_gx_validator,
    abi_support_face_driver_name,
    abi_sfnt_load_name_diagnostic,
    abi_open_face_non_driver_diagnostic,
    abi_truetype_context_allocation_failure_diagnostic,
    abi_set_unsupported_glyph_slot,
    abi_set_malformed_get_glyph_slot,
    abi_set_outline_glyph_slot_advance,
    abi_glyphslot_own_bitmap_copy_allocation_failure,
    abi_fvar_namedstyle_coords,
    fontdone_wasm_svg_renderer_capture,
);

#[cfg(all(test, feature = "abi-test-support"))]
mod abi_contract_tests {
    //! ABI-only checks for raw helpers without a pinned-C parity analogue.
    //!
    //! These run in the package preflight and are excluded from the unified
    //! coverage matrix; they protect the direct WASM handle and allocator
    //! contract without counting unit-only execution as parity evidence.

    use std::ptr;

    use fontdone::ffi::{FT_Err_Invalid_Argument, FT_Err_Invalid_Face_Handle, FT_Err_Ok};

    #[test]
    fn raw_helper_lifecycle_and_null_contract() {
        assert!(super::fontdone_wasm_bitmap_buffer(0).is_null());
        assert_eq!(super::fontdone_wasm_bitmap_len(0), 0);
        assert_eq!(super::fontdone_wasm_bitmap_width(0), 0);
        assert_eq!(super::fontdone_wasm_bitmap_rows(0), 0);
        assert_eq!(super::fontdone_wasm_bitmap_pitch(0), 0);
        assert_eq!(
            i64::from(super::fontdone_wasm_done_face(0)),
            FT_Err_Invalid_Face_Handle
        );

        let zero_allocation = super::fontdone_wasm_malloc(0);
        assert!(!zero_allocation.is_null());
        super::fontdone_wasm_free(zero_allocation, 0);
        super::fontdone_wasm_free(ptr::null_mut(), usize::MAX);

        let mut wasm_error = FT_Err_Ok;
        assert_eq!(
            super::fontdone_wasm_open_face_handle(ptr::null(), 0, 0, 20.0, ptr::null_mut(),),
            0
        );
        assert_eq!(
            super::fontdone_wasm_open_face_handle(ptr::null(), 0, 0, 20.0, &mut wasm_error,),
            0
        );
        assert_eq!(wasm_error, FT_Err_Invalid_Argument);

        let font = include_bytes!("../../tests/fixtures/input/fonts/DejaVuSans.ttf");
        let handle = super::fontdone_wasm_open_face_handle(
            font.as_ptr(),
            font.len(),
            0,
            20.0,
            &mut wasm_error,
        );
        assert_ne!(handle, 0);
        assert_eq!(wasm_error, FT_Err_Ok);
        assert!(super::fontdone_wasm_bitmap_buffer(handle).is_null());
        assert_eq!(super::fontdone_wasm_bitmap_len(handle), 0);
        assert_eq!(super::fontdone_wasm_bitmap_width(handle), 0);
        assert_eq!(super::fontdone_wasm_bitmap_rows(handle), 0);
        assert_eq!(super::fontdone_wasm_bitmap_pitch(handle), 0);
        assert_eq!(super::fontdone_wasm_done_face(handle), FT_Err_Ok);
    }
}
