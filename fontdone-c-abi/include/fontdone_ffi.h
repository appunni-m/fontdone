#ifndef FONTDONE_C_ABI_H
#define FONTDONE_C_ABI_H

#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>

/* Common constants used by the maintained integration path. The complete
 * measured function disposition is packaged as API_SUPPORT.md. */
#define FT_Err_Ok 0

#ifdef __cplusplus
extern "C" {
#endif

typedef int32_t FT_Error;
typedef unsigned char FT_Bool;
typedef int FT_Int;
typedef unsigned int FT_UInt;
typedef int32_t FT_Int32;
typedef uint32_t FT_UInt32;
typedef FT_UInt32 FT_Tag;
typedef unsigned char FT_Byte;
typedef const FT_Byte* FT_Bytes;
typedef signed char FT_Char;
typedef short FT_FWord;
typedef unsigned short FT_UFWord;
typedef long FT_Long;
typedef unsigned long FT_ULong;
typedef size_t FT_Offset;
typedef ptrdiff_t FT_PtrDist;
typedef long FT_Pos;
typedef long FT_Fixed;
typedef long FT_Angle;
typedef long FT_F26Dot6;
typedef short FT_F2Dot14;
typedef short FT_Short;
typedef unsigned short FT_UShort;
typedef char FT_String;

#ifdef __cplusplus
#define FT_STATIC_CAST(type, var) static_cast<type>(var)
#define FT_REINTERPRET_CAST(type, var) reinterpret_cast<type>(var)
#define FT_STATIC_BYTE_CAST(type, var) \
  static_cast<type>(static_cast<unsigned char>(var))
#else
#define FT_STATIC_CAST(type, var) (type)(var)
#define FT_REINTERPRET_CAST(type, var) (type)(var)
#define FT_STATIC_BYTE_CAST(type, var) (type)(unsigned char)(var)
#endif
#define FT_UNUSED(arg) ((arg) = (arg))

#include "fontdone_macros.h"

typedef enum FT_Encoding_ {
  FT_ENC_TAG(FT_ENCODING_NONE, 0, 0, 0, 0),
  FT_ENC_TAG(FT_ENCODING_MS_SYMBOL, 's', 'y', 'm', 'b'),
  FT_ENC_TAG(FT_ENCODING_UNICODE, 'u', 'n', 'i', 'c'),
  FT_ENC_TAG(FT_ENCODING_SJIS, 's', 'j', 'i', 's'),
  FT_ENC_TAG(FT_ENCODING_PRC, 'g', 'b', ' ', ' '),
  FT_ENC_TAG(FT_ENCODING_BIG5, 'b', 'i', 'g', '5'),
  FT_ENC_TAG(FT_ENCODING_WANSUNG, 'w', 'a', 'n', 's'),
  FT_ENC_TAG(FT_ENCODING_JOHAB, 'j', 'o', 'h', 'a'),
  FT_ENCODING_GB2312 = FT_ENCODING_PRC,
  FT_ENCODING_MS_SJIS = FT_ENCODING_SJIS,
  FT_ENCODING_MS_GB2312 = FT_ENCODING_PRC,
  FT_ENCODING_MS_BIG5 = FT_ENCODING_BIG5,
  FT_ENCODING_MS_WANSUNG = FT_ENCODING_WANSUNG,
  FT_ENCODING_MS_JOHAB = FT_ENCODING_JOHAB,
  FT_ENC_TAG(FT_ENCODING_ADOBE_STANDARD, 'A', 'D', 'O', 'B'),
  FT_ENC_TAG(FT_ENCODING_ADOBE_EXPERT, 'A', 'D', 'B', 'E'),
  FT_ENC_TAG(FT_ENCODING_ADOBE_CUSTOM, 'A', 'D', 'B', 'C'),
  FT_ENC_TAG(FT_ENCODING_ADOBE_LATIN_1, 'l', 'a', 't', '1'),
  FT_ENC_TAG(FT_ENCODING_OLD_LATIN_2, 'l', 'a', 't', '2'),
  FT_ENC_TAG(FT_ENCODING_APPLE_ROMAN, 'a', 'r', 'm', 'n')
} FT_Encoding;

typedef enum FT_Glyph_Format_ {
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_NONE, 0, 0, 0, 0),
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_COMPOSITE, 'c', 'o', 'm', 'p'),
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_BITMAP, 'b', 'i', 't', 's'),
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_OUTLINE, 'o', 'u', 't', 'l'),
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_PLOTTER, 'p', 'l', 'o', 't'),
  FT_IMAGE_TAG(FT_GLYPH_FORMAT_SVG, 'S', 'V', 'G', ' ')
} FT_Glyph_Format;

typedef enum FT_Glyph_BBox_Mode_ {
  FT_GLYPH_BBOX_UNSCALED = 0,
  FT_GLYPH_BBOX_SUBPIXELS = 0,
  FT_GLYPH_BBOX_GRIDFIT = 1,
  FT_GLYPH_BBOX_TRUNCATE = 2,
  FT_GLYPH_BBOX_PIXELS = 3
} FT_Glyph_BBox_Mode;

typedef enum FT_Kerning_Mode_ {
  FT_KERNING_DEFAULT = 0,
  FT_KERNING_UNFITTED,
  FT_KERNING_UNSCALED
} FT_Kerning_Mode;

typedef enum FT_LcdFilter_ {
  FT_LCD_FILTER_NONE = 0,
  FT_LCD_FILTER_DEFAULT = 1,
  FT_LCD_FILTER_LIGHT = 2,
  FT_LCD_FILTER_LEGACY1 = 3,
  FT_LCD_FILTER_LEGACY = 16,
  FT_LCD_FILTER_MAX
} FT_LcdFilter;

typedef enum FT_Orientation_ {
  FT_ORIENTATION_TRUETYPE = 0,
  FT_ORIENTATION_POSTSCRIPT = 1,
  FT_ORIENTATION_FILL_RIGHT = FT_ORIENTATION_TRUETYPE,
  FT_ORIENTATION_FILL_LEFT = FT_ORIENTATION_POSTSCRIPT,
  FT_ORIENTATION_NONE
} FT_Orientation;

typedef enum FT_PaintExtend_ {
  FT_COLR_PAINT_EXTEND_PAD = 0,
  FT_COLR_PAINT_EXTEND_REPEAT = 1,
  FT_COLR_PAINT_EXTEND_REFLECT = 2
} FT_PaintExtend;

typedef enum FT_Composite_Mode_ {
  FT_COLR_COMPOSITE_CLEAR = 0,
  FT_COLR_COMPOSITE_SRC = 1,
  FT_COLR_COMPOSITE_DEST = 2,
  FT_COLR_COMPOSITE_SRC_OVER = 3,
  FT_COLR_COMPOSITE_DEST_OVER = 4,
  FT_COLR_COMPOSITE_SRC_IN = 5,
  FT_COLR_COMPOSITE_DEST_IN = 6,
  FT_COLR_COMPOSITE_SRC_OUT = 7,
  FT_COLR_COMPOSITE_DEST_OUT = 8,
  FT_COLR_COMPOSITE_SRC_ATOP = 9,
  FT_COLR_COMPOSITE_DEST_ATOP = 10,
  FT_COLR_COMPOSITE_XOR = 11,
  FT_COLR_COMPOSITE_PLUS = 12,
  FT_COLR_COMPOSITE_SCREEN = 13,
  FT_COLR_COMPOSITE_OVERLAY = 14,
  FT_COLR_COMPOSITE_DARKEN = 15,
  FT_COLR_COMPOSITE_LIGHTEN = 16,
  FT_COLR_COMPOSITE_COLOR_DODGE = 17,
  FT_COLR_COMPOSITE_COLOR_BURN = 18,
  FT_COLR_COMPOSITE_HARD_LIGHT = 19,
  FT_COLR_COMPOSITE_SOFT_LIGHT = 20,
  FT_COLR_COMPOSITE_DIFFERENCE = 21,
  FT_COLR_COMPOSITE_EXCLUSION = 22,
  FT_COLR_COMPOSITE_MULTIPLY = 23,
  FT_COLR_COMPOSITE_HSL_HUE = 24,
  FT_COLR_COMPOSITE_HSL_SATURATION = 25,
  FT_COLR_COMPOSITE_HSL_COLOR = 26,
  FT_COLR_COMPOSITE_HSL_LUMINOSITY = 27,
  FT_COLR_COMPOSITE_MAX = 28
} FT_Composite_Mode;

typedef enum FT_PaintFormat_ {
  FT_COLR_PAINTFORMAT_COLR_LAYERS = 1,
  FT_COLR_PAINTFORMAT_SOLID = 2,
  FT_COLR_PAINTFORMAT_LINEAR_GRADIENT = 4,
  FT_COLR_PAINTFORMAT_RADIAL_GRADIENT = 6,
  FT_COLR_PAINTFORMAT_SWEEP_GRADIENT = 8,
  FT_COLR_PAINTFORMAT_GLYPH = 10,
  FT_COLR_PAINTFORMAT_COLR_GLYPH = 11,
  FT_COLR_PAINTFORMAT_TRANSFORM = 12,
  FT_COLR_PAINTFORMAT_TRANSLATE = 14,
  FT_COLR_PAINTFORMAT_SCALE = 16,
  FT_COLR_PAINTFORMAT_ROTATE = 24,
  FT_COLR_PAINTFORMAT_SKEW = 28,
  FT_COLR_PAINTFORMAT_COMPOSITE = 32,
  FT_COLR_PAINT_FORMAT_MAX = 33,
  FT_COLR_PAINTFORMAT_UNSUPPORTED = 255
} FT_PaintFormat;

typedef enum FT_Pixel_Mode_ {
  FT_PIXEL_MODE_NONE = 0,
  FT_PIXEL_MODE_MONO,
  FT_PIXEL_MODE_GRAY,
  FT_PIXEL_MODE_GRAY2,
  FT_PIXEL_MODE_GRAY4,
  FT_PIXEL_MODE_LCD,
  FT_PIXEL_MODE_LCD_V,
  FT_PIXEL_MODE_BGRA,
  FT_PIXEL_MODE_MAX
} FT_Pixel_Mode;

typedef enum FT_Render_Mode_ {
  FT_RENDER_MODE_NORMAL = 0,
  FT_RENDER_MODE_LIGHT,
  FT_RENDER_MODE_MONO,
  FT_RENDER_MODE_LCD,
  FT_RENDER_MODE_LCD_V,
  FT_RENDER_MODE_SDF,
  FT_RENDER_MODE_MAX
} FT_Render_Mode;

typedef enum FT_Sfnt_Tag_ {
  FT_SFNT_HEAD,
  FT_SFNT_MAXP,
  FT_SFNT_OS2,
  FT_SFNT_HHEA,
  FT_SFNT_VHEA,
  FT_SFNT_POST,
  FT_SFNT_PCLT,
  FT_SFNT_MAX
} FT_Sfnt_Tag;

typedef enum FT_Size_Request_Type_ {
  FT_SIZE_REQUEST_TYPE_NOMINAL,
  FT_SIZE_REQUEST_TYPE_REAL_DIM,
  FT_SIZE_REQUEST_TYPE_BBOX,
  FT_SIZE_REQUEST_TYPE_CELL,
  FT_SIZE_REQUEST_TYPE_SCALES,
  FT_SIZE_REQUEST_TYPE_MAX
} FT_Size_Request_Type;

typedef enum FT_StrokerBorder_ {
  FT_STROKER_BORDER_LEFT = 0,
  FT_STROKER_BORDER_RIGHT
} FT_StrokerBorder;

typedef enum FT_TrueTypeEngineType_ {
  FT_TRUETYPE_ENGINE_TYPE_NONE = 0,
  FT_TRUETYPE_ENGINE_TYPE_UNPATENTED,
  FT_TRUETYPE_ENGINE_TYPE_PATENTED
} FT_TrueTypeEngineType;

typedef enum T1_Blend_Flags_ {
  T1_BLEND_UNDERLINE_POSITION = 0,
  T1_BLEND_UNDERLINE_THICKNESS,
  T1_BLEND_ITALIC_ANGLE,
  T1_BLEND_BLUE_VALUES,
  T1_BLEND_OTHER_BLUES,
  T1_BLEND_STANDARD_WIDTH,
  T1_BLEND_STANDARD_HEIGHT,
  T1_BLEND_STEM_SNAP_WIDTHS,
  T1_BLEND_STEM_SNAP_HEIGHTS,
  T1_BLEND_BLUE_SCALE,
  T1_BLEND_BLUE_SHIFT,
  T1_BLEND_FAMILY_BLUES,
  T1_BLEND_FAMILY_OTHER_BLUES,
  T1_BLEND_FORCE_BOLD,
  T1_BLEND_MAX
} T1_Blend_Flags;

typedef enum FT_Stroker_LineJoin_ {
  FT_STROKER_LINEJOIN_ROUND = 0,
  FT_STROKER_LINEJOIN_BEVEL = 1,
  FT_STROKER_LINEJOIN_MITER_VARIABLE = 2,
  FT_STROKER_LINEJOIN_MITER = FT_STROKER_LINEJOIN_MITER_VARIABLE,
  FT_STROKER_LINEJOIN_MITER_FIXED = 3
} FT_Stroker_LineJoin;
typedef enum FT_Stroker_LineCap_ {
  FT_STROKER_LINECAP_BUTT = 0,
  FT_STROKER_LINECAP_ROUND,
  FT_STROKER_LINECAP_SQUARE
} FT_Stroker_LineCap;
typedef enum FT_Color_Root_Transform_ {
  FT_COLOR_INCLUDE_ROOT_TRANSFORM,
  FT_COLOR_NO_ROOT_TRANSFORM,
  FT_COLOR_ROOT_TRANSFORM_MAX
} FT_Color_Root_Transform;
typedef FT_Error (*FT_DebugHook_Func)(void* arg);
typedef void* FT_Pointer;
typedef FT_Pointer FT_Module_Interface;
typedef void (*FT_Generic_Finalizer)(void* object);

typedef struct FT_LibraryRec_* FT_Library;
typedef struct FT_FaceRec_* FT_Face;
typedef struct FT_SizeRec_* FT_Size;
typedef struct FT_GlyphSlotRec_* FT_GlyphSlot;
typedef struct FT_GlyphRec_* FT_Glyph;
typedef struct FT_BitmapGlyphRec_* FT_BitmapGlyph;
typedef struct FT_OutlineGlyphRec_* FT_OutlineGlyph;
typedef struct FT_SvgGlyphRec_* FT_SvgGlyph;
typedef struct FT_SVG_DocumentRec_* FT_SVG_Document;
typedef struct FT_RendererRec_* FT_Renderer;
typedef struct FT_ModuleRec_* FT_Module;
typedef struct FT_DriverRec_* FT_Driver;
typedef struct FT_CharMapRec_* FT_CharMap;
typedef struct FT_StrokerRec_* FT_Stroker;
typedef struct FT_SubGlyphRec_* FT_SubGlyph;
typedef struct FT_Face_InternalRec_* FT_Face_Internal;
typedef struct FT_Size_InternalRec_* FT_Size_Internal;
typedef struct FT_Slot_InternalRec_* FT_Slot_Internal;
typedef struct FT_ListNodeRec_* FT_ListNode;
typedef struct FT_ListRec_* FT_List;
typedef struct FT_MemoryRec_* FT_Memory;
typedef struct FT_RasterRec_* FT_Raster;
typedef struct FT_IncrementalRec_* FT_Incremental;
typedef struct FTC_NodeRec_* FTC_Node;
typedef struct FTC_ManagerRec_* FTC_Manager;
typedef struct FTC_CMapCacheRec_* FTC_CMapCache;
typedef struct FTC_ImageCacheRec_* FTC_ImageCache;
typedef struct FTC_SBitCacheRec_* FTC_SBitCache;
typedef FT_Pointer FTC_FaceID;

typedef struct FTC_ScalerRec_ {
  FTC_FaceID face_id;
  FT_UInt width;
  FT_UInt height;
  FT_Int pixel;
  FT_UInt x_res;
  FT_UInt y_res;
} FTC_ScalerRec;
typedef struct FTC_ScalerRec_* FTC_Scaler;

typedef struct FTC_ImageTypeRec_ {
  FTC_FaceID face_id;
  FT_UInt width;
  FT_UInt height;
  FT_Int32 flags;
} FTC_ImageTypeRec;
typedef struct FTC_ImageTypeRec_* FTC_ImageType;

typedef struct FTC_SBitRec_ {
  FT_Byte width;
  FT_Byte height;
  FT_Char left;
  FT_Char top;
  FT_Byte format;
  FT_Byte max_grays;
  FT_Short pitch;
  FT_Char xadvance;
  FT_Char yadvance;
  FT_Byte* buffer;
} FTC_SBitRec;
typedef struct FTC_SBitRec_* FTC_SBit;

typedef FT_Error (*FT_Module_Constructor)(FT_Module module);
typedef void (*FT_Module_Destructor)(FT_Module module);
typedef FT_Module_Interface (*FT_Module_Requester)(FT_Module module, const char* name);
typedef void (*FT_Custom_Log_Handler)(const char* ft_component, const char* fmt, va_list args);
typedef FT_Error (*FTC_Face_Requester)(
    FTC_FaceID face_id,
    FT_Library library,
    FT_Pointer req_data,
    FT_Face* aface);

typedef struct FT_ListNodeRec_ {
  FT_ListNode prev;
  FT_ListNode next;
  FT_Pointer data;
} FT_ListNodeRec;

typedef struct FT_ListRec_ {
  FT_ListNode head;
  FT_ListNode tail;
} FT_ListRec;

typedef struct FT_Generic_ {
  FT_Pointer data;
  FT_Generic_Finalizer finalizer;
} FT_Generic;

typedef FT_Error (*FT_List_Iterator)(FT_ListNode node, void* user);
typedef void* (*FT_Alloc_Func)(FT_Memory memory, long size);
typedef void (*FT_Free_Func)(FT_Memory memory, void* block);
typedef void* (*FT_Realloc_Func)(FT_Memory memory, long cur_size, long new_size, void* block);
typedef void (*FT_List_Destructor)(FT_Memory memory, void* data, void* user);

typedef struct FT_MemoryRec_ {
  void* user;
  FT_Alloc_Func alloc;
  FT_Free_Func free;
  FT_Realloc_Func realloc;
} FT_MemoryRec;

typedef union FT_StreamDesc_ {
  long value;
  void* pointer;
} FT_StreamDesc;

typedef struct FT_StreamRec_* FT_Stream;
typedef unsigned long (*FT_Stream_IoFunc)(FT_Stream stream, unsigned long offset, unsigned char* buffer, unsigned long count);
typedef void (*FT_Stream_CloseFunc)(FT_Stream stream);

typedef struct FT_StreamRec_ {
  unsigned char* base;
  unsigned long size;
  unsigned long pos;
  FT_StreamDesc descriptor;
  FT_StreamDesc pathname;
  FT_Stream_IoFunc read;
  FT_Stream_CloseFunc close;
  FT_Memory memory;
  unsigned char* cursor;
  unsigned char* limit;
} FT_StreamRec;

typedef struct FT_Vector_ {
  FT_Pos x;
  FT_Pos y;
} FT_Vector;

typedef struct FT_UnitVector_ {
  FT_F2Dot14 x;
  FT_F2Dot14 y;
} FT_UnitVector;

typedef struct FT_Matrix_ {
  FT_Fixed xx;
  FT_Fixed xy;
  FT_Fixed yx;
  FT_Fixed yy;
} FT_Matrix;

typedef struct FT_BBox_ {
  FT_Pos xMin;
  FT_Pos yMin;
  FT_Pos xMax;
  FT_Pos yMax;
} FT_BBox;

typedef FT_Error (*FT_Glyph_InitFunc)(FT_Glyph glyph, FT_GlyphSlot slot);
typedef void (*FT_Glyph_DoneFunc)(FT_Glyph glyph);
typedef void (*FT_Glyph_TransformFunc)(
    FT_Glyph glyph,
    const FT_Matrix* matrix,
    const FT_Vector* delta);
typedef void (*FT_Glyph_GetBBoxFunc)(FT_Glyph glyph, FT_BBox* abbox);
typedef FT_Error (*FT_Glyph_CopyFunc)(FT_Glyph source, FT_Glyph target);
typedef FT_Error (*FT_Glyph_PrepareFunc)(FT_Glyph glyph, FT_GlyphSlot slot);

#define FT_Glyph_Init_Func FT_Glyph_InitFunc
#define FT_Glyph_Done_Func FT_Glyph_DoneFunc
#define FT_Glyph_Transform_Func FT_Glyph_TransformFunc
#define FT_Glyph_BBox_Func FT_Glyph_GetBBoxFunc
#define FT_Glyph_Copy_Func FT_Glyph_CopyFunc
#define FT_Glyph_Prepare_Func FT_Glyph_PrepareFunc

typedef int (*FT_Outline_MoveToFunc)(const FT_Vector* to, void* user);
typedef int (*FT_Outline_LineToFunc)(const FT_Vector* to, void* user);
typedef int (*FT_Outline_ConicToFunc)(
    const FT_Vector* control,
    const FT_Vector* to,
    void* user);
typedef int (*FT_Outline_CubicToFunc)(
    const FT_Vector* control1,
    const FT_Vector* control2,
    const FT_Vector* to,
    void* user);

#define FT_Outline_MoveTo_Func FT_Outline_MoveToFunc
#define FT_Outline_LineTo_Func FT_Outline_LineToFunc
#define FT_Outline_ConicTo_Func FT_Outline_ConicToFunc
#define FT_Outline_CubicTo_Func FT_Outline_CubicToFunc

typedef struct FT_Outline_Funcs_ {
  FT_Outline_MoveToFunc move_to;
  FT_Outline_LineToFunc line_to;
  FT_Outline_ConicToFunc conic_to;
  FT_Outline_CubicToFunc cubic_to;
  int shift;
  FT_Pos delta;
} FT_Outline_Funcs;

typedef struct FT_Data_ {
  const FT_Byte* pointer;
  FT_UInt length;
} FT_Data;

typedef struct FT_Incremental_MetricsRec_ {
  FT_Long bearing_x;
  FT_Long bearing_y;
  FT_Long advance;
  FT_Long advance_v;
} FT_Incremental_MetricsRec;

typedef struct FT_Incremental_MetricsRec_* FT_Incremental_Metrics;
typedef FT_Error (*FT_Incremental_GetGlyphDataFunc)(
    FT_Incremental incremental,
    FT_UInt glyph_index,
    FT_Data* adata);
typedef void (*FT_Incremental_FreeGlyphDataFunc)(
    FT_Incremental incremental,
    FT_Data* data);
typedef FT_Error (*FT_Incremental_GetGlyphMetricsFunc)(
    FT_Incremental incremental,
    FT_UInt glyph_index,
    FT_Bool vertical,
    FT_Incremental_MetricsRec* ametrics);

typedef struct FT_Incremental_FuncsRec_ {
  FT_Incremental_GetGlyphDataFunc get_glyph_data;
  FT_Incremental_FreeGlyphDataFunc free_glyph_data;
  FT_Incremental_GetGlyphMetricsFunc get_glyph_metrics;
} FT_Incremental_FuncsRec;

typedef struct FT_Incremental_InterfaceRec_ {
  const FT_Incremental_FuncsRec* funcs;
  FT_Incremental object;
} FT_Incremental_InterfaceRec;

typedef FT_Incremental_InterfaceRec* FT_Incremental_Interface;

typedef struct FT_ClipBox_ {
  FT_Vector bottom_left;
  FT_Vector top_left;
  FT_Vector top_right;
  FT_Vector bottom_right;
} FT_ClipBox;

typedef struct FT_Parameter_ {
  FT_ULong tag;
  void* data;
} FT_Parameter;

typedef struct FT_Module_Class_ {
  FT_ULong module_flags;
  FT_Long module_size;
  const FT_String* module_name;
  FT_Fixed module_version;
  FT_Fixed module_requires;
  const void* module_interface;
  FT_Module_Constructor module_init;
  FT_Module_Destructor module_done;
  FT_Module_Requester get_interface;
} FT_Module_Class;

typedef struct FT_MM_Axis_ {
  FT_String* name;
  FT_Long minimum;
  FT_Long maximum;
} FT_MM_Axis;

typedef struct FT_Multi_Master_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_MM_Axis axis[4];
} FT_Multi_Master;

typedef struct FT_Var_Axis_ {
  FT_String* name;
  FT_Fixed minimum;
  FT_Fixed def;
  FT_Fixed maximum;
  FT_ULong tag;
  FT_UInt strid;
} FT_Var_Axis;

typedef struct FT_Var_Named_Style_ {
  FT_Fixed* coords;
  FT_UInt strid;
  FT_UInt psid;
} FT_Var_Named_Style;

typedef struct FT_MM_Var_ {
  FT_UInt num_axis;
  FT_UInt num_designs;
  FT_UInt num_namedstyles;
  FT_Var_Axis* axis;
  FT_Var_Named_Style* namedstyle;
} FT_MM_Var;

typedef struct FT_WinFNT_HeaderRec_ {
  FT_UShort version;
  FT_ULong file_size;
  FT_Byte copyright[60];
  FT_UShort file_type;
  FT_UShort nominal_point_size;
  FT_UShort vertical_resolution;
  FT_UShort horizontal_resolution;
  FT_UShort ascent;
  FT_UShort internal_leading;
  FT_UShort external_leading;
  FT_Byte italic;
  FT_Byte underline;
  FT_Byte strike_out;
  FT_UShort weight;
  FT_Byte charset;
  FT_UShort pixel_width;
  FT_UShort pixel_height;
  FT_Byte pitch_and_family;
  FT_UShort avg_width;
  FT_UShort max_width;
  FT_Byte first_char;
  FT_Byte last_char;
  FT_Byte default_char;
  FT_Byte break_char;
  FT_UShort bytes_per_row;
  FT_ULong device_offset;
  FT_ULong face_name_offset;
  FT_ULong bits_pointer;
  FT_ULong bits_offset;
  FT_Byte reserved;
  FT_ULong flags;
  FT_UShort A_space;
  FT_UShort B_space;
  FT_UShort C_space;
  FT_UShort color_table_offset;
  FT_ULong reserved1[4];
} FT_WinFNT_HeaderRec;

typedef struct FT_WinFNT_HeaderRec_* FT_WinFNT_Header;

typedef enum BDF_PropertyType_ {
  BDF_PROPERTY_TYPE_NONE = 0,
  BDF_PROPERTY_TYPE_ATOM = 1,
  BDF_PROPERTY_TYPE_INTEGER = 2,
  BDF_PROPERTY_TYPE_CARDINAL = 3
} BDF_PropertyType;

typedef struct BDF_PropertyRec_ {
  BDF_PropertyType type;
  union {
    const char* atom;
    FT_Int32 integer;
    FT_UInt32 cardinal;
  } u;
} BDF_PropertyRec;

typedef BDF_PropertyRec* BDF_Property;

typedef enum T1_EncodingType_ {
  T1_ENCODING_TYPE_NONE = 0,
  T1_ENCODING_TYPE_ARRAY = 1,
  T1_ENCODING_TYPE_STANDARD = 2,
  T1_ENCODING_TYPE_ISOLATIN1 = 3,
  T1_ENCODING_TYPE_EXPERT = 4
} T1_EncodingType;

typedef enum PS_Dict_Keys_ {
  PS_DICT_FONT_TYPE = 0,
  PS_DICT_FONT_MATRIX = 1,
  PS_DICT_FONT_BBOX = 2,
  PS_DICT_PAINT_TYPE = 3,
  PS_DICT_FONT_NAME = 4,
  PS_DICT_UNIQUE_ID = 5,
  PS_DICT_NUM_CHAR_STRINGS = 6,
  PS_DICT_CHAR_STRING_KEY = 7,
  PS_DICT_CHAR_STRING = 8,
  PS_DICT_ENCODING_TYPE = 9,
  PS_DICT_ENCODING_ENTRY = 10,
  PS_DICT_NUM_SUBRS = 11,
  PS_DICT_SUBR = 12,
  PS_DICT_STD_HW = 13,
  PS_DICT_STD_VW = 14,
  PS_DICT_NUM_BLUE_VALUES = 15,
  PS_DICT_BLUE_VALUE = 16,
  PS_DICT_BLUE_FUZZ = 17,
  PS_DICT_NUM_OTHER_BLUES = 18,
  PS_DICT_OTHER_BLUE = 19,
  PS_DICT_NUM_FAMILY_BLUES = 20,
  PS_DICT_FAMILY_BLUE = 21,
  PS_DICT_NUM_FAMILY_OTHER_BLUES = 22,
  PS_DICT_FAMILY_OTHER_BLUE = 23,
  PS_DICT_BLUE_SCALE = 24,
  PS_DICT_BLUE_SHIFT = 25,
  PS_DICT_NUM_STEM_SNAP_H = 26,
  PS_DICT_STEM_SNAP_H = 27,
  PS_DICT_NUM_STEM_SNAP_V = 28,
  PS_DICT_STEM_SNAP_V = 29,
  PS_DICT_FORCE_BOLD = 30,
  PS_DICT_RND_STEM_UP = 31,
  PS_DICT_MIN_FEATURE = 32,
  PS_DICT_LEN_IV = 33,
  PS_DICT_PASSWORD = 34,
  PS_DICT_LANGUAGE_GROUP = 35,
  PS_DICT_VERSION = 36,
  PS_DICT_NOTICE = 37,
  PS_DICT_FULL_NAME = 38,
  PS_DICT_FAMILY_NAME = 39,
  PS_DICT_WEIGHT = 40,
  PS_DICT_IS_FIXED_PITCH = 41,
  PS_DICT_UNDERLINE_POSITION = 42,
  PS_DICT_UNDERLINE_THICKNESS = 43,
  PS_DICT_FS_TYPE = 44,
  PS_DICT_ITALIC_ANGLE = 45,
  PS_DICT_MAX = PS_DICT_ITALIC_ANGLE
} PS_Dict_Keys;

typedef struct PS_FontInfoRec_ {
  char* version;
  char* notice;
  char* full_name;
  char* family_name;
  char* weight;
  FT_Fixed italic_angle;
  FT_Bool is_fixed_pitch;
  FT_Short underline_position;
  FT_UShort underline_thickness;
} PS_FontInfoRec;

typedef PS_FontInfoRec* PS_FontInfo;
typedef PS_FontInfoRec T1_FontInfo;

typedef struct PS_PrivateRec_ {
  FT_Int unique_id;
  FT_Int lenIV;
  FT_Byte num_blue_values;
  FT_Byte num_other_blues;
  FT_Byte num_family_blues;
  FT_Byte num_family_other_blues;
  FT_Short blue_values[14];
  FT_Short other_blues[10];
  FT_Short family_blues[14];
  FT_Short family_other_blues[10];
  FT_Fixed blue_scale;
  FT_Int blue_shift;
  FT_Int blue_fuzz;
  FT_UShort standard_width[1];
  FT_UShort standard_height[1];
  FT_Byte num_snap_widths;
  FT_Byte num_snap_heights;
  FT_Bool force_bold;
  FT_Bool round_stem_up;
  FT_Short snap_widths[13];
  FT_Short snap_heights[13];
  FT_Fixed expansion_factor;
  FT_Long language_group;
  FT_Long password;
  FT_Short min_feature[2];
} PS_PrivateRec;

typedef PS_PrivateRec* PS_Private;
typedef PS_PrivateRec T1_Private;

typedef struct FT_Open_Args_ {
  FT_UInt flags;
  const FT_Byte* memory_base;
  FT_Long memory_size;
  FT_String* pathname;
  FT_Stream stream;
  FT_Module driver;
  FT_Int num_params;
  FT_Parameter* params;
} FT_Open_Args;

typedef struct FT_Outline_ {
  FT_UShort n_contours;
  FT_UShort n_points;
  FT_Vector* points;
  unsigned char* tags;
  unsigned short* contours;
  FT_Int flags;
} FT_Outline;

typedef struct FT_Glyph_Class_ {
  FT_Long glyph_size;
  FT_Glyph_Format glyph_format;
  FT_Glyph_InitFunc glyph_init;
  FT_Glyph_DoneFunc glyph_done;
  FT_Glyph_CopyFunc glyph_copy;
  FT_Glyph_TransformFunc glyph_transform;
  FT_Glyph_GetBBoxFunc glyph_bbox;
  FT_Glyph_PrepareFunc glyph_prepare;
} FT_Glyph_Class;

typedef struct FT_GlyphRec_ {
  FT_Library library;
  const FT_Glyph_Class* clazz;
  FT_Glyph_Format format;
  FT_Vector advance;
} FT_GlyphRec;

typedef struct FT_Glyph_Metrics_ {
  FT_Pos width;
  FT_Pos height;
  FT_Pos horiBearingX;
  FT_Pos horiBearingY;
  FT_Pos horiAdvance;
  FT_Pos vertBearingX;
  FT_Pos vertBearingY;
  FT_Pos vertAdvance;
} FT_Glyph_Metrics;

typedef struct FT_Size_Metrics_ {
  FT_UShort x_ppem;
  FT_UShort y_ppem;
  FT_Fixed x_scale;
  FT_Fixed y_scale;
  FT_Pos ascender;
  FT_Pos descender;
  FT_Pos height;
  FT_Pos max_advance;
} FT_Size_Metrics;

typedef struct FT_Size_RequestRec_ {
  FT_Size_Request_Type type;
  FT_Long width;
  FT_Long height;
  FT_UInt horiResolution;
  FT_UInt vertResolution;
} FT_Size_RequestRec;
typedef FT_Size_RequestRec* FT_Size_Request;

const char* FT_Error_String(FT_Error error_code);

typedef struct FT_Bitmap_ {
  uint32_t rows;
  uint32_t width;
  FT_Int pitch;
  unsigned char* buffer;
  FT_UShort num_grays;
  unsigned char pixel_mode;
  unsigned char palette_mode;
  void* palette;
} FT_Bitmap;

typedef struct FT_Bitmap_Size_ {
  FT_Short height;
  FT_Short width;
  FT_Pos size;
  FT_Pos x_ppem;
  FT_Pos y_ppem;
} FT_Bitmap_Size;

typedef struct FT_BitmapGlyphRec_ {
  FT_GlyphRec root;
  FT_Int left;
  FT_Int top;
  FT_Bitmap bitmap;
} FT_BitmapGlyphRec;

typedef struct FT_OutlineGlyphRec_ {
  FT_GlyphRec root;
  FT_Outline outline;
} FT_OutlineGlyphRec;

typedef struct FT_SVG_DocumentRec_ {
  FT_Byte* svg_document;
  FT_ULong svg_document_length;
  FT_Size_Metrics metrics;
  FT_UShort units_per_EM;
  FT_UShort start_glyph_id;
  FT_UShort end_glyph_id;
  FT_Matrix transform;
  FT_Vector delta;
} FT_SVG_DocumentRec;

typedef struct FT_SvgGlyphRec_ {
  FT_GlyphRec root;
  FT_Byte* svg_document;
  FT_ULong svg_document_length;
  FT_UInt glyph_index;
  FT_Size_Metrics metrics;
  FT_UShort units_per_EM;
  FT_UShort start_glyph_id;
  FT_UShort end_glyph_id;
  FT_Matrix transform;
  FT_Vector delta;
} FT_SvgGlyphRec;

typedef struct FT_Prop_GlyphToScriptMap_ {
  FT_Face face;
  FT_UShort* map;
} FT_Prop_GlyphToScriptMap;

typedef struct FT_Prop_IncreaseXHeight_ {
  FT_Face face;
  FT_UInt limit;
} FT_Prop_IncreaseXHeight;

typedef struct FT_Span_ {
  unsigned short x;
  unsigned short len;
  unsigned char coverage;
} FT_Span;

typedef void (*FT_SpanFunc)(int y, int count, const FT_Span* spans, void* user);
typedef int (*FT_Raster_BitTest_Func)(int y, int x, void* user);
typedef void (*FT_Raster_BitSet_Func)(int y, int x, void* user);

#define FT_Raster_Span_Func FT_SpanFunc

typedef struct FT_Raster_Params_ {
  const FT_Bitmap* target;
  const void* source;
  int flags;
  FT_SpanFunc gray_spans;
  FT_SpanFunc black_spans;
  FT_Raster_BitTest_Func bit_test;
  FT_Raster_BitSet_Func bit_set;
  void* user;
  FT_BBox clip_box;
} FT_Raster_Params;

typedef int (*FT_Raster_NewFunc)(void* memory, FT_Raster* raster);
typedef void (*FT_Raster_DoneFunc)(FT_Raster raster);
typedef void (*FT_Raster_ResetFunc)(
    FT_Raster raster,
    unsigned char* pool_base,
    unsigned long pool_size);
typedef int (*FT_Raster_SetModeFunc)(
    FT_Raster raster,
    unsigned long mode,
    void* args);
typedef int (*FT_Raster_RenderFunc)(
    FT_Raster raster,
    const FT_Raster_Params* params);

#define FT_Raster_New_Func FT_Raster_NewFunc
#define FT_Raster_Done_Func FT_Raster_DoneFunc
#define FT_Raster_Reset_Func FT_Raster_ResetFunc
#define FT_Raster_Set_Mode_Func FT_Raster_SetModeFunc
#define FT_Raster_Render_Func FT_Raster_RenderFunc

typedef struct FT_Raster_Funcs_ {
  FT_Glyph_Format glyph_format;
  FT_Raster_NewFunc raster_new;
  FT_Raster_ResetFunc raster_reset;
  FT_Raster_SetModeFunc raster_set_mode;
  FT_Raster_RenderFunc raster_render;
  FT_Raster_DoneFunc raster_done;
} FT_Raster_Funcs;

typedef FT_Error (*FT_Renderer_RenderFunc)(
    FT_Renderer renderer,
    FT_GlyphSlot slot,
    FT_Render_Mode mode,
    const FT_Vector* origin);
typedef FT_Error (*FT_Renderer_TransformFunc)(
    FT_Renderer renderer,
    FT_GlyphSlot slot,
    const FT_Matrix* matrix,
    const FT_Vector* delta);
typedef void (*FT_Renderer_GetCBoxFunc)(
    FT_Renderer renderer,
    FT_GlyphSlot slot,
    FT_BBox* cbox);
typedef FT_Error (*FT_Renderer_SetModeFunc)(
    FT_Renderer renderer,
    FT_ULong mode_tag,
    FT_Pointer mode_ptr);

typedef struct FT_Renderer_Class_ {
  FT_Module_Class root;
  FT_Glyph_Format glyph_format;
  FT_Renderer_RenderFunc render_glyph;
  FT_Renderer_TransformFunc transform_glyph;
  FT_Renderer_GetCBoxFunc get_glyph_cbox;
  FT_Renderer_SetModeFunc set_mode;
  const FT_Raster_Funcs* raster_class;
} FT_Renderer_Class;

typedef struct FT_Color_ {
  FT_Byte blue;
  FT_Byte green;
  FT_Byte red;
  FT_Byte alpha;
} FT_Color;

typedef struct FT_Palette_Data_ {
  FT_UShort num_palettes;
  const FT_UShort* palette_name_ids;
  const FT_UShort* palette_flags;
  FT_UShort num_palette_entries;
  const FT_UShort* palette_entry_name_ids;
} FT_Palette_Data;

typedef struct FT_LayerIterator_ {
  FT_UInt num_layers;
  FT_UInt layer;
  FT_Byte* p;
} FT_LayerIterator;

typedef struct FT_Opaque_Paint_ {
  FT_Byte* p;
  FT_Bool insert_root_transform;
} FT_OpaquePaint;

typedef struct FT_ColorStopIterator_ {
  FT_UInt num_color_stops;
  FT_UInt current_color_stop;
  FT_Byte* p;
  FT_Bool read_variable;
} FT_ColorStopIterator;

typedef struct FT_ColorIndex_ {
  uint16_t palette_index;
  FT_F2Dot14 alpha;
} FT_ColorIndex;

typedef struct FT_ColorStop_ {
  FT_Fixed stop_offset;
  FT_ColorIndex color;
} FT_ColorStop;

typedef struct FT_ColorLine_ {
  FT_PaintExtend extend;
  FT_ColorStopIterator color_stop_iterator;
} FT_ColorLine;

typedef struct FT_Affine_23_ {
  FT_Fixed xx;
  FT_Fixed xy;
  FT_Fixed dx;
  FT_Fixed yx;
  FT_Fixed yy;
  FT_Fixed dy;
} FT_Affine23;

typedef struct FT_PaintColrLayers_ {
  FT_LayerIterator layer_iterator;
} FT_PaintColrLayers;

typedef struct FT_PaintSolid_ {
  FT_ColorIndex color;
} FT_PaintSolid;

typedef struct FT_PaintLinearGradient_ {
  FT_ColorLine colorline;
  FT_Vector p0;
  FT_Vector p1;
  FT_Vector p2;
} FT_PaintLinearGradient;

typedef struct FT_PaintRadialGradient_ {
  FT_ColorLine colorline;
  FT_Vector c0;
  FT_Pos r0;
  FT_Vector c1;
  FT_Pos r1;
} FT_PaintRadialGradient;

typedef struct FT_PaintSweepGradient_ {
  FT_ColorLine colorline;
  FT_Vector center;
  FT_Fixed start_angle;
  FT_Fixed end_angle;
} FT_PaintSweepGradient;

typedef struct FT_PaintGlyph_ {
  FT_OpaquePaint paint;
  FT_UInt glyphID;
} FT_PaintGlyph;

typedef struct FT_PaintColrGlyph_ {
  FT_UInt glyphID;
} FT_PaintColrGlyph;

typedef struct FT_PaintTransform_ {
  FT_OpaquePaint paint;
  FT_Affine23 affine;
} FT_PaintTransform;

typedef struct FT_PaintTranslate_ {
  FT_OpaquePaint paint;
  FT_Fixed dx;
  FT_Fixed dy;
} FT_PaintTranslate;

typedef struct FT_PaintScale_ {
  FT_OpaquePaint paint;
  FT_Fixed scale_x;
  FT_Fixed scale_y;
  FT_Fixed center_x;
  FT_Fixed center_y;
} FT_PaintScale;

typedef struct FT_PaintRotate_ {
  FT_OpaquePaint paint;
  FT_Fixed angle;
  FT_Fixed center_x;
  FT_Fixed center_y;
} FT_PaintRotate;

typedef struct FT_PaintSkew_ {
  FT_OpaquePaint paint;
  FT_Fixed x_skew_angle;
  FT_Fixed y_skew_angle;
  FT_Fixed center_x;
  FT_Fixed center_y;
} FT_PaintSkew;

typedef struct FT_PaintComposite_ {
  FT_OpaquePaint source_paint;
  FT_Composite_Mode composite_mode;
  FT_OpaquePaint backdrop_paint;
} FT_PaintComposite;

typedef union FT_COLR_PaintUnion_ {
  FT_PaintColrLayers colr_layers;
  FT_PaintGlyph glyph;
  FT_PaintSolid solid;
  FT_PaintLinearGradient linear_gradient;
  FT_PaintRadialGradient radial_gradient;
  FT_PaintSweepGradient sweep_gradient;
  FT_PaintTransform transform;
  FT_PaintTranslate translate;
  FT_PaintScale scale;
  FT_PaintRotate rotate;
  FT_PaintSkew skew;
  FT_PaintComposite composite;
  FT_PaintColrGlyph colr_glyph;
} FT_COLR_PaintUnion;

typedef struct FT_COLR_Paint_ {
  FT_PaintFormat format;
  FT_COLR_PaintUnion u;
} FT_COLR_Paint;

void FT_Bitmap_Init(FT_Bitmap* abitmap);
void FT_Bitmap_New(FT_Bitmap* abitmap);
FT_Error FT_Gzip_Uncompress(FT_Memory memory, FT_Byte* output, FT_ULong* output_len, const FT_Byte* input, FT_ULong input_len);
FT_Error FT_Stream_OpenBzip2(FT_Stream stream, FT_Stream source);
FT_Error FT_Stream_OpenGzip(FT_Stream stream, FT_Stream source);
FT_Error FT_Stream_OpenLZW(FT_Stream stream, FT_Stream source);
void FTC_Node_Unref(FTC_Node node, FTC_Manager manager);
FT_Error FTC_Manager_New(FT_Library library, FT_UInt max_faces, FT_UInt max_sizes, FT_ULong max_bytes, FTC_Face_Requester requester, FT_Pointer req_data, FTC_Manager* amanager);
void FTC_Manager_Reset(FTC_Manager manager);
void FTC_Manager_Done(FTC_Manager manager);
FT_Error FTC_Manager_LookupFace(FTC_Manager manager, FTC_FaceID face_id, FT_Face* aface);
FT_Error FTC_Manager_LookupSize(FTC_Manager manager, FTC_Scaler scaler, FT_Size* asize);
void FTC_Manager_RemoveFaceID(FTC_Manager manager, FTC_FaceID face_id);
FT_Error FTC_CMapCache_New(FTC_Manager manager, FTC_CMapCache* acache);
FT_UInt FTC_CMapCache_Lookup(FTC_CMapCache cache, FTC_FaceID face_id, FT_Int cmap_index, FT_UInt32 char_code);
FT_Error FTC_ImageCache_New(FTC_Manager manager, FTC_ImageCache* acache);
FT_Error FTC_ImageCache_Lookup(FTC_ImageCache cache, FTC_ImageType type, FT_UInt gindex, FT_Glyph* aglyph, FTC_Node* anode);
FT_Error FTC_ImageCache_LookupScaler(FTC_ImageCache cache, FTC_Scaler scaler, FT_ULong load_flags, FT_UInt gindex, FT_Glyph* aglyph, FTC_Node* anode);
FT_Error FTC_SBitCache_New(FTC_Manager manager, FTC_SBitCache* acache);
FT_Error FTC_SBitCache_Lookup(FTC_SBitCache cache, FTC_ImageType type, FT_UInt gindex, FTC_SBit* sbit, FTC_Node* anode);
FT_Error FTC_SBitCache_LookupScaler(FTC_SBitCache cache, FTC_Scaler scaler, FT_ULong load_flags, FT_UInt gindex, FTC_SBit* sbit, FTC_Node* anode);
FT_Error FT_Bitmap_Copy(FT_Library library, const FT_Bitmap* source, FT_Bitmap* target);
FT_Error FT_Bitmap_Convert(FT_Library library, const FT_Bitmap* source, FT_Bitmap* target, FT_Int alignment);
FT_Error FT_Bitmap_Done(FT_Library library, FT_Bitmap* bitmap);
FT_Error FT_Bitmap_Embolden(FT_Library library, FT_Bitmap* bitmap, FT_Pos xStrength, FT_Pos yStrength);
FT_Error FT_Bitmap_Blend(FT_Library library, const FT_Bitmap* source, const FT_Vector source_offset, FT_Bitmap* target, FT_Vector* atarget_offset, FT_Color color);
FT_Error FT_GlyphSlot_Own_Bitmap(FT_GlyphSlot slot);
FT_Error FT_Palette_Data_Get(FT_Face face, FT_Palette_Data* apalette_data);
FT_Error FT_Palette_Select(FT_Face face, FT_UShort palette_index, FT_Color** apalette);
FT_Error FT_Palette_Set_Foreground_Color(FT_Face face, FT_Color foreground_color);
FT_Bool FT_Get_Color_Glyph_Layer(FT_Face face, FT_UInt base_glyph, FT_UInt* aglyph_index, FT_UInt* acolor_index, FT_LayerIterator* iterator);
FT_Bool FT_Get_Color_Glyph_ClipBox(FT_Face face, FT_UInt base_glyph, FT_ClipBox* clip_box);
FT_Bool FT_Get_Color_Glyph_Paint(FT_Face face, FT_UInt base_glyph, FT_Color_Root_Transform root_transform, FT_OpaquePaint* paint);
FT_Bool FT_Get_Paint(FT_Face face, FT_OpaquePaint opaque_paint, FT_COLR_Paint* paint);
FT_Bool FT_Get_Paint_Layers(FT_Face face, FT_LayerIterator* layer_iterator, FT_OpaquePaint* paint);
FT_Bool FT_Get_Colorline_Stops(FT_Face face, FT_ColorStop* color_stop, FT_ColorStopIterator* iterator);
void FT_TrueTypeGX_Free(FT_Face face, FT_Bytes table);
void FT_ClassicKern_Free(FT_Face face, FT_Bytes table);
FT_Error FT_ClassicKern_Validate(FT_Face face, FT_UInt validation_flags, FT_Bytes* ckern_table);
FT_Error FT_TrueTypeGX_Validate(FT_Face face, FT_UInt validation_flags, FT_Bytes* tables, FT_UInt table_length);

typedef struct FT_SfntName_ {
  FT_UShort platform_id;
  FT_UShort encoding_id;
  FT_UShort language_id;
  FT_UShort name_id;
  FT_Byte* string;
  FT_UInt string_len;
} FT_SfntName;

typedef struct FT_SfntLangTag_ {
  FT_Byte* string;
  FT_UInt string_len;
} FT_SfntLangTag;

typedef struct FT_CharMapRec_ {
  FT_Face face;
  FT_Encoding encoding;
  FT_UShort platform_id;
  FT_UShort encoding_id;
} FT_CharMapRec;

typedef struct TT_Header_ {
  FT_Fixed Table_Version;
  FT_Fixed Font_Revision;
  FT_Long CheckSum_Adjust;
  FT_Long Magic_Number;
  FT_UShort Flags;
  FT_UShort Units_Per_EM;
  FT_ULong Created[2];
  FT_ULong Modified[2];
  FT_Short xMin;
  FT_Short yMin;
  FT_Short xMax;
  FT_Short yMax;
  FT_UShort Mac_Style;
  FT_UShort Lowest_Rec_PPEM;
  FT_Short Font_Direction;
  FT_Short Index_To_Loc_Format;
  FT_Short Glyph_Data_Format;
} TT_Header;

typedef struct TT_HoriHeader_ {
  FT_Fixed Version;
  FT_Short Ascender;
  FT_Short Descender;
  FT_Short Line_Gap;
  FT_UShort advance_Width_Max;
  FT_Short min_Left_Side_Bearing;
  FT_Short min_Right_Side_Bearing;
  FT_Short xMax_Extent;
  FT_Short caret_Slope_Rise;
  FT_Short caret_Slope_Run;
  FT_Short caret_Offset;
  FT_Short Reserved[4];
  FT_Short metric_Data_Format;
  FT_UShort number_Of_HMetrics;
  void* long_metrics;
  void* short_metrics;
} TT_HoriHeader;

typedef struct TT_MaxProfile_ {
  FT_Fixed version;
  FT_UShort numGlyphs;
  FT_UShort maxPoints;
  FT_UShort maxContours;
  FT_UShort maxCompositePoints;
  FT_UShort maxCompositeContours;
  FT_UShort maxZones;
  FT_UShort maxTwilightPoints;
  FT_UShort maxStorage;
  FT_UShort maxFunctionDefs;
  FT_UShort maxInstructionDefs;
  FT_UShort maxStackElements;
  FT_UShort maxSizeOfInstructions;
  FT_UShort maxComponentElements;
  FT_UShort maxComponentDepth;
} TT_MaxProfile;

typedef struct TT_OS2_ {
  FT_UShort version;
  FT_Short xAvgCharWidth;
  FT_UShort usWeightClass;
  FT_UShort usWidthClass;
  FT_UShort fsType;
  FT_Short ySubscriptXSize;
  FT_Short ySubscriptYSize;
  FT_Short ySubscriptXOffset;
  FT_Short ySubscriptYOffset;
  FT_Short ySuperscriptXSize;
  FT_Short ySuperscriptYSize;
  FT_Short ySuperscriptXOffset;
  FT_Short ySuperscriptYOffset;
  FT_Short yStrikeoutSize;
  FT_Short yStrikeoutPosition;
  FT_Short sFamilyClass;
  FT_Byte panose[10];
  FT_ULong ulUnicodeRange1;
  FT_ULong ulUnicodeRange2;
  FT_ULong ulUnicodeRange3;
  FT_ULong ulUnicodeRange4;
  FT_Char achVendID[4];
  FT_UShort fsSelection;
  FT_UShort usFirstCharIndex;
  FT_UShort usLastCharIndex;
  FT_Short sTypoAscender;
  FT_Short sTypoDescender;
  FT_Short sTypoLineGap;
  FT_UShort usWinAscent;
  FT_UShort usWinDescent;
  FT_ULong ulCodePageRange1;
  FT_ULong ulCodePageRange2;
  FT_Short sxHeight;
  FT_Short sCapHeight;
  FT_UShort usDefaultChar;
  FT_UShort usBreakChar;
  FT_UShort usMaxContext;
  FT_UShort usLowerOpticalPointSize;
  FT_UShort usUpperOpticalPointSize;
} TT_OS2;

typedef struct TT_PCLT_ {
  FT_Fixed Version;
  FT_ULong FontNumber;
  FT_UShort Pitch;
  FT_UShort xHeight;
  FT_UShort Style;
  FT_UShort TypeFamily;
  FT_UShort CapHeight;
  FT_UShort SymbolSet;
  FT_Char TypeFace[16];
  FT_Char CharacterComplement[8];
  FT_Char FileName[6];
  FT_Char StrokeWeight;
  FT_Char WidthType;
  FT_Byte SerifStyle;
  FT_Byte Reserved;
} TT_PCLT;

typedef struct TT_Postscript_ {
  FT_Fixed FormatType;
  FT_Fixed italicAngle;
  FT_Short underlinePosition;
  FT_Short underlineThickness;
  FT_ULong isFixedPitch;
  FT_ULong minMemType42;
  FT_ULong maxMemType42;
  FT_ULong minMemType1;
  FT_ULong maxMemType1;
} TT_Postscript;

typedef struct TT_VertHeader_ {
  FT_Fixed Version;
  FT_Short Ascender;
  FT_Short Descender;
  FT_Short Line_Gap;
  FT_UShort advance_Height_Max;
  FT_Short min_Top_Side_Bearing;
  FT_Short min_Bottom_Side_Bearing;
  FT_Short yMax_Extent;
  FT_Short caret_Slope_Rise;
  FT_Short caret_Slope_Run;
  FT_Short caret_Offset;
  FT_Short Reserved[4];
  FT_Short metric_Data_Format;
  FT_UShort number_Of_VMetrics;
  void* long_metrics;
  void* short_metrics;
} TT_VertHeader;

typedef struct FT_GlyphSlotRec_ {
  FT_Library library;
  FT_Face face;
  FT_GlyphSlot next;
  FT_UInt glyph_index;
  FT_Generic generic;
  FT_Glyph_Metrics metrics;
  FT_Fixed linearHoriAdvance;
  FT_Fixed linearVertAdvance;
  FT_Vector advance;
  FT_Glyph_Format format;
  FT_Bitmap bitmap;
  FT_Int bitmap_left;
  FT_Int bitmap_top;
  FT_Outline outline;
  FT_UInt num_subglyphs;
  FT_SubGlyph subglyphs;
  void* control_data;
  long control_len;
  FT_Pos lsb_delta;
  FT_Pos rsb_delta;
  void* other;
  FT_Slot_Internal internal;
} FT_GlyphSlotRec;

typedef struct FT_SizeRec_ {
  FT_Face face;
  FT_Generic generic;
  FT_Size_Metrics metrics;
  FT_Size_Internal internal;
} FT_SizeRec;

typedef struct FT_FaceRec_ {
  FT_Long num_faces;
  FT_Long face_index;
  FT_Long face_flags;
  FT_Long style_flags;
  FT_Long num_glyphs;
  FT_String* family_name;
  FT_String* style_name;
  FT_Int num_fixed_sizes;
  FT_Bitmap_Size* available_sizes;
  FT_Int num_charmaps;
  FT_CharMap* charmaps;
  FT_Generic generic;
  FT_BBox bbox;
  FT_UShort units_per_EM;
  FT_Short ascender;
  FT_Short descender;
  FT_Short height;
  FT_Short max_advance_width;
  FT_Short max_advance_height;
  FT_Short underline_position;
  FT_Short underline_thickness;
  FT_GlyphSlot glyph;
  FT_Size size;
  FT_CharMap charmap;
  FT_Driver driver;
  FT_Memory memory;
  FT_Stream stream;
  FT_ListRec sizes_list;
  FT_Generic autohint;
  void* extensions;
  FT_Face_Internal internal;
} FT_FaceRec;

struct FT_LibraryRec_ {
  void* internal;
};

FT_Error FT_Init_FreeType(FT_Library* alibrary);
FT_Error FT_Done_FreeType(FT_Library library);
FT_Error FT_New_Library(FT_Memory memory, FT_Library* alibrary);
FT_Error FT_Reference_Library(FT_Library library);
FT_Error FT_Done_Library(FT_Library library);
FT_Error FT_Get_MM_Var(FT_Face face, FT_MM_Var** amaster);
FT_Error FT_Done_MM_Var(FT_Library library, FT_MM_Var* amaster);
FT_Error FT_Get_Var_Axis_Flags(FT_MM_Var* master, FT_UInt axis_index, FT_UInt* flags);
FT_Error FT_Library_SetLcdFilter(FT_Library library, FT_LcdFilter filter);
FT_Error FT_Library_SetLcdFilterWeights(FT_Library library, unsigned char* weights);
FT_Error FT_Library_SetLcdGeometry(FT_Library library, FT_Vector* sub);
FT_TrueTypeEngineType FT_Get_TrueType_Engine_Type(FT_Library library);
FT_Error FT_Property_Get(FT_Library library, const FT_String* module_name, const FT_String* property_name, void* value);
FT_Error FT_Property_Set(FT_Library library, const FT_String* module_name, const FT_String* property_name, const void* value);
void FT_Set_Default_Properties(FT_Library library);
FT_Error FT_Face_Properties(FT_Face face, FT_UInt num_properties, FT_Parameter* properties);
void FT_Add_Default_Modules(FT_Library library);
FT_Error FT_Add_Module(FT_Library library, const FT_Module_Class* clazz);
FT_Module FT_Get_Module(FT_Library library, const char* module_name);
FT_Error FT_Remove_Module(FT_Library library, FT_Module module);
void FT_Set_Debug_Hook(FT_Library library, FT_UInt hook_index, FT_DebugHook_Func debug_hook);
void FT_Trace_Set_Level(const char* tracing_level);
void FT_Trace_Set_Default_Level(void);
void FT_Set_Log_Handler(FT_Custom_Log_Handler handler);
void FT_Set_Default_Log_Handler(void);
FT_Renderer FT_Get_Renderer(FT_Library library, FT_Glyph_Format format);
FT_Error FT_Set_Renderer(FT_Library library, FT_Renderer renderer, FT_UInt num_params, FT_Parameter* parameters);
FT_Long FT_MulDiv(FT_Long a, FT_Long b, FT_Long c);
FT_Long FT_MulFix(FT_Long a, FT_Long b);
FT_Long FT_DivFix(FT_Long a, FT_Long b);
FT_Fixed FT_RoundFix(FT_Fixed a);
FT_Fixed FT_CeilFix(FT_Fixed a);
FT_Fixed FT_FloorFix(FT_Fixed a);
FT_Fixed FT_Sin(FT_Angle angle);
FT_Fixed FT_Cos(FT_Angle angle);
FT_Fixed FT_Tan(FT_Angle angle);
FT_Angle FT_Atan2(FT_Fixed dx, FT_Fixed dy);
FT_Angle FT_Angle_Diff(FT_Angle angle1, FT_Angle angle2);
void FT_Vector_Unit(FT_Vector* vector, FT_Angle angle);
void FT_Vector_Rotate(FT_Vector* vector, FT_Angle angle);
FT_Fixed FT_Vector_Length(FT_Vector* vector);
void FT_Vector_Polarize(FT_Vector* vector, FT_Fixed* length, FT_Angle* angle);
void FT_Vector_From_Polar(FT_Vector* vector, FT_Fixed length, FT_Angle angle);
void FT_Vector_Transform(FT_Vector* vector, const FT_Matrix* matrix);
void FT_Matrix_Multiply(const FT_Matrix* a, FT_Matrix* b);
FT_Error FT_Matrix_Invert(FT_Matrix* matrix);
FT_Error FT_Open_Face(FT_Library library, const FT_Open_Args* args, FT_Long face_index, FT_Face* aface);
FT_Error FT_New_Face(FT_Library library, const char* filepathname, FT_Long face_index, FT_Face* aface);
FT_Error FT_Attach_Stream(FT_Face face, const FT_Open_Args* parameters);
FT_Error FT_Attach_File(FT_Face face, const char* filepathname);
FT_Error FT_New_Memory_Face(FT_Library library, const FT_Byte* file_base, FT_Long file_size, FT_Long face_index, FT_Face* aface);
FT_Error FT_Reference_Face(FT_Face face);
FT_Error FT_Done_Face(FT_Face face);
FT_Error FT_New_Size(FT_Face face, FT_Size* asize);
FT_Error FT_Done_Size(FT_Size size);
FT_Error FT_Activate_Size(FT_Size size);
FT_Bool FT_Face_CheckTrueTypePatents(FT_Face face);
FT_Bool FT_Face_SetUnpatentedHinting(FT_Face face, FT_Bool value);
void FT_Outline_Get_CBox(const FT_Outline* outline, FT_BBox* acbox);
void FT_Glyph_Get_CBox(FT_Glyph glyph, FT_UInt bbox_mode, FT_BBox* acbox);
FT_Error FT_Get_Glyph(FT_GlyphSlot slot, FT_Glyph* aglyph);
FT_Error FT_New_Glyph(FT_Library library, FT_Glyph_Format format, FT_Glyph* aglyph);
FT_Error FT_Glyph_Copy(FT_Glyph source, FT_Glyph* target);
void FT_Done_Glyph(FT_Glyph glyph);
FT_Error FT_Glyph_Transform(FT_Glyph glyph, const FT_Matrix* matrix, const FT_Vector* delta);
FT_Error FT_Glyph_To_Bitmap(FT_Glyph* the_glyph, FT_Render_Mode render_mode, const FT_Vector* origin, FT_Bool destroy);
FT_Error FT_Glyph_Stroke(FT_Glyph* pglyph, FT_Stroker stroker, FT_Bool destroy);
FT_Error FT_Glyph_StrokeBorder(FT_Glyph* pglyph, FT_Stroker stroker, FT_Bool inside, FT_Bool destroy);
FT_Error FT_Outline_Get_BBox(FT_Outline* outline, FT_BBox* abbox);
FT_Error FT_Outline_Get_Bitmap(FT_Library library, FT_Outline* outline, const FT_Bitmap* abitmap);
FT_Error FT_Outline_Render(FT_Library library, FT_Outline* outline, FT_Raster_Params* params);
FT_Error FT_Outline_Decompose(FT_Outline* outline, const FT_Outline_Funcs* func_interface, void* user);
FT_Error FT_Outline_Check(FT_Outline* outline);
FT_Error FT_Outline_Copy(const FT_Outline* source, FT_Outline* target);
FT_Error FT_Outline_New(FT_Library library, FT_UInt numPoints, FT_Int numContours, FT_Outline* anoutline);
FT_Error FT_Outline_Done(FT_Library library, FT_Outline* outline);
FT_Error FT_Outline_Embolden(FT_Outline* outline, FT_Pos strength);
FT_Error FT_Outline_EmboldenXY(FT_Outline* outline, FT_Pos xstrength, FT_Pos ystrength);
FT_StrokerBorder FT_Outline_GetInsideBorder(FT_Outline* outline);
FT_StrokerBorder FT_Outline_GetOutsideBorder(FT_Outline* outline);
FT_Error FT_Stroker_New(FT_Library library, FT_Stroker* astroker);
void FT_Stroker_Set(FT_Stroker stroker, FT_Fixed radius, FT_Stroker_LineCap line_cap, FT_Stroker_LineJoin line_join, FT_Fixed miter_limit);
void FT_Stroker_Rewind(FT_Stroker stroker);
FT_Error FT_Stroker_BeginSubPath(FT_Stroker stroker, FT_Vector* to, FT_Bool open);
FT_Error FT_Stroker_ParseOutline(FT_Stroker stroker, FT_Outline* outline, FT_Bool opened);
FT_Error FT_Stroker_LineTo(FT_Stroker stroker, FT_Vector* to);
FT_Error FT_Stroker_ConicTo(FT_Stroker stroker, FT_Vector* control, FT_Vector* to);
FT_Error FT_Stroker_CubicTo(FT_Stroker stroker, FT_Vector* control1, FT_Vector* control2, FT_Vector* to);
FT_Error FT_Stroker_EndSubPath(FT_Stroker stroker);
FT_Error FT_Stroker_GetBorderCounts(FT_Stroker stroker, FT_StrokerBorder border, FT_UInt* anum_points, FT_UInt* anum_contours);
FT_Error FT_Stroker_GetCounts(FT_Stroker stroker, FT_UInt* anum_points, FT_UInt* anum_contours);
void FT_Stroker_Done(FT_Stroker stroker);
void FT_Stroker_ExportBorder(FT_Stroker stroker, FT_StrokerBorder border, FT_Outline* outline);
void FT_Stroker_Export(FT_Stroker stroker, FT_Outline* outline);
FT_Orientation FT_Outline_Get_Orientation(FT_Outline* outline);
void FT_Outline_Reverse(FT_Outline* outline);
void FT_Outline_Transform(const FT_Outline* outline, const FT_Matrix* matrix);
void FT_Outline_Translate(const FT_Outline* outline, FT_Pos xOffset, FT_Pos yOffset);
FT_Error FT_Set_Char_Size(FT_Face face, FT_F26Dot6 char_width, FT_F26Dot6 char_height, FT_UInt horz_resolution, FT_UInt vert_resolution);
FT_Error FT_Set_Pixel_Sizes(FT_Face face, FT_UInt pixel_width, FT_UInt pixel_height);
void FT_Set_Transform(FT_Face face, FT_Matrix* matrix, FT_Vector* delta);
void FT_Get_Transform(FT_Face face, FT_Matrix* matrix, FT_Vector* delta);
FT_Error FT_Request_Size(FT_Face face, FT_Size_Request req);
FT_Error FT_Select_Size(FT_Face face, FT_Int strike_index);
FT_UInt FT_Get_Char_Index(FT_Face face, FT_ULong char_code);
FT_UInt FT_Face_GetCharVariantIndex(FT_Face face, FT_ULong charcode, FT_ULong variant_selector);
FT_Int FT_Face_GetCharVariantIsDefault(FT_Face face, FT_ULong charcode, FT_ULong variant_selector);
FT_UInt32* FT_Face_GetVariantSelectors(FT_Face face);
FT_UInt32* FT_Face_GetVariantsOfChar(FT_Face face, FT_ULong charcode);
FT_UInt32* FT_Face_GetCharsOfVariant(FT_Face face, FT_ULong variant_selector);
FT_Error FT_Get_Kerning(FT_Face face, FT_UInt left_glyph, FT_UInt right_glyph, FT_UInt kern_mode, FT_Vector* akerning);
FT_Error FT_Get_Track_Kerning(FT_Face face, FT_Fixed point_size, FT_Int degree, FT_Fixed* akerning);
FT_Error FT_Get_PFR_Kerning(FT_Face face, FT_UInt left_glyph, FT_UInt right_glyph, FT_Vector* avector);
FT_Error FT_Get_PFR_Metrics(FT_Face face, FT_UInt* aoutline_resolution, FT_UInt* ametrics_resolution, FT_Fixed* ametrics_x_scale, FT_Fixed* ametrics_y_scale);
FT_Error FT_Get_PFR_Advance(FT_Face face, FT_UInt gindex, FT_Pos* aadvance);
FT_Error FT_Select_Charmap(FT_Face face, FT_Encoding encoding);
FT_Error FT_Set_Charmap(FT_Face face, FT_CharMap charmap);
FT_Int FT_Get_Charmap_Index(FT_CharMap charmap);
FT_Long FT_Get_CMap_Format(FT_CharMap charmap);
FT_ULong FT_Get_CMap_Language_ID(FT_CharMap charmap);
FT_UShort FT_Get_FSType_Flags(FT_Face face);
FT_Int FT_Get_Gasp(FT_Face face, FT_UInt ppem);
void FT_List_Add(FT_List list, FT_ListNode node);
void FT_List_Insert(FT_List list, FT_ListNode node);
FT_ListNode FT_List_Find(FT_List list, void* data);
void FT_List_Remove(FT_List list, FT_ListNode node);
void FT_List_Up(FT_List list, FT_ListNode node);
FT_Error FT_List_Iterate(FT_List list, FT_List_Iterator iterator, void* user);
void FT_List_Finalize(FT_List list, FT_List_Destructor destroy, FT_Memory memory, void* user);
FT_Error FT_Get_Glyph_Name(FT_Face face, FT_UInt glyph_index, FT_Pointer buffer, FT_UInt buffer_max);
FT_UInt FT_Get_Name_Index(FT_Face face, const FT_String* glyph_name);
const char* FT_Get_Postscript_Name(FT_Face face);
const char* FT_Get_Font_Format(FT_Face face);
const char* FT_Get_X11_Font_Format(FT_Face face);
FT_Error FT_Set_Named_Instance(FT_Face face, FT_UInt instance_index);
FT_Error FT_Get_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Multi_Master(FT_Face face, FT_Multi_Master* amaster);
FT_Error FT_Set_MM_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Long* coords);
FT_Error FT_Set_MM_WeightVector(FT_Face face, FT_UInt len, FT_Fixed* weightvector);
FT_Error FT_Get_MM_WeightVector(FT_Face face, FT_UInt* len, FT_Fixed* weightvector);
FT_Error FT_Get_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_MM_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_Var_Blend_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Set_Var_Design_Coordinates(FT_Face face, FT_UInt num_coords, FT_Fixed* coords);
FT_Error FT_Get_Default_Named_Instance(FT_Face face, FT_UInt* instance_index);
FT_Error FT_Get_WinFNT_Header(FT_Face face, FT_WinFNT_HeaderRec* aheader);
FT_Error FT_Get_BDF_Property(FT_Face face, const char* prop_name, BDF_PropertyRec* aproperty);
FT_Error FT_Get_BDF_Charset_ID(FT_Face face, const char** acharset_encoding, const char** acharset_registry);
FT_Error FT_Get_CID_Is_Internally_CID_Keyed(FT_Face face, FT_Bool* is_cid);
FT_Error FT_Get_CID_From_Glyph_Index(FT_Face face, FT_UInt glyph_index, FT_UInt* cid);
FT_Error FT_Get_CID_Registry_Ordering_Supplement(FT_Face face, const char** registry, const char** ordering, FT_Int* supplement);
FT_Error FT_Get_PS_Font_Info(FT_Face face, PS_FontInfo afont_info);
FT_Error FT_Get_PS_Font_Private(FT_Face face, PS_Private afont_private);
FT_Int FT_Has_PS_Glyph_Names(FT_Face face);
FT_Long FT_Get_PS_Font_Value(FT_Face face, PS_Dict_Keys key, FT_UInt idx, void* value, FT_Long value_len);
FT_UInt FT_Get_Sfnt_Name_Count(FT_Face face);
FT_Error FT_Get_Sfnt_Name(FT_Face face, FT_UInt idx, FT_SfntName* aname);
FT_Error FT_Get_Sfnt_LangTag(FT_Face face, FT_UInt langID, FT_SfntLangTag* alangTag);
void* FT_Get_Sfnt_Table(FT_Face face, FT_Sfnt_Tag tag);
FT_Error FT_Load_Sfnt_Table(FT_Face face, FT_ULong tag, FT_Long offset, FT_Byte* buffer, FT_ULong* length);
FT_Error FT_Sfnt_Table_Info(FT_Face face, FT_UInt table_index, FT_ULong* tag, FT_ULong* length);
FT_Error FT_OpenType_Validate(FT_Face face, FT_UInt validation_flags, FT_Bytes* BASE_table, FT_Bytes* GDEF_table, FT_Bytes* GPOS_table, FT_Bytes* GSUB_table, FT_Bytes* JSTF_table);
void FT_OpenType_Free(FT_Face face, FT_Bytes table);
FT_ULong FT_Get_First_Char(FT_Face face, FT_UInt* agindex);
FT_ULong FT_Get_Next_Char(FT_Face face, FT_ULong char_code, FT_UInt* agindex);
void FT_Library_Version(FT_Library library, FT_Int* amajor, FT_Int* aminor, FT_Int* apatch);
FT_Error FT_Load_Char(FT_Face face, FT_ULong char_code, FT_Int32 load_flags);
FT_Error FT_Load_Glyph(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags);
FT_Error FT_Get_Advance(FT_Face face, FT_UInt glyph_index, FT_Int32 load_flags, FT_Fixed* padvance);
FT_Error FT_Get_Advances(FT_Face face, FT_UInt start, FT_UInt count, FT_Int32 load_flags, FT_Fixed* padvances);
FT_Error FT_Get_SubGlyph_Info(FT_GlyphSlot glyph, FT_UInt sub_index, FT_Int* p_index, FT_UInt* p_flags, FT_Int* p_arg1, FT_Int* p_arg2, FT_Matrix* p_transform);
FT_Error FT_Render_Glyph(FT_GlyphSlot slot, FT_Render_Mode render_mode);
void FT_GlyphSlot_AdjustWeight(FT_GlyphSlot slot, FT_Fixed xdelta, FT_Fixed ydelta);
void FT_GlyphSlot_Embolden(FT_GlyphSlot slot);
void FT_GlyphSlot_Oblique(FT_GlyphSlot slot);
void FT_GlyphSlot_Slant(FT_GlyphSlot slot, FT_Fixed xslant, FT_Fixed yslant);

#ifdef __cplusplus
}
#endif

#endif
