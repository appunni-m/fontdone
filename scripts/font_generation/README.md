# Font-Generation Tooling

This directory is the only repository location for scripts that create or
modify font fixture files. Keeping the generators separate from oracle,
benchmark, and audit scripts makes their licensing and provenance boundary
explicit.

## 1. Licensing

The generator source code is part of `fontdone` and is distributed under the
root FreeType License (`FTL.TXT`). It uses FontTools, which is an independently
licensed build-time dependency. Running FontTools does not transfer FontTools'
license to an output font.

The root FTL does not replace the copyright or license of an input font.
Generated output falls into one of these classes:

1. **Synthetic:** tables, outlines, names, and byte streams are authored by
   this project. The generator must emit an attribution identifying the
   fixture as synthetic where the format supports it.
2. **Derived:** the generator opens, copies, subsets, or mutates an existing
   font. The output retains the input font's license. Its embedded attribution
   and any required standalone license text must be preserved.
3. **Malformed derivative:** the generator deliberately corrupts a synthetic
   or properly licensed base fixture. Corruption does not erase the base
   fixture's license.

No script in this directory downloads fonts or reads from system font
directories. Inputs must be repository-relative, reviewed fixtures.

`scripts/build_compressed_fixtures.py` is outside this directory because it
does not generate a font: it deterministically wraps an existing
project-authored 8-byte PCF probe and project-authored byte strings as gzip,
bzip2, zlib, and Unix-compress payloads. Python's standard-library codecs are
the generators; decompression at runtime uses pure-Rust dependencies whose
licenses are enforced by `make supply-chain`.

## 2. Reviewed generators

| Generator | Classification | Input or provenance |
|---|---|---|
| `build_autohint_script_fixtures.py` | Synthetic | Project-authored outlines and tables; one internal copy remains within the generated synthetic family, including malformed `loca` boundary controls. |
| `build_cff_fixtures.py` | Synthetic | Project-authored CFF1/CFF2, TrueType control, and malformed CFF1/CFF2 table and INDEX data. |
| `build_render_fixtures.py` | Synthetic | Project-authored outlines and TrueType programs. |
| `build_type1_fixtures.py` | Synthetic | Project-authored Type 1 charstrings, dictionaries, AFM data, notices, and a naked CID-keyed Type 1 resource. |
| `build_type42_fixtures.py` | Synthetic | Project-authored embedded TrueType tables, outlines, names, and Type 42 wrapper. |
| `generate_malformed_bdf_fixtures.py` | Synthetic | Project-authored BDF text, including intentionally malformed variants. |
| `generate_winfnt_fixtures.py` | Synthetic | Project-authored binary WinFNT records and bitmap data. |
| `build_cpal_palette_fixtures.py` | Derived | `tests/fixtures/input/fonts/DejaVuSans.ttf`; DejaVu/Bitstream terms remain applicable. |
| `build_cmap_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; only cmap/table mutations. |
| `build_gasp_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_hinter_edge_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_metric_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_post_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_sbit_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; bitmap outputs add project-authored EBLC/EBDT records, including the maintained `embedded-strikes.ttf` face-record input, the SFNT-BDF derivative, and a CBLC/CBDT strike-metrics normalization matrix. |
| `build_sbix_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; adds project-authored 24 ppem `sbix` controls for PNG/JPEG/TIFF/RGBL/unknown graphic types, `dupe`/`flip` recursion, missing and malformed glyph ranges, and malformed optional-table face-open guards. The pinned oracle build has PNG decoding disabled, so the public error paths remain deterministic. |
| `build_interpreter_version_fixtures.py` | Synthetic | Project-authored off-grid TrueType outlines and GETINFO-branching glyph programs (`MDAP`), plus a no-instruction control font. Loaded with `FT_LOAD_RENDER`, the hinted outlines and advances differ between interpreter versions 35 and 40 exactly as pinned FreeType reports. |
| `build_pcf_fixtures.py` | Synthetic | Project-authored PCF directory, properties, accelerators, metrics, bitmap, and encoding tables. |
| `build_pfr_fixtures.py` | Synthetic | Project-authored PFR v4 logical/physical font records, fixed and proportional character advances, all descriptor-width flags, optional logical fields, and narrow/wide kerning pairs; no third-party font material. |
| `build_svg_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; adds one project-authored plain-XML OpenType SVG document, deterministic vertical metrics, and malformed optional-table controls for list offsets, records, document ranges, gzip rejection, and short tables. |
| `build_sfnt_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; the seven valid OpenType fixtures contain only project-authored BASE/GDEF/GPOS/GSUB/JSTF/MATH data. Seven malformed variants replace selected tables with deterministic one-byte payloads; `partial-malformed-layout.otf` retains valid GDEF/GPOS/GSUB and fails on the later MATH validation step. No third-party font is used. |
| `build_ftmm_future_variable_fixtures.py` | Synthetic derivative | Repository-generated compact variable, `avar`, packed `gvar`, HVAR store/map and active mixed-width delta fixtures, and MVAR guard/record fixtures, including malformed optional-table face-open controls, private all-point and partial-point IUP runs, and empty-outline variation loads. |
| `build_fvar_fixtures.py` | Synthetic derivative | Repository-generated `compact-variable.ttf`. |
| `build_mvar_fixtures.py` | Synthetic derivative | Repository-generated `compact-variable.ttf`. |
| `build_name_fixtures.py` | Synthetic derivative | Repository-generated static and variable base fixtures. |

The active third-party fixture families and their license classes are recorded
in `tests/fixtures/THIRD_PARTY_NOTICES.md`.

The color generator also writes the maintained COLRv1 controls under
`tests/fixtures/input/fonts/color/`.  `colr-v1-all-paints.ttf` is the reviewed
DejaVu-derived paint graph.  `malformed-colr-v1-paints.ttf` is a deterministic
malformed derivative: after serialization, the generator sets the COLR v1
`BaseGlyphV1List` offset to zero while leaving the surrounding SFNT openable.
The control therefore exercises rejection at the root lookup without relying
on a truncated file or an unreviewed external font.  Its SHA-256 is
`e6c68052444afec852031d662ecebe08ca587070c4bdcc253f4832b097774052`.

## 3. Review requirements

Before adding or changing a generator:

- State whether every output is synthetic or derived.
- For each external base font, record upstream URL, version or commit, original
  filename, SHA-256, license, modifications, and output SHA-256 in the fixture
  inventory.
- Preserve copyright, license, and reserved-font-name records.
- Add any standalone license text required for redistribution.
- Never fetch an unpinned URL, scan host fonts, or copy an unreviewed local
  font.
- Generate deterministically and fail when a required base fixture is absent.
- Review `cargo package --list`; generated fixtures and these scripts must
  remain outside the runtime crate.

Run generators through their Makefile targets from the repository root. Do not
invoke them from an assumed working directory.

The OpenType validator family is regenerated and checked through:

```bash
make test-opentype-validator
```

That gate compares selection, absence, returned table bytes, malformed-table
errors, partial-failure cleanup, and `FT_OpenType_Free` face-memory ownership
against pinned FreeType in the Rust, C ABI, independently linked external-C,
and WASM lanes.

The GX/AAT and classic-kern validator family is regenerated and checked
through:

```bash
make test-gx-validator
```

`build_gxvalid_fixtures.py` derives compact SFNT containers from the
project-authored hinter control font and inserts only synthetic, minimal
`feat`, `mort`, `morx`, `bsln`, `just`, `kern`, `opbd`, `trak`, `prop`, and
`lcar` table bytes. It also creates deterministic absent/malformed controls and
copies the project-generated Type 1 fixture for the non-SFNT error lane. The
gate compares all maintained selection, table-length, dialect, exact-error, and
face-memory ownership behavior against pinned FreeType, then repeats the
function contract through an independently linked external-C consumer.

The same generator writes the focused `tests/fixtures/input/generated/fonts/aat-gx/`
inputs used by the public-contract matrix (`all-validation-tables.ttf`, the
short `valid-feat-morx-kern-lcar.ttf` control, and one project-authored valid
fixture per validator). These are deterministic test inputs, not imported font
assets: the only base material is the repository's generated hinter control
font and the table bytes above, so no third-party font license is introduced.

The SFNT-BDF strike used by `FT_Get_BDF_Property` and
`FT_Get_BDF_Charset_ID` is regenerated through:

```bash
make font-fixture-sbit
```

`tests/fixtures/input/fonts/bdf/sfnt-bdf-table.otb` is a synthetic derivative
of the project-generated `hinter-control-matrix.ttf`. Its project-authored
`BDF ` table contains 1 strike at 20 ppem and 5 properties:
`CHARSET_REGISTRY=ISO10646`, `CHARSET_ENCODING=1`,
`FAMILY_NAME=Fontdone SFNT BDF`, `PIXEL_SIZE=20`, and `POINT_SIZE=200`.
The reviewed output is 5,240 bytes with SHA-256
`81adb1735aa8b4219324c3ff5002c6c51795e18038292b4ee99e37522f9177ff`.
It contains no third-party font material and needs no third-party license
notice.

The fixed-size face-record input is regenerated by the same target and lives
at `tests/fixtures/input/fonts/bitmap/embedded-strikes.ttf`. It is the same
project-generated base with one project-authored 20 ppem EBLC/EBDT strike and
no third-party font material. The reviewed output is 4,996 bytes with SHA-256
`79d8dbe88acd78551717cc8b3b2b7ae22464ffaf3a5bf6971cc0411b1dd1a446`.

The PCF property fixture is regenerated through:

```bash
make font-fixture-pcf
```

`tests/fixtures/input/fonts/pcf/properties-signed-only.pcf` is a
project-authored 1-glyph PCF with 5 required PCF tables and 7 properties. It
proves the PCF service rule that all numeric values—including
`POINT_SIZE=-120`—are exposed as signed `BDF_PROPERTY_TYPE_INTEGER` values.
The reviewed output is 400 bytes with SHA-256
`4d840c337be40b056873b9cbe5a8ed5a23081d174761b04233bcac9cdd53cec7`.
It contains no third-party font material and needs no third-party license
notice.

### 3.1. CID Type 1 fixture

The naked CID-keyed Type 1 resource is regenerated through:

```bash
make font-fixture-type1
```

`tests/fixtures/input/fonts/cid/fontinfo-populated.cid` is a project-authored
3-glyph CIDFont resource with `Adobe-Identity-0` ROS metadata, 1 FDArray entry,
an 8-byte hexadecimal CIDMap, and populated FontInfo strings and scalars. It
provides deterministic non-SFNT coverage for CID-keyed state, glyph-index to
CID mapping, FontInfo output, and the absent private-dictionary service. The
reviewed output is 900 bytes with SHA-256
`8bad30fd383566771d5498ba12bbf04f941e876dc9538c50290ee46fae2c5f2e`.
It contains no third-party font material and needs no third-party license
notice.

### 3.2. CFF1 Type2 coverage fixture

The CFF1 Type2 coverage family is regenerated through:

```bash
make font-fixture-cff
```

`tests/fixtures/input/fonts/cff/pure-cff-cubic.otf` is a project-authored
46-glyph OpenType/CFF1 face. Its append-only Type2 matrix includes successful
escaped arithmetic, an unknown escaped operator, and a one-operand
`hvcurveto` boundary so the public glyph-load parity suite exercises both the
pinned Adobe success and error paths. The reviewed output is 2,648 bytes with
SHA-256
`08dfa08cd8d2d27ec2c4ff80bd521a2bb06ea6357e8f50b51bb9619451513063`.
The malformed CFF1 derivatives are regenerated from the same synthetic base;
they contain no third-party font material and need no third-party license
notice. The Top DICT error matrix includes truncated positive and negative
two-byte operands and signed integer-clamp operands before the required
`CharStrings` field. CFF1 byte `255` remains reserved by the pinned parser and
is intentionally not treated as a fixed Top DICT operand; the CFF2 malformed
matrix separately exercises its valid 16.16 fixed-number encoding before the
required `CharStrings` field.

`tests/fixtures/input/fonts/cff/fontinfo-populated.otf` is a project-authored
2-glyph face whose CFF Top DICT populates the public `PS_FontInfo` strings,
`isFixedPitch`, `ItalicAngle`, `UnderlinePosition`, and `UnderlineThickness`
fields. The reviewed output is 1,032 bytes with SHA-256
`f025afe97f11f22a04cf000f067a829375fb3ef55e979c8846ebc2ce0866df3f`.
The derived `hybrid-otto-face-info.otf` is 1,124 bytes with SHA-256
`1f2d2392f2fc6948f37b1a8cb5980fa1a4aa850edd8d1d1cd2d35e1519a9f196`.

### 3.3. CFF2 fixture

The CFF2 FontInfo control is regenerated through:

```bash
make font-fixture-cff
```

`tests/fixtures/input/fonts/cff2/fontinfo-invalid-argument.otf` is a
project-authored 2-glyph OpenType/CFF2 face. Its CFF2 table contains 1 FDArray
entry, 2 direct Type2 charstrings, no subroutines, and no variation regions.
It proves that a valid CFF2 face opens successfully while
`FT_Get_PS_Font_Info` returns `FT_Err_Invalid_Argument` and clears its output.
The reviewed output is 804 bytes with SHA-256
`6f457efaafee3496f42e8e2dd977acf39730a86252066135076050563949c4e3`.
It contains no third-party font material and needs no third-party license
notice.
