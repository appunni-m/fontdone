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

The autohint fixture generator also produces the valid public Batch215
bottom-tilde crossed-neighbor witness, Batch216 thin crossed-tilde threshold
witness, and Batch218 ordered bottom-distance witness used by the parity
campaign.

| Generator | Classification | Input or provenance |
|---|---|---|
| `build_autohint_script_fixtures.py` | Synthetic | Project-authored outlines and tables; one internal copy remains within the generated synthetic family, including malformed `loca` boundary controls, six append-only valid Hebrew Batch123 long-blue contour variants, six normal-scale Batch126 Latin/Han branch probes, six valid Batch127 CJK edge-link predicate probes, one valid CJK edge-order/link-reduction coverage font, fifteen valid Batch145 CJK edge-interpolation witnesses, six valid Batch152 Latin adjustment-database flag probes, six valid Batch153 Latin no-extremum blue-string probes, six valid Batch159 Latin fallback/adjustment probes, one valid Batch190 Hebrew late-on-curve long-blue witness, one valid Batch191 Hebrew off-curve apex witness, one valid Batch194 lowered Khmer sub-top witness, one valid Batch196 Hebrew near-top span witness, one valid Batch197 mirrored Hebrew near-top span witness, one valid Batch199 Latin vertical-cusp segment-merge witness, one valid Batch200 Latin top-tilde minimum witness, one valid Batch201 Latin top-tilde predecessor-control witness, one valid Batch202 Latin bottom-tilde maximum witness, one valid Batch203 Latin lowest-contour tie-break witness, one valid Batch204 Latin horizontal-overlap witness, one valid Batch210 Latin top-tilde successor-control witness, one valid Batch211 Latin crossed-neighbor measurement witness, one valid Batch212 Latin thin crossed-tilde witness, one valid Batch213 Latin bottom-tilde predecessor-control witness, and one valid Batch214 Latin bottom-tilde successor-control witness. |
| `build_cff_fixtures.py` | Synthetic and derived | Project-authored CFF1/CFF2, TrueType control, valid cubic-bbox extrema probes, source-reviewed global-subroutine EOF, fixed-operand arithmetic, Type2 `mul`, and fixed-valued `callgsubr` error controls, random-operator Private-dictionary boundary controls including one-byte/reserved-number/reserved-operator parser cases, and malformed CFF1/CFF2 table and INDEX data, plus malformed CID CFF derivatives of the maintained OFL-1.1 `FDArrayTest257` face. |
| `build_render_fixtures.py` | Synthetic | Project-authored outlines and TrueType programs. |
| `build_type1_fixtures.py` | Synthetic | Project-authored Type 1 charstrings, dictionaries, AFM data, notices, a naked CID-keyed Type 1 resource, six valid no-op movement/curve controls, and a valid post-contour `setcurrentpoint` control. |
| `build_type42_fixtures.py` | Synthetic | Project-authored embedded TrueType tables, outlines, names, and Type 42 wrapper. |
| `generate_malformed_bdf_fixtures.py` | Synthetic | Project-authored BDF text, including valid atom/integer/cardinal property controls, malformed constructor variants, the Batch232 numeric-prefix/no-value/sign/saturation property matrix, and the Batch235 malformed `SIZE` decimal-prefix fixed-strike matrix. |
| `generate_winfnt_fixtures.py` | Synthetic | Project-authored binary WinFNT records and bitmap data, including short-header and declared-size validation controls. |
| `build_cpal_palette_fixtures.py` | Derived | `tests/fixtures/input/fonts/DejaVuSans.ttf`; DejaVu/Bitstream terms remain applicable. |
| `build_cmap_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; only cmap/table mutations, including maintained malformed format-13 and format-14 parser matrices beside valid format-6 controls. |
| `build_gasp_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_hinter_edge_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`, including a 102-level composite chain, a 77-glyph opcode stack-underflow matrix, isolated TrueType VM operand/branch matrices, 30 valid public Batch61 VM branch witnesses, 30 valid public Batch121 pedantic WS/WCVTP error witnesses, 30 valid public Batch122 IDEF/predicate/scan-control branch witnesses, and a prep-time empty-contour SHZ control. |
| `build_metric_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_post_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`. |
| `build_sbit_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; bitmap outputs add project-authored EBLC/EBDT records, including the maintained `embedded-strikes.ttf` face-record input, valid and malformed SFNT-BDF derivatives, a valid zero-width positive-row gray format-1 strike for Batch182, a CBLC/CBDT strike-metrics normalization matrix, and 100-strike `FT_Select_Size` matrices with hhea, `OS/2`, and `vmtx` metric variants. |
| `build_sbix_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; adds project-authored 24 ppem `sbix` controls for PNG/JPEG/TIFF/RGBL/unknown graphic types, `dupe`/`flip` recursion, missing and malformed glyph ranges, and malformed optional-table face-open guards. The pinned oracle build has PNG decoding disabled, so the public error paths remain deterministic. |
| `build_interpreter_version_fixtures.py` | Synthetic | Project-authored off-grid TrueType outlines and GETINFO-branching glyph programs (`MDAP`), plus a no-instruction control font. Loaded with `FT_LOAD_RENDER`, the hinted outlines and advances differ between interpreter versions 35 and 40 exactly as pinned FreeType reports. |
| `build_pcf_fixtures.py` | Synthetic | Project-authored PCF directory, properties, accelerators, metrics, bitmap, and encoding tables. |
| `build_pfr_fixtures.py` | Synthetic | Project-authored PFR v4 logical/physical font records, fixed and proportional character advances, all descriptor-width flags, optional logical fields, narrow/wide kerning pairs, and maintained malformed header/record controls; no third-party font material. |
| `build_svg_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; adds project-authored plain-XML OpenType SVG documents, including a later-record range-gap lookup control, deterministic vertical metrics, and malformed optional-table controls for list offsets, records, document ranges, gzip rejection, and short tables. |
| `build_sfnt_fixtures.py` | Synthetic derivative | Repository-generated `hinter-control-matrix.ttf`; the valid OpenType fixtures contain only project-authored BASE/GDEF/GPOS/GSUB/JSTF/MATH data, including GPOS/GSUB version-1.1 layout variants with zero and in-table non-zero FeatureVariations offsets and the valid minimum-upem/full-range `extreme-hadvance.ttf` advance-boundary control. The malformed and missing-table variants replace or remove selected tables deterministically, including loadable unknown-version, required-offset, and truncated-record headers; `partial-malformed-layout.otf` retains valid GDEF/GPOS/GSUB and fails on the later MATH validation step. No third-party font is used. |
| `build_ftmm_future_variable_fixtures.py` | Synthetic derivative | Repository-generated compact variable, `avar`, packed `gvar`, HVAR store/map and active mixed-width delta fixtures, and MVAR guard/record fixtures, including malformed optional-table face-open controls, runtime-short gvar records, tuple-header/glyph-data-offset/point-run bounds, embedded-peak bounds, gvar- and no-gvar composite-targeted runtime parse guards, private all-point and partial-point IUP runs, empty-outline variation loads including a valid non-zero-length empty simple-glyph record, native mixed XY/point-attached composite controls. |
| `build_fvar_fixtures.py` | Synthetic derivative | Repository-generated `compact-variable.ttf`. |
| `build_mvar_fixtures.py` | Synthetic derivative | Repository-generated `compact-variable.ttf`. |
| `build_name_fixtures.py` | Synthetic derivative | Repository-generated static and variable base fixtures, including the OS/2 version-sentinel WWS-selection control. |

The active third-party fixture families and their license classes are recorded
in `tests/fixtures/THIRD_PARTY_NOTICES.md`.

The color generator also writes the maintained COLRv1 controls under
`tests/fixtures/input/fonts/color/`.  `colr-v1-all-paints.ttf` is the reviewed
DejaVu-derived paint graph covering every maintained non-variable paint form,
including centered and uniform transforms plus UFWORD radius edge bytes.  Its
SHA-256 is
`f5ccb8eda4fbef3230ecf4a9898b78c8e9bf66f628ce58adc158eda478227fe6`.
`malformed-colr-v1-paints.ttf` is a deterministic
malformed derivative: after serialization, the generator sets the COLR v1
`BaseGlyphV1List` offset to zero while leaving the surrounding SFNT openable.
The control therefore exercises rejection at the root lookup without relying
on a truncated file or an unreviewed external font.  Its SHA-256 is
`a9c54b4d6b36ce895591003ae6f5abe6b6f4272b1710261947ec32ccd26eb25c`.

The `malformed/colr-v1-paint-format-unsupported.ttf` and
`malformed/colr-v1-paint-format-max-and-above.ttf` controls are deterministic
derivatives of `colr-v1-all-paints.ttf`.  They replace only the first two root
Paint format bytes with `(33, 255)` and `(33, 34)`, respectively, while
retaining the third valid control glyph.  This keeps face opening and
`FT_Get_Color_Glyph_Paint` successful, then exercises lazy `FT_Get_Paint`
rejection for formats at or above `FT_COLR_PAINT_FORMAT_MAX`.

`malformed/colr-v1-malformed-child-paints.ttf` is another deterministic
derivative.  It preserves root records for eight wrapper formats, replacing
their child offsets with zero or out-of-range values while retaining glyph 50
as a valid solid control.  The fixture therefore exercises the pinned lazy
`FT_Get_Paint` failure format for PaintGlyph, PaintTransform, PaintTranslate,
PaintScale, PaintRotate, PaintSkew, and both PaintComposite child pointers.

`malformed/colr-v1-malformed-paint-payloads.ttf` is a deterministic derivative
that repoints eight base roots to the final bytes of the COLR table.  Its
format bytes remain discoverable while the first unavailable PaintColrLayers,
PaintSolid, gradient, PaintGlyph, and PaintComposite payload fields exercise
the pinned lazy reader's boundary failures without truncating the SFNT.

`malformed/colr-v1-malformed-colorline-paints.ttf` is a deterministic derivative
that points linear, radial, sweep, and variable-linear roots at one invalid
ColorLine extend byte.  The shared malformed ColorLine reaches each gradient
family's pinned rejection path while the SFNT and glyph 50 control remain
openable.

`malformed/colr-v1-malformed-gradient-payloads.ttf` is a deterministic
derivative whose linear, variable-linear, sweep, and skew roots point at a
valid zero-stop ColorLine but end during their fixed payload fields.  The
fixture reaches the pinned `ENSURE_READ_BYTES` failures without truncating the
SFNT and retains glyph 50 as a valid solid control.

`malformed/colr-v1-malformed-layer-list.ttf` is a deterministic derivative
that preserves the first PaintColrLayers root but replaces its
`FirstLayerIndex` with `4`, beyond the maintained three-entry LayerV1List.  The
root remains discoverable through `FT_Get_Color_Glyph_Paint`, while
`FT_Get_Paint` exercises the pinned layer-bounds rejection; glyph 50 remains
the valid solid control.

The `malformed/colr-v1-nested-child-failure-01.ttf` through
`malformed/colr-v1-nested-child-failure-30.ttf` controls preserve an in-range
child pointer but place a distinct nested paint format at the final bytes of
the COLR table.  They keep the SFNT openable and exercise the nested
`FT_Get_Paint` payload rejection separately from zero or out-of-range child
offsets.

The three `malformed/colr-v0-invalid-layer-*.ttf` and
`malformed/colr-v0-truncated-layer-array.ttf` controls are synthetic
DejaVu-derived COLR v0 faces with one layer record each.  They retain an
openable SFNT while separately exercising an out-of-range layer glyph, an
out-of-range CPAL index, and a base record whose layer count exceeds the
layer array.  `FT_Get_Color_Glyph_Layer` must reject each malformed record
lazily with the pinned output and iterator mutation behavior.

The `malformed/colr-v0-table-bounds-01.ttf` through
`malformed/colr-v0-table-bounds-30.ttf` controls are 30 distinct, openable
COLR v0 derivatives.  The first 15 place the base-record extent beyond the
table and the remaining 15 place the layer-record extent beyond it, with
different declared offsets and counts.  They exercise the public lazy table
bounds rejection through `FT_Get_Color_Glyph_Layer`.

The seven `malformed/colr-v1-clip*.ttf` controls are deterministic derivatives
of `colr-v1-clipbox-format1-format2.ttf`, with the LayerV1List control derived
from `colr-v1-all-paints.ttf`. They retain an openable SFNT and mutate only
COLRv1 bounds: unsupported list format 0, unsupported ClipBox format 3,
truncated ClipList records, coordinates or variation index, a 24-bit box
offset outside the table, and truncated LayerV1List offsets. The public
clipbox parity variants compare the pinned false-return and caller-output-
preservation behavior for each control.

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
`lcar` table bytes. It includes separate classic/version-0 and Apple
version-1 `kern` header controls so both GX validator layouts are executable.
It also creates deterministic absent/malformed controls and copies the
project-generated Type 1 fixture for the non-SFNT error lane. The
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
`BDF ` table contains 1 strike at 20 ppem and 8 properties:
`CHARSET_REGISTRY=ISO10646`, `CHARSET_ENCODING=1`,
`FAMILY_NAME=Fontdone SFNT BDF`, `PIXEL_SIZE=20`, `POINT_SIZE=200`, an
undefined property record, a defined `RESOLUTION_X` cardinal, and a defined
unknown-format record. The latter three records are parser controls and are
not requested by the public property case.

The malformed controls
`sfnt-bdf-table-invalid-version.otb`,
`sfnt-bdf-table-strings-before-directory.otb`, and
`sfnt-bdf-table-strings-out-of-range.otb`,
`sfnt-bdf-table-properties-beyond-strings.otb` use the same project-authored
SFNT bitmap data but make the optional `BDF ` table invalid. Pinned FreeType
defers loading that table until a BDF service query, so `FT_New_Memory_Face`
still succeeds; the maintained open-face route keeps these inputs to exercise
the corresponding Rust parser rejection paths.
The reviewed output is 5,324 bytes with SHA-256
`eacad2a21d685995749c089e2e032ce3c568b502aec153379eec2468b77ba6bd`.
It contains no third-party font material and needs no third-party license
notice.

The BDF parser control
`tests/fixtures/input/fonts/bdf/properties-duplicate-and-empty.bdf` is
generated by `make font-fixture-malformed-bdf`. It repeats `POINT_SIZE` so
the later value replaces the earlier one and includes an unknown property with
no value, matching the pinned BDF parser's duplicate-property and empty-atom
handling. It is synthetic and contains no third-party font material.

The same target generates the thirty
`tests/fixtures/input/fonts/bdf/malformed-numeric/batch232-*.bdf` controls.
Each keeps a valid BDF face while assigning one known integer or cardinal
property a deliberately malformed token. Pinned FreeType's `bdf_atol_` and
`bdf_atoul_` accept no-value/no-digit tokens as zero, retain decimal prefixes,
ignore trailing non-digits, and saturate overflowing prefixes; these inputs
are the public parity evidence for those semantics and the corresponding Rust
parser fix. They are synthetic and contain no third-party font material.

The same generator emits the `charset-registry-iso8859.bdf`,
`charset-registry-iso8859-other.bdf`, and `charset-registry-iso646.bdf`
controls. Their `CHARSET_REGISTRY` and `CHARSET_ENCODING` pairs exercise the
pinned Unicode-charmap classifications for `ISO8859/1`, the non-Unicode
`ISO8859/2` branch, and `ISO646.1991/IRV`; they are consumed only by
maintained input-only parity variants.

The fixed-size face-record input is regenerated by the same target and lives
at `tests/fixtures/input/fonts/bitmap/embedded-strikes.ttf`. It is the same
project-generated base with one project-authored 20 ppem EBLC/EBDT strike and
no third-party font material. The reviewed output is 4,996 bytes with SHA-256
`79d8dbe88acd78551717cc8b3b2b7ae22464ffaf3a5bf6971cc0411b1dd1a446`.

The PCF property fixtures are regenerated through:

```bash
make font-fixture-pcf
```

`tests/fixtures/input/fonts/pcf/properties-signed-only.pcf` and
`tests/fixtures/input/fonts/pcf/properties-msb.pcf` are project-authored
1-glyph PCFs with 5 required PCF tables and 7 properties. They prove the PCF
service rule that all numeric values—including `POINT_SIZE=-120`—are exposed
as signed `BDF_PROPERTY_TYPE_INTEGER` values, and cover both little-endian and
big-endian property records and encoding-table decoding. The companion
`tests/fixtures/input/fonts/pcf/properties-uncompressed-metrics.pcf` uses the
uncompressed six-field metric record through the same public property route.
The generator also keeps malformed PCF controls for an invalid file version,
truncated directories, table ranges outside the stream, overlapping table
ranges, a missing required table, a properties-format mismatch, and an
unsupported properties format; those are routed through the existing face-open
error matrix.
The reviewed compressed/property outputs are 400 bytes with SHA-256
`4d840c337be40b056873b9cbe5a8ed5a23081d174761b04233bcac9cdd53cec7` and
`c636e9b9bd46941afd35159b654cc214fbfa6358a488ce321aba40e8b2c762aa`,
respectively. They contain no third-party font material and need no
third-party license notice.

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
`hvcurveto` boundary so the public glyph-load parity suite
exercises both the pinned Adobe success and error paths. The reviewed output is
2,648 bytes with SHA-256
`08dfa08cd8d2d27ec2c4ff80bd521a2bb06ea6357e8f50b51bb9619451513063`.

`tests/fixtures/input/fonts/cff/pure-cff-cubic-last-delta.otf` is a separate
project-authored 48-glyph face. It appends five-operand `hvcurveto` and
`vhcurveto` cases without changing the 46-glyph control face's SFNT-wide
auto-hint metrics. The reviewed output is 2,720 bytes with SHA-256
`ace3fc00642f8d4810810b6124cbe0dbfc44327a8788507d4529ff367f8ba006`.

`tests/fixtures/input/fonts/cff/pure-cff-bbox-extrema.otf` is a
project-authored seven-glyph face with six valid cubic contours whose control
points exercise positive and negative X/Y extrema through public
`FT_Load_Glyph` calls. It is regenerated with `build_cff_fixtures.py`; its
reviewed output is 1,144 bytes with SHA-256
`22e1c6c775d0f3d52bca345093bdbad05c1b82197d8881474b9045411a577bdc`.

`tests/fixtures/input/fonts/cff/pure-cff-cubic-peak-shifts.otf` is a
project-authored seven-glyph face with six valid large-coordinate cubic
contours and a 16-unit UPEM, used by 30 public loads across five large ppem
values to exercise cubic-peak scaling shifts. It is regenerated with
`build_cff_fixtures.py`; its reviewed output is 1,248 bytes with SHA-256
`39ab88a1e0dde160ecc1832a05d4efaac50e9049730f1d84551d08eba6e7426c`.

`tests/fixtures/input/fonts/cff/pure-cff-below-baseline-no-vmtx.otf` is a
project-authored two-glyph face whose valid glyph sits below the baseline and
whose SFNT omits `vmtx` and `vhea`. It is regenerated with
`build_cff_fixtures.py`; its reviewed output is 1,000 bytes with SHA-256
`1c2f115dff082f453da2994f4054f6e68a25702bd82c1d57af6649403672bf61`.
Thirty public `FT_Load_Glyph` variants use vertical layout to exercise the
pinned CFF driver's synthesized vertical metrics and vertical grid fitting.

`tests/fixtures/input/fonts/cff/pure-cff-baseline-touch-no-vmtx.otf` is a
project-authored two-glyph face whose valid glyph's top edge touches the
baseline and whose SFNT omits `vmtx` and `vhea`. It is regenerated with
`build_cff_fixtures.py`; its reviewed output is 996 bytes with SHA-256
`6197434871a86cd21a5a1c07d700f8377b1779ee7b42f6cf951c9749fd90649e`.
Thirty public `FT_Load_Glyph` variants use vertical layout to exercise the
pinned baseline arm of synthesized vertical metrics.

The same generator emits three source-reviewed malformed CFF1 glyph-load
controls for the public Type2 post-validation guards. `pure-cff-negative-global-
subr-index.otf` supplies an integer `-108` `callgsubr` operand against the
standard 107 bias; `pure-cff-global-subr-recursion.otf` supplies a self-calling
global subroutine; and `pure-cff-top-level-return.otf` executes `return` from
the top-level glyph charstring. The pinned FreeType 2.14.3 oracle rejects all
three public loads with `Invalid_File_Format` rather than accepting the
malformed charstrings. Their reviewed outputs are 1,348 bytes / SHA-256
`a5cead88bfd487e9f01289361a20506229e96ef2ed00bcb43d6a370eb0eb79e6`, 1,084
bytes / SHA-256
`a22966e26dba776c3ebb1fc9a488b6a66ad3ef464a561c2fd3acb109d20ab7b2`, and
1,008 bytes / SHA-256
`2359894b2dad50abce8ce38a00189b3d48f9e549a3e38154e9827c79882961b5`.

`tests/fixtures/input/fonts/cid/ot-cff-cid-keyed-standard-ros.otf` is derived
from the maintained CID-keyed CFF source with its ROS set to the standard CFF
string SIDs for `Roman` and `Semibold`. It is used by the CID registry and
ordering parity supplement to exercise standard-SID decoding rather than
custom-string decoding. The reviewed output is 145,192 bytes with SHA-256
`57c537c193f26af6ec2681ac00104b3358ba0a6bb73349c8cbd60171972dd896`.

`tests/fixtures/input/fonts/cid/ot-cff-cid-keyed-standard-ros-weight-names.otf`
is derived from the same source with ROS standard CFF string SIDs for `Black`
and `Bold`. It keeps the standard-string table parity route from covering only
the three trailing style names and is regenerated with `build_cff_fixtures.py`;
its SHA-256 is recorded in the retention inventory.

`tests/fixtures/input/fonts/cid/ot-cff-cid-keyed-single-glyph.otf` is derived
from the same maintained CID-keyed CFF source after reducing its CharStrings,
charset, metrics, and FDSelect records to the required `.notdef` glyph. It is
used by the CID registry and ordering parity supplement to exercise the
single-glyph CID charset boundary. The reviewed output is regenerated with
`build_cff_fixtures.py` and its SHA-256 is recorded in the retention inventory.

`tests/fixtures/input/fonts/cid/ot-cff-cid-keyed-unresolved-ordering.otf` is
derived from the same maintained CID-keyed CFF source after replacing its ROS
ordering SID with `800`, outside the face's String INDEX. Pinned FreeType keeps
the CID service valid and returns a null ordering string for this input. The
reviewed output is regenerated with `build_cff_fixtures.py` and its SHA-256 is
recorded in the retention inventory.

The same generator emits four malformed and one zero-glyph CID CFF derivatives from the maintained
OFL-1.1 source: `ot-cff-cid-keyed-missing-charset.otf`,
`ot-cff-cid-keyed-predefined-charset.otf`,
`ot-cff-cid-keyed-unsupported-charset-format.otf`, and
`ot-cff-cid-keyed-truncated-charset-range.otf`, plus
`ot-cff-cid-keyed-zero-glyph.otf`. They preserve the source
license and exercise the missing, predefined, unsupported-format, and
truncated-range CID charset guards and the zero-glyph charset short-circuit
during face opening. Their output hashes and exact byte mutations are recorded in
`tests/fixtures/input/fonts/cid/FDArrayTest257.PROVENANCE.md`.

`tests/fixtures/input/fonts/cid/ot-cff-non-cid-sentinel-registry.otf` is
derived from the same source with the ROS registry set to CFF's `0xFFFF`
absent-CID sentinel. It keeps the CFF table size and offsets stable by removing
the optional `CIDFontVersion` dictionary entry, and is used to compare the
pinned non-CID classification through `FT_Get_CID_Registry_Ordering_Supplement`.
The reviewed output is 145,212 bytes with SHA-256
`6c6cb6e3b8a2aee04d97014afbf1e3bf888a35335dc32612aa5079be4afa3f7d`.

The malformed CFF1 derivatives are regenerated from the same synthetic base;
they contain no third-party font material and need no third-party license
notice. The Top DICT error matrix includes truncated positive and negative
two-byte operands and signed integer-clamp operands before the required
`CharStrings` field, including a reserved BCD real-number nibble. CFF1 byte
`255` remains reserved by the pinned parser and is intentionally not treated as
a fixed Top DICT operand; the CFF2 malformed matrix separately exercises its
valid 16.16 fixed-number encoding before the required `CharStrings` field.

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
