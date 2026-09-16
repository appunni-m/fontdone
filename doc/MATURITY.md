# Supported features and limitations

<!-- release:summary -->
**Latest release: [2.14.3-alpha.11](https://github.com/appunni-m/fontdone/releases/tag/v2.14.3-alpha.11).**
<!-- /release:summary -->

fontdone is an alpha font engine with selected FreeType-compatible behavior.
Use the compact APIs for glyph rendering, or review the individual function
contracts when migrating an existing FreeType application.

| Need | Support and limits |
| --- | --- |
| TrueType/OpenType font bytes and collections | Available through the compact Rust constructors |
| Glyph masks, bounding boxes, and advances | Available; `getmask`/`getbbox` use the first Unicode scalar, `getlength` sums unkerned advances |
| Explicit glyph loading and rendering | Available through the Rust face API and selected C/WASM calls |
| Node.js and browser rendering | One npm package with initialization, face handles, and owned bitmap results |
| Other font formats | Selected BDF, WinFNT, Type 1, CFF, PCF, and PFR routes in the FreeType-shaped APIs; not all compact constructors |
| Full FreeType replacement | Partial; consult the per-function reference before migrating |
| Variable fonts and color fonts | Partial; do not assume full feature coverage |
| Shaping, ligatures, bidi, fallback, paragraph layout | Outside the compact API; use a shaping/layout library |
| WOFF/WOFF2 in compact constructors | Not a supported application contract |
| Shared faces across threads | Unsupported; open a separate face per thread |
| API/ABI stability | Not promised between alpha versions |

See the [Rust guide](INTEGRATION.md), [JavaScript guide](../fontdone-wasm/npm/README.md),
or [C SDK guide](../fontdone-c-abi/README.md) for installation and lifecycle rules.
The C SDK uses its own library name, `fontdone_c_abi`; it is not a system
`libfreetype` replacement by filename.

For an exact `FT_*` call, consult the [function reference](FREETYPE_SUPPORT.md).
It distinguishes complete, partial, and planned behavior. A matching function
name alone does not establish complete compatibility.

Verify your application's fonts, render modes, flags, errors, and platforms
before deployment. The contributor [evidence guide](EVIDENCE.md) contains
measured parity, coverage, and platform details; the [roadmap](ROADMAP.md)
tracks unfinished implementation work.
