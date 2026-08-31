# Development guide

This is the contributor reference for building, testing, debugging, fixtures,
CI, performance, and repository retention. Run commands from the repository
root. Use `make help` as the command index.

## 1. Environment

### 1.1 Host evidence

| Host or target | What CI proves |
|---|---|
| Ubuntu 24.04 x86-64 | Rust, oracle parity, native C, packaging, and supply chain |
| macOS 15 Apple Silicon | fresh checkout, native C, layout, exports, and install tree |
| Windows Server 2025 x86-64 MSVC | native C, LLP64 layout, DLL/import library, exports, and install tree |
| Linux i686 | cross-built and QEMU-executed C consumer/layout contract |
| Linux powerpc64 | cross-built and QEMU-executed big-endian C consumer/layout contract |
| `wasm32-unknown-unknown` on Node 20 | raw WASM consumer plus packed and installed `fontdone` npm consumer |

Only Ubuntu and macOS are normal pinned-oracle development hosts. Windows and
the cross targets are claimed only to the extent recorded above.
CI validates the browser entry point through Node's standards-compatible ESM,
fetch, and WebAssembly APIs; it does not claim a named-browser version matrix.
Release evidence should add a real-browser run of the maintained HTML example
when browser behavior or asset loading changes.

### 1.2 Required tools

| Tool | Version | Used for |
|---|---:|---|
| Rust | MSRV 1.87; repository toolchain 1.96.1 | runtime and packages |
| Rust nightly | 2026-07-16 in CI | LLVM branch and region coverage |
| cargo-llvm-cov | 0.8.7 | combined coverage report |
| cargo-deny / cargo-audit | 0.20.2 / 0.22.2 | supply-chain policy |
| GNU Make | 3.81 or newer | maintained command interface |
| Python | 3.9 or newer | audits, fixtures, consumers, releases |
| CMake | 3.20 or newer | offline FreeType oracle |
| Clang/GCC/MSVC | C11-capable | oracle and C consumers |
| Git, curl, tar, XZ | maintained OS versions | source and archive handling |
| Node.js and npm | Node 20 or newer | raw Wasm, browser-wrapper, and npm archive verification |

Install the pinned supply-chain tools with `make setup-tools` and the coverage
frontend with `make setup-coverage-tools`. Font generators use one pinned
Python environment:

```bash
python3 -m venv target/font-generation-venv
target/font-generation-venv/bin/python -m pip install \
  --requirement requirements-font-generation.txt
```

## 2. Build boundary and generated state

Runtime packages are pure Rust. They do not build, link, or dynamically load
FreeType C. `make setup` downloads checksum-pinned FreeType 2.14.3 into ignored
`freetype/`, builds the offline oracle, and refreshes generated public
constants.

| Command | Purpose | Persistent output |
|---|---|---|
| `make build` | Build the root runtime | `target/` |
| `cargo build --workspace --locked` | Build all three packages | `target/` |
| `make setup` | Fetch/build the pinned oracle and constants | ignored `freetype/`, `target/`; generated constants |
| `make generate-contracts` | Regenerate support, C header, WASM, and legal derivatives | tracked generated files |
| `make check-generated` | Reject generated drift | no intended writes |
| `make npm-package-verify` | Build, inspect, install, and execute the browser npm tarball | `target/npm-package/`, release evidence |
| `make fresh-checkout-check` | Exercise non-parity checkout contracts | `target/` |

The first oracle fetch and uncached Cargo dependency resolution need network
access. The compact `Font` API performs no file, network, environment, or
process I/O: callers supply font bytes. The FreeType-shaped safe facade and its
native wrapper deliberately implement path-based `FT_New_Face`/`FT_Attach_File`
and read `FREETYPE_PROPERTIES` for the corresponding compatibility routes.
Runtime packages perform no network access or subprocess execution.

## 3. Verification model

Run the smallest useful gate first:

| Scope | Command | Meaning |
|---|---|---|
| Rust workspace | `make test-fast` | Tests/checks excluding full parity and ignored trace diagnostics |
| Runtime smoke | `make test-parity-smoke` | Eight fixed `load_char` cases across the Rust, C ABI, and WASM routes |
| One operation | `make test-op OP=ftadvanc.get_advance` | Exact selected C/Rust/facade comparison |
| One case | `make test-case CASE=freetype.FT_Load_Glyph.no_scale` | Exact selected case comparison |
| Full parity | `make test-parity` | Every runnable exact case, route audit, facades, and purity guard |
| Record parity evidence | `make record-parity-snapshot` | Promote the latest passing, source-matched report into committed evidence |
| Performance smoke | `make bench-quick` | Run two C/Rust samples without qualifying them as baseline evidence |
| Performance evidence | `make bench` | Run ten release-mode samples with latency, throughput, peak RSS, and artifact sizes |
| Record performance | `make record-performance-baseline` | Append a qualifying clean report to committed measured evidence |
| Performance contract | `make bench-regression` | Fail unless every reviewed performance threshold is active and passing |
| Integrations | `make test-integrations` | Downstream Rust, external C, raw Wasm, and packed npm consumers |
| Browser npm package | `make npm-package-verify` | Prepack, exact file inventory, clean install, shipped self-test, and glyph render |
| C contract | `make c-abi-contract` | Report all 12 categories and remaining debt |
| Five-platform C evidence | `make c-abi-contract-all-platforms` | Validate five assembled bundles and report current debt without claiming completion |
| Complete C contract | `make c-abi-contract-complete` | Fail unless all categories and all five platform bundles complete |
| Rust docs | `make doc` and `make doc-test` | Strict rustdoc and compiled examples |
| Static quality | `make lint` | rustfmt and workspace Clippy policy |
| Per-commit local CI | `make ci` | Fast commit gates suitable for ordinary branch protection |
| Requested local audit | `make ci-thorough` | Fast gates plus full parity, integrations, coverage, performance, contract, package, and supply-chain evidence |

For the real-browser check required after browser loader or example changes,
build the package asset and serve the repository root:

```bash
npm run build --prefix fontdone-wasm/npm
python3 -m http.server 8000 --bind 127.0.0.1
```

Open this local URL in the browser under review:

```text
http://127.0.0.1:8000/fontdone-wasm/npm/examples/browser.html?font=/tests/fixtures/input/fonts/DejaVuSans.ttf
```

Success is the visible `Rendered glyph 36` status and a non-empty glyph canvas.
Stop the local server after the check. This example uses a maintained checkout
fixture; the font is not included in the npm archive.

The unified parity harness compares backend routes in bounded parallel workers;
it defaults to twice the host's available parallelism and caps that value at 16.
Set `FONTDONE_UNIFIED_WORKERS` for a reproducible local profile or a constrained
CI runner. The setting changes scheduling only; it does not change the selected
input cases or the exact comparison. The default is intentionally bounded
oversubscription because each worker drives independent Rust, C-ABI, and WASM
facades and the measured full matrix is faster with that scheduling shape.
The all-lane coverage target sets `FONTDONE_UNIFIED_WORKERS=1` through
`COVERAGE_UNIFIED_WORKERS` by default. LLVM instrumentation makes the C-ABI and
WASM comparison path contend heavily when those backend calls run in parallel;
the measured single-worker lane is substantially faster and avoids the stalled
multi-worker behavior. It sets `CARGO_PROFILE_TEST_OPT_LEVEL=1` through
`COVERAGE_TEST_OPT_LEVEL` by default. A current-head comparison on the same
checkout completed in 54.563 seconds at opt-level 1 versus 65.332 seconds at
opt-level 3, with identical parity and coverage totals; the lower level avoids
the slower optimized/instrumented backend path on this host. Set the coverage
profile, worker, and `cargo llvm-cov` flag variables only for an explicitly
measured instrumented profile. Coverage uses the lightweight
`api-abi-runtime-check` and prepares only the default oracle needed by the
instrumented parity matrix. The optional-feature contract is a separate gate
already exercised by `make test-parity-smoke`; coverage does not rebuild those
non-instrumented bundles by default. Set
`COVERAGE_PREPARE_OPTIONAL_FEATURES=1` when an isolated coverage invocation
explicitly needs that extra contract work.
The optional-feature C build contract remains isolated from the normal coverage
report. The coverage recipes retain the instrumented Cargo target with
`cargo llvm-cov --no-clean` and remove only stale `.profraw` files before each
measurement, so repeated local runs reuse the compiled coverage binary without
merging prior execution data. Because `--no-report` retains old instrumented
workspace artifacts, the all-lane target also stores a source/configuration
state marker and runs `cargo llvm-cov clean --workspace` once when that state
changes. The marker keys source reuse to the newest commit touching compiler-
relevant inputs rather than every `HEAD`, while a dirty compiler-input tree
still forces a rebuild; fixture/docs-only commits therefore keep the
instrumented binary warm. Worker count and lane splitting are deliberately
excluded because they only change process orchestration. This prevents two
feature/cfg builds of the same source file from being merged while preserving
the fast warm-repeat path. The all-lane target
keeps workspace report scope
for the C-ABI and host-compiled WASM facades but selects only the
`unified_fixture_parity` integration binary; the workspace's empty unit and
pipe-trace targets add no parity inputs and can duplicate cfg-dependent FFI
coverage. The harness helpers are excluded from LLVM counters with the
nightly `coverage(off)` attribute, and the split lanes use the exact default
test name `parity_fixture::unified_fixture_parity` through
`COVERAGE_UNIFIED_TEST_NAME`; using the old top-level exact name would run zero
tests. Its `COVERAGE_ALL_TARGET_DIR` cache is separate from other coverage
profiles, and its state marker detects compiler-input, toolchain, profile, and
coverage-flag changes. Run `make coverage-clean` after changing coverage inputs
outside that tracked state or when manually changing the coverage toolchain,
profile flags, or instrumentation configuration. The ABI-only package
preflight remains available as `make coverage-abi-preflight`, but the default
all-lane coverage command does not rerun it: `make test-fast` already executes
the same test-support contract, and `make ci-thorough` runs that gate before
coverage. Set `COVERAGE_ABI_PREFLIGHT=1` when an isolated coverage invocation
also needs the extra preflight.

Oracle and API-audit preparation has its own state marker because those inputs
are independent of the instrumented Cargo target. A warm `make
test-coverage-all` therefore reuses the generated oracle and route-audit
artifacts instead of rerunning the phony preparation targets; the marker keys
the tracked fixture, manifest, contract, script, source, and coverage-option
state. A changed preparation input or either optional-preparation flag reruns
the preparation, and `make coverage-clean` removes both state markers.

The `FT_Glyph_Stroke` parity inputs include a maintained malformed-outline case.
It mutates a loaded outline's first tag to cubic, then compares the Rust,
C-ABI, WASM, and pinned-oracle Invalid_Outline result through the public glyph
stroke wrapper; this keeps the ParseOutline error path input-driven and visible
to the all-lane coverage target.

The matching `FT_Glyph_StrokeBorder` parity input uses the same maintained
malformed outline and exercises the public border-stroke ParseOutline rejection
through all three runtime backends and the pinned oracle.

The maintained `FT_Outline_Get_BBox` synthetic curve matrix now exercises both
conic control-box sides, cubic x/y extrema, and small/large cubic peak scaling
through the Rust, C-ABI, WASM, and pinned-oracle routes. It is an input-driven
coverage case rather than a unit-test-only probe.

The maintained `FT_Load_Glyph` auto-hint matrix also includes six normal-scale
Batch126 Latin/Han topology probes across five ppem values. They exercise
reversed top/bottom tilde measurement, multi-contour accent ordering, narrow
and near-bound stem fitting, and descending CJK linked bars through the same
Rust, C-ABI, WASM, and pinned-oracle parity routes.

The same maintained `FT_Load_Glyph` matrix includes six valid Batch127 CJK
edge-link predicate probes across five normal-scale hint targets. Their
serif-winding, spacing, long/short-link, duplicate-backlink, and skipped-edge
topologies exercise reachable CJK edge selection and interpolation through
the Rust, C-ABI, WASM, and pinned-oracle parity routes.

The maintained `FT_Load_Glyph` matrix now includes six valid Batch152 Latin
adjustment-database probes across five legal force-auto-hint targets. Each
probe has a unique cmap glyph index for capital-top/bottom and small-top/bottom
ignore flags, so the public parity route reaches the corresponding blue-zone
arms without a unit-only input. Focused parity passed 30/30; Coverage MCP
snapshot `35a4fd77-749c-4073-82f3-710ee5d13e32` retains +2 regions and +3
branches over the strict baseline, with all coverage denominators unchanged.

The maintained `FT_Load_Glyph` matrix also includes six valid Batch153 Latin
blue-empty probes across five legal force-auto-hint targets. The face maps every
standard Latin blue-string character to a valid multi-contour outline whose
contours each contain one point, so the public parity route reaches the
no-extremum arms of the face-global Latin metrics scan without a unit-only input.
Focused parity passed 30/30; Coverage MCP snapshot
`d658cb15-3204-4f19-80bc-5c4200773858` retains +5 regions, +6 branches, and +2
lines over the strict baseline, with all coverage denominators unchanged.

The maintained `FT_Parameter` matrix now includes thirty valid non-SFNT PCF,
BDF, and Windows FNT bitmap faces with the public
`FT_PARAM_TAG_IGNORE_SBIX` parameter. The parity probe records the successful
open status through the Rust, C-ABI, WASM, and pinned-oracle routes, exercising
the no-SBIX short-circuit in the public parameter dispatch. Focused and managed
parity passed 30/30; Coverage MCP snapshot
`3c35e2a0-8a6e-4642-b0c1-f04282d3c453` retains +1 region and +2 branches over
the strict baseline, with all coverage denominators unchanged.

The maintained `FT_Outline_Render` matrix now includes thirty distinct valid
synthetic public outlines rendered with `FT_RASTER_FLAG_AA`, `DIRECT`, and
`CLIP` into a zero-width Gray target with an out-of-box clip. The exact span
parity route reaches the smooth rasterizer zero-width guard in `src/grays.rs`
through Rust, C-ABI, WASM, and the pinned oracle. Focused and managed parity
passed 30/30; Coverage MCP snapshot
`267b6b9f-ca22-40ed-81a6-5c04d3d4bdbe` retains +1 region and +3 branches over
the strict baseline, with all coverage denominators unchanged.

The maintained `FT_GlyphSlot_AdjustWeight` matrix now includes thirty valid
20 ppem DejaVuSans horizontal-LCD cases with zero vertical strength and unique
16.16 horizontal deltas that round to one LCD pixel while reusing the existing
row pitch. The public load-and-adjust route reaches the bitmap padding-reuse
loop with exact Rust, C-ABI, WASM, and pinned-oracle parity. Focused and managed
parity passed 30/30; Coverage MCP snapshot
`d0e8301b-e1f5-4ca7-97f5-33617b6effd7` retains +9 regions, +4 branches, and +5
lines over the strict baseline, with all coverage denominators unchanged.

The maintained `FT_Outline_Embolden` matrix now includes thirty valid empty
public outlines with distinct 64-bit `FT_Pos` strengths below `i32::MIN`.
Through the existing public gap-matrix route, these inputs exercise both
negative conversion-fallback arms without malformed geometry. Focused and
managed parity passed 30/30; Coverage MCP snapshot
`69514e52-8777-458c-bb2e-0b1cfdc33d8b` retains +11 regions, +6 branches, and +5
lines over the strict baseline, with all coverage denominators unchanged.




The maintained `FT_Outline_Decompose` closure input covers both C conic-first
start rules, consecutive conic midpoint emission, conic closure, and a cubic
pair that closes directly to the computed start through the Rust, C-ABI, WASM,
and pinned-oracle routes. It is an input-driven coverage case rather than a
unit-test-only probe.

The maintained `FT_Stroker_ParseOutline` input also includes the four conic and
eight cubic small-vector threshold combinations used by the pinned stroker
implementation. This keeps the curve subdivision and angle-classification
branches input-driven across all runtime routes.

The malformed optional-table face-open matrix now includes CPAL v1 short-header,
short-count, short-color-record, and short-palette-record inputs. FreeType keeps
the surrounding SFNT face open while ignoring those optional-table failures;
the same maintained inputs exercise the Rust parser's short-read exits through
the Rust, C-ABI, WASM, and pinned-oracle routes.

The color layer matrix also includes separate COLR v0 records for an invalid
layer glyph, an out-of-range CPAL index, and a base record whose layer array is
truncated. FreeType defers these checks until `FT_Get_Color_Glyph_Layer`; the
maintained variants compare the lazy false return, caller-output mutation, and
iterator state across all four parity routes.

By default, `COVERAGE_UNIFIED_LANE_SPLIT=1` builds one instrumented
`unified_fixture_parity` binary, then runs it in three independent shards on
hosts with at least 12 logical CPUs, or two shards on smaller runners, for each
of the Rust FFI, C ABI, and host-WASM backends. `FONTDONE_UNIFIED_BACKEND`
selects the backend, `FONTDONE_UNIFIED_SHARD_INDEX` and
`FONTDONE_UNIFIED_SHARD_COUNT` select a disjoint case slice, and every process
writes a distinct `LLVM_PROFILE_FILE`; the final `cargo llvm-cov report` merges
all shard profiles. The shard profiles must live under
`$(COVERAGE_ALL_TARGET_DIR)/llvm-cov-target`, the nested target directory that
`cargo llvm-cov report` scans. Reusing the binary avoids reacquiring Cargo's
build lock and repeating the test-profile setup. LLVM source-based coverage
counters are process-local, so the extra process-level parallelism removes the
counter contention measured with multiple workers in one process without
changing the input matrix or oracle comparison. Set
`COVERAGE_UNIFIED_SHARDS=2` to reproduce the six-process baseline,
`COVERAGE_UNIFIED_SHARDS=1` to reproduce the three-process split, or
`COVERAGE_UNIFIED_LANE_SPLIT=0` for the legacy single-process diagnostic path.

The clean cold baseline Coverage MCP run `9df27f92-54ba-46a3-8755-c0cf61dddb4b`
(snapshot `abf119b0-7acf-4e69-8378-d3c2c2ddc2cf`) took 64.242 seconds,
including the 45-second instrumented rebuild. With the binary warm, the
two-shard baseline `6a2c26b8-2afd-485e-9ced-3f660cf4e9cf` took 19.747 seconds;
the adaptive three-shard runs `f422f061-f58a-4084-8339-347bf31ba296`
(snapshot `1ccbe0d4-b870-4abf-b09e-b97ae187a06e`) and
`076f95b7-1b71-4e70-9b3c-d72e27b25a6d` took 16.411 and 16.377 seconds; the
final run `d9ba5ed2-51e7-4eda-a6b3-0e30a742ca3a` took 15.283 seconds.
All nine shard processes passed their disjoint 2,524 / 2,525 comparisons, and
the current snapshot remains 50,125 / 54,388 lines, 9,995 / 12,599 branches,
3,410 / 3,822 functions, and 68,870 / 75,559 regions. A four-shard trial
passed but expanded to 26.974 seconds under sustained load, so it is not the
default. The build-state marker excludes worker, lane-split, and shard settings
because they only change process orchestration; compiler-input or
instrumentation changes still force a clean instrumented rebuild.

The report names `fontdone`, `fontdone-c-abi`, and `fontdone-wasm` explicitly
because `cargo llvm-cov report` does not accept the workspace flag; this keeps
the C-ABI and WASM source in the measured denominator.

The `FT_Prop_IncreaseXHeight.limit_changes_autohint_x_height` matrix keeps one
fresh face per `(font, limit, ppem)` cell while loading its three requested
glyphs. This matches the pinned face-scoped property lifecycle and removes
repeated face construction from the coverage hot path; its focused route
history remains separate from the current 7,539-case source-bound snapshot.

`make test-parity` prints these values separately:

- runnable, passed, and failed exact-comparison cases;
- explicitly pending cases;
- covered manifest cases;
- validated public input files;
- function route evidence.

Success requires `passed == runnable` and `failed == 0`. Pending cases are not
passes. A function with a null-validation or narrow success route has runtime
evidence, but not necessarily complete behavior. The root README and
committed compatibility files contain the last recorded measurement.
Missing service or optional-module behavior should be represented by a
maintained oracle-backed matrix input that names the setup state (such as a
valid face opened without `gxvalid`), rather than by a unit-only coverage call.
COLRv1 recursion and malformed-payload coverage controls belong in maintained
font fixtures and full parity cases; do not replace those graph inputs with
unit-only calls into the paint parser.
Separate maintained COLRv1 transform-boundary fixtures cover the initial
static-scale read and final centered-rotate coordinate read without aliasing
their paint records in one malformed table.
Separate translate-boundary fixtures cover the dx read and the dy read after a
successful dx read without aliasing their paint records.

The wrapper also writes `target/parity-evidence/test-parity.log` and
`target/parity-evidence/runtime_parity.json`. The JSON binds the measurement to
the exact parity-relevant path set and contents, toolchain, oracle binary, log
digest, and CI identity when available. To refresh the committed snapshot:

```bash
make test-parity
make record-parity-snapshot
make check-docs
```

The record command never reruns the expensive matrix. It refuses a missing,
failed, or stale report. `doc/runtime_parity_evidence.json` and
`doc/compatibility_snapshot.json` preserve the last committed passing
denominators; reports under `target/` describe the current worktree.

### 3.1 C-contract evidence

The C scorecard measures functions, constants, types, layouts, callbacks,
ownership, state transitions, errors, modules, headers, binary artifacts, and
platform behavior. Its fixed denominators live in
`tests/data/c_contract_inventory.json`.

The functions category includes a blocking all-runtime-row measurement in
addition to 218-name function routing. Consequently, a pending record, macro,
or composite-operation route prevents contract completion even when every
bare function has at least one traced call.

```bash
make api-abi-audit
make c-abi-contract
```

The outputs are:

```text
target/api-abi-audit/api_abi_audit.{json,md}
target/api-abi-audit/route_audit.{json,md}
target/api-abi-audit/c_abi_contract_status.{json,md}
```

`make platform-contract` records native layout, shared/static consumers,
installed-layout checks, exports, and artifact hashes for the active host.
Linux i686 and powerpc64 use `make platform-contract-cross` with explicit
compiler, symbol inspector, sysroot, and QEMU runner. Cross-compilation alone
does not earn runtime credit.

`make c-abi-contract-all-platforms` expects exactly five fresh bundles
assembled under `target/api-abi-audit/platform-contract/`. Requested thorough
CI creates those bundles in three native and two cross jobs, downloads them
into an aggregate job, validates them, and writes the current 12-category
scorecard. This command succeeds only when the evidence is internally exact;
unfinished contract debt remains explicit in the scorecard.

`make c-abi-contract-complete` performs the same evidence validation and then
fails unless all 12 categories are complete. It is the final completion and
release-strength gate, not an ordinary development check. A single-host
checkout normally runs `make c-abi-contract`.

### 3.2 Coverage

Coverage and parity answer different questions. Executing a line or branch
does not prove that its result matches C.

The latest retained strict-30 all-lane Coverage MCP run is
`f1097acb-5a03-4c9c-bcb0-86955faf6187` (snapshot
`fe1caa05-5468-4008-802e-4b5f98e4ec2d`). It completed in 72.157 seconds with
exit code 0 on source commit `7c64f804c590d4d7cd048d1edd8f7be6c869a9df`.
The overall report is 63,240 / 65,417 lines, 11,510 / 13,370 branches,
3,668 / 3,956 functions, and 87,546 / 91,349 regions. The one-time
cache-state migration run
`fe269459-78fa-40cd-a389-1adb9ab32772` took 71.954 seconds, including 51.33
seconds of instrumented test-profile compilation; the preceding cold run
`8c71cb2b-4195-4c87-8d95-2c6f7f799efd` took 71.430 seconds, including 50.54
seconds of compilation. The warm repeat is roughly 81% faster, confirming
that cache-miss instrumented compilation—not Coverage MCP ingestion—is the
speed bottleneck.

The measured MONO plus x-only-strength `FT_Bitmap_Embolden` parity row now
reaches the packed-bit tail-mask branch in `src/ffi/handles.rs`; the focused
case, full matrix, and C-ABI contract all pass without a unit-only coverage
route. The follow-up cleanup removed bitmap-buffer bounds exits that are
unreachable after the helper validates `bytes.len() >= pitch * rows` and uses
checked row arithmetic; it preserved the same exact parity outputs.
The malformed PCF face-open matrix now also reaches metrics, accelerator,
bitmap, and encoding metadata validation boundaries through maintained input
fixtures; focused and full parity remain exact.
The maintained `ftoutln.FT_Outline_Decompose` callback cases now exercise the
exported C-ABI wrapper itself, including callback event delivery, callback
error propagation, missing callbacks, null interfaces, and malformed outlines;
the C oracle remains the independent callback-event reference. The existing
WASM outline-render parity cases likewise invoke the exported facade for
DIRECT rendering and pointer-validation scenarios, while the support helper
retains span capture for exact output comparison.
The maintained `ftsystem.FT_Memory.custom_allocator_runtime_events` route now
installs and exercises the exported C-ABI allocator callbacks under nightly
coverage, including realloc success, failure, zero-size, null-block, and
unknown-block behavior while retaining the same normalized lifecycle output.
The existing U+0245 target-mono `FT_Load_Char` variant also invokes a
coverage-only `GlyphHints` contour probe to reach both control branches of the
Latin segment merge; neither change adds a fixture or unit-only parity case.
The existing two-axis Adobe MM `FT_Set_MM_WeightVector` parity case also
invokes a coverage-only probe for the valid three-axis and four-axis
`type1_mm_weights_unmap` mappings, which the maintained font cannot reach by
itself; the normal two-axis behavior and all parity outputs remain unchanged.
The existing `FT_Select_Size` parity case also invokes a coverage-only probe
using the maintained WinFNT and sbix inputs, reaching the WinFNT strike path,
the sbix strike path, and their invalid-index errors without changing parity
outputs.
The existing `FT_Load_Glyph` Type 1 route also invokes a coverage-only probe
for all Type 1 number encodings and malformed or truncated encodings before
normal charstring lookup; it adds no parity case and leaves normal outputs
unchanged.
The existing `FT_Select_Size` coverage-only probe also opens the maintained
CBLC/CBDT gray-format-1 input and loads missing glyph 0 with
`FT_LOAD_SBITS_ONLY`, reaching the Cblc empty-bitmap fallback and checking its
zero dimensions and empty storage without changing parity outputs.
The `FTC_Manager` ownership parity input also probes missing and zero node
references plus face, size, SBit, and reset calls after manager completion;
these lifecycle edges run through the Rust, C-ABI, and WASM routes and remain
part of the exact input-driven comparison.

Recent fast-loop coverage probes found two parity-adapter defects. The
`FT_Glyph_To_Bitmap` bitmap-strike probe initially forced the Rust route through
`FT_Get_Bitmap_Glyph` even when the selected glyph had no embedded bitmap;
matching the pinned oracle requires falling back to outline extraction and
`FT_Outline_Glyph_To_Bitmap` after `FT_Err_Invalid_Glyph_Format`. The
`FT_Outline_Copy` self-copy case initially gave the WASM wrapper separate source
and target allocations, so it could not reach the exported same-pointer guard;
the adapter now passes one pointer for that contract. Both fixes retain exact
Rust, C-ABI, WASM, and oracle parity; the self-copy 100-case probe was reduced
to one retained witness after Coverage MCP showed no additional line benefit.
The direct `FT_Outline_Render` validation probe likewise added 100 null-
`FT_Raster_Params` variants to reach the WASM invalid-argument guard, then kept
one retained witness after Coverage MCP confirmed the remaining variants added
no new lines.
The `FT_New_Size` null-face probe also exposed an adapter-only mismatch: the
oracle preserves `output_size_nullness` in the error payload even when the
face handle is null. The Rust, C-ABI, and WASM adapters now preserve that
payload while returning the same `Invalid_Face_Handle` status; the 100-case
probe was reduced to one witness after coverage confirmation.

The later null-handle `FT_Done_Glyph` probe initially emitted the wrong JSON
envelope from the offline oracle helper, so the route comparison could not
trust the probe's `status`/`output` shape. The emitter now preserves the
standard top-level status and output object, including the probe label, and
the retained `c65-null-handle-001` row passes all four routes. A c71 focused
compile failure also attempted to read `FT_STROKER_LINECAP_ROUND` and
`FT_STROKER_LINEJOIN_ROUND` from the WASM facade module; those constants are
not exported there. The test adapter now imports the generated constants from
the Rust FFI surface, which is the same source used by the runtime wrapper.
Both were harness defects, not Rust-vs-C behavior differences, and each was
fixed before the subsequent non-coverage parity check.

For each new coverage batch, Coverage MCP is the source of the next target:
query the current snapshot's uncovered or partial lines, read bounded source
context, and advance the file/line cursor after every retained batch. A
structural closing-brace mapping is skipped only when its executable body is
already covered and the following meaningful arm is the actual gap. Per-line
attempt counts are retained in the working notes; a line tried three times is
not selected again unless a code-path change creates a new, evidence-backed
route. The strict region campaign uses exactly 30 different public input
variants per batch. Inputs may be valid or deliberately malformed when the
malformed shape is a public operation input and the pinned C oracle provides
the expected acceptance or rejection. Cases are added only to the parity
matrix and the full batch is measured before any pruning. A batch is retained
only when its parity run is clean and Coverage MCP can attribute its
covered-line, branch, or region delta to the new inputs.

The current strict-30 Batch 9 uses probe family c114 in the maintained
ftsystem.FT_Memory parity matrix. Its 30 variants stay on valid public
load/render, bitmap-conversion, advance, service-query, and size-transform
routes across maintained fonts. The full MCP parity run passed 16,222 / 16,222
runnable comparisons with four explicit pending cases. Coverage MCP snapshot
a90e4463-96db-4845-884f-0339f2fbd4a5 is retained against the previous retained
snapshot 3d0858f9-d728-4cf0-8659-667a9d0ec29f: it adds 177 covered lines,
3 covered branches, 3 covered functions, and 192 covered regions (with the
corresponding source-denominator increases recorded by MCP).

Strict-30 Batch 13 adds 30 valid public `FTC_SBitCache_Lookup` variants to
the existing `returns_cache_owned_sbit` parity case: ten OT-SVG missing-hook
loads and ten composite-slot loads each from DejaVu Sans and Liberation Serif.
The registered full parity run `fc35c0fc-b908-495e-b8d7-e43eb9a76602` passed
16,252 / 16,252 runnable comparisons with four explicitly pending cases.
The corrected cache route converts non-OOM post-load render failures into the
pinned unavailable-SBit sentinel. Coverage MCP run
`f1097acb-5a03-4c9c-bcb0-86955faf6187`, snapshot
`fe1caa05-5468-4008-802e-4b5f98e4ec2d`, adds 14 covered lines, 3 covered
branches, and 22 covered regions against `a90e4463-96db-4845-884f-0339f2fbd4a5`;
the source-denominator increases are recorded by MCP.

The strict-30 Batch61 continuation adds 30 valid public `FT_Load_Glyph`
inputs to the generated TrueType VM branch matrix. The cases cover twilight
zone SHZ execution, non-pedantic out-of-range point writes, and additional
valid stack/control values. Focused parity passed all 30 cases; the registered
full parity run `2193cdff-862e-4794-8294-6483885d759d` passed, and Coverage MCP
snapshot `265f7a5c-44d4-4a89-bb72-fc4409a01560` improves retained baseline
`99baef01-9ad1-4506-810f-8cb37e419cfe` by 1 line, 3 branches, and 2 regions.

The strict-30 Batch63 continuation adds 30 valid public `FT_Load_Glyph`
inputs for six project-authored Type 1 no-op movement and curve programs,
crossed with five public load modes. Focused parity passed all 30 cases; the
registered full parity run `5d74d7cd-b59e-4f00-b2ce-e9e238f2f9ec` passed, and
Coverage MCP snapshot `60f34183-9180-4c51-b671-585e9bfb2200` improves the
retained baseline `99baef01-9ad1-4506-810f-8cb37e419cfe` by 1 line, 8
branches, and 7 regions.

```bash
make test-coverage
make test-coverage-all
```

The focused command writes core Rust JSON. The all-lane command schedules the
independent oracle/audit preparation, then uses nightly branch coverage for the
complete parity matrix. It builds and instruments the core, native C ABI, and
host-compiled WASM facade once, executes the adaptive shard count for each
backend, and merges the raw profiles into
`target/coverage/unified-runtime-all-lanes.json`.

For fast Coverage MCP incremental runs, pass a comma-separated exact allowlist
of public parity `case_id` values as a GNU Make command-line variable. For
example:

```sh
make test-coverage-all \
  MIGRATION_COVERAGE_CASE_IDS='ftbitmap.FT_Bitmap_Copy.success_deep_copy_all_public_fields,ftbitmap.FT_Bitmap_Copy.success_source_equals_target_noop'
```

For the registered Coverage MCP wrapper command, pass the flag and its
comma-separated value as arguments. The flag is repeatable, which keeps each
argument below Coverage MCP's size limit for a larger 30-case batch:

```json
"arguments": [
  "--migration-coverage-case-ids",
  "case-a,case-b"
]
```

If an exact public `case_id` itself contains a comma, escape that comma as
`\\,` in the plural form, or pass the ID with the exact singular flag so it
is not split:

```json
"arguments": [
  "--migration-coverage-case-id",
  "freetype.FT_Set_Pixel_Sizes.set_pixel_size@s[0,_12]"
]
```

For a larger batch, repeat the flag/value pair with another comma-separated
chunk. The registered wrapper is
`RUSTC_WRAPPER= python3 scripts/run_coverage_command.py`; it combines the
chunks and exact IDs, validates duplicate IDs, and invokes the existing `make
test-coverage-all` matrix. The selector is inherited by every Rust, C-ABI, and
WASM coverage lane; omitting it retains the complete matrix and denominator.
The parity harness rejects an explicit selector that matches no runnable or
pending fixture case. Incremental runs still require `execution.mode=incremental`
and an explicit base snapshot.
Optional feature profiles remain a separate `make optional-feature-contract`
gate so the default report does not attribute multiple runtime contracts to the
same LLVM source path.

Repeated local runs reuse the instrumented target and binary. Recent
source-bound current-host warm runs are Coverage MCP
`f422f061-f58a-4084-8339-347bf31ba296` and
`076f95b7-1b71-4e70-9b3c-d72e27b25a6d`, which took 16.411 and 16.377 seconds
with three shards per backend. The final run `3a435c00-9b3e-4abe-9070-89a4f3566e7f`
(snapshot `ecc4a9dd-efe6-4db7-9c82-32cdf7f1bdf3`) took 15.414 seconds. This is
faster than the two-shard baseline
`6a2c26b8-2afd-485e-9ced-3f660cf4e9cf` at 19.747 seconds. The clean cold run
`9df27f92-54ba-46a3-8755-c0cf61dddb4b` took 64.242 seconds, including a
45-second instrumented build, so cache-miss compilation remains the dominant
cold delay; shard execution is concurrent and report/ingestion are small. Use
`COVERAGE_UNIFIED_SHARDS=1` for a three-process comparison, or allow roughly
two minutes for host variation and four to six minutes after a cache reset.
`COVERAGE_TEST_DEBUG=0` omits DWARF line tables while retaining LLVM source
coverage mapping; this reduces cold instrumented-link time without changing the
coverage totals. Face-cache keys also reuse preloaded
font content digests instead of rehashing every expanded case, and the
read-only SFNT table-load/info routes reuse those content-bound handles while
keeping variation-sequence cases isolated. Oracle preparation also preserves
the mtime of unchanged generated constants and validator overlay sources,
avoids needless helper rebuilds and relinks, and reuses the FreeType CMake
configuration when its inputs are unchanged, so repeated oracle builds do not
recompile all C sources. It runs in requested thorough CI, not on every
commit. The parity harness also keeps an ignored per-case oracle cache at
`tests/fixtures/outputs/unified_oracle_case_cache.jsonl`; when the aggregate
cache key changes because a maintained input is added or changed, only missing
case keys are sent to the pinned C batch oracle. A short directory lock
serializes cache population when split coverage lanes start together. Set
`FONTDONE_UNIFIED_ORACLE_REFRESH=1` for focused probes that intentionally
require a fresh C result. In the normal full parity process, worker partitions
now keep all operations for one content-bound font/face index together; the
previous round-robin schedule reopened those faces in every worker. On the
current host this reduced a warm full-matrix run from 227.03 seconds to
192.75 seconds, with 7,535 / 7,535 runnable comparisons passing; the new run
opened 924 cached face handles and spent 81.55 seconds in face prewarming.
The split `make test-coverage-all` lanes set
`FONTDONE_UNIFIED_SKIP_ORACLE_CASE_CACHE_SEED=1`. An aggregate cache hit already
contains the exact ordered output needed by that lane, so rereading and
validating the full ignored per-case cache is redundant; the retained profile
measured roughly 3.3 seconds for that scan in one parity process. Normal parity
and focused runs leave the setting unset and continue to seed or consult the
per-case cache. Set `COVERAGE_SKIP_ORACLE_CASE_CACHE_SEED=0` when diagnosing
cache population itself.

The latest managed warm run completed in 13.605 seconds. Shard timers run
concurrently, so their sum is not wall time; report finalization and artifact
ingestion are included in the wall time but are not separately exposed by
Coverage MCP. The one-time state-marker migration run took 71.954 seconds,
including 51.33 seconds of instrumented compilation; the warm repeat compiled
the test profile in 0.03 seconds. Instrumented compilation remains the
dominant cold component.
The coverage build-state marker intentionally excludes
`tests/unified_fixture_parity.rs`: harness-only coverage probes are ignored
from the report denominator, so Cargo can rebuild the changed integration test
executable while reusing unchanged instrumented runtime libraries and maps.
Runtime source, dependency, toolchain, and coverage-profile changes still
invalidate the instrumented workspace.

| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 63,240 / 65,417 | 96.67% |
| Branches | 11,510 / 13,370 | 86.09% |
| Functions | 3,668 / 3,956 | 92.72% |
| Regions | 87,546 / 91,349 | 95.84% |

That latest run passed all 16,252 runnable parity comparisons with 0 failures;
4 cases remained explicitly pending. Its immutable coverage snapshot is
`fe1caa05-5468-4008-802e-4b5f98e4ec2d`. Coverage MCP accepts the current LLVM
JSON directly, so `COVERAGE_NORMALIZE_SEGMENTS=0` skips the compatibility-only
rewrite; set it to `1` only for an older LLVM JSON producer. The percentages
apply only to the named source commit, suite, and toolchain. They are not a
FreeType-parity percentage, and a covered line or branch does not prove an exact
result.
Generate a new report for the worktree being reviewed. LLVM JSON segments are
normalized to segment start lines by the coverage parser; aggregate region
coverage is preserved from LLVM summaries.

## 4. Diagnose parity failures

1. Select one font, glyph, size, load flag set, and endpoint.
2. Capture C and Rust at the same pipeline stages.
3. Identify the first divergent value.
4. Read the exact pinned FreeType function responsible for that stage.
5. Fix the Rust cause.
6. Rerun the focused case and then `make test-parity`.

Useful stages are raw load, pre-hint scaled outline, bytecode/autohint state,
phantom points and advances, final outline, bbox/cbox, raster cells/spans,
bitmap bytes, and public metadata.

Do not delete a row, narrow a filter, weaken a threshold, bless Rust output as
the oracle, or edit an expected hash to obtain a pass. Permanent diagnostics
use guarded `log::trace!`. The large interactive pipe trace is opt-in:

```bash
PIPE_FONT_PATH=tests/fixtures/input/fonts/DejaVuSans.ttf \
PIPE_SIZE=10 PIPE_CHAR=A RUST_LOG=autohint::pipeline=trace \
make test-pipe-trace
```

### 4.1 Malformed public-input coverage log

Malformed bytes are in scope when they are supplied through a maintained
public parity case. The case is retained only when the pinned FreeType oracle
either reaches the targeted guard or explicitly accepts the malformed input;
Rust output is never promoted to define the expected behavior. The six-case
investigation below was run through `scripts/run_runtime_parity.py` on
2026-08-29, with no unit-test-only coverage input.

| Public case ID | Input expansion reason | Pinned FreeType result | First divergence and resolution |
|---|---|---|---|
| `freetype.FT_Load_Glyph.default_load@pure-cff-hvcurveto-single-operand-no-hinting` | Put one operand in a Type 2 `hvcurveto` stream to reach the alternating-curve operand guard and its public error conversion. | Rejects with `FT_Err_Invalid_File_Format` (`3`). | The Rust parser raised `Invalid_Outline` (`20`); the CFF parser now reports the matching `InvalidFileFormat` variant, and the FFI conversion preserves it. |
| `freetype.FT_Load_Glyph.default_load@pure-cff-type2-unknown-escape-no-hinting` | Put escaped operator `12,99` before `endchar` to exercise the unknown escaped-operator arm. | Accepts the glyph. `psintrp.c:1215-1218` traces an unknown CFF operator without returning an error. | Rust now treats an unknown escaped operator as the same no-op, so the case is an exact success case rather than an expected error. |
| `freetype.FT_Load_Glyph.default_load@glyf-malformed-composite-instruction-length-overflow-hinted` | End a composite component record immediately before its declared instruction length, exercising the deferred composite-instruction read. | Accepts glyph 15 on `FT_LOAD_DEFAULT`. `ftobjs.c:1003-1016` selects the auto-hinter for this SFNT (no `fpgm`, tiny `prep`), so `ttgload.c:1900-1914` does not call the composite instruction processor. | Rust eagerly read the trailing bytes before selecting the fallback. The scaler now reloads auto-hinted outlines without composite instructions, and the glyph loader defers the instruction read. |
| `freetype.FT_Load_Glyph.default_load@glyf-malformed-composite-instructions-overflow-hinted` | Declare two composite instruction bytes but provide no payload, exercising the same public auto-hinter boundary with a different malformed tail. | Accepts glyph 16 for the same reason as glyph 15. | The same default-auto-hinter selection fix prevents an invalid pre-fallback `Invalid_Outline`; the contract records the pinned success. |
| `freetype.FT_Load_Glyph.default_load@glyf-depth-overflow-no-scale` | Use a recursive composite chain deeper than the loader limit to reach the unscaled recursion guard. | Rejects with `FT_Err_Invalid_Composite` (`21`). | Rust reached the guard as `Invalid_Outline`; the public FFI mapping now exposes `Invalid_Composite` while retaining the guard. |
| `freetype.FT_Load_Glyph.default_load@glyf-depth-overflow-no-hinting` | Send the same recursive composite chain through the scaled no-hinting route to verify the guard is not specific to `NO_SCALE`. | Rejects with `FT_Err_Invalid_Composite` (`21`). | The same depth guard and FFI mapping now match the oracle on the no-hinting route. |

The C source makes the two permissive results deliberate rather than
accidental: unknown CFF escapes only emit a trace, and composite instruction
bytes are read later by `TT_Process_Composite_Glyph` (`ttgload.c:1210-1228`)
after the hinted-load condition (`ttgload.c:1900-1914`) is satisfied. The
focused public parity result is 6/6. Keep this log append-only when a new
malformed case is added; record its exact case ID, why it reaches a target
region, the oracle result, and the first Rust divergence.

The next five-case expansion was source-reviewed against the pinned FreeType
implementation before it was probed. These are the exact IDs and reasons; the
two CID rows intentionally use malformed or unusual metadata because the
oracle accepts those inputs. Coverage MCP attribution determines retention.

| Public case ID | Target and expansion reason | Pinned FreeType review | Focused result and coverage disposition |
|---|---|---|---|
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior.batch35_cjk_latin_size_modes@batch36-load-001-cjk-tiny-stem-mono-snap-above-reference` | Valid U+7530 (`30000`) from `cjk-tiny-stem.ttf`, 20 ppem, `FORCE_AUTOHINT | TARGET_MONO`; intended to reach the near-round upper snap arm in `src/autohint/cjk.rs:cjk_snap_width`. | `afcjk.c:1440-1480` accepts the glyph and applies the same reference-width snap. | Pass; MCP still reports the target region uncovered, so this zero-yield coverage probe is pruned. |
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_glyph_force_autohint_behavior.batch35_cjk_latin_size_modes@batch36-load-002-cjk-tiny-stem-normal-first-width-fallback` | The same valid glyph and size with `FORCE_AUTOHINT | TARGET_NORMAL`; intended to reach the first standard-width fallback in `compute_stem_width`. | `afcjk.c:1489-1557` accepts the glyph and uses the first standard width when the distance is close enough. | Pass; MCP still reports the target region uncovered, so this zero-yield coverage probe is pruned. |
| `ftcid.FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face@cid-type1-missing-supplement-default-zero` | Project-owned CID Type 1 derivative with `/Supplement` omitted; verifies whether the apparent required-field guard is real public behavior. | `cidload.c:396-504` parses the dictionary without requiring Supplement; zero-initialized `CID_FaceInfo` yields Supplement `0`, and C returns success. | Initially exposed Rust-only error `3`; Rust now defaults the missing field to `0`, then all endpoints agree. |
| `ftcid.FT_Get_CID_Registry_Ordering_Supplement.success_cid_keyed_face@cid-type1-zero-fdbytes-single-fd` | Project-owned CID Type 1 derivative with `/FDBytes 0 def` and one-byte CIDMap entries; verifies the loader's lower-bound rule. | `cidload.c:831-867` rejects `GDBytes == 0` and values above four, but does not reject `FDBytes == 0`; C returns success. | Initially exposed Rust-only error `3`; Rust now accepts the pinned `0..=4` FDBytes range, then all endpoints agree. |
| `ftstroke.FT_Glyph_Stroke.destroy_original_option@generic-rectangle-glyph0` | Valid project-owned rectangle glyph outside the existing DejaVu special case; probes the public generic parse/count/export stroker route and ownership flag. | `ftstroke.c:2283-2315` accepts the outline and replaces the glyph while honoring destruction. | Pass; MCP still reports the generic Rust route uncovered, so this zero-yield coverage probe is pruned. |

The five exact cases passed 5/5 after the CID fixes. The two CID failures were
therefore implementation mismatches, not evidence that the public inputs should
be removed, and those two rows are retained. The CJK and stroker inputs were
oracle-accepted parity probes, but their intended target regions remained
uncovered in the incremental source review, so they were pruned from the
coverage batch rather than counted as gains.

Coverage MCP run `f2cf80dc-2b2b-42b6-be25-3fd842e527e3` produced snapshot
`10a4c5a8-fff7-4f31-868f-3de5586b3714` from the five-case probe against
explicit baseline `d77068bd-5110-4d9a-8328-7a1a9d6d708d`. Its selected-subset
union reported +2 functions, +7 branches, +2,591 regions, and 0 net covered
lines; the replacement-style selected diff reported 17 newly covered line
identities and 43,786 `not_observed` baseline observations with no regression.
These figures are incremental evidence only, not a complete-denominator
coverage percentage.

The surrounding 50-case `FT_Load_Glyph` public batch also passed 50/50. It was
sent to Coverage MCP as repeatable `--migration-coverage-case-ids` arguments in
four-case chunks because the MCP limits one argument value to 512 bytes. The
incremental run is `7dd7d94a-905c-45a7-859e-f714d2313f3b`, measured snapshot
`2ed9fdbd-e685-4ba0-a8bf-ca43311946dd`, and explicit baseline
`d77068bd-5110-4d9a-8328-7a1a9d6d708d`. Coverage MCP reports an additive union
of +1 covered function, +6 covered branches, 0 covered lines, and +1,085
covered region identities; the merge is marked `exact=false` because the
selected run is not a complete denominator. Its replacement-style diff is
therefore `claim_status=limited`, with 44,226 baseline observations marked
`not_observed`, not regressed. Do not present this selected-subset result as a
new full-run percentage; obtain a complete source-matched snapshot before
making a strict denominator claim.

### 4.2 Pre-expansion source-review ledger

Before adding another public-input batch, each candidate receives a stable
runtime ID and a source-backed reason for existing. The input is expanded only
when the pinned FreeType implementation either accepts the value or reaches
the defensive branch that the case is intended to exercise. A candidate that
is structurally unreachable, already represented by an existing public case,
or rejected before the target region is marked rejected here rather than
being added as coverage noise. This ledger is deliberately kept beside the
malformed-input results so that a passing parity result and a coverage gain
remain separately auditable.

The first pre-expansion group is the BDF `PIXEL_SIZE` edge matrix:

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-001` | Omit the optional `PIXEL_SIZE` property while retaining a parseable BDF face; this is the only public input in the group that can reach Rust's fallback at `src/font.rs:1232-1242`. | `bdfdrivr.c:532-547` only enters the pixel-size block when `bdf_get_font_property` finds the property, so absence is accepted and leaves the default strike value unchanged. | Add with the logical case's fixed public property list; `PIXEL_SIZE` is expected to return FreeType's `Invalid_Argument` while face construction still succeeds. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-002` | Set `PIXEL_SIZE 0` to distinguish an explicitly present zero from an absent property and reach the fallback after the positive-value filter. | `bdfdrivr.c:532-547` accepts zero and assigns a zero ppem; there is no positive-value rejection in the oracle. | Add and query `PIXEL_SIZE`; retain only if all public endpoints match. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-003` | Set `PIXEL_SIZE -12` to exercise signed absolute-value handling rather than only the ordinary positive integer path. | `bdflib.c:642-644` parses the built-in integer with `bdf_atol_`; `bdfdrivr.c:535-546` applies `FT_ABS` and accepts the negative value. | Add and query `PIXEL_SIZE`; retain only if all public endpoints match. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-004` | Set `PIXEL_SIZE 40000` to reach the C driver's out-of-range clamp and Rust's saturating `i16::MAX` conversion. | `bdfdrivr.c:539-543` clamps values beyond `+/-0x7FFF` to `0x7FFF << 6`; `bdflib.c:642-644` does not reject the decimal input at parse time. | Add and query `PIXEL_SIZE`; retain only if all public endpoints match. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-005` | Try a quoted/atom `PIXEL_SIZE` value to reach Rust's `Atom` arm in the metadata match. | `bdflib.c:92` declares `PIXEL_SIZE` as `BDF_INTEGER`; `bdf_add_property_` uses that fixed format at `bdflib.c:642-644`, regardless of token spelling. A quoted or nonnumeric value is parsed as integer zero/error behavior, not as an Atom record. | Reject as a structurally unreachable Atom branch for this public BDF route. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-006` | Try to force a Cardinal `PIXEL_SIZE` record. | `bdflib.c:92` fixes the built-in property type to `BDF_INTEGER`; Cardinal is used for properties such as `RESOLUTION_X` (`bdflib.c:123-127`), not `PIXEL_SIZE`. | Reject as structurally unreachable; use the existing `RESOLUTION_X` rows for the Cardinal path. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-007` | Remove `FONTBOUNDINGBOX` while keeping the property matrix, to reach a later metadata fallback. | `bdflib.c:1424-1426` explicitly returns `Missing_Fontboundingbox_Field` before the face property API; the candidate fails before the target metadata line. | Reject for this batch; it is a constructor-error candidate, not a `PIXEL_SIZE` witness. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-008` | Use an oversized `STARTPROPERTIES` count to try to reach property-allocation defenses. | `bdflib.c:1273-1279` bounds the property count against the input size and returns `Invalid_Argument` before the face property API. | Reject for this target; keep it in the constructor-error inventory if a public error row is needed. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-009` | Omit `FONT_ASCENT`/`FONT_DESCENT` while retaining `PIXEL_SIZE`, probing the driver's metric fallbacks. | `bdfdrivr.c:450-468` explicitly falls back to the parsed font bounding box for both metrics, so this is a separate, valid target rather than a `PIXEL_SIZE` type case. | Defer to a separate metric-fallback batch so its reason and expected fields remain isolated. |
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@cc50-bdf-010` | Use a malformed `BBX` numeric prefix alongside `PIXEL_SIZE`, probing permissive decimal parsing. | `bdflib.c:1027-1033` uses permissive `bdf_atous_`/`bdf_atos_`, but the public face route also has constructor and bitmap consistency requirements; this candidate needs an independent oracle reduction before inclusion. | Defer; do not mix an unverified BBX parser probe into the property batch. |

The four accepted candidates above are public parity inputs, not unit-test-only
probes. Their fixtures and manifest variants were added after this ledger
checkpoint was committed and pushed.

The first focused parity attempt deliberately queried `FONT_ASCENT` and
`FONT_DESCENT` to probe the same BDF property machinery, but it exposed an
oracle-contract issue rather than a runtime mismatch: the exact
`success_bdf_string_integer_cardinal_properties` oracle uses its fixed public
property list (`FAMILY_NAME`, `POINT_SIZE`, `PIXEL_SIZE`, `RESOLUTION_X`). The
variants were corrected to use that list, and the final source-reviewed run
passed 4 / 4 comparisons with no failures. This matters because the C oracle
does allow the four font shapes, but it does not allow an arbitrary per-variant
property list through this logical case.

Coverage MCP then ran exactly those four concrete IDs with
`--migration-coverage-case-ids` in incremental mode against explicit baseline
`d77068bd-5110-4d9a-8328-7a1a9d6d708d`:

```text
run:      c4a0919c-290b-4b61-9ab1-734bffdcf333
snapshot: 610e6988-067d-47f5-8277-2ef2e1ffe6aa
status:   passed; ingested=1
```

The bounded MCP union reports +452 covered region identities and 0 covered
line, function, or branch identities. Its measurement scope is
`selected_subset`, `exact=false`, and test attribution is unavailable, so the
result is evidence that these cases execute the selected BDF paths, not a new
full-suite coverage percentage. The source review and parity result justify
retaining the four inputs; the bounded coverage result does not by itself
justify adding look-alike cases.

The next reachability check covered five already-maintained CFF global-subr
variants. Their IDs were selected because the fixture has a valid first
`callgsubr`, then a deterministic second reload into an out-of-range global
subroutine; pinned `psintrp.c:2241-2258` accepts the first call and reports the
second error. This is the exact public shape that could reach the parent-data
restore at `src/tt/cff.rs:1260-1265`.

| Existing public case ID | Why this input is relevant | Pinned FreeType review | Result and disposition |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-001` | Valid CFF1 global-subroutine return/reload witness with the default hinting mode. | `psintrp.c:2241-2258` accepts the first `callgsubr` and rejects the deterministic out-of-range reload. | Parity passed; retained as an existing public witness, no new coverage identity. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-002` | Same valid CFF structure through the Adobe selector and `NO_SCALE` mode. | The CFF subroutine checks are independent of this public mode selection; the first load is accepted and the second reload errors. | Parity passed; no new coverage identity. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-003` | Same valid structure through rendered normal output, exercising the post-error route after a successful first load. | The pinned decoder preserves the first subroutine return before the second invalid index is reported. | Parity passed; no new coverage identity. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-004` | Same valid structure with `NO_HINTING | RENDER`, isolating the CFF reload behavior from auto-hinting. | `psintrp.c:2241-2258` still governs the accepted first call and later invalid reload. | Parity passed; no new coverage identity. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-005` | Same valid structure with `NO_AUTOHINT | RENDER`, covering the alternate public selector. | The decoder accepts the valid first call before returning the documented invalid-format error on reload. | Parity passed; no new coverage identity. |

Coverage MCP run `b429c96f-a91e-4c84-aa3f-8a8eca6ea938` measured snapshot
`18a989ff-6dbb-4f3b-9a7b-5148b1dd0c70` against explicit baseline
`a817755c-319c-41c8-b56a-f8f52a0441d7`. The additive selected-subset union
reported +0 covered regions, +0 lines, +0 branches, and +0 functions. Its
scope was `selected_subset` with `exact=false`, so it is reachability evidence,
not a complete-denominator claim; no new CFF input was added.

The next candidate is one distinct state transition, not another copy of the
c101 witness. The stable ID and reason are recorded before measurement:

| Candidate public case ID | Why expand this input | Pinned FreeType review | Focused disposition |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c102-ps-global-subr-first-error-second-success` | Keep the valid 108-global-subroutine CFF font, but set the public `random_seed` to `65535`. CFF's first random value then has low 16 bits `0xffff`, selecting biased global subroutine 108 (out of range); the xorshifted second value selects subroutine 107 (valid). This is the missing first-error/second-success post-validation state at `fontdone-wasm/src/implementation.rs:2079-2089`. | `psintrp.c:2241-2258` derives the random value used by `callgsubr`, `psobjs.c:2552-2559` advances it with the pinned xorshift, and `psintrp.c:979-1029` rejects only the out-of-range first call. The valid CFF input is therefore accepted by the oracle up to the intended glyph error; it is not a malformed-font assumption. | Focused parity passed 1/1 across Rust, C ABI, and WASM. No implementation mismatch was found; this input is retained as the source-reviewed reachability witness. Coverage MCP measurement is recorded after the commit below. |

The exact-head Coverage MCP run was then performed after commit `290e745` was
pushed: run `9870cb7c-0332-467a-adb3-c42c2d0e722b` used command
`99e08e7c-f522-4446-885e-0af0d2be0a23`, selected only the C102 ID, and ingested
snapshot `d4c95909-bf99-4bc5-ac7f-3883c26e2c85` against explicit baseline
`05c364db-9864-49d8-8dde-b45169061bbc`. The run passed and its selected WASM
report covered the success side of `second_load.is_err()` at line 2089. The
additive union reported +76 covered lines and +351 regions but no net branch
delta; its `selected_subset` scope is reachability evidence, not a new full
denominator percentage. Existing C86 error/error cases remain the oracle-backed
witness for the opposite side, so no additional malformed input or Rust fix is
justified by this check.

The CJK snap-width candidates were then checked against both the pinned C
implementation and the current source map:

| Existing public case ID | Why this input is relevant | Pinned FreeType review | Result and disposition |
|---|---|---|---|
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-snap-below-standard-target-mono-20` | Valid U+4ED6 rectangle narrower than the U+7530 standard glyph; intended to test the below-reference snap arm. | `afcjk.c:1440-1478` reads `widths[n].cur`, while initialization at `afcjk.c:217-218` writes only `.org`. | Parity passed; the Rust red arm remains unreachable under the pinned data flow. |
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-snap-far-below-standard-target-mono-20` | Valid U+4E1E rectangle far below the standard width; checks the other side of the same snap decision. | `afcjk.c:1440-1478` still compares against zero-initialized `.cur`; a nonnegative working width cannot enter the `width < reference` side. | Parity passed; no input expansion justified. |
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-tiny-stem-20` | Valid narrow CJK stem intended to exercise the minimum-width clamp. | `afcjk.c:1497-1511` normalizes `dist` nonnegative; `afcjk.c:1521-1525` can only take the `<48` side after matching a `.cur` standard width. | Parity passed; the current Rust false side at `src/autohint/cjk.rs:772-774` gains no coverage. |
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-multi-width-snap-target-mono-20` | Valid multiple-standard-width fixture intended to vary the selected reference. | CJK initialization still leaves every width's `.cur` unset; `af_cjk_metrics_scale_dim` at `afcjk.c:648-679` scales blue zones only. | Parity passed; no new coverage identity. |
| `freetype.FT_LOAD_FORCE_AUTOHINT.load_char_force_autohint_behavior@cjk-quantized-width-boundary-target-mono-160` | Valid high-ppem width-boundary fixture intended to test reference ordering and the below-reference comparison. | The same `.org`/`.cur` separation applies; `afcjk.c:742-756` does not scale the width array. | Parity passed; no input expansion justified. |

Coverage MCP run `c57f33fe-eb85-4ed2-abba-8867bedb573c` measured snapshot
`e106908a-50f9-4bf1-a8b2-bafa4087ff32` against the same explicit baseline. The
additive selected-subset union reported +0 covered regions, +0 lines, +0
branches, and +0 functions. This is consistent with the source proof: the
Rust implementation mirrors the pinned C behavior by leaving CJK width `.cur`
values at zero (`src/autohint/cjk.rs:275-279`), and `compute_stem_width` first
normalizes its working distance (`src/autohint/cjk.rs:757-762`). These red CJK
regions are therefore latent dead branches in the pinned FreeType behavior,
not missing malformed or valid public inputs. They should not be covered by
fabricating an input or by changing the denominator.

The next source-reviewed slice targets the gray rasterizer's per-contour
ordering guard. These are deliberately malformed public outline records, but
they are not rejected by the public render entry point before renderer
dispatch: pinned FreeType 2.14.3's `FT_Outline_Render` performs the CBox and
final-endpoint setup, then `ftgrays.c` discovers the earlier contour-order
error during decomposition. `FT_Outline_Check` would reject the same records
when called separately; it is not an implicit preflight of
`FT_Outline_Render` or the outline glyph renderer. The final contour endpoint
is kept equal to `n_points - 1` in every row so the input reaches the intended
inner guard rather than an earlier public-record check.

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftoutln.FT_Outline_Render.bitmap_render_matches_c@invalid-intermediate-contour-order-001` | Use contour ends `[1, 0, 3]` to make the second contour's end fall below its first point with the smallest multi-contour witness. | `freetype/src/base/ftoutln.c:606-667` does not call `FT_Outline_Check`; `freetype/src/smooth/ftgrays.c:1484-1487` checks `last < first` during decomposition. | Focused parity passed: C, Rust FFI, and WASM returned exact `FT_Err_Invalid_Outline` with the sentinel bitmap preserved. |
| `ftoutln.FT_Outline_Render.bitmap_render_matches_c@invalid-intermediate-contour-order-002` | Use `[0, 0, 4]` to exercise the equality boundary where a later contour end equals the preceding contour end and is still below its computed first point. | The same `FT_Outline_Render` dispatch and `FT_Outline_Decompose` guard apply; C does not normalize equal/repeated endpoints before dispatch. | Focused parity passed: C, Rust FFI, and WASM returned exact `FT_Err_Invalid_Outline` with the sentinel bitmap preserved. |
| `ftoutln.FT_Outline_Render.bitmap_render_matches_c@invalid-intermediate-contour-order-003` | Use `[2, 1, 3]` so the first contour is fully in range and a later contour end is below its computed first point, without relying on an out-of-range endpoint. | `freetype/src/base/ftoutln.c:606-667` still dispatches without `FT_Outline_Check`; `freetype/src/smooth/ftgrays.c:1484-1487` returns `Invalid_Outline` at the second contour. | Focused parity passed: C, Rust FFI, and WASM returned exact `FT_Err_Invalid_Outline` with the sentinel bitmap preserved. |
| `ftoutln.FT_Outline_Render.bitmap_render_matches_c@invalid-intermediate-contour-order-006` | Use `[1, 3, 2, 6]` so two complete contours are traversed before a later contour fails, testing that the guard is not only a second-contour artifact. | `ftgrays.c:1484-1487` runs for each contour in order and returns the first decomposition error; `ftoutln.c:647-659` propagates any non-`Cannot_Render_Glyph` renderer error. | Focused parity passed: C, Rust FFI, and WASM returned exact `FT_Err_Invalid_Outline` with the sentinel bitmap preserved. |
| `ftoutln.FT_Outline_Render.bitmap_render_matches_c@invalid-intermediate-contour-order-009` | Use `[3, 3, 8]` to place the repeated endpoint after a four-point valid contour and probe a later equality boundary. | The C renderer accepts the structurally final endpoint, then the gray decomposer computes the next contour's first index and rejects `last < first`. | Focused parity passed: C, Rust FFI, and WASM returned exact `FT_Err_Invalid_Outline` with the sentinel bitmap preserved. |

The initially proposed `[0, -1, 5]` negative-endpoint witness was probed
against the pinned oracle and consistently terminated with `SIGSEGV`; it is
not retained as a public parity case because it has no stable observable
`FT_Error` result. The five IDs above are therefore the safe first
implementation slice from the larger campaign packet. No WASM
`active_size == 0` case is included here: source
review of `fontdone-wasm/src/implementation.rs:2121-2126` found no exposed
public operation that leaves that state immediately before the helper runs.
That target remains deferred until a real public lifecycle path is found.

The focused parity command compared all five retained IDs and passed 5/5
cases across the C oracle, Rust FFI, and WASM backends. No runtime
implementation fix was needed: the existing Rust guard at `src/grays.rs:822`
already matches FreeType's `last < first` check. Coverage MCP run
`e8dc1669-4cd6-408e-9d4e-fdf266403273` then ingested child snapshot
`fdd39eab-6031-4f1c-9422-db87afaad682` against explicit baseline
`a817755c-319c-41c8-b56a-f8f52a0441d7`. Its additive union added one covered
region and one covered line, identifying `src/grays.rs:817` as newly covered;
the canonical region rate moved from `0.9604448061` to `0.9604556550`.
Because this was a selected-subset incremental measurement, its unobserved
baseline hits are not regressions and it is not a replacement for the full
denominator run.

The next CFF random-seed candidate was source-reviewed before it was retained.
The candidate ID was `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-001`. Its intended target was the zero-seed branch at `src/tt/cff.rs:205-207`: the public `FT_Property_Set` value `0` is accepted by pinned `ftpsprop.c:198-220`. The first probe used `value: 0`, which selects the legacy `FT_HINTING_FREETYPE` engine rather than the Adobe path. In that path `cffdecode.c:1727-1741` consumes `top_font.random`; the focused public parity result was C `horiBearingX = -1000` versus Rust `horiBearingX = -592`. The probe was removed before campaign ingestion and is not counted as coverage.

The corrected Adobe-selector batch below uses the generated fixture's exact
Top DICT `Private=(0,172)` entry. In pinned FreeType,
`cffload.c:1889-1890` exits before private-dictionary defaults when either
private size or offset is zero. Therefore this exact empty-private fixture
leaves `priv->initial_random_seed` zero; the `987654321` sanitization at
`cffload.c:1935-1940` applies only after a non-empty private dictionary has
been parsed. `cffload.c:2084-2131` then initializes the Adobe subfont's
random state from the accepted driver seed and falls back to that private
seed. This is the source-reviewed reason the zero-seed input is a valid
witness for the Rust fallback and why the implementation must preserve zero
for this fixture. The corrected batch was added only after that review.

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-001` | Set the accepted CFF driver seed to `0` with the Adobe selector and the default glyph-load route; targets `src/tt/cff.rs:205-207` with the smallest public witness. | `ftpsprop.c:198-220` accepts zero; this fixture's `Private=(0,172)` takes `cffload.c:1889-1890`, so the Adobe subfont random state remains zero through `cffload.c:2084-2131`. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-002` | Repeat the zero-seed Adobe path through `FT_LOAD_NO_SCALE`, separating unscaled public metrics from the default scaled route. | The same property and CFF seed initialization runs before glyph loading; `FT_LOAD_NO_SCALE` changes scaling, not the accepted property or empty-private behavior. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-003` | Use the public string spelling `adobe` plus `FT_LOAD_NO_HINTING`, checking the selector parser and zero-seed fallback together. | `ftpsprop.c:198-220` accepts numeric zero; the Adobe selector reaches the same CFF load, where this fixture's empty `Private` entry leaves the fallback seed at zero. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-004` | Render the zero-seed Adobe CFF glyph at 16 ppem, exercising the fallback before bitmap production. | The CFF property is set before `FT_New_Memory_Face`; the renderer consumes the outline after the same accepted CFF load and zero random state. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c88-ps-random-zero-seed-005` | Render at 24 ppem with `FT_LOAD_NO_AUTOHINT`, covering the alternate public load selector while preserving the same CFF seed witness. | `cffload.c:2084-2131` initializes the subfont before Type2 interpretation; disabling the auto-hinter does not bypass CFF charstring random handling. | Add for focused parity; retain only on exact C/Rust/WASM output. |

The next five CFF inputs are a separate source-reviewed expansion. Their
stable IDs identify the exact CFF byte mutation or Private DICT value, so each
case has a reason independent of its eventual coverage result. FreeType's
`cffload.c:1884-1890` deliberately accepts an absent or zero-valued Private
entry by leaving the private state untouched; `cffload.c:1919-1930` parses a
non-empty entry and propagates parser errors; and
`cffload.c:1935-1940` sanitizes a parsed zero seed to `987654321`. The
operand-less case is intentionally malformed: `cffparse.c:1361-1365` detects
the missing recognized-field operand and `cffparse.c:1540-1542` exposes it as
`Invalid_Argument`. These are pinned C behaviors, not coverage-only
expectations.

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c89-cff-private-omitted-001` | Use `pure-cff-random-no-private.otf`, whose unsupported one-byte Top DICT operator removes `Private` without moving `CharStrings`; targets the Rust `let-else` no-private route at `src/tt/cff.rs:466-469`. | `cffparse.c:1520-1527` ignores unsupported operators and clears the stack; `cffload.c:1884-1890` accepts the resulting absent Private state and exits before defaults. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c89-cff-private-offset-zero-002` | Use a preserved-width Top DICT stack of `1 0 0 Private` to make the size non-zero and offset zero; targets `src/tt/cff.rs:471-474`'s second short-circuit arm. | `cffparse.c:789-815` consumes the first two Private operands and ignores the extra stack value; `cffload.c:1889-1890` exits before reading the pointed-to bytes because the offset is zero. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c89-cff-private-positive-seed-003` | Use the valid non-empty Private DICT `initialRandomSeed 123`; targets the ordinary Private scan, recognized seed operand, and positive return at `src/tt/cff.rs:480-505` and `513-518`. | `cffload.c:1919-1930` parses the Private bytes, while `cffload.c:1935-1940` leaves a positive seed unchanged and `cffload.c:2084-2131` uses it when the accepted driver seed is zero. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c89-cff-private-default-seed-004` | Use the valid non-empty Private DICT `BlueShift 8` with no seed operator; the unrelated field proves the parser reaches the defaulting arm at `src/tt/cff.rs:513-516`. | `cffload.c:1892-1899` establishes defaults, `cffload.c:1935-1940` changes parsed zero `initialRandomSeed` to `987654321`, and `cffload.c:2130-2131` consumes it. | Add for focused parity; retain only on exact C/Rust/WASM output. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c89-cff-private-missing-seed-operand-005` | Use a malformed but openable SFNT whose Private bytes begin with operand-less `initialRandomSeed`; targets the newly corrected Rust error path at `src/tt/cff.rs:496-503`. | `cffparse.c:1361-1365` routes the recognized field with zero operands to Stack_Underflow, and `cffparse.c:1540-1542` returns `Invalid_Argument`; `cffload.c:1924-1930` propagates it from face loading. | Add for focused parity; retain only if C/Rust/WASM report the same error-shaped result. |

The following c90 candidates were reviewed against the pinned C parser before
being proposed. In `cffparse.c:1177-1182`, FreeType classifies every byte at
least 27 as a number except 31 and 255; this is why the reserved byte-27 case
is intentionally retained even though it is outside the documented DICT
number encodings. Bytes below 27, 31, and 255 enter the operator path, and
`cffparse.c:1520-1527` ignores an unsupported operator after clearing the
operand stack. `cfftoken.h:90` confirms that `StdHW` is a real one-byte
Private operator. The one-byte escape is different: `cffparse.c:1337-1343`
requires its second byte and `cffparse.c:1544-1546` exposes the truncated form
as `Invalid_Argument`. Thus the malformed cases below are not assumed to be
valid fonts; each ID states whether it probes FreeType's permissive acceptance
or its public rejection guard.

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c90-cff-private-one-byte-op-001` | Use the valid `StdHW 50` Private DICT value to reach the non-escaped operator arm at `src/tt/cff.rs:489-499`; the ID names the exact field family rather than adding another generic CFF font. | `cfftoken.h:90` registers `StdHW` as one-byte operator 10; `cffparse.c:1350-1373` finds and stores its single operand while `cffload.c:1919-1930` accepts the parsed Private DICT. | Add only if exact C/Rust/WASM parity holds. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c90-cff-private-reserved-byte-22-002` | Use a one-byte Private payload `0x16` to exercise the new reserved-operator classifier and stack-clear path at `src/tt/cff.rs:485-510`. | `cffparse.c:1177-1182` sends byte 22 to the operator path; no field matches, and `cffparse.c:1520-1527` explicitly ignores unsupported operators. Pinned FreeType therefore allows this malformed Private payload during face loading. | Add only if exact C/Rust/WASM parity holds; retain as an oracle-permitted malformed input. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c90-cff-private-reserved-byte-255-003` | Use a one-byte CFF1 Private payload `0xff` to cover the explicit reserved-255 exception at `src/tt/cff.rs:489-499` without conflating it with CFF2 fixed-number operands. | `cffparse.c:1177-1182` explicitly excludes 255 from numbers, so it is handled as an operator; `cffparse.c:1520-1527` ignores it as unsupported. This is permissive CFF1 behavior even though the comment says the byte should not appear in fonts. | Add only if exact C/Rust/WASM parity holds; retain as an oracle-permitted malformed input. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c90-cff-private-reserved-number-27-004` | Use Private bytes `0x1b 0x16` to cover the newly restored `read_dict_number` byte-27 arm at `src/tt/cff.rs:511-515` and then prove the following reserved operator clears the stack. | `cffparse.c:1180-1186` pushes byte 27 as a number, `cff_parse_integer` returns `27-139` (`cffparse.c:94-124`), and the following byte 22 is ignored by `cffparse.c:1520-1527`. FreeType accepts this reserved-number encoding. | Add only if exact C/Rust/WASM parity holds; retain as an oracle-permitted malformed input. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c90-cff-private-truncated-escape-005` | Use a declared one-byte Private range containing only `0x0c` to reach the Rust escaped-op bounds error at `src/tt/cff.rs:491-494`. | `cffparse.c:1337-1343` advances past escape 12, detects that the second byte is at the limit, and jumps to `Syntax_Error`; `cffparse.c:1544-1546` returns `Invalid_Argument`, propagated by `cffload.c:1924-1930`. | Add only if C/Rust/WASM expose the same error-shaped result; this is a deliberately malformed rejection case, not a valid-font claim. |

The c90 focused parity command retained all five cases: C, Rust, and WASM
matched 5/5. Coverage MCP run `c6e0e118-71a1-4692-a221-2671019f1983`
completed at pushed commit `ac7add84bbaeb64a9d0922daa48102aa8639bee5` and
ingested child snapshot `03084b57-e553-4e1d-989b-1d7931524f7e`. The child
metadata again resolved to an older internal commit, so the same generated
LLVM report was imported with explicit provenance as authoritative snapshot
`239f1431-f188-436b-8e78-11688e494e92`. Against the explicit post-c89
baseline `5e2acea8-59f3-4a71-8b1a-a9cdded31d5c`, the MCP additive union
reported `+3` branches, `+1` function, `+52` lines, and `+452` regions;
the selected-subset result has no regression claim, and LLVM test attribution
is unavailable. The bounded source review confirms the c90-selected
measurement reaches the reserved-byte classifier, non-escaped operator arm,
truncated escaped-operator error, and byte-27 number arm. Its remaining
compound-condition branch is the explicit `byte == 31` side at
`src/tt/cff.rs:489`, while the size-zero early return remains a separate
source-reviewed target at `src/tt/cff.rs:471-474`.

The c91 selection deliberately reuses the existing c88 size-zero witness
instead of adding a duplicate font: `c88-ps-random-zero-seed-001` already
documents the valid `Private=(0,172)` input and its `cffload.c:1889-1890`
early exit. The four new c91 IDs were source-reviewed before generation. The
pinned build leaves `CFF_CONFIG_OPTION_OLD_ENGINE` disabled
(`freetype/include/freetype/config/ftoption.h:905-910`), so byte 31 follows
the modern unknown-operator path. The signed seed cases use the existing
`cff_parse_integer` four-byte range and then the explicit sanitization in
`cffload.c:1935-1940`; they are valid numeric Private DICT values, not parser
fuzzing. The unterminated real is the only new permissive malformed control:
`cffparse.c:1194-1198` intentionally exits successfully at the Private range
boundary.

| Candidate runtime ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c91-cff-private-legacy-byte-31-001` | Use a one-byte Private payload `0x1f` to close the `byte == 31` side of the classifier at `src/tt/cff.rs:489-499`; the ID records the legacy operator value. | `ftoption.h:905-910` leaves the old engine disabled; `cffparse.c:1177-1182` excludes 31 from numbers, and the modern operator path ignores unsupported byte 31 through `cffparse.c:1520-1527`. | Add only if exact C/Rust/WASM parity holds; retain as an oracle-permitted malformed input. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c91-cff-private-unterminated-real-002` | Use a declared one-byte Private range containing only `0x1e` to verify the newly corrected harmless-EOF route at `src/tt/cff.rs:1234-1242`. | `cffparse.c:1189-1198` skips the real and exits successfully when the range ends before a terminator; `cffload.c:1924-1940` continues with the parsed/default seed. | Add only if exact C/Rust/WASM parity holds; retain as an oracle-permitted malformed input. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c91-cff-private-negative-seed-003` | Use valid `initialRandomSeed -123` to exercise signed seed input and the positive-sanitization behavior at `src/tt/cff.rs:517-522`. | `cff_parse_integer` accepts the four-byte/signed CFF number forms (`cffparse.c:94-147`), and `cffload.c:1935-1940` negates a negative seed before `cffload.c:2130-2131` consumes it. | Add only if exact C/Rust/WASM parity holds. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_cff_random_batch@c91-cff-private-minimum-seed-004` | Use valid four-byte `initialRandomSeed -2147483648` to test the signed CFF integer minimum and the Rust absolute-value/cast boundary at `src/tt/cff.rs:517-522`. | `cff_parse_integer` reads the signed long integer (`cffparse.c:111-119`); on the pinned 64-bit build, `cffload.c:1937-1938` converts it to `2147483648` before the `FT_UInt32` cast at `cffload.c:2130-2131`. | Add only if exact C/Rust/WASM parity holds. |

The c91 focused selection will include the existing c88 size-zero ID plus
these four new IDs; no duplicate size-zero fixture is retained.

The c91 focused parity command retained five cases: the existing c88 size-zero
witness plus the four new IDs above; C, Rust, and WASM matched 5/5. Coverage
MCP run `d5fff2f2-0ba3-43b8-88e7-9852ef51cff4` completed at pushed commit
`975158afc43215f297900a06b5724597fbf8b1d9` and ingested child snapshot
`f0615e72-b819-4eca-882a-02e338c5033a`. The child metadata was stale, so the
same generated LLVM report was imported with explicit provenance as
authoritative snapshot `fb02b38a-bd4d-46aa-b2b6-c4cfecce7632`. Against the
explicit c90 baseline `239f1431-f188-436b-8e78-11688e494e92`, the MCP
additive union reported `+9` branches, `+2` functions, `+39` lines, and
`+351` regions. The selected-subset result has no regression claim, and LLVM
test attribution is unavailable. The union newly observes the size-zero
return, the byte-31 operator witness, the recognized seed operand, the signed
seed sanitization, and the harmless unterminated-real EOF route; selected-only
source views may still show c90/c89 hits as `not_observed` and must not be read
as parity failures.

The c89 focused parity command retained all five cases: C, Rust, and WASM
matched 5/5. Coverage MCP run `59bf6605-493b-4144-a5b1-675ef769852b`
completed at commit `33feaf63fbab4213ce3fd6c91b6ec60f28a7378d` and ingested
child snapshot `55a8c88f-1dd2-49fa-91c6-b30a4d4c0a67`. Because the child
metadata resolved to an older internal commit, the same generated report was
imported with explicit provenance as snapshot
`5e2acea8-59f3-4a71-8b1a-a9cdded31d5c`; use that snapshot for source review.
Against explicit baseline `a817755c-319c-41c8-b56a-f8f52a0441d7`, the MCP
additive union reported `+16` branches, `+3` functions, `+0` lines, and
`+712` regions. The selected-subset union is measured evidence, not a full
denominator regression run; its rate changes are not used as a claim of lost
coverage. The bounded source review confirms the c89 cases cover the no-
Private return, nonzero-size/zero-offset return, non-empty scan, default seed,
positive seed, and operand-less-seed error. It still reports one short-circuit
branch gap at `src/tt/cff.rs:471` (the size-zero condition), the one-byte
operator arm at `src/tt/cff.rs:487-494`, and the truncated escaped-operator
guard at `src/tt/cff.rs:489-490`; those are the next source-review targets,
not reasons to weaken or remove the retained cases.

The next expansion is a callback-stream error pair for the remaining WASM
`bzip2_source_bytes` error arm. Both IDs reuse the maintained valid compressed
payload; the input difference is the caller-owned public `FT_Stream_IoFunc`
behavior. This was reviewed against the pinned FreeType source before adding
the cases.

| Candidate public case ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `ftbzip2.FT_Stream_OpenBzip2.error_callback_seek_failure` | Return a nonzero status for the zero-byte seek probe, targeting the WASM callback-materialization error return while verifying the sentinel target remains untouched. | `ftsystem.h:247-259` defines nonzero on `count == 0` as seek failure; `ftstream.c:56-86` maps it to `Invalid_Stream_Operation` and does not update `source.pos`; `ftbzip2.c:122-148` propagates the error before allocation or target reset. | Add as a deliberately malformed public callback contract case; retain only on exact C/Rust/C-ABI/WASM error output. |
| `ftbzip2.FT_Stream_OpenBzip2.error_callback_short_header_read` | Return two bytes for the four-byte header read, targeting the same WASM error arm with a valid EOF-style short-read failure and the source-position update. | `ftsystem.h:247-259` permits a short count for `count > 0`; `ftstream.c:118-158` advances `source.pos` by the returned count and returns `Invalid_Stream_Operation`; `ftbzip2.c:122-148` propagates it before allocating or resetting the target. | Add as a public malformed-stream callback case; retain only if all endpoints agree on status, `source.pos == 2`, and unchanged target fields. |

The seek-failure row is intentionally labeled malformed because the callback
violates the usual zero-byte seek success convention; it is still a documented
public error signal, not a fabricated Rust-only branch. The short-read row is a
normal public stream failure shape. Neither row adds a binary fixture or
changes the full denominator.

The focused parity selection for the two new IDs passed 2 / 2 across Rust, the
C ABI, and WASM. The first managed run executed at pushed commit
`af32944fb37775c570356d833e3b6dee2c67a3d8` but Coverage MCP marked its report
stale during automatic ingestion. That exact generated LLVM report was
therefore imported with explicit provenance as snapshot
`71d97363-cf71-44e6-b179-1384116714f9`; a current-command rerun of the two
existing success IDs ingested snapshot `e55d7b20-e09c-4949-b2b5-05ddc18b35cf`,
and the six-case Bzip2 union ingested snapshot
`33011723-25b3-4f0a-9629-ccc194dc4cc7`. Bounded source review of the latter
snapshot shows the callback seek-failure and short-read arms covered in both
ABI facades, while the existing success and invalid-header cases cover the
success and post-call error decisions. These are selected-subset measurements;
they are reachability evidence, not a new full-denominator percentage.

The pinned source review also rejects three tempting look-alike inputs for this
target. A non-null `FT_Stream` with nonzero size but both `base` and `read`
null is not a stable C error case: `ftstream.c:56-86` accepts the seek, then
`ftstream.c:118-158` would copy through `stream->base` during the header read.
The `FT_ULong`-to-`usize` conversion failure is a 32-bit compile-time arm, not
an input distinction on the supported 32-bit ABI. Finally, the `FT_QNEW`/Rust
allocation-failure branches at `ftbzip2.c:485-508` and the corresponding
materialization helper require allocator fault injection; no byte input can
reliably produce them. The feature-disabled Bzip2/LZW returns are likewise a
separate `#else` build, already represented by the dedicated disabled-build
cases, and cannot be reached by expanding an enabled-build input.

The next source-reviewed audit reused an existing bitmap-copy family rather
than adding duplicate inputs. The logical case ID
`ftbitmap.FT_Bitmap_Copy.error_array_too_large_dimensions` expands into five
concrete runtime IDs; Coverage MCP and the runtime filter require the
`@variant` suffix when selecting those inputs.

| Concrete case ID | Why this exact input exists | Pinned FreeType result | Decision |
|---|---|---|---|
| `ftbitmap.FT_Bitmap_Copy.success_deep_copy_all_public_fields` | Reach the normal validated copy and WASM success-record writeback. | `ftbitmap.c:73-124` copies the public record and copies `rows * abs(pitch)` bytes after the checks. | Reuse existing public case. |
| `ftbitmap.FT_Bitmap_Copy.success_null_source_buffer` | Reach the public empty-payload success route and verify the descriptor is still copied. | `ftbitmap.c:88-94` copies `*target = *source` and explicitly returns `FT_Err_Ok` when `source->buffer == NULL`; this is intentional C behavior. | Reuse existing public case. |
| `ftbitmap.FT_Bitmap_Copy.success_flow_flip` | Reach the opposite-pitch flow writeback and the normal success side of the WASM `if err == FT_Err_Ok`. | `ftbitmap.c:82-91` negates the copied pitch when flow differs, then `ftbitmap.c:104-120` reverses rows. | Reuse existing public case. |
| `ftbitmap.FT_Bitmap_Copy.ownership_replaces_target_buffer` | Reach the non-null target-buffer release before the source record replaces it. | `ftbitmap.c:85-88` calls `FT_FREE(target->buffer)` before assigning the source descriptor. | Reuse existing public case. |
| `...error_array_too_large_dimensions@rows-2-pitch-1073741824` | Test the first `rows * pitch > FT_INT_MAX` boundary with a non-null but only one-byte source payload; the guard must run before any read. | `ftmemory.h:214-220` routes to `ft_mem_qrealloc`; `ftutil.c:127-139` returns `Array_Too_Large` before allocation. | Reuse existing concrete variant. |
| `...error_array_too_large_dimensions@rows-3-pitch-715827883` | Test the next multiplication boundary without relying on a single extreme field. | Same `ft_mem_qrealloc` overflow guard returns `Array_Too_Large`. | Reuse existing concrete variant. |
| `...error_array_too_large_dimensions@rows-4-pitch-536870912` | Test the exact power-of-two multiplication boundary above `FT_INT_MAX`. | Same guard rejects the count before allocation or source access. | Reuse existing concrete variant. |
| `...error_array_too_large_dimensions@rows-2147483648-pitch-1` | Test `FT_INT_MAX + 1` rows with unit pitch. | `new_count > FT_INT_MAX / item_size` returns `Array_Too_Large` on the supported 64-bit C ABI. | Reuse existing concrete variant. |
| `...error_array_too_large_dimensions@rows-4294967295-pitch-1` | Test the maximum public `FT_UInt` row count with unit pitch. | The same pre-allocation guard returns `Array_Too_Large`; no huge buffer is dereferenced. | Reuse existing concrete variant. |

The five concrete error variants are malformed bitmap descriptors in the
ordinary public ABI sense, but they are deterministic and safe for this
oracle because FreeType rejects them before reading the one-byte payload. They
are not allocator-fault cases and do not rely on undefined behavior. The
focused Coverage MCP run `1d8f54d4-374d-49dd-874c-985b8b36e747` passed all 14
selected concrete IDs across Rust, C ABI, WASM, and the pinned C oracle; source
snapshot `863223ce-421b-41cb-8d55-77976afa670d` marks both sides of the WASM
`if err == FT_Err_Ok` at `fontdone-wasm/src/implementation.rs:1207-1209` and
all executable lines in `src/ffi/handles.rs:289-360` covered. This is
selected-subset reachability evidence, not a full-denominator coverage claim.
No new input or implementation change was required.

The next expansion is a deliberately oversized HVAR item-variation data
header. Its stable ID and oracle boundary are recorded before the fixture is
created:

| Candidate public case ID | Why expand this input | Pinned FreeType review | Decision before parity |
|---|---|---|---|
| `freetype.FT_New_Memory_Face.success_malformed_optional_tables_ignored@hvar-store-delta-size-too-large` | Set `itemCount=65535`, `longWords=1`, `wordDeltaCount=16385`, and `regionIdxCount=16385`, so the declared delta set is larger than the public allocation limit without allocating or reading gigabytes of payload. This targets the corrected guard at `src/tt/varstore.rs:133-136` (the old uncovered region was `129-130`). | `ttgxvar.c:696-702` computes the same per-item size and calls `FT_QALLOC_MULT`; `ftutil.c:127-139` rejects the request as `Array_Too_Large` before `FT_STREAM_READ`. The optional HVAR failure is then ignored by `ft_var_load_hvvar`, so face construction remains the observable public success path. | Add one malformed public face-open case after the Rust parser returns the same `ArrayTooLarge` classification. Do not claim that FreeType accepts the malformed HVAR internally; it accepts the surrounding SFNT face and rejects this optional table during HVAR loading. |

This ID is intentionally distinct from the existing
`hvar-store-delta-set-truncated` case: that case reaches the C stream-read
failure after allocation sizing, while this case reaches the allocation-size
guard itself. The distinction is the reason for expanding the input rather
than treating another truncated payload as equivalent coverage.

The source review and focused parity result justify this expansion. The
concrete fixture is
`tests/fixtures/input/fonts/variable/hvar-store-delta-size-too-large.ttf`.
Its HVAR header declares `65535 * 65540 = 4,295,163,900` delta bytes, but the
fixture is only 238,264 bytes because no delta payload is needed: pinned
FreeType rejects the allocation request first. The exact public case passed
1/1 in the Rust, C ABI, and WASM lanes, with matching public face-open output.
This means FreeType allows the *surrounding face-open operation* to succeed;
it does not accept the malformed HVAR allocation internally. That distinction
is why this remains a valid public face-open error-containment witness rather
than a claim that the malformed table is valid.

Coverage MCP snapshot
`959840ad-5e25-4f4a-97d7-400747b139ca`, measured at commit
`cb4bb285b3a6e4f62f7252f5a6e7ec3a18f6e3b8` against baseline
`05c364db-9864-49d8-8dde-b45169061bbc`, attributes the corrected
`src/tt/varstore.rs:133-136` guard as covered. The incremental comparison is
selected-subset reachability evidence (`complete=false`), not a new
full-denominator percentage claim.

### Batch 219: CFF EOF return and fixed-operand arithmetic

Before expanding the public parity matrix, the pinned FreeType interpreter was
reviewed for two remaining CFF regions. FreeType does allow both behaviors:

| Concrete ID family | Public input | Why this is a distinct witness | Pinned FreeType boundary |
|---|---|---|---|
| `b219-eof-08-no-hinting-001` through `b219-eof-64-target-lcd-025` | `input/fonts/cff/pure-cff-global-subr-eof.otf`, glyph 1, sizes 8/12/20/32/64, five legal load modes | The glyph calls a global subroutine whose INDEX object ends at EOF. This is not a malformed-input rejection: the C interpreter synthesizes an implicit `RETURN` when the subroutine buffer ends, exercising Rust `src/tt/cff.rs:1345-1351`. | `freetype/src/psaux/psintrp.c:640-667` and `:979-1050` |
| `b219-add-08-no-hinting-026` through `b219-add-64-target-lcd-050` | `input/fonts/cff/pure-cff-fixed-add.otf`, glyph 1, the same size/mode grid | The glyph executes valid Type2 fixed-real followed by integer `add`. FreeType parses the fixed operand and performs the mixed-operand fixed add, exercising Rust `src/tt/cff.rs:1295-1300`. | `freetype/src/psaux/psintrp.c:1560-1575` |

The exact 50 IDs and their individual reasons are maintained in
`tests/fixtures/inputs/public-api/freetype.FT_Load_Glyph.json`. The first
global-subroutine probe used explicit `return` and produced zero new
coverage, so it was not treated as evidence for the EOF-resume lines. These
two generated fonts isolate the source-reviewed shapes while keeping the
public operation, expected output, and C/Rust comparison unchanged. The fixed-add witness is deterministic; stateful `random` remains covered by the separate seed-controlled property cases. The
source review therefore confirms that the original implementation knowingly
allows the inputs; the expansion is for an uncovered interpreter path, not an
attempt to make a rejected malformed input pass.

### Batch 220: CFF `mul` and fixed `callgsubr` error

Batch 220 adds two deliberately separate CFF interpreter witnesses. Each
family expands over five ppem sizes and five public load modes so the same
interpreter operation is checked through the maintained load-flag dispatch
surface; the concrete ID explains the size, mode, fixture, and source reason.
The expansion is therefore traceable to a distinct source behavior rather than
being a collection of duplicate coverage probes.

| Concrete ID family | Public input | Why this is a distinct witness | Pinned FreeType boundary and result |
|---|---|---|---|
| `b220-mul-08-no-hinting-001` through `b220-mul-64-target-lcd-025` | `input/fonts/cff/pure-cff-mul.otf`, glyph 1, sizes 8/12/20/32/64, five legal load modes | The valid CFF1 charstring evaluates integer operands `220` and `2` with Type2 `mul`, then uses the resulting 440-unit edge in a closed contour. This reaches Rust `src/tt/cff.rs:1303-1304` without relying on a malformed font. | `freetype/src/psaux/psintrp.c:2260-2274` pops both operands with `popFixed`, multiplies them, and pushes the fixed result. FreeType accepts the program and emits the same glyph result as Rust. |
| `b220-fixed-subr-08-no-hinting-026` through `b220-fixed-subr-64-target-lcd-050` | `input/fonts/cff/pure-cff-fixed-global-subr-index.otf`, glyph 1, the same size/mode grid | The CFF1 charstring supplies fixed `0.5` to `callgsubr`. The fixture contains 108 trivial global subroutines so the coerced numeric index is in range; this isolates the operand-type validation at Rust `src/tt/cff.rs:1705-1710` from an unrelated subroutine-bounds failure. | `freetype/src/psaux/psstack.c:130-151` makes `popInt` record `Syntax_Error` for a fixed operand; `psintrp.c:979-1050` performs the lookup, `:3030-3035` propagates interpreter failure, and `psft.c:433-435` exposes `Invalid_File_Format`. FreeType rejects the malformed program; it does not unknowingly allow it. |

The exact 50 reasons are stored alongside their IDs in
`tests/fixtures/inputs/public-api/freetype.FT_Load_Glyph.json`, and both
synthetic fonts are generated by
`scripts/font_generation/build_cff_fixtures.py`. Focused parity passed all 50
rows across Rust, the C ABI, WASM, and the pinned oracle before this batch was
committed. The source review confirms that the legal arithmetic case is an
oracle-permitted input, while the malformed operand case is an oracle-matched
error contract; neither family is coverage-only padding.

### Batch 231: CFF global-subroutine post-validation guards

The next CFF gaps were checked against the pinned FreeType interpreter before
expanding the parity matrix. All three new font shapes are malformed Type2
charstrings, and the C oracle rejects each public `FT_Load_Glyph` call with
`Invalid_File_Format` (error code 3); FreeType is therefore not silently
accepting these inputs. The distinction matters: the inputs are valid public
API values for a glyph-load operation, but their font bytes intentionally test
the implementation's defensive error contract.

| Concrete ID family | Public input and reason | Pinned FreeType boundary | Rust target |
|---|---|---|---|
| `b231-negative-gsubr-08-default-001` through `b231-negative-gsubr-12-target-mono-010` | `input/fonts/cff/pure-cff-negative-global-subr-index.otf`, glyph 1, sizes 8/12, five public load modes. The integer `-108` plus the CFF1 bias 107 produces a negative global-subroutine index. | `freetype/src/psaux/psintrp.c:1019-1029` calls the global-region lookup; `freetype/src/psaux/psft.c:630-643` converts the biased value to an unsigned index and rejects it. | `src/tt/cff.rs:1715` |
| `b231-recursion-08-default-001` through `b231-recursion-12-target-mono-010` | `input/fonts/cff/pure-cff-global-subr-recursion.otf`, glyph 1, the same size/mode grid. The sole global subroutine calls itself until the nesting guard. | `freetype/src/psaux/psintrp.c:988-992` rejects a call beyond the maximum nested-subroutine depth. | `src/tt/cff.rs:1725` |
| `b231-top-return-08-default-001` through `b231-top-return-12-target-mono-010` | `input/fonts/cff/pure-cff-top-level-return.otf`, glyph 1, the same size/mode grid. The top-level charstring executes `return` without a saved subroutine frame. | `freetype/src/psaux/psintrp.c:1052-1060` explicitly rejects return from the top charstring. | `src/tt/cff.rs:1737` |

The exact 30 IDs and individual reasons are maintained in
`tests/fixtures/inputs/public-api/freetype.FT_Load_Glyph.json`. The three
fixtures are generated by `scripts/font_generation/build_cff_fixtures.py` and
their hashes are retained in `tests/fixtures/input/fonts/PROVENANCE.md`.
Focused parity passed 30/30 across Rust, C ABI, WASM, and the pinned oracle.
Coverage MCP run `de8d6e6a-6149-428c-a0c2-7b5a7184fbec`, snapshot
`c66ddd05-a1c5-466a-aa58-6a9a950a1784`, used the pushed `2960bc8` checkout and
the explicit full baseline `7405fcdf-db54-48a4-877f-eca87142b938`. It reports
`+3` newly covered lines (`src/tt/cff.rs:1715, 1725, 1737`), `+3` branches,
and `+4` regions, with no function delta. The selected-subset result is
additive reachability evidence only, not a replacement full-denominator score.

### Batch 221: BDF empty atom pointer

The next uncovered WASM region was not a generic failure branch. Coverage MCP
identified `fontdone-wasm/src/implementation.rs:8941-8942`, where a successful
`FT_Get_BDF_Property` call must report an atom with a null pointer as length
zero. The maintained fixture already contains the exact public input needed:

| Concrete ID | Public input | Why expand this input | Pinned FreeType review and first divergence |
|---|---|---|---|
| `ftbdf.FT_Get_BDF_Property.success_bdf_empty_atom_returns_null` | `input/fonts/bdf/properties-duplicate-and-empty.bdf`, property `UNNAMED_PROPERTY_WITHOUT_VALUE`, face index 0 | The no-value user-property line is the smallest public parity input that reaches the WASM `atom.is_null()` arm. No size/mode matrix is justified for this metadata API because the property result does not depend on glyph loading. | `bdflib.c:544-604` classifies the line as an atom; `:657-699` creates the user property and leaves its atom pointer `NULL`; `bdfdrivr.c:897-905` returns `BDF_PROPERTY_TYPE_ATOM` with that pointer; `ftbdf.c:71-86` returns success. Before the fix, Rust skipped the no-value line in `src/font.rs:1049-1050` and returned `Invalid_Argument`; focused parity exposed that exact status mismatch. |

The correction accepts a no-value atom line, preserves an empty atom's null C
pointer, and maps that pointer to `BDF_PROPERTY_TYPE_ATOM` plus `FT_Err_Ok` in
the Rust FFI. The existing WASM wrapper then executes its null-pointer length
branch. This is an oracle-permitted malformed-looking input, not an invented
success case: pinned FreeType knowingly accepts it. The exact input, source
references, and expansion reason are recorded in
`tests/fixtures/inputs/public-api/ftbdf.FT_Get_BDF_Property.json`; one case is
kept until the incremental measurement demonstrates whether further distinct
property forms are necessary.

### Batch 222: WASM invalid-face `FT_Get_MM_Var` route

Coverage MCP left `fontdone-wasm/src/implementation.rs:8686` uncovered even
though the public `FT_Get_MM_Var` contract already described a null-face error.
The existing matrix did not execute that row through the runtime: its harness
opened a real face first and then exercised only the non-variable-face path.
This batch adds one explicit null-handle case so the input ID matches the
uncovered wrapper branch.

| Concrete ID | Public input | Why expand this input | Pinned FreeType review and decision |
|---|---|---|---|
| `ftmm.FT_Get_MM_Var.invalid_face_handle_wasm_route` | Null `FT_Face`, valid `FT_MM_Var**` output initialized to a sentinel; the maintained DejaVuSans asset remains attached for the public route manifest. | Reach the WASM wrapper's `face_mut(handle)` failure at `fontdone-wasm/src/implementation.rs:8685-8686`, which the existing combined null/non-variable matrix did not dispatch. The case is a distinct public handle shape, not a duplicate font or glyph variant. | `freetype/src/base/ftmm.c:130-143` delays face validation to `ft_face_get_mm_service`; a null face returns `Invalid_Face_Handle` before `amaster` is written. Focused parity matched the pinned oracle, Rust FFI, C ABI, and WASM. FreeType intentionally permits this error input; no implementation mismatch was found. |

The case is classified as `real-parity` and its route audit records the exact
oracle-backed reason. The oracle command uses a dedicated null-face invocation,
so the C reference and all Rust-facing lanes receive the same call shape. It
does not add a size, glyph, or format matrix because none of those values can
affect this pre-service face validation.

The focused parity run passed 1 / 1 comparison at pushed commit `a312c70`.
Coverage MCP run `e75d339c-9128-42a7-aa64-bc03e5c14ac7` produced child
snapshot `64e5e2c3-8741-42c4-8479-f790de7c2f97` against explicit baseline
`ab608526-5fc6-4840-bc94-fd702aa0fce9`. Its additive-union review reported 5
newly covered lines and 4 newly covered regions; the target WASM line
`fontdone-wasm/src/implementation.rs:8686` is green. This is a selected-subset
measurement, so it is evidence for the targeted branch only and not a new
full-denominator percentage.

### Batch 223: invalid-face size wrapper routes

Coverage MCP marked the error arms at the baseline lines
`fontdone-wasm/src/implementation.rs:8015` and `:8051` red. The maintained
`error_null_face` cases already exercised the public C-shaped call, but their
harness intentionally short-circuited before the thin C ABI and WASM handle
wrappers. Source review showed that this was a real wrapper mismatch, not an
unreachable branch: FreeType first normalizes the request and then defers
validation to `FT_Request_Size`, whose null-face guard returns
`Invalid_Face_Handle`.

| Concrete ID | Public input | Why expand this input | Pinned FreeType review and decision |
|---|---|---|---|
| `freetype.FT_Set_Pixel_Sizes.invalid_face_handle_abi_routes` | `face:null`, `pixel_width:12`, `pixel_height:12`; the maintained DejaVuSans asset is attached only so the ABI route is selected. | Reach `fontdone-c-abi/src/implementation.rs:23061-23064` and `fontdone-wasm/src/implementation.rs:8014-8017`, which the existing null-face row bypassed; zero is the WASM representation of the null face. | `freetype/src/base/ftobjs.c:3572-3596` performs normalization and calls `FT_Request_Size`; `:3446-3447` returns `Invalid_Face_Handle`. Before the fix both wrappers returned `Invalid_Argument`; the focused oracle comparison exposed that first divergence. |
| `freetype.FT_Set_Char_Size.invalid_face_handle_abi_routes` | `face:null`, `char_width:768`, `char_height:768`, `horz_resolution:72`, `vert_resolution:72`; the same maintained asset is route-only. | Reach `fontdone-c-abi/src/implementation.rs:23037-23040` and `fontdone-wasm/src/implementation.rs:8052-8055`, which were likewise bypassed by the existing null-face row. | `freetype/src/base/ftobjs.c:3532-3558` performs point-size/resolution normalization and calls `FT_Request_Size`; `:3446-3447` returns `Invalid_Face_Handle`. Before the fix both wrappers returned `Invalid_Argument`; the focused oracle comparison exposed that first divergence. |

The runtime fix maps zero/null face handles to `Invalid_Face_Handle` in both
size wrappers, matching the already-correct `FT_Request_Size`, `FT_Select_Size`,
and face-lifecycle wrapper policy. The focused parity run passed 2 / 2
comparisons after the fix, with no new font or glyph variants added. This batch
is committed and pushed before measurement; its incremental Coverage MCP run
must use the explicit campaign baseline and report the selected-subset result
separately from the full denominator.

Coverage MCP run `f9897189-cefc-4470-a4bc-c5e5fd33f0e5` passed at pushed
commit `6fd4b5a977a6b061fe402f26ebf48eb0d0bfbdcb` and ingested child snapshot
`1d559ec9-2e39-42ec-aa1e-db7285668881` against baseline
`ab608526-5fc6-4840-bc94-fd702aa0fce9`. The additive-union review reported 16
newly covered lines and 4 newly covered regions, with no branch/function gain;
the exact-region projection includes the C ABI and WASM wrapper routes. The
snapshot source projection retains stale pre-batch line text, so the local
current source is authoritative for the fixed return lines (`8017`, `8055`,
`23040`, and `23064`). As with the prior batches, this is a selected-subset
measurement and its replacement-style negative percentages are not full
regression or denominator results.

### Batch 224: zero-glyph CFF glyph-map guard

The next source-reviewed witness uses an existing maintained malformed-but-openable
font rather than adding a new generated asset. Its stable variant ID records the
reason for expansion:

| Concrete ID | Public input | Why expand this input | Pinned FreeType review and first divergence |
|---|---|---|---|
| `ftdriver.FT_Prop_GlyphToScriptMap.map_mutation_affects_autohint_script@b224-zero-glyph-cff-map-guard` | `input/fonts/cid/ot-cff-cid-keyed-zero-glyph.otf`, face index 0, the existing four mutation values and three ppems | The existing map witness only exercises a non-empty map. This fixture declares zero CFF CharStrings and zero SFNT `maxp.numGlyphs`, so a cmap entry is clamped to glyph 0 and the WASM map-mutation route must not dereference an empty map. The case keeps the public operation and adds no duplicate asset or padding matrix. | `cffload.c:2478-2508` assigns `num_glyphs` from the CharStrings INDEX and intentionally skips Charset/Encoding loading when it is zero; `afglobal.c:331-392` permits the zero-count globals allocation; `ftobjs.c:3929-3943` clamps an out-of-range cmap result to zero. The first parity run exposed Rust returning glyph 121 instead of 0. After that clamp, the oracle's explicit glyph-zero precondition (`scripts/gen_unified_oracle.c:33353-33357`) was aligned across the three parity adapters. |

The pinned oracle was run directly before implementation changes and returned
`property_error=0`, `glyph_index=0`, `initial_map_value=0`, and
`Invalid_Glyph_Index` for both load and render on all 12 rows. The source review
therefore confirms that FreeType knowingly opens this zero-glyph shape and safely
declines to index its empty map; it is not an invented successful parse. Note that
the direct CFF driver's out-of-range `FT_Load_Glyph` branch is `Invalid_Argument`
(`cffgload.c:238-239`); the parity operation's oracle intentionally short-circuits
before that driver call, so the adapters preserve that explicit route contract.

The runtime fix is in `src/font.rs:5850-5865`: `Font::char_index` now applies the
same public glyph-range clamp as `FT_Get_Char_Index`. The parity harness also keeps
the oracle's glyph-zero precondition consistent in
`tests/unified_fixture_parity.rs:30364-30450` and skips WASM map mutation for that
non-entry. Focused parity passed 1 / 1 comparison across Rust FFI, C ABI, WASM,
and the pinned oracle. The input's `expansion_reason`, `oracle_verdict`, and
source references are stored in
`tests/fixtures/inputs/public-api/ftdriver.FT_Prop_GlyphToScriptMap.json`.

Coverage MCP run `8eee096c-2878-41ec-a420-2ad51350740c` passed at pushed commit
`b37c3cba6c08419148def61d40242a4ad0c02d7f` and ingested child snapshot
`7a83dbf0-1c6d-4097-8cfa-75c1ab90ae87` against the explicit campaign baseline
`ab608526-5fc6-4840-bc94-fd702aa0fce9`. The selected-subset additive review
reported 427 newly covered lines. It did not cover the target WASM helper
(`fontdone-wasm/src/implementation.rs:6250-6294`), because the zero-glyph
variant intentionally skips map mutation; no selected-only percentage is used
as a full-denominator claim.

### Batch 225: missing-property glyph-map error

This batch uses one malformed public-property variant with a stable ID; it does
not add another font, glyph, or size matrix:

| Concrete ID | Public input | Why expand this input | Pinned FreeType review and first divergence |
|---|---|---|---|
| `ftdriver.FT_Prop_GlyphToScriptMap.map_mutation_affects_autohint_script@b225-missing-property-glyph-map-error` | The maintained `input/fonts/autohint/mixed-script-map.ttf`, face index 0, the existing four mutation values and three ppems, with `module_name=autofitter` and `property_name=fixture-missing-property` | The valid-property and zero-glyph variants cannot enter the WASM helper's non-OK post-validation return while keeping the face and glyph valid. This unknown property is the smallest public malformed call that targets that error side without padding the campaign. | `ftobjs.c:5301-5382` accepts the public lookup, resolves the `autofitter` module and dispatches its property service; `afmodule.c:296-304` handles the glyph-map property and `:366-368` returns `Missing_Property` for the unknown name. The direct oracle resolved `x` to glyph 34 and returned error 12 for property, load, and render. Before the helper fix, parity reported WASM `/rows/0/glyph_slot`: expected null, actual rendered slot, because the helper still supplied the valid hardcoded property. |

The implementation now passes the case's module/property strings through the
WASM test-support helper. The parity adapters also propagate a non-OK property
error to the load/render prefix, matching the pinned oracle's
`load_and_render_property_effect` guard. Focused parity passed 1 / 1 across
Rust FFI, C ABI, WASM, and the pinned oracle. The stable ID, expansion reason,
oracle verdict, and source references are stored in
`tests/fixtures/inputs/public-api/ftdriver.FT_Prop_GlyphToScriptMap.json`.

Coverage MCP run `fbd33288-5e3a-4cd9-8e4d-1440ced4a7c3` passed at pushed commit
`8b87dee2801639066fec06d94fc6781a44fd61a8` and ingested child snapshot
`1dcf5af3-ab5a-48c8-9cc1-46cb1b1e5917` against the explicit baseline
`ab608526-5fc6-4840-bc94-fd702aa0fce9`. The supported selected-subset
additive review reported 441 newly covered lines; its region list includes the
current local `fontdone-wasm/src/implementation.rs:6287` error return. The
source-view projection is anchored to stale embedded source commit `f0b1ce...`
and displays pre-parameterization text, so the local checked-out source is the
authority for the current line mapping. Test attribution was unavailable, and
the selected-only replacement percentages and non-exact region fallback are
not full-denominator claims.


### Batch 232: malformed BDF numeric-property parity

This batch deliberately exercises malformed-but-public BDF property lines. The
question is whether the public `FT_Get_BDF_Property` route matches pinned
FreeType, not whether a strict parser would choose to reject the input. Each
fixture has one known integer or cardinal property, one malformed numeric token,
and a stable concrete ID with its expansion reason in
`tests/fixtures/inputs/public-api/ftbdf.FT_Get_BDF_Property.json`.

| Variant ID | Property / raw token | Why this input is expanded |
|---|---|---|
| `int-average-width-no-value` | `AVERAGE_WIDTH` / empty | Verifies known signed integer with no digits becomes zero. |
| `int-avg-capital-width-junk` | `AVG_CAPITAL_WIDTH` / `junk` | Verifies no leading digit becomes zero. |
| `int-avg-lowercase-width-prefix` | `AVG_LOWERCASE_WIDTH` / `42tail` | Verifies decimal-prefix parsing and the first pre-fix divergence. |
| `int-cap-height-negative-prefix` | `CAP_HEIGHT` / `-17tail` | Verifies an optional minus before a decimal prefix. |
| `int-end-space-plus-sign` | `END_SPACE` / `+9` | Verifies that a leading plus is not consumed. |
| `int-figure-width-hex-prefix` | `FIGURE_WIDTH` / `-0x1` | Verifies decimal-only parsing of a hexadecimal-looking token. |
| `int-font-ascent-no-value` | `FONT_ASCENT` / empty | Verifies known no-value integer replacement with zero. |
| `int-font-descent-prefix` | `FONT_DESCENT` / `12oops` | Verifies a signed decimal prefix with trailing junk. |
| `int-italic-angle-real-prefix` | `ITALIC_ANGLE` / `3.5` | Verifies integer-prefix parsing before a decimal point. |
| `int-max-space-i32-prefix` | `MAX_SPACE` / `2147483647tail` | Verifies the positive i32 boundary without token exhaustion. |
| `int-min-space-i32-negative-prefix` | `MIN_SPACE` / `-2147483648tail` | Verifies the negative i32 boundary without token exhaustion. |
| `int-norm-space-saturated-prefix` | `NORM_SPACE` / overflowing decimal | Verifies pinned signed saturation before public truncation. |
| `int-pixel-size-no-digit` | `PIXEL_SIZE` / `junk` | Verifies a known no-digit integer is retained as zero. |
| `int-point-size-prefix` | `POINT_SIZE` / `120oops` | Verifies the prefix survives face-size conversion. |
| `int-quad-width-leading-zero-prefix` | `QUAD_WIDTH` / `007suffix` | Verifies leading zeroes and stop-at-junk behavior. |
| `int-raw-ascent-negative-prefix` | `RAW_ASCENT` / `-7tail` | Verifies signed prefix storage for a raw metric. |
| `int-raw-average-width-no-value` | `RAW_AVERAGE_WIDTH` / empty | Verifies no-value raw integer zero semantics. |
| `int-raw-cap-height-prefix` | `RAW_CAP_HEIGHT` / `5x` | Verifies raw integer prefix retention. |
| `int-raw-descent-plus-sign` | `RAW_DESCENT` / `+11` | Verifies plus-sign rejection in the signed parser. |
| `int-raw-pixel-size-real-prefix` | `RAW_PIXEL_SIZE` / `16.0` | Verifies integer prefix of a real-looking token. |
| `int-small-cap-size-prefix` | `SMALL_CAP_SIZE` / `4rest` | Verifies prefix parsing for another known integer. |
| `int-strikeout-ascent-negative-prefix` | `STRIKEOUT_ASCENT` / `-3tail` | Verifies signed prefix retention for strikeout metrics. |
| `int-subscript-x-prefix` | `SUBSCRIPT_X` / `9abc` | Verifies alphabetic trailing data is ignored. |
| `int-underline-position-no-value` | `UNDERLINE_POSITION` / empty | Verifies known no-value underline metric becomes zero. |
| `cardinal-default-char-no-value` | `DEFAULT_CHAR` / empty | Verifies known unsigned integer with no digits becomes zero. |
| `cardinal-destination-prefix` | `DESTINATION` / `42tail` | Verifies unsigned decimal-prefix parsing. |
| `cardinal-relative-setwidth-plus-sign` | `RELATIVE_SETWIDTH` / `+9` | Verifies unsigned parsing does not consume a plus sign. |
| `cardinal-relative-weight-minus-sign` | `RELATIVE_WEIGHT` / `-1` | Verifies unsigned parsing does not consume a minus sign. |
| `cardinal-resolution-x-prefix` | `RESOLUTION_X` / `75oops` | Verifies prefix parsing before resolution normalization. |
| `cardinal-resolution-y-prefix` | `RESOLUTION_Y` / `96tail` | Verifies the matching Y-resolution path. |

The source review answers the compatibility question directly. In the pinned
FreeType tree, `freetype/src/bdf/bdflib.c:289-339` implements `bdf_atol_` and
`bdf_atoul_`: they consume an optional minus only where applicable, return zero
when no decimal digit is present, stop at the first non-digit, and saturate an
overflowing prefix. `bdflib.c:608-720` retains the known property type and
stores that parsed value; `bdflib.c:1135-1188` tokenizes the public BDF
property line without rejecting the malformed numeric suffix. The public
conversion is then exposed by `freetype/src/bdf/bdfdrivr.c:886-937`, through
the service wrapper in `freetype/src/base/ftbdf.c:62-86`. Thus pinned FreeType
is knowingly permissive for these public property tokens; the fixtures model
observed behavior rather than inventing acceptance of bad input.

The concrete pre-fix witness was
`ftbdf.FT_Get_BDF_Property.batch232_bdf_malformed_numeric_prefixes@int-avg-lowercase-width-prefix`,
using `input/fonts/bdf/malformed-numeric/batch232-03-avg_lowercase_width-prefix.bdf`.
Pinned FreeType returned `OK` and integer `42` for `AVG_LOWERCASE_WIDTH 42tail`;
the old Rust `raw_value.trim().parse::<i64>().ok()?` path in `src/font.rs`
returned `Missing_Property` (error code 6). The fix adds source-matched signed
and unsigned decimal-prefix parsers and keeps the known property rather than
dropping it on parse error. Focused parity then passed all 30 variants across Rust
FFI, C ABI, WASM, and the pinned oracle: 30 / 30 comparisons.

Coverage MCP run `2b2f3b8e-4fe9-404c-b4b4-905b451c70fa` passed at pushed commit
`edb6f0385e240aad4584825a4ed5aa7052b4f248` and ingested child snapshot
`89a9d16c-80b6-4a93-97bc-39e2203570a8` against explicit baseline
`7405fcdf-db54-48a4-877f-eca87142b938`. The explicit incremental review was
supported with `measurement_scope=selected_subset` and `complete=false`; its
baseline-union summary reported +2 covered functions, +931 covered regions,
and no covered-line or covered-branch delta. The selected source projection
identified the new parser/property paths in `src/font.rs:1057-1108`,
`src/font.rs:1201-1296`, and `src/font.rs:1514-1586`. These are additive
selected-run observations only; the non-exact merge and replacement-style
negative percentages must not be read as full-denominator regression or
coverage claims.

### Batch 233: caller-selected CMap restoration

This batch adds an explicit caller preselection to the public
`FTC_CMapCache_Lookup` sequence. The IDs are intentionally distinct by code
point so the reason for each input is reviewable; they do not change the
coverage denominator or hide any existing case. The maintained font has
charmap index 1 as a selectable format-6 map and index 2 as a selectable
format-4 map; format-14 index 0 is not used because the public
`FT_Set_Charmap` contract rejects it.

| Variant ID | Public input difference | Why this input is expanded |
|---|---|---|
| `batch233-cmap-preselect-001` | Preselect index 1, query index 2, U+0041 | Mapped format-4 glyph; first positive witness for save/select/index/restore. |
| `batch233-cmap-preselect-002` | Same maps, U+0042 | Second mapped glyph; confirms the restore is not tied to one cache value. |
| `batch233-cmap-preselect-003` | Same maps, U+0020 | Format-6-only code queried through format 4; restore must still run when the glyph is zero. |
| `batch233-cmap-preselect-004` | Same maps, U+0021 | Paired unmapped-code witness for the preceding boundary. |
| `batch233-cmap-preselect-005` | Same maps, U+0000 | Lower-boundary code; exercises the same state transition with no glyph hit. |
| `batch233-cmap-preselect-006` | Same maps, U+007F | ASCII upper-boundary code; keeps the input public and minimal. |
| `batch233-cmap-preselect-007` | Same maps, U+00A0 | Upper single-byte boundary; distinguishes ordinary ASCII from extended input. |
| `batch233-cmap-preselect-008` | Same maps, U+20AC | Extended Unicode code; checks that restore does not depend on byte-sized input. |
| `batch233-cmap-preselect-009` | Same maps, U+FFFF | BMP upper-boundary code; exercises the public `FT_UInt32` path. |
| `batch233-cmap-preselect-010` | Same maps, U+1F600 | Supplementary-plane code; completes the scalar-width boundary pair. |

The pinned source review answers whether FreeType knowingly permits this
sequence. In `freetype/src/cache/ftccmap.c:244-311`, a negative index is the
special no-change form; a nonnegative valid index saves `face->charmap`,
temporarily assigns the requested `face->charmaps[cmap_index]`, calls
`FT_Get_Char_Index`, and restores the saved pointer. The restore is independent
of whether the resulting glyph index is zero. The first implementation change
therefore threads `preselect_cmap_index` through the parity adapters and the
offline oracle; it does not alter the runtime's public behavior.

Focused parity passed all 10 variants across Rust FFI, C ABI, WASM, and the
pinned oracle. Coverage MCP run `68f7597b-2026-4f0e-bf02-a76b394b00e0` passed
at pre-push commit `cbfcfa2d471538d595d3f1b70f472a8a397e10c` and ingested child
snapshot `799a558e-0338-4723-aff4-de9bdba014fd` against explicit baseline
`7405fcdf-db54-48a4-877f-eca87142b938`. Its supported additive review was a
selected subset (`complete=false`); it observed the lookup body, but the
bounded source review still marks the false side of the restoration condition
at `fontdone-c-abi/src/implementation.rs:2088` unobserved. The selected-only
line/region counters are not a full-denominator claim.

### Batch 234: CMap cache restoration from a null active map

This batch targets the false side of the CMap cache restore path with a valid
public face that has charmaps but no initially selectable Unicode charmap. The
maintained `input/fonts/charmap/cmap-nonunicode-format6.ttf` contains one
platform-1/encoding-0 format-6 map, so cmap index 0 is a valid cache target;
the face itself opens with `face->charmap == NULL`. Each ID below has the same
font, target cmap, repeat lookup, and public route, and differs only in the
codepoint used to make the reason for the expansion explicit.

| Variant ID | Codepoint/result class | Why this input is expanded |
|---|---|---|
| `batch234-cmap-no-active-001` | U+0041, mapped | Positive witness that a valid target map can be used and the saved null state restored. |
| `batch234-cmap-no-active-002` | U+0000, lower boundary/unmapped | Confirms restoration is independent of a zero glyph result at the low boundary. |
| `batch234-cmap-no-active-003` | U+0001, unmapped | Distinguishes the adjacent low codepoint from the boundary case while keeping the same state path. |
| `batch234-cmap-no-active-004` | U+0020, unmapped | Exercises an ordinary ASCII query through the non-Unicode format-6 target. |
| `batch234-cmap-no-active-005` | U+0042, adjacent to mapped code | Confirms the no-active state is not synthesized after a query next to the only mapped code. |
| `batch234-cmap-no-active-006` | U+007F, ASCII upper boundary | Covers the upper ASCII boundary without changing the cache or face setup. |
| `batch234-cmap-no-active-007` | U+0080, first extended-byte value | Checks the same restore arm at the first value beyond ASCII. |
| `batch234-cmap-no-active-008` | U+00FF, extended-byte upper boundary | Covers the format-6 byte-range boundary with a zero glyph result. |
| `batch234-cmap-no-active-009` | U+0100, first value beyond the byte range | Verifies that the temporary target is restored for a codepoint outside format 6's range. |
| `batch234-cmap-no-active-010` | U+10FFFF, maximum Unicode scalar | Completes the public `FT_UInt32` upper-boundary witness without changing the face state. |

The pinned FreeType review confirms that these are accepted public inputs, not
invented success cases. `freetype/src/base/ftobjs.c:1371-1453` searches for a
Unicode charmap and returns `Invalid_CharMap_Handle` without selecting a
non-Unicode record when none exists, leaving the active pointer null.
`freetype/src/cache/ftccmap.c:298-315` then saves that pointer, assigns the
requested `face->charmaps[cmap_index]`, calls `FT_Get_Char_Index`, and restores
the saved pointer. The restore is unconditional for a valid nonnegative index,
including when the saved pointer is null and when the glyph result is zero.

Before the fix, all ten focused comparisons exposed the same first divergence:
the Rust FFI route returned `active_charmap_after=0` instead of the oracle's
`-1`; C ABI, WASM, and glyph values otherwise matched. The fix carries an
explicit no-selected-charmap sentinel through face construction and variation
rebuilds, and centralizes the cache's direct-select/direct-restore operation so
all three Rust-facing façades restore `None` exactly. It intentionally bypasses
the public `FT_Set_Charmap` format-14 rejection because the pinned cache code
assigns the face pointer directly.

Focused parity passed 10 / 10 variants across Rust FFI, C ABI, WASM, and the
pinned oracle at pushed commit `e89ae3d`. Coverage MCP run
`dc3e1068-f8b2-4818-bb9d-8b0b6248d379` ingested child snapshot
`0b716c72-4b98-4e58-b7c1-600e891ac8b9` against explicit baseline
`7405fcdf-db54-48a4-877f-eca87142b938`. The incremental review is a selected
subset (`complete=false`); its additive union reports 461 newly covered line
identities and the targeted CMap helper/adapter regions. Its merge is marked
non-exact, so the selected-only counters are reachability evidence and not a
replacement full-denominator percentage or regression claim.

+### Batch 235: malformed BDF SIZE decimal-prefix fixed-strike metadata

This batch uses 30 distinct maintained BDF inputs through the public
`freetype.FT_FaceRec.available_sizes_public_fields_match_c` parity operation.
The input is malformed by design; each ID records exactly which `SIZE` token
was expanded and why. The campaign is checking whether the pinned FreeType
driver knowingly accepts the token and whether the Rust public
`FT_Bitmap_Size` record matches it.

| Concrete ID suffix | SIZE line | Why expand this input |
|---|---|---|
| `batch235-bdf-size-001` | `SIZE 12tail 75 75` | Stops point-size parsing at trailing alphabetic data while preserving the `12` prefix. |
| `batch235-bdf-size-002` | `SIZE 12 75tail 75` | Stops X-resolution parsing at trailing alphabetic data while preserving the `75` prefix. |
| `batch235-bdf-size-003` | `SIZE 12 75 96tail` | Stops Y-resolution parsing at trailing alphabetic data while preserving the `96` prefix. |
| `batch235-bdf-size-004` | `SIZE 12tail 75tail 96tail` | Applies decimal-prefix parsing independently to all three malformed fields. |
| `batch235-bdf-size-005` | `SIZE +12 75 75` | Confirms `bdf_atoul_` does not consume a leading plus sign. |
| `batch235-bdf-size-006` | `SIZE -12 75 75` | Confirms `bdf_atoul_` does not consume a leading minus sign. |
| `batch235-bdf-size-007` | `SIZE junk 75 75` | Confirms a no-digit point-size token becomes zero while the face remains openable. |
| `batch235-bdf-size-008` | `SIZE 0 75 75` | Distinguishes an explicitly zero point size from a nonzero parsed strike. |
| `batch235-bdf-size-009` | `SIZE 12 0 0` | Exercises zero X/Y resolutions and the no-resolution `x_ppem=y_ppem` fallback. |
| `batch235-bdf-size-010` | `SIZE 12 75 0` | Exercises a zero Y resolution with a nonzero X resolution. |
| `batch235-bdf-size-011` | `SIZE 12 0 75` | Exercises a zero X resolution with a nonzero Y resolution. |
| `batch235-bdf-size-012` | `SIZE 12 75 99999` | Reaches the public resolution clamp for an oversized Y resolution. |
| `batch235-bdf-size-013` | `SIZE 12 99999 75` | Reaches the public resolution clamp for an oversized X resolution. |
| `batch235-bdf-size-014` | `SIZE 12 99999 99999` | Reaches both resolution clamps in the same accepted BDF header. |
| `batch235-bdf-size-015` | `SIZE 12 75 75junk` | Stops a trailing suffix after the final resolution field. |
| `batch235-bdf-size-016` | `SIZE 2147483648 75 75` | Reaches the point-size clamp for a value above `0x7fff`. |
| `batch235-bdf-size-017` | `SIZE 32768 75 75` | Covers the first exact point-size value above the signed-short limit. |
| `batch235-bdf-size-018` | `SIZE 32767 75 75` | Covers the largest non-clamped point size. |
| `batch235-bdf-size-019` | `SIZE 32767tail 75 75` | Combines the non-clamped boundary with decimal-prefix suffix handling. |
| `batch235-bdf-size-020` | `SIZE 999999999999999999999 75 75` | Reaches `bdf_atoul_` saturation from a very long positive prefix. |
| `batch235-bdf-size-021` | `SIZE -999999999999999 75 75` | Confirms a very long negative-looking token has no unsigned digits and becomes zero. |
| `batch235-bdf-size-022` | `SIZE +999999999999999 75 75` | Confirms a leading plus still yields zero even for a very long numeric tail. |
| `batch235-bdf-size-023` | `SIZE 12 +75 75` | Confirms a leading plus on X resolution yields zero. |
| `batch235-bdf-size-024` | `SIZE 12 -75 75` | Confirms a leading minus on X resolution yields zero. |
| `batch235-bdf-size-025` | `SIZE 12 75 +75` | Confirms a leading plus on Y resolution yields zero. |
| `batch235-bdf-size-026` | `SIZE 12 75 -75` | Confirms a leading minus on Y resolution yields zero. |
| `batch235-bdf-size-027` | `SIZE 12 75 075suffix` | Preserves a zero-padded Y-resolution decimal prefix before its suffix. |
| `batch235-bdf-size-028` | `SIZE 12 00000000000000000000075 75` | Preserves a zero-padded oversized X-resolution prefix and reaches its clamp. |
| `batch235-bdf-size-029` | `SIZE 12 75 00000000000000000000096` | Preserves a zero-padded oversized Y-resolution prefix and reaches its clamp. |
| `batch235-bdf-size-030` | `SIZE 00012 00075 00096` | Covers zero-padded prefixes in all three `SIZE` fields. |

The pinned source review confirms that these are public, input-driven states.
`freetype/src/bdf/bdflib.c:289-339` implements `bdf_atoul_`: it consumes
decimal digits only, returns zero when the token has no leading digit, stops at
the first non-digit, and saturates an overflowing prefix. The `SIZE` parser at
`bdflib.c:1364-1401` applies that helper independently to point size and both
resolutions without rejecting malformed suffixes. The public strike is then
constructed at `freetype/src/bdf/bdfdrivr.c:435-602`: FreeType always exposes
one `FT_Bitmap_Size`, clamps ascent/descent and resolutions, derives a
heuristic width when `AVERAGE_WIDTH` is absent, and computes size and ppem
fields from the parsed values. This is observed permissiveness in the pinned
oracle, not a Rust-defined acceptance rule.

The first focused case was
`freetype.FT_FaceRec.available_sizes_public_fields_match_c@batch235-bdf-size-001`.
Before the fix, the pinned C route returned
`{height:8,width:5,size:768,x_ppem:800,y_ppem:800}`, while Rust exposed no
available-size record (`available_sizes[0] = null`). The cause was twofold:
`parse_bdf_metadata` used strict `parse::<i32>()` for the first `SIZE`
token, and `available_sizes_to_ffi` had no BDF branch. The implementation now
uses the pinned decimal-prefix interpretation and builds the BDF fixed-strike
record in `Font::bdf_bitmap_size`, including the C driver's fallback,
clamping, and resolution formulas.

After the fix, the 30 concrete IDs passed 30/30 through Rust FFI, C ABI, WASM,
and the pinned FreeType oracle.

Coverage MCP run `c8da8ae9-1ea5-483b-8fbe-cd6dd145c990` passed at pushed commit
`c642ba1` and ingested snapshot
`bf6d5567-2043-496e-a5ec-47e63bf8e91c` against explicit baseline
`7405fcdf-db54-48a4-877f-eca87142b938`. The explicit incremental review is
supported, but its scope is `selected_subset` with `complete=false` and its
merge is `exact=false` using `conservative_max_fallback`. The canonical
additive union reports +13 covered functions, +2,913 covered regions, and
+0 covered lines/+0 covered branches. The bounded replacement diff reports
37 newly covered line identities, 0 regressions, and 47,081 baseline
observations not observed by the selected run. Test attribution was
unavailable. These are reachability/additive observations only, not a
full-denominator percentage.

### Batch 236: malformed public outline tags reach renderer error returns

This batch uses 30 distinct public `FT_Glyph_To_Bitmap` inputs whose outline
counts and contour endpoints remain valid while the public tag array is
malformed. The C, Rust FFI, C ABI, and WASM routes all extract an outline glyph,
apply the same tag-only mutation, request the selected renderer, and compare
the exact `FT_Err_Invalid_Outline` result and unchanged caller-handle class.
The cases cover normal gray, overlap-normal, and SDF renderers, three distinct
tag-sequence failures, several outline shapes or sizes, and both `destroy`
values.

| Concrete ID suffix | Renderer/input variation | Why expand this input |
|---|---|---|
| `batch236-normal-first-cubic-g36-destroy-false` | normal; DejaVu glyph 36; 16 ppem; first tag cubic; destroy=false | Reaches the normal renderer with a structurally valid outline whose first tag is an invalid cubic start. |
| `batch236-normal-first-cubic-g36-destroy-true` | normal; glyph 36; 20 ppem; first tag cubic; destroy=true | Checks that the same renderer error preserves the caller glyph under delayed destruction. |
| `batch236-normal-first-cubic-g43-destroy-false` | normal; glyph 43; 16 ppem; first tag cubic; destroy=false | Separates the first-cubic guard from one glyph's outline geometry. |
| `batch236-normal-first-cubic-g43-destroy-true` | normal; glyph 43; 20 ppem; first tag cubic; destroy=true | Pairs the alternate glyph with the ownership-preservation observation. |
| `batch236-normal-first-cubic-g74-destroy-false` | normal; glyph 74; 18 ppem; first tag cubic; destroy=false | Exercises the same tag guard with a larger outline and raster box. |
| `batch236-normal-first-cubic-g74-destroy-true` | normal; glyph 74; 22 ppem; first tag cubic; destroy=true | Checks destroy=true after the larger-glyph renderer failure. |
| `batch236-normal-bad-conic-g36-destroy-false` | normal; glyph 36; 16 ppem; conic followed by cubic; destroy=false | Keeps `FT_Outline_Check`-visible structure valid while reaching the pinned bad-conic decomposition error. |
| `batch236-normal-bad-conic-g36-destroy-true` | normal; glyph 36; 20 ppem; conic/cubic sequence; destroy=true | Distinguishes tag-sequence rejection from the C ownership policy. |
| `batch236-normal-bad-conic-g43-destroy-false` | normal; glyph 43; 16 ppem; conic/cubic sequence; destroy=false | Confirms the bad-conic rule is independent of one font glyph. |
| `batch236-normal-bad-conic-g43-destroy-true` | normal; glyph 43; 20 ppem; conic/cubic sequence; destroy=true | Retains the paired error and handle-preservation witness. |
| `batch236-normal-unpaired-cubic-g36-destroy-false` | normal; glyph 36; 16 ppem; one cubic control; destroy=false | Reaches the gray renderer's invalid cubic-sequence guard without changing counts or endpoints. |
| `batch236-normal-unpaired-cubic-g36-destroy-true` | normal; glyph 36; 20 ppem; one cubic control; destroy=true | Verifies the same failure before successful replacement. |
| `batch236-normal-unpaired-cubic-g43-destroy-false` | normal; glyph 43; 16 ppem; one cubic control; destroy=false | Provides an independent outline witness for the unpaired-cubic guard. |
| `batch236-normal-unpaired-cubic-g43-destroy-true` | normal; glyph 43; 20 ppem; one cubic control; destroy=true | Completes the ownership pair for the alternate glyph. |
| `batch236-normal-first-cubic-g77-destroy-false` | normal; glyph 77; 18 ppem; first tag cubic; destroy=false | Adds a fifth outline shape so first-cubic reachability is not inferred from four glyphs only. |
| `batch236-normal-first-cubic-g77-destroy-true` | normal; glyph 77; 22 ppem; first tag cubic; destroy=true | Completes the normal-mode first-cubic destroy pair. |
| `batch236-normal-bad-conic-g74-destroy-false` | normal; glyph 74; 18 ppem; conic/cubic sequence; destroy=false | Checks bad-conic rejection after a different normal raster allocation. |
| `batch236-normal-bad-conic-g74-destroy-true` | normal; glyph 74; 22 ppem; conic/cubic sequence; destroy=true | Checks delayed destruction for the larger bad-conic outline. |
| `batch236-normal-unpaired-cubic-g74-destroy-false` | normal; glyph 74; 18 ppem; one cubic control; destroy=false | Exercises the cubic guard with a separate outline and allocation size. |
| `batch236-normal-unpaired-cubic-g74-destroy-true` | normal; glyph 74; 22 ppem; one cubic control; destroy=true | Completes the larger-glyph cubic error-cleanup pair. |
| `batch236-overlap-first-cubic-g6-destroy-false` | overlap-normal; render-coverage glyph 6; 16 ppem; first tag cubic; destroy=false | Selects `render_normal_overlap` through `FT_OUTLINE_OVERLAP` before the malformed-tag failure. |
| `batch236-overlap-first-cubic-g6-destroy-true` | overlap-normal; glyph 6; 20 ppem; first tag cubic; destroy=true | Checks overlap renderer ownership behavior on the same pinned error. |
| `batch236-overlap-bad-conic-g6-destroy-false` | overlap-normal; glyph 6; 16 ppem; conic/cubic sequence; destroy=false | Confirms overlap oversampling delegates to the same decomposition guard. |
| `batch236-overlap-bad-conic-g6-destroy-true` | overlap-normal; glyph 6; 20 ppem; conic/cubic sequence; destroy=true | Checks the bad-conic error after the overlap branch has been selected. |
| `batch236-sdf-first-cubic-g7-destroy-false` | SDF; render-coverage glyph 7; 16 ppem; first tag cubic; destroy=false | Reaches the SDF subdivision renderer's invalid-outline return. |
| `batch236-sdf-first-cubic-g7-destroy-true` | SDF; glyph 7; 20 ppem; first tag cubic; destroy=true | Checks SDF caller-handle preservation under destroy=true. |
| `batch236-sdf-bad-conic-g7-destroy-false` | SDF; glyph 7; 16 ppem; conic/cubic sequence; destroy=false | Verifies SDF uses the pinned bad-conic decomposition semantics. |
| `batch236-sdf-bad-conic-g7-destroy-true` | SDF; glyph 7; 20 ppem; conic/cubic sequence; destroy=true | Pairs SDF bad-conic rejection with the alternate requested size. |
| `batch236-sdf-unpaired-cubic-g7-destroy-false` | SDF; glyph 7; 16 ppem; one cubic control; destroy=false | Reaches the SDF unpaired-cubic guard instead of silently accepting the public tags. |
| `batch236-sdf-unpaired-cubic-g7-destroy-true` | SDF; glyph 7; 20 ppem; one cubic control; destroy=true | Completes the SDF malformed-cubic ownership matrix. |

The pinned source review shows that these malformed records are deliberately
accepted by the first validation stage, not fabricated through a private
runtime-only path. `freetype/src/base/ftoutln.c:351-393` checks point and
contour counts and contour endpoints but contains no tag-array validation.
`freetype/src/smooth/ftgrays.c:1480-1685` then rejects a first cubic, an
invalid conic sequence, and an unpaired cubic while decomposing the outline.
`freetype/src/base/ftglyph.c:771-869` frees the replacement bitmap only on
failure and changes the caller handle only after successful rendering. This is
therefore pinned FreeType behavior, not an assumption that malformed tags are
valid glyph data. The Rust runtime already matched the oracle; the code change
adds only parity-route test support for applying the public tag mutations to
the C ABI and WASM facades.

Focused parity passed all 30 concrete IDs across Rust FFI, C ABI, WASM, and the
pinned oracle. Coverage MCP run `d9f750ef-3e17-44d8-bd59-903f788d79ef` passed
at pushed commit `7839228` and ingested current snapshot
`4d6a8135-6c6d-4def-a888-ff361e24f15b`. The managed run used the explicit
post-probe base `72664c6f-d547-4873-bfab-0b942349c88e` and the repeatable
argument form `--migration-coverage-case-ids` with ten comma-separated ID
chunks; it did not execute the full case matrix.

Because that post-probe base is itself a selected snapshot, the stored current
snapshot was also reviewed against the fixed full baseline
`7405fcdf-db54-48a4-877f-eca87142b938`. MCP reports the comparison as a
measured `selected_subset` with `complete=false`, `diff.claim_status=limited`,
and `merge.exact=false`; its additive union reports +1 covered function and
the targeted source review marks `src/render.rs:449`, `:530`, and `:605`
green. The normal, overlap-normal, and SDF success bodies are red in that
source view only because the selected snapshot contains error cases; they are
not a full-denominator regression. Test attribution was unavailable. These
are reachability observations only, not a replacement full-denominator
percentage or strict-100% claim.

The maintained PS-hinting post-error batches are classified as `real-parity`
by the route audit because their concrete variants execute the maintained
`ftdriver.hinting_engine_property` Rust FFI, C ABI, and WASM paths. This covers
the CFF Private, ordinary post-error, failure, global-subroutine, and Type 1
mode batches and keeps those existing public inputs in the Coverage MCP
public-case range; they are not generic fallback or unit-test-only coverage.

### Batch 237: CFF2 `random` acceptance and Type2 arithmetic underflow

The next CFF gaps were reviewed against the pinned FreeType 2.14.3 source
before adding input. This batch deliberately combines one oracle-permitted
valid program with two public glyph-load error witnesses; the malformed bytes
are allowed because the user-facing contract is the error returned by
`FT_Load_Glyph`, not successful rendering of every font byte sequence.

| Concrete ID family | Public input and expansion reason | Pinned FreeType behavior | Rust target |
|---|---|---|---|
| `b237-cff2-random-08-default-001` through `b237-cff2-random-12-target-mono-010` | `input/fonts/cff2/pure-cff2-random.otf`, glyph 1, sizes 8/12, five public load modes. Each stable ID names the size and mode because the same valid CFF2 glyph must remain accepted through each public dispatch combination. | `freetype/src/psaux/psintrp.c:2241-2258` executes `random` for CFF2; `freetype/src/cff/cffload.c:2075-2132` leaves the CFF2 subfont state zero-initialized because the Private-dictionary seed initialization is CFF1-only. The glyph's `random`/`drop` program is accepted, not rejected as malformed. | `src/tt/cff.rs:1793-1799` |
| `b237-cff1-mul-underflow-08-default-011` through `b237-cff1-mul-underflow-12-target-mono-020` | `input/fonts/cff/pure-cff-type2-arithmetic-underflow.otf`, glyph 1, sizes 8/12, five public load modes. The glyph invokes `mul` with fewer than two operands; every ID records the exact public mode that reaches the same interpreter guard. | `freetype/src/psaux/psintrp.c:2260-2274` calls `cf2_stack_popFixed`; `freetype/src/psaux/psstack.c:160-178` records `Stack_Underflow`, and `freetype/src/psaux/psft.c:433-435` exposes `Invalid_File_Format` at glyph load. FreeType opens the face, then intentionally rejects this glyph program. | `src/tt/cff.rs:1771-1772` |
| `b237-cff1-eq-underflow-08-default-021` through `b237-cff1-eq-underflow-12-target-mono-030` | The same maintained CFF1 face, glyph 2, sizes 8/12, and five modes. The separate glyph and stable ID family isolate `eq` from `mul` while preserving the same public error comparison. | `freetype/src/psaux/psintrp.c:1636-1649` pops the two fixed operands for `eq`; the same `Stack_Underflow` and `Invalid_File_Format` boundaries apply. This is an oracle-matched malformed-input error, not a request to make FreeType accept it. | `src/tt/cff.rs:1782-1783` |

The exact 30 IDs, individual reasons, and source references are maintained in
`tests/fixtures/inputs/public-api/freetype.FT_Load_Glyph.json`. Focused parity
passed 30/30 across Rust, C ABI, WASM, and the pinned oracle. The implementation
fix gives CFF2 its own zero-seeded `Cell<u32>` random state and shares the
accepted Type2 `random` path with CFF1; the previous CFF2-only unsupported
error was a behavioral mismatch. The two arithmetic error families confirm
the existing Rust underflow guards against the public C error mapping.

Coverage MCP run `c994085a-9054-48b1-a3e7-83a138b187c9`, snapshot
`ca524035-e4df-4ae3-a061-8d432e816437`, used pushed commit `9968720` and the
explicit baseline `e7f7dd3e-86ec-445d-a5e7-e4b1ac66ee6e`. It passed with ten
repeatable `--migration-coverage-case-ids` arguments, each carrying three
comma-separated runtime IDs. The incremental union reports 110 newly covered
line identities, 571 newly covered regions, +3 covered branches, and no new
functions. The selected-subset review is `complete=false`,
`diff.claim_status=limited`, and `merge.exact=false`; unselected baseline
observations are `not_observed`, not regressions. The MCP source projection
retained the older `f0b1ce` source metadata for line text, so the current
target lines above were verified against the ingested LLVM report and the
pushed `9968720` file. This is additive reachability evidence, not a new
full-denominator percentage or strict-100% claim.

### Batch 238: WASM PostScript null-pointer validation

The remaining red WASM regions in the public-range review are the two raw
pointer guards in `fontdone-wasm/src/implementation.rs`: the null
`file_base` return at lines 1974-1978 and the null `out` return at lines
1980-1984. The `size_error == FT_Err_Ok` and `out.load_error == FT_Err_Ok`
arms later in the same wrapper are already line-covered; they are not the
target of this batch.

The maintained public parity input expands each guard into fifteen stable
variants: `c82-ps-null-file-001` through `-015` and
`c83-ps-null-output-001` through `-015`. The variants use valid PostScript
module selectors 5, 6, and 7, distinct glyph/load/size/property controls,
null and non-null property-string pointers, and (for the output family)
zero- or one-byte file sizes. These controls are deliberately sent to the
WASM entry point but must not be consumed after the guard; the exact-error
comparison therefore checks the status, probe label, and pointer classes.

The pinned FreeType review found no original public API equivalent to
`fontdone_wasm_ps_hinting_engine_open`; `freetype/include/freetype/ftdriver.h`
documents the related `hinting-engine` property only. The offline oracle
therefore intentionally emits `FT_Err_Invalid_Argument` in
`scripts/gen_unified_oracle.c:38095-38114` rather than passing an invalid raw
pointer to FreeType C. This is a wrapper-level safety contract, not a claim
that the original FreeType property API accepts a null file or output pointer.
The route audit classifies these concrete rows as `real-null-validation`, and
both focused parity families pass 15/15 across Rust FFI, C ABI, WASM, and the
pinned oracle.

Coverage MCP run `c6a5c18f-4d86-4a3b-af7c-1767624a160c` passed at pushed
commit `1fe4541` and ingested snapshot
`fb1c4d69-d3e8-43a4-99e4-247baf5fe4d1` against explicit baseline
`25dd6475-f59f-4c2e-a785-5b7b27d88fc0`. It used the argument-based
`--migration-coverage-case-ids` form ten times, with three comma-separated
runtime IDs per argument, and did not execute the full matrix. The incremental
review reports two newly covered line identities and target regions at
`fontdone-wasm/src/implementation.rs:1975` and `:1981`. Its scope is
`selected_subset`, so the replacement diff is `claim_status=limited` and
unobserved baseline hits are not regressions; this is additive reachability
evidence, not a full-denominator percentage or strict-100% claim. The MCP
measurement metadata currently retains the older source commit
`f0b1ce7522edcd151a699923b9eae0df6dbca0ef`; the run provenance records the
pushed commit above.

### Batch 239: BDF permissive empty lines and CFF subroutine-bias boundaries

This batch followed the source-review rule in section 4.2: each input was
first checked against pinned FreeType 2.14.3, then retained only after the
public parity route agreed. The BDF witness is malformed text accepted by the
original parser; the CFF witnesses are structurally valid CFF1 fonts with
unusual but legal global-subroutine counts.

| Concrete ID family | Why expand this input | Pinned FreeType review | Result and target |
|---|---|---|---|
| `ftbdf.FT_Get_BDF_Property.success_bdf_string_integer_cardinal_properties@batch239-bdf-empty-line-001` | Put an empty line inside `STARTPROPERTIES` so `parse_bdf_property_line` receives a line whose trimmed property name is empty. This is the public way to test the `None` path rather than calling the private parser directly. | `freetype/src/bdf/bdflib.c:544-604` classifies the empty line as an atom; `bdf_parse_properties_` (`:1137-1188`) and `bdf_add_property_` (`:600-745`) retain it without rejecting the face. The fixed public property query still succeeds. | Focused parity passed 1/1. Coverage MCP newly covered `src/font.rs:1130` and `src/font.rs:1253` (2 lines/regions), with no branch or function delta. |
| `b239-cff-bias-middle-08-default-001` through `b239-cff-bias-middle-16-target-mono-015` | Use `input/fonts/cff/pure-cff-subroutine-bias-middle.otf` with exactly 1,240 global subroutines and operand `-1,131`, then exercise three sizes and five existing public load modes. The biased index is zero only when the middle threshold is selected. | `freetype/src/psaux/cffdecode.c:407-417` selects bias 1,131 for counts from 1,240 through 33,899; `cffdecode.c:2182-2196` applies it and accepts the zero-returning subroutine. | Focused parity passed 15/15. Coverage MCP marks `src/tt/cff.rs:1822-1823` newly covered. |
| `b239-cff-bias-high-08-default-016` through `b239-cff-bias-high-16-target-mono-030` | Use `input/fonts/cff/pure-cff-subroutine-bias-high.otf` with exactly 33,900 global subroutines and operand `-32,768`, across the same public size/load matrix. The biased index is zero only when the high threshold is selected. | `freetype/src/psaux/cffdecode.c:407-417` selects bias 32,768 at 33,900 or more globals; the same `callgsubr` path accepts the zero-returning subroutine. | Focused parity passed 15/15. Coverage MCP marks `src/tt/cff.rs:1825` newly covered. |

The CFF batch is maintained as 30 concrete public IDs in
`tests/fixtures/inputs/public-api/freetype.FT_Load_Glyph.json`, not as a
direct unit call to `cff_subroutine_bias`. The generator records both exact
thresholds in `scripts/font_generation/build_cff_fixtures.py`; rerunning it
recreates the tracked fonts byte-for-byte. No Rust behavior fix was needed:
the existing `src/tt/cff.rs:1819-1827` implementation agrees with the pinned
C oracle at both boundaries.

Coverage MCP run `80f84a64-577c-4899-a3bf-0dd7c5a472f0` passed and ingested
snapshot `f3872557-6256-4892-a1f9-ec17b4e3ffef` against the source-matched
full baseline `df2e52bb-a159-44d0-9e83-88cb5c9ea49a` (commit `c4ce368`). It
used eight repeatable `--migration-coverage-case-ids` arguments, with four
comma-separated IDs per value where possible; this works within Coverage
MCP's 512-byte per-argument limit while executing all 30 IDs in one managed
run. The additive union reports +3 covered branches, +3 covered regions, and
three newly covered line identities; its canonical metric deltas report
`covered_lines_delta=0` because the selected report uses conservative summary
fallback for detailed line merging. The selected-subset scope is
`complete=false`, `merge.exact=false`, and the replacement diff is
`claim_status=limited` with unobserved baseline hits treated as
`not_observed`, not regressions. This is additive reachability evidence, not
a replacement full-denominator percentage or strict-100% claim.

The BDF witness was measured separately in Coverage MCP run
`0f0b6c6c-c180-44c7-ae5e-7e3432f7239c`, snapshot
`039c22f3-d62f-4115-a9ad-c44881742bf4`, with the same explicit baseline. Its
additive union reports two newly covered lines/regions at `src/font.rs:1130`
and `:1253`; branch and function deltas are zero. The run passed and its
selected-subset comparison is likewise limited, so neither result changes
the full-suite denominator claim.

Two tempting expansions were rejected after the same pinned-source review.
The `PIXEL_SIZE` Atom and Cardinal arms in `src/font.rs:1318` cannot be
constructed through public BDF text because FreeType's built-in property
table declares `PIXEL_SIZE` as `BDF_INTEGER` (`freetype/src/bdf/bdflib.c:92`);
Cardinal remains represented by properties such as `RESOLUTION_X`. The `_`
arm in `src/tt/cff.rs:1876` is also not reachable through a public Type2 byte
stream: bytes 0--31 are operators and every byte from 32 through 255 is
handled by a preceding number arm. These are documented defensive branches,
not fabricated parity inputs.

### Batch 240: rendered empty-outline SBit descriptors

This batch followed the same source-review rule, but targeted the public
`FTC_SBitCache_Lookup` route with an empty-outline glyph rather than a direct
cache or renderer unit call. The maintained input is
`input/fonts/autohint/latin-empty-standard.ttf`; glyphs 1 (`space`) and 2
(`latin_o_empty`) have empty outlines. Thirty concrete IDs cover those glyphs
at 8x8, 16x12, and 24x16 sizes with `FT_LOAD_DEFAULT`, `FT_LOAD_NO_HINTING`,
`FT_LOAD_TARGET_LIGHT`, `FT_LOAD_TARGET_MONO`, and `FT_LOAD_RENDER`. Each ID
uses one lookup and requests a newly allocated node, matching the oracle's
public cache contract.

| Concrete ID family | Why expand this input | Pinned FreeType review | Result and target |
|---|---|---|---|
| `b240-sbit-empty-outline-g1-*` and `b240-sbit-empty-outline-g2-*` | Exercise successful SBit conversion when the rendered glyph has no outline and therefore has a null bitmap buffer. The target is the successful empty descriptor, including its pixel mode and cache-node state. | `freetype/src/cache/ftcbasic.c:141-149` always adds `FT_LOAD_RENDER` to the family load flags. `freetype/src/cache/ftcsbits.c:90-98` treats only allocation failure as a lookup error, while `:165-189` copies the rendered bitmap descriptor and buffer into the SBit record. A direct pinned-oracle MONO lookup produces a 1x1 bitmap with pitch 2 and bytes `0000`. | Focused parity passed 30/30 on Rust FFI, the C ABI, WASM, and the pinned oracle. Coverage MCP marks `fontdone-c-abi/src/implementation.rs:2379` newly covered. |

The first focused attempt exposed a test-contract mismatch rather than a
runtime mismatch: the oracle generator treats a new logical scenario as a
non-null node request, so the variants were corrected to use `anode_output:
"nonnull"` and no repeat lookup. The next attempt exposed the behavioral
divergence: both Rust cache implementations loaded the glyph without the
cache family's render flag, then rendered it in normal grayscale mode. For a
MONO target that produced an empty buffer instead of the pinned 1x1 MONO
descriptor. The fix ORs `FT_LOAD_RENDER` into the load flags in
`src/ffi/handles.rs` and `fontdone-c-abi/src/implementation.rs`, preserving
the caller's target-mode bits. The C ABI empty-render predicate also now
checks the adapter's C-visible `FT_PIXEL_MODE_GRAY` descriptor instead of
treating pixel mode zero as the successful case.

Coverage MCP run `db67f825-7853-446c-9bf4-567dfee73464` passed and ingested
snapshot `11db54cd-e0b9-4d14-9b65-e185f677bafe` against explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`. It used eight repeatable
`--migration-coverage-case-ids` arguments to select all 30 concrete IDs and
ran three 10-case shards with no pending cases. The explicit compact review
reports the C ABI target region as newly covered. Its selected-subset scope is
`complete=false`, the merge is `exact=false`, and the replacement diff is
`claim_status=limited`; this is additive reachability evidence, not a full
denominator or strict-100% claim. As in earlier batches, the stored measurement
metadata retains source commit
`f0b1ce7522edcd151a699923b9eae0df6dbca0ef`, while command provenance records
the pushed implementation commit `9608b1f`.

### Batch 241: BDF strike-size clamp boundaries

This batch followed the source-first input expansion rule for the remaining
source-reachable BDF strike metadata branches in `src/font.rs:4953-5058`.
Thirty maintained BDF faces under
`tests/fixtures/input/fonts/bdf/malformed-strike-size/` exercise the public
`freetype.inspect_available_sizes` route. The faces keep a valid glyph and
vary only the public BDF metadata that `BDF_Face_Init` consumes: exact and
near-boundary `AVERAGE_WIDTH` values, positive and negative clamps, decimal
prefixes, no-value and plus-sign forms, and the corresponding
`POINT_SIZE` boundaries and malformed forms. Three final variants combine
both fields so the two clamps are observed together.

| Concrete ID family | Why expand this input | Pinned FreeType review | Result and target |
|---|---|---|---|
| `freetype.FT_FaceRec.available_sizes_bdf_strike_size_clamp_batch241@batch241-bdf-strike-01` through `-015` | Reach the `AVERAGE_WIDTH` strike-width boundary and its signed, prefix, no-value, and non-clamping neighbors through a public `FT_FaceRec.available_sizes` observation. | `freetype/src/bdf/bdflib.c:289-339` makes integer properties decimal-prefix and minus-sign tolerant. `freetype/src/bdf/bdfdrivr.c:472-493` clamps values outside `0x7fff * 10 - 5` (327665) to `0x7fff`; otherwise it rounds the decipoint value and takes its absolute value. | 15/15 exact parity cases passed. The selected source projection reaches the clamp body at `src/font.rs:5010`; the incremental union reports that line as newly covered. |
| `...@batch241-bdf-strike-16` through `-027` | Reach the `POINT_SIZE` conversion boundary, both signs, decimal-prefix handling, and the zero/no-value/plus-sign outcomes while preserving a loadable face. | `freetype/src/bdf/bdfdrivr.c:495-520` converts point size with `FT_MulDiv` unless the absolute raw value exceeds `0x504C2` (328898), in which case it stores `0x7fff`. The property parser at `bdflib.c:608-720` supplies the raw signed decimal value. | 12/12 exact parity cases passed. The selected source projection reaches the oversized `src/font.rs:5019` arm with no C/Rust/WASM mismatch. |
| `...@batch241-bdf-strike-28` through `-030` | Confirm both clamp rules and the exact threshold values in one public strike record, including a signed mixed-boundary case. | The same `BDF_Face_Init` width and point-size rules are applied independently before PPEM derivation in `bdfdrivr.c:521-602`. | 3/3 exact parity cases passed; no additional Rust behavior fix was needed. |

Coverage MCP run `dac789ec-cd54-477c-ba85-f457cf67ce20` passed and ingested
snapshot `5e781671-3317-4358-b621-b4bd74312745` against explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`. It used ten repeatable
`--migration-coverage-case-ids` arguments, three comma-separated concrete IDs
per argument, and ran three 10-case shards. The exact incremental target
review reports `src/font.rs:5010` newly covered; the selected source review
reaches both the average-width and point-size clamp arms, and all 30 cases
agree across Rust, the C ABI, WASM, and the pinned oracle. The selected scope
is `complete=false`, the replacement diff is limited, and the merge is not
exact; unselected baseline observations are `not_observed`, not regressions.
This is additive reachability evidence, not a full-denominator percentage or
a strict-100% claim.

### Batch 242: absent-OS/2 public queries reach the WASM table error

This batch added exactly 30 explicit public `sfnt.get_os2_unicode_ranges`
variants under the stable case
`tttables.FT_SFNT_OS2.os2_absent_query_batch242`. The inputs are loadable
public font assets: three SFNT faces without an OS/2 table, accepted BDF and
PCF inputs with non-SFNT headers, and accepted WinFNT inputs. Every variant
uses `face_index: 0` and `preserve_initial_size: true` so the observation is
isolated from unrelated size-selection behavior. The public helper converts
the ABI error into its normal `table_present: false` result, so these are
`expect_error: false` parity cases even though they execute an internal WASM
error return.

| Concrete ID family | Why expand this input | Pinned FreeType review | Result and target |
|---|---|---|---|
| `batch242-os2-absent-01` through `-003` | Query explicit SFNT faces whose parsed OS/2 table is absent. | `freetype/src/sfnt/sfdriver.c:131-133` returns NULL when `ttface->os2.version == 0xFFFFU`; `freetype/src/base/ftobjs.c:4357-4372` returns that NULL through `FT_Get_Sfnt_Table`. | The public observation reports `table_present=false` and reaches the WASM NULL-table error arm. |
| `batch242-os2-absent-04` through `-024` | Keep loadable BDF/PCF inputs in the public format matrix, including the accepted malformed-property and encoding forms already maintained by the oracle fixtures. | The same `FT_Get_Sfnt_Table` precondition requires `FT_IS_SFNT(face)`, so these faces return NULL before an SFNT service lookup. The pinned oracle was checked for each candidate; inputs that failed earlier face-open or cmap behavior were not retained. | All 21 cases preserve the public no-table result without a Rust/C/WASM mismatch. |
| `batch242-os2-absent-25` through `-030` | Exercise loadable WinFNT faces through the same public query rather than treating a non-SFNT format as an untested theoretical path. | `ftobjs.c:4357-4372` makes the non-SFNT NULL result part of the public table-query contract. | All six cases reach the same WASM error mapping and agree with the pinned oracle. |

The candidate review was deliberately source-first. The initial default-size
attempts exposed unrelated invalid-pixel-size and stream/cmap divergences, so
the final rows retain only loadable public assets, preserve the face's initial
size, and use neutral unmapped probes where a candidate's cmap behavior would
otherwise obscure the OS/2 result. This records the actual pinned behavior; it
does not weaken an expected result or hide a runtime mismatch. No Rust
implementation change was needed because the existing
`fontdone_wasm_get_sfnt_os2` wrapper already returns
`FT_Err_Invalid_Table` and the public operation maps that error to
`table_present=false`.

Focused parity passed all 30 rows across Rust, the C ABI, WASM, and the pinned
oracle. Public-input validation also passed. Managed Coverage MCP run
`fee7951c-21b1-4f6b-98e2-cdc988121d2e` used ten repeated
`--migration-coverage-case-ids` arguments with three comma-separated concrete
IDs per argument, plus the explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`; it passed and ingested snapshot
`d3e145af-ff0f-4cda-bf1c-2d0abfa86759`. The bounded source review marks
`fontdone-wasm/src/implementation.rs:9164` covered, while the adjacent
predicate at `:9163` still has one unobserved branch and the earlier null
output/invalid-handle returns remain pre-validation paths. The incremental
review reports no regression, but its measurement scope is
`selected_subset` (`complete=false`) and its replacement diff is
`claim_status=limited`; unselected baseline observations are
`not_observed`, not regressions. This is additive reachability evidence, not a
full-denominator percentage or a strict-100% claim.

### Batch 243: native gvar composite runtime-error control

This batch adds five public `FT_Set_Var_Design_Coordinates` cases using the
maintained `variable-native-gvar-runtime-composite-error.ttf` fixture. The
composite record remains structurally valid; a compact runtime-short `gvar`
tuple is applied only after composite loading, so the cases distinguish the
native composite parser from the later gvar delta application path. The five
rows vary active design coordinates, ppem, and public load flags while keeping
the same source-level reason for expansion.

| Concrete ID family | Why expand this input | Pinned FreeType review | Result and target |
|---|---|---|---|
| `batch243-native-gvar-error-001` through `-005` | Exercise a runtime gvar error after a valid composite has been accepted, including neutral, no-autohint, vertical, pedantic, and light-target load routes. | `freetype/src/truetype/ttgload.c` and `ttgxvar.c` apply composite loading before gvar deltas; the fixture builder documents the generated table bytes and the runtime-short tuple. | All five focused cases pass across Rust, the C ABI, WASM, and the pinned oracle. The trace shows the candidate error is already reported by the runtime gvar scaler path rather than the intended `src/tables.rs:165` loader guard, so no unrelated implementation change is justified. |

The split all-lane coverage recipe now explicitly merges each backend/shard's
raw LLVM profile before exporting `unified-runtime-all-lanes.json`; without
that merge, a focused MCP artifact could contain only the conventional
single-profile subset and under-report the selected source execution.

### Batch 244: zero-glyph glyph-map mutation validation

The existing public `ftdriver.glyph_to_script_map_effect` zero-glyph CFF case
is the source-backed witness for the remaining WASM post-validation arm. The
maintained face opens successfully, `FT_Property_Get` returns `FT_Err_Ok`, and
the public route derives glyph index zero from a character lookup while the
face reports zero glyphs. FreeType leaves the map untouched and reports
`FT_Err_Invalid_Glyph_Index` only through the explicit load/render missing-glyph
precondition.

The parity output now records a separate
`mutation_validation_error`. It is a checked test-support safety observation:
Rust, C-ABI, and WASM all report `FT_Err_Invalid_Glyph_Index` for the attempted
out-of-range map write, while `property_error` remains the pinned public
`FT_Property_Get` result. The WASM adapter uses the helper's detailed status
without converting that safety result into a public property failure. The
existing zero-glyph variant provides 12 concrete mutation-by-ppem rows, so
five duplicate cases would add no new reachability information.

The source evidence is `freetype/src/base/ftobjs.c:5301-5382`,
`freetype/src/autofit/afmodule.c:285-305`,
`fontdone-wasm/src/implementation.rs:6275-6348`,
`src/ffi/handles.rs:11490-11500`, and
`scripts/gen_unified_oracle.c:33458-33500`. Focused parity passes the valid,
zero-glyph, and missing-property variants across all four endpoints; the
managed Coverage MCP run `22acbe61-59ec-465f-9d26-8d20cb630f9d` passed from
committed revision `dfaef9e` and ingested snapshot
`361fff20-9a7e-45aa-96b7-666cf414ab96` against explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`. The exact LLVM projection records 36
helper calls and 12 true/12 false executions of the nested `error ==
FT_Err_Ok` decision, including the `FT_Err_Invalid_Glyph_Index` arm. The
incremental union reports +1,140 regions with conservative metric fallback;
its selected-subset replacement diff is `complete=false` and
`claim_status=limited`, so this is additive reachability evidence rather than
a full-denominator or strict-100% claim.

### Batch 246: malformed legacy kern inputs confirm `FT_Get_Kerning` success

This batch adds five public malformed-input variants to
`freetype.FT_Get_Kerning.malformed_legacy_kern_is_tolerated`. Each asset is
openable but contains a damaged or truncated legacy `kern` table:
`malformed-classic-kern.ttf`, its length, offset, and pair-order controls, and
`kerning/kern-truncated.ttf`. The rows use the same public glyph pair and
`FT_KERNING_DEFAULT` mode so the only expanded dimension is the table shape.

The source-first question was whether one of these inputs makes the pinned C
`FT_Get_Kerning` return an error after face and output validation, thereby
reaching the false arm of the WASM wrapper's `if err == FT_Err_Ok`. The pinned
implementation answers no: `freetype/src/base/ftobjs.c:3603-3675` delegates to
the driver's callback, while the built-in TrueType, CFF, Type 1, and PFR
callbacks all return `FT_Err_Ok`; `freetype/src/sfnt/ttkern.c:185-260` bounds
malformed pair data and returns a value rather than an error. Direct oracle
probes confirmed all five files open and return a zero vector. The Rust probe
returned the same status and vector for all three kerning modes, so inventing a
post-validation Rust error would be a parity regression.

Focused parity passed 5/5 across Rust, the C ABI, WASM, and the pinned oracle.
Coverage MCP run `f766b5a2-4680-44fa-8174-81ae2cf39370` passed and ingested
snapshot `95c66b9c-cb10-4fc5-8a9f-af5da81b912d` against explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`. The additive union reports 927 newly
covered LLVM region identities, with no covered-branch, covered-function, or
covered-line summary-count increase; the selected-subset scope is
`complete=false`, the merge is conservative, and unselected baseline hits are
`not_observed`. The WASM `FT_Get_Kerning` post-call false arm remains
source-unreachable under the pinned built-in driver contract. No runtime fix is
justified; the five rows are retained as a regression guard for FreeType's
permissive malformed-input behavior.

### Batch 256: null BDF property name reaches the WASM pre-validation branch

The remaining BDF WASM gap is the `prop_name.is_null` branch in
`fontdone-wasm/src/implementation.rs`. The smallest public witness is an
actual BDF face with a null `prop_name` pointer; no malformed font or private
helper is needed.

| Concrete ID | Public input | Why expand this input | Pinned FreeType review |
|---|---|---|---|
| `ftbdf.FT_Get_BDF_Property.error_null_property_name` | `input/fonts/bdf/properties-atoms-integers-cardinals.bdf`, face index 0, null `prop_name`, sentinel `BDF_PropertyRec` | Reach the WASM pointer-to-`Option<&str>` conversion branch and verify the null name is forwarded as `None` across all public parity endpoints. | `freetype/src/base/ftbdf.c:62-86` initializes `type` to `BDF_PROPERTY_TYPE_NONE` before dispatch; `bdf/bdflib.c:1763-1772` returns no property for a null name and `sfnt/ttbdf.c:158-180` explicitly rejects a null name with `Invalid_Argument`. FreeType therefore allows this public error probe and preserves the caller's union fields. |

The case is recorded in
`tests/fixtures/inputs/public-api/ftbdf.FT_Get_BDF_Property.json` with the
same BDF asset used by the existing null-face and missing-property rows. It is
an input expansion only until focused parity demonstrates a first divergence;
the Rust implementation should be changed only if the pinned oracle disagrees
with one of the Rust FFI, C ABI, or WASM ABI results.

### Batch 257: negative face-index probes expose `FT_Select_Size` validation

This batch adds five distinct variants to
`freetype.FT_Select_Size.error_no_fixed_sizes_or_null_face`. Each opens a
maintained fixed-strike font with the public `face_index: -1` metadata probe,
preserves the initial size state, and requests strike index 0:
`negative-probe-eblc-gray`, `negative-probe-cblc-matrix`,
`negative-probe-cblc-no-os2`, `negative-probe-cblc-vmtx`, and
`negative-probe-strike-metrics`.

The expansion is source-backed rather than a private-handle probe. In pinned
FreeType, `freetype/src/truetype/ttobjs.c:707-717` and
`freetype/src/cff/cffobjs.c:535-546` return early for a negative face index,
before the full face load populates fixed-size metadata. The public
`freetype/src/base/ftobjs.c:3387-3388` guard therefore sees no
`FT_FACE_FLAG_FIXED_SIZES` and returns `Invalid_Face_Handle` before selecting a
strike. Direct C oracle probes returned error code 35 for all five assets.

Focused parity then found the first implementation divergence: the C ABI and
WASM routes matched the oracle, but the direct Rust FFI route returned `OK` for
all five probes. The shared Rust `FT_Select_Size` implementation already tracks
metadata-only faces as `probe_only`, but did not apply the same pre-callback
guard. It now returns `FT_Err_Invalid_Face_Handle` before parsing the strike
index in `src/ffi/handles.rs`, matching the pinned public validation order.
Focused parity passed 5/5 after the fix.

Coverage MCP run `579cb2e8-06ca-4276-a3fa-94fb39fc9ba7` passed and ingested
snapshot `8574b273-bb48-4444-8404-ad7b6c46329e` against explicit baseline
`df2e52bb-a159-44d0-9e83-88cb5c9ea49a`. The measured incremental union reports
380 newly covered line identities and 990 newly covered LLVM regions; its
selected-subset scope is `complete=false`, with conservative merge fallback,
so the result is reachability evidence and not a full-denominator score.
The five inputs remain retained public parity regressions for the discovered
Rust-vs-C validation mismatch.

### Batch 265/266: public outline-support guards and the remaining WASM invariant

The next WASM helper window was reviewed from the canonical full baseline
before adding inputs. The five reachable guards in
`fontdone-wasm/src/implementation.rs` were:

| Concrete public ID family | Guard exercised | Why this input is valid evidence |
|---|---|---|
| `ftglyph.FT_Glyph_To_Bitmap.error_outline_support_guards@batch265-render-failure-*` | render-failure null glyph and empty-outline checks | The probe starts from the public glyph-to-bitmap operation and uses the documented zero/null handle or a real loaded outline; it does not dereference an invented pointer. |
| `...@batch265-stroke-parse-*` | stroke-parse null glyph and empty-tag checks | The same public outline observation reaches the parser preconditions through the ABI support surface. |
| `...@batch265-render-tags-invalid-kind-005` | invalid render-tag kind fallback | The invalid kind is an explicit public byte-valued tag observation, compared with the pinned oracle's fallback result. |
| `...@batch266-render-tags-null-glyph-001` and `...@batch266-record-sync-null-glyph-002` | null render-tags and record-sync ownership guards | A zero handle is the public null-handle contract and is passed through the C, Rust, C-ABI, and WASM routes. |
| `...@batch266-record-sync-empty-contours-003` and `...@batch266-points-sync-empty-points-005` | empty-contour and empty-point guards | DejaVuSans glyph 3 supplies a real public outline with the corresponding empty vector, rather than a private synthetic record. |
| `...@batch266-points-sync-null-glyph-004` | null point-sync ownership guard | The zero handle exercises the same documented invalid-handle path as the pinned C probe. |

The pinned source supports these cases. `freetype/src/base/ftglyph.c:786-803`
validates the glyph output handle and ownership before conversion, while
`freetype/src/smooth/ftgrays.c:1981-1989` treats a null outline as invalid and
an empty outline as a successful no-op before dereferencing outline arrays.
Focused parity passed all five Batch265 and all five Batch266 variants across
the pinned C oracle, direct Rust FFI, thin C ABI, and WASM ABI. The custom
oracle route reads the actual `FT_Glyph` produced by `FT_Load_Glyph` and
`FT_Get_Glyph`, so the support result remains tied to a public input and an
observable FreeType object.

Batch265 Coverage MCP run `a075d833-38e1-4db2-8a5a-15257bf58a7c` ingested
snapshot `83900a87-9b3f-458c-995b-964b1d5c6568`; its bounded source review
added WASM lines 2515, 2521, 2532, 2535, and 2564. Batch266 run
`bbd0ca58-2df6-44b3-9137-85d194269f0c` ingested snapshot
`781005c4-6b8a-413a-a7eb-950cb4ed9aec`; its bounded review added lines 2548,
2574, 2577, 2591, and 2594. Both reviews are selected-subset incremental
measurements (`complete=false`, additive union), so they prove reachability of
the named regions without changing the strict full-denominator claim.

The only red region left in the reviewed helper window is the `FT_UShort`
conversion at `fontdone-wasm/src/implementation.rs:2518`. It is a defensive
private invariant, not a public-input gap: `freetype/include/freetype/ftimage.h`
limits `FT_Outline.n_points` to `FT_OUTLINE_POINTS_MAX`, and
`freetype/src/base/ftgloadr.c:229-240`, `ftoutln.c:327`, and
`truetype/ttgload.c:389` reject point counts above that limit. A public
FreeType `FT_Glyph` therefore cannot expose a Rust point vector longer than
`u16::MAX`; adding an oversized private vector would violate the parity input
boundary. The main public WASM glyph-to-bitmap handle range
`fontdone-wasm/src/implementation.rs:3938-3995` is already green in the
canonical baseline, including its error side, so no runtime change is
justified for that hypothesis.

The next ranked red WASM line, `update_wasm_active_size_metrics` at line 2125,
is likewise source-unreachable from a successful public size request. Pinned
`freetype/src/base/ftobjs.c:3438-3458` rejects `face->size == NULL` before
`FT_Request_Size` and `FT_Set_Pixel_Sizes`/`FT_Set_Char_Size` delegate to it;
normal face construction allocates the initial size. It remains documented as
a defensive invariant, and the campaign advances to the next public
source-backed target rather than fabricating a null-size success case.

### Batch 267: malformed face-open errors reach the WASM SVG capture export

The transform helper's false arms were source-reviewed before this batch. The
outline and SVG glyph transform hooks in pinned
`freetype/src/base/ftglyph.c:403-448` and `209-224` are `void` callbacks;
`FT_Glyph_Transform` only returns an error for a null glyph/class or a class
without a transform hook at `ftglyph.c:691-714`. Those failures return before
the WASM wrapper's `if error == FT_Err_Ok` refresh blocks at lines 3897 and
3915, so no public outline or SVG glyph input can produce their false arms.
The campaign therefore moved to the reachable face-construction arm at
`fontdone-wasm/src/implementation.rs:2810-2813`.

Batch267 adds five variants to
`otsvg.FT_SVG_Document.mcp_invalid_font_open_batch`, using the existing tracked
malformed-font inputs `not-a-font.bin`, the short CFF header, the malformed CFF2
charstrings index, the invalid Type1 first segment, and the invalid PCF version.
Each is passed through the public `fontdone_wasm_svg_renderer_capture` input
boundary with installed hooks. Pinned C returns face-open errors 3, 8, 8, 2,
and 85 respectively before allocating a face, so the expected capture output
is null and the error is exact; the inputs are not private handles or invented
memory states.

The WASM parity adapter previously discarded the export's returned error and
reported the untouched default capture as success. It now propagates a
non-`OK` capture status, matching the direct Rust and C ABI face-open errors.
The oracle batch command also treats an already-emitted malformed-face row as
a successful process row, preserving exact per-case comparison. Focused parity
initially exposed one real mismatch: the all-zero `not-a-font.bin` payload
reached the Rust SFNT parser as `unknown sfVersion` (error 2), while pinned C
returned error 3. The source trace is the final BDF probe: `bdf_readstream_`
skips all bytes below ASCII space, `bdf_load_font` returns
`Invalid_File_Format` when no line was ever found, and `BDF_Face_Init` keeps
that non-unknown error (`freetype/src/bdf/bdflib.c:1472-1684` and
`freetype/src/bdf/bdfdrivr.c:360-392`). Rust now mirrors that bounded
multi-driver outcome for streams of at least the Type42 probe frame whose
bytes are all below ASCII space in `src/font.rs`; ordinary printable unknown
data still follows the existing `Unknown_File_Format` mapping.

Focused parity passed 5/5 after the fix. Coverage MCP incremental run
`69231675-2151-45f8-8eb2-3c5809026550` ingested snapshot
`0523de38-ca33-444a-80b5-54f3698f4daf` against explicit baseline
`a761e764-3db0-4dde-9ea6-4fff6074c589`. Its bounded review reports
`fontdone-wasm/src/implementation.rs:2812` as newly covered and the source
review marks the `Err(error) => return error` arm green. The review is a
selected subset (`complete=false`), so this is additive reachability evidence;
the neighboring success/setup arms remain unclaimed by these intentionally
face-rejecting inputs.

### Full-snapshot parity repair: zero-sized BDF bitmap row

The first source-matched full Coverage MCP run on the current tree failed before
ingestion on the existing cases
`freetype.FT_New_Memory_Face.error_malformed_cff_table@bdf-batch-01`,
`...@bdf-batch-18`, and
`ftsystem.FT_Memory.mcp_cabi_wrapper_edges_batch@c98-cabi-edge-006`. All three
use the maintained input
`tests/fixtures/input/fixtures/assets/bdf/bbx_malformed_field.bdf`, whose glyph
contains `BBX nope 8 0 -2` followed by a `BITMAP` row. This is an input-driven
constructor error, not a private-state or coverage-only case.

The pinned BDF parser uses `bdf_atous_`/`bdf_atos_` for the BBX fields, so the
malformed width becomes zero. In
`freetype/src/bdf/bdflib.c:1081-1120`, the resulting zero bitmap size skips the
bitmap-row callback; the following `00` is therefore parsed by the glyph
record parser as an unknown record and returns `FT_Err_Invalid_File_Format`
(error 3). Direct oracle execution confirmed that exact face-open error. The
Rust constructor parser previously ignored the row and returned success. It
now mirrors the C state transition in `src/font.rs:1798-1921`: nonzero bitmap
rows are consumed as bitmap data, while a row after a zero-sized bitmap returns
`InvalidFileFormat`; comments and metric records remain accepted until
`ENDCHAR`, matching the pinned parser's state machine.

Focused unified parity passed 3/3 for the two public face-open cases and the
C-ABI wrapper case after the fix. No input, expected result, denominator, or
filter was weakened; this repair is required before the next full snapshot can
provide valid coverage evidence.

The follow-up full Coverage MCP run `3a3f3db5-b86c-49bd-8bd3-7efa83f8347d`
passed all nine backend shards (`6,524/6,524` comparisons per shard, with no
failure buckets) and ingested complete snapshot
`a46f05b4-3a54-4d56-a5cc-c49bb087d7aa`. The strict full-snapshot counters are
11,966/13,652 branches (87.650%), 3,759/4,044 functions (92.953%),
64,757/66,821 lines (96.911%), and 89,270/92,848 regions (96.146%). The
remaining ranked gaps are now re-baselined from this valid snapshot; the
selected 50-case BDF incremental run is retained only as additive reachability
evidence, not as a replacement denominator.

### Batch 274: malformed composite point-limit and ABI guard parity

The next public malformed-input investigation followed the first divergence,
not a coverage-only state. A synthetic `glyf` face with a 2,001-point child
contour repeated across 40 identity components expands to 80,040 points while
`maxp.maxCompositePoints` remains `65,535`. Pinned FreeType rejects the
composite in `freetype/src/base/ftgloadr.c:222-291` with
`FT_Err_Array_Too_Large` (error 10). Before this repair Rust accumulated the
components and reached the checked `u16` conversion in `src/casts.rs:106`,
which panicked instead of returning the public loader error. The loader now
checks cumulative point and contour counts before copying each component in
`src/tt/glyf.rs`; the maintained witness is
`input/fonts/glyf/large-composite-point-limit.ttf` (3,312 bytes,
SHA-256 `fed9a07c751662b79d1229735a504a8fb8f4e75a309a175a0c0f0040b222e13d`).
Focused parity passed 1/1 with exact error and cleared-slot shape.

The same batch also adds two explicit Fontdone ABI safety probes to the
existing outline-support parity case: an owned oversized internal outline and
a non-SVG glyph passed to the SVG mutation helper. Both are rejected by the
Rust and C ABI wrappers (`false`); the pinned oracle has no public operation
that constructs either private state and records the same rejected result.
These probes document wrapper invariants separately from public FreeType
behavior; focused parity passed 12/12 for the support group.

Coverage MCP run `a26c41d9-f0df-4bad-abc2-f6b5d87899b1` ingested snapshot
`032525a5-4f15-40b1-9dff-4ef741f330db` against explicit baseline
`901eae85-6066-41d8-88d0-9db3459701a8`. Its bounded incremental review reports
269 additional union regions and 5 branches, with no regression. The
measurement is a selected subset (`complete=false`), so it is reachability
evidence only; the strict denominator remains the full snapshot above. The
earlier conclusion that no oversized internal vector was useful applied to the
then-scoped valid-input campaign. Once malformed inputs were admitted, the
public composite witness exposed this real loader mismatch; the ABI oversized
vector remains a separate helper-only state.

### Coverage checkpoint: full denominator and stale-gap reconciliation

The full Coverage MCP run `b68ae015-abbc-46cb-9a8c-cd6e723f0e78` passed with
complete snapshot `901eae85-6066-41d8-88d0-9db3459701a8`. Its strict denominator
is 89,270/92,848 regions (96.146%), 64,757/66,821 lines (96.911%),
3,759/4,044 functions (92.953%), and 11,966/13,652 branches (87.650%). The
remaining strict region count is 3,578. This is the campaign denominator;
selected incremental runs do not replace it.

The registered Coverage MCP command still records source commit
`4c982ce98572420a07922abf120b36ccf82f9061`, while the full run metadata names
worktree commit `06a030ac8a39356368b75c984eeadd849bf73740`. Because the current
tree contains source changes after the registered commit, source-gap line
numbers are used only as locators and are checked against the current local
file before any input expansion.

The diagnostic run `d10d176d-fe9f-4f71-9841-66c40036a097` selected 30 existing
`c102` C-ABI cases by repeated argument flags against the explicit full base
`901eae85-6066-41d8-88d0-9db3459701a8`; snapshot
`a7fe63e2-c30c-4920-a814-8e44a42f3134` added 380 union regions and one branch
with no regression. The cases exercise the existing bzip/LZW/gzip, allocator,
list, and bitmap wrapper families. The result is selected-subset evidence
(`complete=false`) and therefore is not a full-regression percentage.

The follow-up run `f13c2692-4c76-4798-bee0-bddcfe19aa81` selected ten existing
public CFF cubic render cases after source inspection; snapshot
`c4579287-d370-4d57-97e2-6ebbd703f33b` added 130 union regions and no branches,
with no regression. Focused public CMap parity independently passed all 33
comparisons. These runs showed that the reported `MonoOutlineProfileBuilder`
gap was a stale/mixed source-map signal: its private `move_to` method had no
callers because decomposition initializes through `move_to_scaled`. Likewise,
the C-ABI `cmap_cache_lookup_glyph_for_test` adapter and its parity helper had
no callers; the public C route already invokes `FTC_CMapCache_Lookup` directly.
Those two dead test/support adapters were removed without changing a public
route, fixture, expected result, denominator filter, or oracle behavior.

### Batch 258: malformed Type 1 Multiple Master dictionaries

The next five public inputs target distinct defensive callbacks in the pinned
Type 1 loader. Each fixture keeps the Adobe MM dictionary recognizable and
changes only one array shape: an empty axis list, five axes (above
`T1_MAX_MM_AXIS`), no design-position rows, a first design-position row with
zero axes, or an empty weight vector. The pinned callbacks in
`freetype/src/type1/t1load.c:764-879` and `:1084-1115` explicitly report
`FT_Err_Invalid_File_Format` for these shapes. These are therefore public
malformed-input witnesses for behavior FreeType actually rejects, not
synthetic private state or padding.

Focused unified parity passed all five cases with exact face-open errors. The
incremental Coverage MCP run `ec91c4ec-fb9a-495d-ad7c-8524458974a5` used the
five exact IDs as `--migration-coverage-case-ids` against explicit full base
`901eae85-6066-41d8-88d0-9db3459701a8`; it passed and ingested snapshot
`bf1eb876-de46-483e-b8ac-459200806990`. The bounded additive review reports
13 newly covered aggregate regions and 132 lines, with no aggregate function
or branch gain and no regression. Because this is a selected subset
(`complete=false`), those numbers are reachability evidence only; the strict
full denominator remains snapshot `901eae85-6066-41d8-88d0-9db3459701a8`.
The current implementation and pinned oracle already agree on these five
guards, so no Rust repair was justified by this batch.

### Public range reachability sweep: 13,051 cases

To distinguish omitted public inputs from unreachable implementation state, the
Coverage MCP wrapper's argument-based range selector was run over all 13,051
eligible `real-parity` and `real-null-validation` case IDs. The windows were
`1:1000`, `1001:2000`, `2001:3000`, `3001:4000`, `4001:5000`, `5001:6000`,
`6001:7000`, `7001:8000`, `8001:9000`, `9001:10000`, `10001:11000`,
`11001:12000`, `12001:13000`, and `13001:13051`; every selected run passed.
This is an argument-level selection, not a new fixture or denominator filter.

The combined incremental run `30ec1ae9-fcab-4d4c-a15a-77ec345f7c98` used all
of those range arguments against explicit full base
`901eae85-6066-41d8-88d0-9db3459701a8` and ingested
`4f4ad3cf-c009-49c9-ae57-a4a5abb8606c`. Its additive union reports +1,000
regions, +217 lines, +1 function, and +8 branches with no regression. A
follow-up full-mode run `5c0d30ab-3bef-4a6a-ae8b-3b9bdcd7c43e` ingested
`32c0d432-95c8-4500-9d97-b0d2bd52d328` for the same selected public range.
That complete selected run is useful for locating public reachability, but it
does not replace the campaign's full no-filter denominator: the allowlist
intentionally excludes safety-extension/private-state rows. Any remaining
regions from this sweep must therefore be classified as public input gaps,
implementation mismatches, or non-public/dead support code before further
changes are made.

### Batch 277: PCF property-count clamp accepted by FreeType

The next source-reviewed public input targets a real defensive branch rather
than inventing private PCF state. The maintained fixture
`input/fonts/pcf/properties-count-clamped.pcf` contains 257 property records;
the first seven carry the normal family and charset metadata and the remaining
records are valid repeated properties. Pinned FreeType 2.14.3 explicitly
checks the original count against the table size, then clamps the loaded count
to 256 at `freetype/src/pcf/pcfread.c:522-539`, skips the unread records and
original padding at `:563-581`, and reads the string table using the original
record count at `:590-595`. The fixture therefore tests whether Rust preserves
the on-disk layout while applying the same allocation cap. Its provenance is
recorded in `tests/fixtures/input/fonts/PROVENANCE.md`.

The first focused public parity run was intentionally before the Rust repair:
the pinned C oracle accepted the face (`FT_Err_Ok`), while Rust returned
`FT_Err_Invalid_Stream_Operation` (85) from the `count > 256` rejection in
`src/font.rs:655-666`. Rust now performs the pinned rough-size check, loads
`min(original_count, 256)` records, and derives the record padding and string
offset from `original_count`. Focused parity then passed 1/1 across Rust, the
C ABI, WASM, and the pinned oracle.

Coverage MCP run `fae2095c-a231-4c70-a229-41ff86dc4aae` used the exact
argument-based case selector for
`freetype.FT_New_Memory_Face.success_pcf_property_metadata_variants@pcf-properties-count-clamped`
against explicit full baseline `f83c31ad-a0ed-40b6-b702-e3e1c4f16a6c`; it
passed and ingested snapshot `25c1f00a-1a40-4e07-aef9-78cdc256aa65`. The
bounded additive review reports +911 covered region identities and +1 covered
function with no net covered-line or branch gain. The selected measurement is
`complete=false` and `exact=false`, with baseline hits outside the selected
case marked `not_observed`; these are reachability results, not a replacement
full-denominator percentage.

The follow-up unfiltered Coverage MCP run `5f090886-260c-4d09-af6b-1da1a6d7f1b7`
passed and ingested full snapshot `9e00dcb7-04ea-4730-9134-85bb7be3f443`.
Its strict current denominator is 89,329 / 92,891 regions (96.165%),
64,804 / 66,855 lines (96.932%), 3,758 / 4,041 functions (92.997%), and
11,982 / 13,664 branches (87.690%); 3,562 regions remain. This supersedes
the pre-repair full snapshot for subsequent incremental baselines.

### Batch 278: PCF metric and bitmap count clamp accepted by FreeType

This batch followed the pinned FreeType source before expanding the public
input set. `freetype/src/pcf/pcfread.c:730-766` performs a rough size check
against the original metrics count, then accepts a count above 65534 while
allocating and loading only the first 65534 records. The matching bitmap path
at `:861-875` applies the same cap and requires the capped bitmap count to
match `face->nmetrics - 1`. The maintained
`input/fonts/pcf/metrics-count-clamped.pcf` declares 65535 complete compressed
metrics and bitmap offsets, so it is a source-backed input for both defensive
acceptance paths rather than a private-state probe. Its reason, size, hash,
and generator are recorded in `tests/fixtures/input/fonts/PROVENANCE.md`.

The first focused parity run intentionally preceded the implementation repair:
the pinned C oracle returned `FT_Err_Ok`, while Rust returned
`FT_Err_Invalid_File_Format` (2) from the existing upper-bound rejection in
`src/font.rs`. Rust now preserves the original-count short-table rejection and
uses `min(original_count, 65534)` for the loaded metrics and bitmap counts.
Focused parity then passed 2/2 for the metrics-clamp case and the existing
PCF property-clamp case across Rust, C ABI, WASM, and the pinned oracle. No unit
test was used to increase coverage.

Coverage MCP run `7f16dda9-1c42-4472-ace0-e1fd43f01af2` used the exact
argument-based case selector for
`freetype.FT_New_Memory_Face.success_pcf_property_metadata_variants@pcf-metrics-count-clamped`
against explicit full baseline
`9e00dcb7-04ea-4730-9134-85bb7be3f443`; it passed and ingested snapshot
`9479e33e-2101-4ba9-90bd-0f719b06193c`. The bounded incremental review was
measured with `measurement_scope.kind=selected_subset` and
`merge.exact=false`; it reported 142 newly covered line identities and a
canonical union delta of 784 covered region identities. Those values are
reachability evidence only, not a replacement strict denominator.

As a zero-yield control, a valid ten-entry PCF TOC was also probed against
`freetype/src/pcf/pcfread.c:118-128`. It passed parity, but Coverage MCP run
`5e8e141c-3bab-4261-8de8-8859c617d4de` (snapshot
`ee852db8-3cef-481d-9957-8c8a859dd5af`) added no new region identities because
the existing malformed table-count cases already execute Rust's TOC clamp.
That fixture and case were removed rather than retained as redundant campaign
input.

### Batch 279: strict full snapshot after PCF metric clamp

The required unfiltered Coverage MCP validation run
`19c31be8-714e-42d5-a880-71d53b882949` passed and ingested snapshot
`c639bfbc-88b7-42d6-93f9-97c8819b921f` after the PCF metric/bitmap clamp repair.
Its strict current denominator is 89,337 / 92,899 regions (96.166%),
64,807 / 66,858 lines (96.932%), 3,758 / 4,041 functions (92.997%), and
11,982 / 13,664 branches (87.690%). The full snapshot is the authoritative
baseline for the next incremental campaign step; 3,562 regions remain
uncovered.

### Batch 280: Type 1 MM parser permissiveness and callback errors

This batch added twenty-five public `FT_New_Memory_Face` PFB inputs under
`freetype.FT_New_Memory_Face.batch280_type1_mm_parser_permissiveness`. Each
fixture has an explicit reason and expected result in the public input
contract, with the source-backed classification preserved in
`tests/fixtures/input/fonts/PROVENANCE.md`. The exact IDs are:

`batch280-mm-empty-axis-name`, `batch280-mm-axis-procedure-empty`,
`batch280-mm-design-procedure-empty`, `batch280-mm-map-procedure-empty`,
`batch280-mm-weight-procedure-empty`, `batch280-mm-design-numeric-token`,
`batch280-mm-design-row-mismatch`, `batch280-mm-map-17-points`,
`batch280-mm-weight-count-three`, `batch280-mm-design-count-three`,
`batch280-mm-design-nonnumeric`, `batch280-mm-map-single-value`,
`batch280-mm-map-nonnumeric`, `batch280-mm-map-fractional-design`,
`batch280-mm-map-extra-coordinate`, `batch280-mm-map-nan`,
`batch280-mm-weight-nonnumeric`, `batch280-mm-weight-nan`,
`batch280-mm-axis-no-slash`, `batch280-mm-axis-nonname`,
`batch280-mm-axis-nonarray`, `batch280-mm-axis-unclosed`,
`batch280-mm-partial-axis-only`, `batch280-mm-partial-map-missing`, and
`batch280-mm-partial-weight-missing`.

The cases were selected after reading the pinned parser and loader rather
than assuming that malformed input must fail. `T1_ToTokenArray` in
`freetype/src/psaux/psobjs.c:595-744` treats square- and procedure-shaped
arrays alike, returns Ignore for unterminated/non-array values, and reports a
zero-element array to its callers. `T1_ToInt` and `T1_ToFixed` in
`freetype/src/psaux/psconv.c:161-350` coerce missing or nonnumeric operands to
zero without a parser error. The MM callbacks in
`freetype/src/type1/t1load.c:764-1155` reject empty/mismatched callback
arrays, consume only the first two design-map operands, and accept seventeen
map points because `freetype/include/freetype/ftmm.h:137` sets
`T1_MAX_MM_MAP_POINTS` to 20. Finally, the cleanup at
`freetype/src/type1/t1load.c:2570-2618` discards incomplete or non-`2^axes`
blends and opens an ordinary Type 1 face, so those cases are intentionally
accepted rather than converted into Rust errors.

The first focused parity run exposed fifteen Rust/C mismatches. After the
source review, `src/font.rs:2854-2981` was repaired to preserve the pinned
boundaries: empty callback arrays error, malformed numeric tokens coerce to
zero, map trailing operands are ignored, fractional design values use the
integer prefix, and incomplete/intermediate blends are discarded during face
construction. Focused parity then passed 25/25 across the Rust, C-ABI, WASM,
and pinned-oracle routes; the existing Type 1 MM focused regression passed
61/61. No unit test was used to increase coverage.

Coverage MCP run `9b95eda6-9230-462d-bd43-060f338194b2` used the explicit
post-snapshot baseline `c639bfbc-88b7-42d6-93f9-97c8819b921f` and ingested
child snapshot `6dd7da0d-bbae-45ae-8454-446626976f70`. The argument-based
selection used five repeated `--migration-coverage-case-ids` pairs because
Coverage MCP caps each argument value at 512 bytes; all twenty-five IDs were
still passed exactly. The measured additive review reported +268 newly
covered line identities, +1,211 canonical covered regions, and +15 canonical
covered functions, with no regressions. It was a selected-subset measurement
(`complete=false`, additive union), so it is reachability evidence and not a
replacement for the strict full denominator. The authoritative full snapshot
above remains at 3,562 uncovered regions.

### Batch 281: post-snapshot C ABI wrapper-edge campaign windows

After Batch 279 established full snapshot
`c639bfbc-88b7-42d6-93f9-97c8819b921f`, three argument-filtered public parity
windows exercised the maintained `mcp_cabi_wrapper_edges_batch` inputs. The
first selected `c94-cabi-edge-001` through `030` and
`c95-cabi-edge-001` through `020`; the second selected the remaining
`c95-cabi-edge-021` through `050` and twenty available `c96` cases; the third
selected the remaining four `c96` cases and `c97-cabi-edge-001` through
`046`. These inputs target public C-ABI wrapper validation, stream and cache
state transitions, outline/bitmap boundary handling, and defensive error
returns. They were run through the public Rust, C-ABI, WASM, and pinned-oracle
parity routes; each window passed 50/50.

Coverage MCP used repeated `--migration-coverage-case-ids` argument pairs and
the explicit full-snapshot baseline. Run
`b80ab23b-5f57-48ff-b44f-6bebbf6d228a` ingested
`c1ba3b36-ce45-439e-afb0-b69b29b9b52d` and reported a +1,580-region
baseline-plus-selection union. Run
`c50c2625-825b-4ab6-9b8c-889c9e792eca` ingested
`c8746da1-9995-4d4b-97ff-c45a6d7a4f2d` and reported 90,383 covered regions
(+1,046 versus the full baseline). Run
`d45c29ac-ff71-4c3a-8b4e-eb1d5688c391` ingested
`0b919ae5-02e0-4f21-89f2-f6086403f8ec` and reported 91,297 covered regions
(+1,960 versus the full baseline). All three are selected-subset additive
measurements (`complete=false`, `merge.exact=false`); their percentages are
not full-suite coverage claims, and the authoritative denominator remains
the unfiltered snapshot above.

### Batch 282: parser, renderer, and autohinter source-targeted cases

The fifty maintained `c98-cabi-edge-001` through `c98-cabi-edge-050` public
cases were selected from uncovered source locations rather than duplicated
valid-font probes. Their inputs cover malformed PCF directory/metrics,
property, encoding, and bitmap records; malformed BDF dimensions and
properties; Type 1 charstring/parser boundaries; explicit normal/monochrome,
LCD/LCD-V, SDF, no-hinting, and autohinting render modes; and CJK/Latin
autohinter and scaler boundary fonts. Each case carries its target source
location in the public fixture contract. All 50 passed through Rust, C ABI,
WASM, and the pinned FreeType oracle, with no unit-only coverage input.

Coverage MCP run `a1e8ed03-fbc6-43ba-ab3e-fed9417a36cb` used the explicit
baseline `c639bfbc-88b7-42d6-93f9-97c8819b921f` and ingested child snapshot
`da5b5146-c0eb-49f4-9e9b-d48609d50229`. Its selected-subset additive review
reported 92,138 covered regions (+2,801 versus the full baseline), +613 newly
covered line identities, and +15 covered functions, with no regressions. The
measurement remains `complete=false` and `merge.exact=false`; it is
reachability evidence only, not a replacement for the strict full denominator.

### Batch 283: C ABI load, handle, API, scaler, and Latin cases

The one-hundred maintained `c99-cabi-edge-001` through `c99-cabi-edge-100`
public cases cover the remaining source-targeted C ABI load variants, handle
operations, API validation, rendering modes, scaler boundaries, and Latin
autohinter branches. They were split into two 50-case argument-filtered
windows. Both public Rust/C-ABI/WASM/pinned-oracle parity runs passed 50/50.

Coverage MCP run `143b92da-a7d5-4ca1-aaad-e30c0cbf613d` ingested
`3875ddd9-64f7-44e9-b3dc-4bcb55717968` for cases 001-050 and reported a
1,826-region baseline-plus-selection union and +461 newly covered line
identities. Run `7e90c6f6-b6c4-4546-a732-329560ccb00d` ingested
`524da2ab-0b5e-44c6-97c6-ead7f8814673` for cases 051-100; it reported the
same 1,826-region union, so the second half added no new region identities
relative to the full baseline. Both are selected-subset measurements
(`complete=false`, `merge.exact=false`) and are not strict full-suite claims.

### Batch 284: render, hinter, grayscale, and format dispatch variants

The one-hundred maintained `c100-cabi-edge-001` through
`c100-cabi-edge-100` public cases exercise source-targeted render-mode,
TrueType-hinter, grayscale, CJK, variable-font conversion, table-dispatch,
bitmap-strike, BDF, and Type 1 paths. Both 50-case argument-filtered public
parity windows passed 50/50 across Rust, C ABI, WASM, and the pinned oracle.

Coverage MCP run `f20dcb43-df53-4288-bff6-3e46c3e2ae16` ingested
`5fc2fd76-7462-4fa8-9ce8-17f5194f8c1e` for cases 001-050 and reported a
2,037-region baseline-plus-selection union. Run
`178765f0-f46c-4d25-938f-959b1e2c4c54` ingested
`cabb9ac0-549b-4aad-83fe-0c72b0197061` for cases 051-100 and reported
2,809 regions (+772 beyond the first half). Both measurements used the
explicit full baseline and remain selected-subset evidence
(`complete=false`, `merge.exact=false`), not strict full-suite coverage.

### Batch 285: TrueType, CFF, bitmap, and autohint defensive variants

The one-hundred maintained `c101-cabi-edge-001` through
`c101-cabi-edge-100` public cases target uncovered TrueType/CFF/bitmap and
autohint defensive routes through the public C ABI wrapper. The two
50-case argument-filtered Rust/C-ABI/WASM/pinned-oracle parity windows passed
50/50 each. Coverage MCP run `914c547f-bf92-4256-a3d8-487d3efafb02`
ingested `ca9a67b0-a2df-4134-87b7-29c5ded3d19d` for cases 001-050 and
reported a 1,566-region baseline-plus-selection union. Run
`5ed23101-ec9c-47a1-997b-f2c5d87f8704` ingested
`6fe3d394-d0fa-4bc8-a07a-6efae816b9f3` for cases 051-100 and reported
1,828 regions, only two beyond the first half. The explicit-baseline
measurements remain selected-subset evidence (`complete=false`,
`merge.exact=false`), not strict full-suite coverage.

### Batch 286: format, bitmap, BDF, and Type 1 defensive variants

The one-hundred maintained `c102-cabi-edge-001` through
`c102-cabi-edge-100` public cases exercise source-targeted format dispatch,
bitmap and BDF handling, and Type 1 defensive paths through the public C ABI
wrapper. Both 50-case argument-filtered parity windows passed 50/50 across
Rust, C ABI, WASM, and the pinned oracle. Coverage MCP run
`fe75638f-a4fd-4393-90d1-208f3f43eccf` ingested
`04455e79-a173-4d58-827c-8e0a5a8e9d3f` for cases 001-050 and reported a
1,939-region baseline-plus-selection union. Run
`ae8c1b00-c5cc-45fe-b61d-77541dbd0a09` ingested
`774f5050-4259-49b2-8751-6e15bad9a48f` for cases 051-100 and reported
1,960 regions, only 21 beyond the first half. The explicit-baseline
measurements remain selected-subset evidence (`complete=false`,
`merge.exact=false`), not strict full-suite coverage.

### Batch 287: C ABI wrapper-edge continuation windows

The maintained `c103-cabi-edge-001` through `c103-cabi-edge-090` cases and
`c104-cabi-edge-001` through `c104-cabi-edge-060` cases continue the public
C ABI wrapper-edge campaign. They target malformed format dispatch, bitmap,
BDF, Type 1, handle, allocator, rendering, and validation inputs selected
against uncovered source regions. The first 50-case window (`c103` 001-050)
and the mixed 50-case window (`c103` 051-090 plus `c104` 001-010) both passed
50/50 across Rust, C ABI, WASM, and the pinned oracle. The subsequent
`c104` 011-060 window also passed 50/50 through the public parity harness.

Coverage MCP run `2cafc1b4-5941-43dd-9e06-73d614dbaa68` ingested
`86a8a6c0-0a2c-4d32-8a0a-297707b34146` for `c103` 001-050 and reported a
1,852-region baseline-plus-selection union. Run
`6ae2832b-48b2-45a4-98c3-17b560d2cfc6` ingested
`78c27f23-bd6b-44c8-9634-ec1e5487834a` for `c103` 051-090 plus `c104` 001-010
and reported 1,875 regions. Run `03db1636-1512-4f39-a401-995f035f3641`
ingested `7335f5bd-83dc-49d9-a558-f0bf80e32db2` for `c104` 011-060 and
reported 1,826 regions, adding no region identities beyond the selected
baseline union already observed by the campaign. These explicit-baseline
measurements remain selected-subset evidence (`complete=false`,
`merge.exact=false`), not strict full-suite coverage.

### Batch 288: C ABI cache, outline, stroker, and service probes

The next fifty existing public parity IDs were selected from the maintained
`c105-cabi-edge-*` through `c107-cabi-edge-*` rows. Their explicit reasons
cover cache-manager/SBit lifecycle boundaries, malformed and empty outline
records, stroker state transitions, invalid handles, stream guard inputs, and
optional GX/Color/PostScript service records. These are public
`FT_Memory`-operation cases whose C ABI support route invokes the named
defensive witness; the Rust, C ABI, WASM, and pinned-oracle parity harness
passed all 50/50 cases. The witness inputs are either malformed records or
publicly observable error/lifecycle conditions; no unit-test-only coverage was
used.

Coverage MCP run `ae190f08-0986-4e0f-9230-9286adb8de4a` ingested
`74068e8a-342e-4f80-942f-7aae7043054f` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. Its selected-subset additive union
reported 91,272 covered regions, +1,935 versus the baseline, +463 newly
covered line identities, and +2 functions, with no regressions. The result is
reachability evidence only (`complete=false`, `merge.exact=false`), not a
strict full-suite percentage.

The following fifty IDs continued with the remaining c107 witnesses, c108
wrapper gaps, and the first c109 Rust-FFI sweep witnesses. Their public parity
run also passed 50/50. Coverage MCP run
`fb22c346-6e0c-49f0-b78f-916cfd07044b` ingested
`4d67825f-1071-417e-abb2-37dfc65f5892` and reported 92,602 covered regions
in the baseline-plus-selection union, +3,265 versus the full baseline and
+1,330 beyond the preceding window, with +753 newly covered line identities,
+15 functions, and no regressions. This remains selected-subset evidence
(`complete=false`, `merge.exact=false`); the full denominator is unchanged.

### Batch 289: C ABI Rust-FFI sweep continuation

The next fifty maintained public `FT_Memory` parity IDs covered the remaining
c109 sweep witnesses and the first c110 Rust-FFI wrapper variants. Their
reasons are source-linked to renderer mode validation, detached outline and
stroker state, cache lifecycle, stream decompression guards, scaler and
autohint boundaries, CFF/PCF/BDF/variable-font dispatch, and optional
PostScript, color, and GX services. All 50 passed through Rust, C ABI, WASM,
and the pinned oracle; no unit test was used to increase coverage.

Coverage MCP run `db47dd07-931e-46e1-83eb-e98824f0d6b8` ingested
`a56b220d-7e72-4d17-8767-27463daa76a2` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. Its additive union reported 92,134
covered regions, +2,797 versus the baseline, +652 newly covered line
identities, and no regressions. As with the preceding windows, this is
selected-subset reachability evidence (`complete=false`, `merge.exact=false`),
not a strict full-suite percentage.

### Batch 290: c110/c111 Rust-FFI service continuation

The next fifty maintained public `FT_Memory` parity IDs covered the remaining
c110 witnesses and the first c111 witnesses. Their explicit reasons target
the Rust-owned renderer, detached glyph and outline transformations, stroker
curve/state guards, cache ownership and invalidation, decompression headers,
variable-font and table-service fallbacks, and malformed outline records.
All 50 passed Rust/C-ABI/WASM/pinned-oracle parity; coverage remained routed
through the public parity operation and did not use a unit test.

Coverage MCP run `b7127e65-9b71-401b-a961-a0df88b2f1fd` ingested
`6b868030-4871-44ca-8d14-2ee31781ac12` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. The selected-subset additive union
reported 92,062 covered regions, +2,725 versus the baseline, +655 newly
covered line identities, and no regressions. This remains reachability
evidence (`complete=false`, `merge.exact=false`), not strict full-suite
coverage.

### Batch 291: c111/c112 wrapper-edge continuation

The next fifty maintained public `FT_Memory` parity IDs were selected without
reusing the c110/c111 window: `c111-cabi-edge-{031-037,039,041,044,046,
054-057,061,074-077,079,081,091,094-097,099}` and
`c112-cabi-edge-{001-022}`. The c111 rows use CFF, GX, COLR, variable-font,
SFNT, embedded-bitmap, malformed-GX, and DejaVu assets to select the existing
Rust FFI stream, glyph-allocation, stroker, cache, handle, optional-service,
outline, bitmap, transform, and fixed-math witnesses. The c112 rows replay
the remaining c107 branch witnesses and c108 wrapper indices through the same
public C ABI parity operation. Their input expansion is therefore tied to
specific uncovered Rust-owned routes; it does not add a private unit-test
entry point. Each selected row passed Rust/C-ABI/WASM/pinned-oracle parity,
50/50, including the malformed and out-of-range values already encoded by the
witness. The pinned FreeType side remains the behavioral oracle: these inputs
are accepted, rejected, or ignored according to its public result, while the
Rust witness only exposes that result through the public parity harness.

Coverage MCP run `25a61c79-923f-40d1-8bc0-f4e205ffb3b1` ingested
`4b14deb4-7c32-4d25-a87e-3d53b13d599d` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. Its canonical baseline-plus-selection
union reported 91,529 covered regions, +2,192 versus the baseline, with no
reported regressions. The selected run is reachability evidence only
(`complete=false`, `merge.exact=false`); its replacement-style selected
percentage is not a full-regression measurement.

### Batch 292: c112 wrapper-index continuation

The next fifty distinct maintained public `FT_Memory` IDs were
`c112-cabi-edge-{023-074}` in fixture order. Each row is a separate public
parity selection and maps to the Rust-owned c112 index sweep: c107 indices
1123-1172 and c108 indices 1223-1272. The active subranges include the
remaining c107 branch witnesses (1138-1143, 1150, 1154, 1156, 1158, and
1161-1168) and the c108 wrapper guard at 1229; the other indices deliberately
exercise the public wrapper's defensive no-op/default handling. The selected
routes cover size/charmap, load/render, SFNT, optional service, cache,
stream, bitmap, outline, and fixed-math behavior through the existing Rust
FFI/C ABI/WASM parity operation. All 50 passed Rust/C-ABI/WASM/pinned-oracle
parity. The pinned FreeType result remains the oracle for each public call,
including its error or no-op result for an unsupported index; no private unit
test was used.

Coverage MCP run `6c5b68e5-d0b9-4cd5-94ff-11766f7fea0b` ingested
`75581f3b-e731-4df7-a9c1-d0cb2fafb54a` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. The canonical baseline-plus-selection
union reported 91,083 covered regions, +1,746 versus the baseline, with no
reported regressions. This is selected-subset reachability evidence only
(`complete=false`, `merge.exact=false`), not a strict full-suite percentage.

### Batch 293: c112/c113 wrapper-edge continuation

The next fifty distinct maintained public `FT_Memory` IDs were
`c112-cabi-edge-{075-100}` followed by `c113-cabi-edge-{001-024}`. The c112
rows finish the c112 index sweep over the c107 branch and c108 wrapper
families; the c113 rows begin the five-row Rust-FFI witness families for
glyph allocation/validation, outline checking and rendering, stroking,
bitmap ownership, and outline tracing. These IDs were expanded because the
corresponding public operation is the measured entry point for those
source-linked defensive and mode branches, including malformed outlines,
invalid formats, negative pitch, optional outputs, and out-of-range
selectors. All 50 passed Rust/C-ABI/WASM/pinned-oracle parity. The pinned
FreeType result remains the oracle for whether each malformed or unsupported
public request succeeds, returns an error, or is ignored; no private unit
test was used.

Coverage MCP run `72558b90-08ce-4631-a800-0e6cc15faad5` ingested
`ee961471-3442-4a02-8986-12b5f24af0b0` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. The canonical baseline-plus-selection
union reported 91,111 covered regions, +1,774 versus the baseline, with no
reported regressions. This remains selected-subset reachability evidence
(`complete=false`, `merge.exact=false`), not a strict full-suite percentage.

### Batch 294: c113 Rust-FFI witness continuation

The next fifty distinct maintained public `FT_Memory` IDs were
`c113-cabi-edge-{025-074}`. These five-row Rust-FFI families continue the
publicly routed allocation/validation, outline geometry and trace, rendering,
stroker, bitmap ownership, cache, stream, PostScript, color, variable-font,
SFNT, GX, size/charmap, and load-policy witnesses. The selected variants
retain the malformed outlines, invalid formats, negative pitches, optional
outputs, invalid selectors, and out-of-range glyph/table requests that make
the defensive branches reachable. All 50 passed Rust/C-ABI/WASM/pinned-oracle
parity. The pinned FreeType result is still the reference for each success,
error, or ignored request; no private unit test was used.

Coverage MCP run `637e8674-5883-4a70-acd1-186eccb20aad` ingested
`83870ee2-65ec-47c7-8efb-c155f51d76dd` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. The canonical baseline-plus-selection
union reported 92,286 covered regions, +2,949 versus the baseline, including
15 newly covered functions, with no reported regressions. This remains
selected-subset reachability evidence (`complete=false`, `merge.exact=false`),
not a strict full-suite percentage.

### Batch 295: c113/c114 public-witness continuation

The next fifty distinct maintained public `FT_Memory` IDs were
`c113-cabi-edge-{075-100}` followed by `c114-cabi-edge-{001-024}`. The c113
rows finish the Rust-FFI witness families for cache/stream, PostScript and
color services, variable-font and GX validation, size/charmap, load policy,
and fixed-vector math. The c114 rows begin the valid-public witness matrix
for empty glyph loading, render targets, bitmap-only behavior, and public
size/charmap selectors. The cases retain the malformed and out-of-range
variants where the source-linked branch requires them, while the c114 rows
keep real maintained font assets on the public route. All 50 passed
Rust/C-ABI/WASM/pinned-oracle parity. The pinned FreeType result remains the
oracle for every success, error, or ignored request; no private unit test was
used.

Coverage MCP run `6d762a1d-3393-40da-b26f-5790631236a9` ingested
`f1c495f7-5c19-4775-ae00-327b3e121f40` against the explicit full baseline
`c639bfbc-88b7-42d6-93f9-97c8819b921f`. The canonical baseline-plus-selection
union reported 91,731 covered regions, +2,394 versus the baseline, and +2
newly covered functions, with no reported regressions. This remains
selected-subset reachability evidence (`complete=false`, `merge.exact=false`),
not a strict full-suite percentage.

### Full snapshot checkpoint: post-c114 campaign

The requested unfiltered full-suite run was
`d511414b-af2c-43d1-a0dc-ad7333d8a927`; Coverage MCP ingested snapshot
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The authoritative strict result is
89,572/93,164 regions (96.1444%), 64,941/66,999 lines (96.9283%),
3,770/4,052 functions (93.0405%), and 12,029/13,724 branches (87.6494%).
The remaining denominator is therefore 3,592 regions, 2,058 lines, 282
functions, and 1,695 branches. This is a complete full-suite measurement;
the selected incremental windows above remain additive reachability evidence
and are not substituted for this result.

### Batch 296: empty-source stream error parity

This batch added three public malformed stream records to reach the error side
of the decompression wrappers' `if err == OK` paths:
`ftgzip.FT_Stream_OpenGzip.error_empty_source_without_base`,
`ftlzw.FT_Stream_OpenLZW.error_empty_source_without_base`, and
`ftbzip2.FT_Stream_OpenBzip2.error_empty_source_without_base`. Each record uses
a caller-owned `FT_StreamRec` with `base == NULL`, `read == NULL`, `size == 0`,
and `pos == 3`, plus a sentinel target whose bytes and pointer class are
compared. This is deliberately malformed, but it is deterministic and safe:
the pinned `FT_Stream_ReadAt` checks `pos >= size` before dereferencing the
source. Nonzero-size records with a null base were not added because the C
oracle could dereference invalid storage and would not define a reproducible
public result.

The pinned FreeType oracle returned `FT_Err_Invalid_Stream_Operation` (code
85) and preserved the sentinel for all three records. The first focused run
exposed two implementation mismatches: Rust LZW classified the empty source
as `FT_Err_Invalid_File_Format`, and the C ABI gzip wrapper rejected the
zero-length null-base record as an invalid handle. The Rust fix applies the
same short-source classification as the C stream-read path; the C ABI and
WASM wrappers construct an explicit zero-length slice and only reject a null
base when the source length is nonzero. This is the original FreeType behavior
under test, not a newly invented acceptance rule.

The 50-case public parity selection consisted of
`c113-cabi-edge-{001-047}` plus the three records above. Rust/C-ABI/WASM/
pinned-oracle parity passed 50/50; no unit test was used to increase coverage.
Coverage MCP run `f8425d7a-e203-43fc-b01d-089b2f4f7fb2` ingested
`d83c81a4-7695-4905-ad84-73ccd468775b` against the explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. Its incremental review reported
1,177 additional covered regions in the additive baseline-plus-selection
union, 59 newly covered line identities in the selected diff, and no
regressions. The selection is incomplete by design (`complete=false`,
`merge.exact=false`), so this is reachability evidence rather than a new
full-suite percentage.

### Batch 297: size, bitmap, glyph, and direct-render reachability

This batch selected 50 distinct maintained public parity inputs around the
next WASM wrapper gaps: all 12 `FT_New_Size`/`FT_Activate_Size`/`FT_Done_Size`
rows, all 15 `FT_Bitmap_Copy` rows, all 12 `FT_Glyph_Transform` and
`FT_Done_Glyph` rows, and `FT_Outline_Render.c32_direct_validation_matrix`
rows `c32-direct-001` through `c32-direct-011`. The expansion reason is
source-specific: these inputs exercise size-handle creation and cleanup,
bitmap deep-copy and dimension guards, glyph class dispatch and invalid-format
handling, and direct-render CBox/clip validation around the remaining WASM
regions at lines 3479, 3501, 3507, 3530, 3541, 3560, 3914, 3932, 4358,
4359, and 4389. They use the existing public operations and compare each
success, error, ownership result, and rendered output to pinned FreeType; no
private unit test was added or used.

Rust/C-ABI/WASM/pinned-oracle parity passed 50/50. Coverage MCP run
`8042cfdb-5c3b-48f3-ade5-9de888f45d44` ingested
`b6b839b9-8c2d-415a-a6dd-45295db20acd` against the explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. Its incremental review reported
1,380 additional covered regions in the additive baseline-plus-selection
union, 106 newly covered line identities in the selected diff, and no
regressions. The selection remains incomplete (`complete=false`,
`merge.exact=false`), so it is reachability evidence rather than a strict
full-suite percentage.

### Batch 298: bitmap-SDF source-mode reachability

This batch selected 50 distinct public parity inputs for the bitmap-SDF source
mode gap: the 30 `freetype.FT_Render_Glyph.matrix_render.batch70_sbit_bitmap_sdf`
rows, the three existing `ftimage.FT_Bitmap.sdf_unsupported_source_preserves_bitmap`
rows for Gray2, Gray4, and BGRA, and 17 existing
`ftimage.FT_Pixel_Mode.bitmap_pixel_mode_matches_render_output` Gray2/Gray4
rows. The expansion reason is source-specific: these are the maintained public
routes that can supply non-8-bit or color bitmap sources to the SDF renderer.
The pinned source rejects those source modes with
`FT_Err_Unimplemented_Feature` while retaining the input slot
(`freetype/src/sdf/ftbsdf.c:805-810` and `freetype/src/sdf/ftsdfrend.c:552-601`).
All 50 passed Rust/C-ABI/WASM/pinned-oracle parity; no private unit test was
used.

Coverage MCP run `c94a0249-da53-4ba9-ab84-4fe1478b68f9` ingested
`348c3b79-1dc3-4550-ba30-6aa8f22ee509` against the explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline-plus-selection
union reported +1,096 covered regions, 61 newly covered line identities in the
selected diff, and no regressions. The source view marked the Gray2, Gray4,
and BGRA arm locations as covered but left the LCD and LCD_V locations
uncovered, so the next input expansion targets those two modes. This remains
selected-subset evidence (`complete=false`, `merge.exact=false`), not a new
full-suite percentage.

### Batch 299: LCD/LCD_V bitmap-SDF error parity

This batch added two maintained public variants to
`ftimage.FT_Bitmap.sdf_unsupported_source_preserves_bitmap`:
`ftimage.FT_Bitmap.sdf_unsupported_source_preserves_bitmap@lcd` and
`ftimage.FT_Bitmap.sdf_unsupported_source_preserves_bitmap@lcd-v`. Each uses
DejaVu Sans glyph 36 at 20 ppem with `FT_LOAD_RENDER` plus
`FT_LOAD_TARGET_LCD` or `FT_LOAD_TARGET_LCD_V`, then requests
`FT_RENDER_MODE_SDF` with `capture_render_error_slot`. This sequence is
necessary because it first creates a real LCD/LCD_V bitmap from an outline and
then sends that bitmap through the public second render call; an SBIT input
would return a bitmap slot before this conversion. The pinned FreeType SDF
renderer rejects both source modes with `FT_Err_Unimplemented_Feature` and
retains the source slot, so the fixture records the exact error and output,
not an invented acceptance rule. Focused parity passed 2/2, and the 50-case
public control slice containing both new variants passed 50/50 across
Rust/C-ABI/WASM/pinned-oracle lanes.

Coverage MCP run `9c3d8dec-c3d0-4fac-acf0-0ac2a69723b7` ingested
`31c4ae96-1dd3-4404-aea1-d8387f284980` against the explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive union reported +1,107
covered regions, zero covered-line/function/branch delta under the MCP's
conservative fallback, and no regressions; the selected-subset diff is not a
full-denominator regression claim. The selected raw artifact records six hits
on the shared current `src/render.rs` error return, but MCP source resolution
still identifies commit `4c982ce98572420a07922abf120b36ccf82f9061` while the
checkout is at a later commit with dirty source edits. Therefore the old
snapshot line markers for the separate LCD/LCD_V pattern lines remain
unresolved evidence rather than proof that distinct regions are closed. No
Rust implementation change was made because the new public cases already
match pinned C behavior; the next step is a commit-aligned source measurement
or a different remaining red region.

### Batch 300: concurrent SVG callback capture and public outline validation

This batch exercised 50 existing public parity cases without adding a new
fixture: two SVG callback observer cases, four outline direct-render/no-gray/
no-target/fallback cases, five malformed-font open cases, five valid extreme-CFF
renderer-error cases, two public WASM null-pointer validation cases, 14
`null_library` C32 ABI variants, and 18 `null_outline` C32 ABI variants. The
groups were selected to cover callback-document population and zeroing, the
public `FT_Outline_Render` validation and fallback routes, face-open defensive
exits for malformed input, post-load `FT_Err_Raster_Overflow` behavior, WASM
wrapper pointer guards, and the C ABI's public invalid-handle matrix. The
malformed cases are intentionally public byte inputs rather than private
unit-test calls. The WASM null-pointer cases are a public wrapper-safety
extension with no direct pinned-FreeType equivalent; the C32 invalid-handle
cases were checked against FreeType's validation contract.

The pinned source review covered the render and callback flow in
`freetype/src/base/ftobjs.c:1129-1178`, raster overflow in
`freetype/src/smooth/ftsmooth.c:589-598`, outline validation and fallback in
`freetype/src/base/ftoutln.c:614-666`, the direct no-op/null-target behavior in
`freetype/src/smooth/ftgrays.c:1998-2006`, and the WASM pointer guards in
`fontdone-wasm/src/implementation.rs:2798-2808`. The malformed-font routes
were reviewed against the existing parser/open-input references recorded in
their fixture metadata.

The first parallel parity run exposed a real race: 48/50 passed, while
`otsvg.FT_SVG_Document.mcp_renderer_error_batch@batch268-render-error-peak-y-min-002`
and `...@batch268-render-error-peak-xy-max-005` reported Rust FFI `delta` as
`{x:0,y:0}` instead of the expected null value. A serial 2/2 rerun passed,
identifying process-global callback capture as the first divergence rather
than an input mismatch. The fix makes the WASM capture and both parity probe
records worker-local `RefCell` state, preserving synchronous callback
observations when public parity cases run concurrently. No unit test was used;
the full focused batch passed 50/50 with normal parallel workers.

Coverage MCP run `55087357-f615-484a-8f03-213aa5ad15e9` ingested child
snapshot `4a90d673-d582-4dee-97ed-79a02d9374e6` against the explicit full
baseline `e97404aa-fb4c-43b3-b057-49a0f79b7473`. The selected run reported
1,171 additive covered regions and 622 newly covered line identities, with no
regressions. Covered-line/function/branch metric deltas remained zero under
the MCP's conservative fallback. This is selected-subset evidence
(`complete=false`, `merge.exact=false`), not a new full-denominator score.
The source review still resolves the snapshot to commit
`4c982ce98572420a07922abf120b36ccf82f9061` while the local checkout is at a
later commit with the dirty fix, so its old line markers are not treated as
exact source-closure proof.

### Batch 301: stream callback edges, SBit sentinels, and outline safety guards

This batch exercised 50 distinct existing public parity IDs. The exact ID
families were `ftgzip.FT_Stream_OpenGzip.mcp_read_close_gap_matrix@c49-gzip-001`
and `@batch98-gzip-01` through `@batch98-gzip-30` (31 IDs),
`ftbzip2.FT_Stream_OpenBzip2.mcp_read_gap_matrix@c47-001`,
`ftlzw.FT_Stream_OpenLZW.mcp_stream_gap_matrix@c48-lzw-001`, all five
`ftcache.FTC_SBitCache_Lookup.mcp_no_scale_outline_sentinel_cff_batch` IDs,
all five `ftcache.FTC_SBitCache_Lookup.mcp_no_scale_outline_sentinel_truetype_batch`
IDs, and seven outline-helper IDs: the five `batch265-*` variants plus
`batch266-render-tags-null-glyph-001` and
`batch266-record-sync-null-glyph-002` under
`ftglyph.FT_Glyph_To_Bitmap.error_outline_support_guards`.

The 31 gzip IDs vary the public header/read plan to reach the optional-header
branches, short header handling, out-of-range read, and null close behavior;
the one bzip ID targets callback short-read position propagation; and the one
LZW ID targets the public dictionary/read-close boundary. Pinned FreeType
performs these stream reads and reports the corresponding stream-operation or
file-format error in `freetype/src/gzip/ftgzip.c:194-264,615-645`,
`freetype/src/bzip2/ftbzip2.c:520-530`,
`freetype/src/base/ftstream.c:118-160`, and
`freetype/src/lzw/ftzopen.c:59-90,327-343`. These are malformed or truncated
byte streams where the original accepts the call boundary and intentionally
returns an error; the cases preserve those exact outcomes.

The ten SBit IDs use valid CFF and TrueType fonts with `FT_LOAD_NO_SCALE`.
FreeType's `freetype/src/cache/ftcsbits.c:89-208` intentionally converts a
successful non-bitmap outline load into an unavailable SBit sentinel instead
of rendering it a second time. The two font families and ten glyphs keep that
public cache behavior observable across both format loaders. The seven
outline-helper IDs exercise null handles, empty outlines, invalid tags, and
deliberately inconsistent internal records through the maintained C ABI/WASM
support surface. The pinned `freetype/src/base/ftglyph.c:786-817` validates
null glyph/class/prepare-hook states and has a bitmap no-op, but the exact
corrupt-record mutations are Fontdone wrapper-safety extensions rather than
claims that a normal public FreeType glyph can have those private states.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `a171c2cf-702a-410d-b6d7-cbd295407408` ingested
snapshot `bcb86021-2118-4e3f-bdba-b7c49dd3f294` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline union reported
`covered_regions_delta=1,221`, 711 newly covered line identities, 18 covered
branch identities, three covered functions, and zero regressions. The
selected comparison is explicitly `measurement_scope.kind=selected_subset`
with `complete=false` and `merge.exact=false`; it is not a replacement full
coverage percentage. The source view still reports the historical snapshot
commit `4c982ce98572420a07922abf120b36ccf82f9061` while the execution was
recorded at the later pushed commit, so MCP source markers are retained as
bounded evidence and not treated as exact closure of the current source.

### Batch 302: outline validation, malformed BDF numerics, and optional face tables

This batch exercised 50 distinct existing public parity IDs after the Batch
301 checkpoint. The exact selection was the five remaining
`batch266`/`batch274` variants plus the three WASM MCP witnesses under
`ftglyph.FT_Glyph_To_Bitmap.error_outline_support_guards` and the three
single-case `mcp_bitmap_noop_batch`, `mcp_null_handle_batch`, and
`mcp_null_glyph_handle_batch` IDs; all 30 variants under
`ftbdf.FT_Get_BDF_Property.batch232_bdf_malformed_numeric_prefixes`; and the
first 12 variants under
`freetype.FT_New_Memory_Face.success_malformed_optional_tables_ignored` (five
`name`, two `cmap`, and five `CPAL` inputs).

The outline cases distinguish the public WASM null-output/null-glyph
validation returns and already-bitmap no-op from the maintained wrapper's
private outline bookkeeping guards. The null and bitmap witnesses correspond
to public `FT_Glyph_To_Bitmap` contract states; the deliberately inconsistent
record mutations remain wrapper-safety extensions, not claims that ordinary
public FreeType callers can manufacture those private records. The pinned
validation and bitmap behavior is in `freetype/src/base/ftglyph.c:786-817`.

The 30 BDF cases use loadable faces whose known integer or cardinal property
tokens contain no value, no leading digit, a sign FreeType does not consume,
or a decimal prefix followed by junk. Pinned FreeType's `bdf_atol_` and
`bdf_atoul_` intentionally keep those permissive prefix results (including
zero and saturation), so these are malformed public inputs accepted at face
open and observed through `FT_Get_BDF_Property`, not invented private states.
The relevant oracle paths are `freetype/src/bdf/bdflib.c:289-339,608-720,
1135-1188` and `freetype/src/bdf/bdfdrivr.c:886-937`; the Rust mapping is in
`src/font.rs:1049-1114,1173-1182`.

The 12 optional-table cases keep face creation successful while truncating
optional `name`, `cmap`, or `CPAL` data. This follows the pinned SFNT open
policy: the table loaders defensively reject unusable optional data, while
face construction retains the loadable face where the format permits it.
The source contract is `freetype/src/sfnt/sfobjs.c:sfnt_open_font` together
with `ttload.c:tt_face_load_name`, `ttcmap.c:tt_face_build_cmaps`, and the
corresponding optional-table loaders. The cases therefore test the original
permissiveness and do not change the public error denominator.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `0c261f3a-7084-4f11-816e-7b94b8c775b6` ingested
snapshot `7ebb7d3e-57eb-434b-8681-538e4645b68a` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. Its additive baseline union reported
`covered_regions_delta=1,307`; the selected run carried 572 newly observed
line identities, while the conservative merged line/function/branch covered
deltas remained zero. The selected diff separately reported 61 newly covered
lines, 69 branch-state changes, 5,114 hit-count-only observations, and zero
regressions. The run increased the observed denominator by 8,853 regions,
126 lines, three functions, and 18 branches because this is a selected
subset, not a replacement full snapshot.

The MCP result is explicitly `measurement_scope.kind=selected_subset` with
`complete=false` and `merge.exact=false`; its selected-union percentage must
not be reported as strict project coverage. The source/measurement metadata
still resolves to historical commit
`4c982ce98572420a07922abf120b36ccf82f9061` even though execution used pushed
commit `ef4df02dc1bd63cc566848147e1736705f31c490`, so source markers remain
bounded evidence rather than exact current-source closure.

### Batch 303: CFF fixed operands and public subroutine errors

This batch exercised the complete 50-ID public
`freetype.FT_Load_Glyph.batch220_cff_mul_and_fixed_subr_error` matrix. The
first 25 IDs are valid CFF1 Type 2 `mul` programs across five sizes and five
public load modes. The remaining 25 use fixed-valued operands for
`callgsubr`, again across the same size/mode grid, to preserve the pinned
public error route. The dimensions are intentionally distinct public calls;
they remain in the campaign until attribution proves that a smaller witness
set reaches the same source regions.

The valid `mul` inputs target the Type 2 fixed-operand decode and arithmetic
path. The malformed `callgsubr` inputs target the integer-only subroutine
index conversion: pinned `popInt` records `Syntax_Error` for a fixed operand,
the subroutine lookup propagates it, and the public glyph load maps it to
`FT_Err_Invalid_File_Format`. The oracle references are
`freetype/src/psaux/psintrp.c:2260-2274,979-1050,3030-3035` and
`freetype/src/psaux/psstack.c:130-151`; the Rust paths are
`src/tt/cff.rs:1303-1304,1705-1710`. These are source-reviewed public CFF
inputs, including the malformed cases that the original accepts at the API
boundary only to return its defined glyph-format error.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `554c046d-bc22-4813-8327-05b405aa8f99` ingested
snapshot `1482bddb-7bc1-40b1-a949-b68dce5aa529` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline union reported
`covered_regions_delta=963` and 450 newly observed line identities; the
conservative merged line/function/branch covered deltas remained zero. The
selected diff separately reported 52 newly covered lines, 35 branch-state
changes, 4,292 hit-count-only observations, and zero regressions. The
selection increased the observed denominator by 8,853 regions, 126 lines,
three functions, and 18 branches because it is a selected subset, not a
replacement full snapshot.

The MCP result is explicitly `measurement_scope.kind=selected_subset` with
`complete=false` and `merge.exact=false`; its selected-union percentage is not
a strict project score. Source and measurement metadata still resolve to
historical commit `4c982ce98572420a07922abf120b36ccf82f9061` while this run
executed at pushed commit `7996eae2135d5489c51027478650afbf9b00470e`, so the
source markers remain bounded reachability evidence rather than exact closure
of current source lines.

### Batch 304: CFF EOF subroutines and fixed-add arithmetic

This batch exercised all 50 concrete IDs under
`freetype.FT_Load_Glyph.batch219_cff_eof_and_fixed_add`: 25 valid CFF1
global-subroutine witnesses whose INDEX object ends at EOF, followed by 25
valid Type 2 fixed-real-plus-integer `add` programs. Each family spans five
sizes and five public load modes. These are separate public inputs from
Batch 303: they exercise CFF subroutine end-of-buffer synthesis and mixed
fixed/integer arithmetic rather than fixed-operand `callgsubr` rejection.

Pinned FreeType synthesizes a `return` when a CFF subroutine buffer reaches
EOF, so the first family is accepted through
`freetype/src/psaux/psintrp.c:640-667,979-1050`. The second family is accepted
through the fixed arithmetic path at `psintrp.c:1560-1575`; the Rust source
locations under test are `src/tt/cff.rs:1295-1300,1345-1351`. The fixture
metadata records these as valid public CFF cases because the original
FreeType accepts both shapes; the campaign is checking implementation parity,
not manufacturing an error to inflate coverage.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `6aed8480-8422-442b-b794-ca951b81740d` ingested
snapshot `ff9f30fc-6e84-4f0c-812a-3850041f3c9d` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline union reported
`covered_regions_delta=963` and 448 newly observed line identities; the
conservative merged line/function/branch covered deltas remained zero. The
selected diff reported 52 newly covered lines, 35 branch-state changes,
4,297 hit-count-only observations, and zero regressions. The observed
denominator increased by 8,853 regions, 126 lines, three functions, and 18
branches because the run is a selected subset rather than a replacement full
snapshot.

The MCP result is explicitly `measurement_scope.kind=selected_subset` with
`complete=false` and `merge.exact=false`; its selected-union percentage is not
a strict project score. Source and measurement metadata still resolve to
historical commit `4c982ce98572420a07922abf120b36ccf82f9061` while execution
used pushed commit `5597837d5f25dbc5eb1264bdfb01042246c3eee2`, so source
markers remain bounded reachability evidence rather than exact closure of
current source lines.

### Batch 305: absent OS/2 table states and Type 1 MM parser boundaries

This batch used 50 concrete IDs added after the retained full baseline: all
30 variants of `tttables.FT_SFNT_OS2.os2_absent_query_batch242` and the first
20 variants of `freetype.FT_New_Memory_Face.batch280_type1_mm_parser_permissiveness`.
The OS/2 rows cover no-OS/2 SFNTs plus loadable BDF, PCF, and WinFNT faces;
the Type 1 rows cover eight malformed callback/cardinality errors and twelve
accepted-but-incomplete or permissively converted dictionaries.

The OS/2 inputs are loadable public fonts chosen to make the table service
return its defined absence result. Pinned FreeType returns `NULL` from
`FT_Get_Sfnt_Table` for non-SFNT faces and for an SFNT whose parsed OS/2
version is `0xFFFF`; the public helper and WASM facade preserve that as
`table_present=false`. The source contract is
`freetype/src/base/ftobjs.c:4357-4372`,
`freetype/src/sfnt/sfdriver.c:131-133`, and
`fontdone-wasm/src/implementation.rs:9151-9165`. These cases exercise the
original format distinction and do not turn a missing optional table into an
invented success.

The Type 1 MM inputs target `T1_Open_Face` and its public dictionary callbacks.
Pinned FreeType rejects empty axis/design/map/weight arrays and mismatched
cardinalities with `FT_Err_Invalid_File_Format`, but accepts several missing
or scalar fields by discarding the blend, coercing nonnumeric values to zero,
or ignoring trailing map operands. The maintained rows therefore preserve
both the error and permissive sides of the original behavior. The reviewed
paths are `freetype/src/type1/t1load.c:764-1165`,
`freetype/src/psaux/psobjs.c:570-744`, and
`freetype/src/psaux/psconv.c:195-353`; the Rust parser is
`src/font.rs:2854-2980`.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `68f62e1a-a6a5-4b52-998d-e1d6663b45c2` ingested
snapshot `4af4ab30-2d53-46b2-aa9e-36cbc6d212dc` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline union reported
`covered_regions_delta=951` and 386 newly observed line identities; the
conservative merged line/function/branch covered deltas remained zero. The
selected diff separately reported 47 newly covered lines, 34 branch-state
changes, 3,715 hit-count-only observations, and zero regressions. The
observed denominator increased by 8,853 regions, 126 lines, three functions,
and 18 branches because this was a selected subset rather than a replacement
full snapshot.

The MCP result is explicitly `measurement_scope.kind=selected_subset` with
`complete=false` and `merge.exact=false`; its selected-union percentage is not
a strict project score. Source and measurement metadata still resolve to
historical commit `4c982ce98572420a07922abf120b36ccf82f9061` while execution
used pushed commit `8945252d26a0cef30a87a29ffea967bc18c4c3e5`, so source
markers remain bounded reachability evidence rather than exact closure of
current source lines.

### Batch 306: CJK width modes, composite limits, and remaining malformed face inputs

This batch exercised 50 concrete IDs added after the retained full baseline:
the new composite-point-limit error and all 30 CJK width/mode variants under
`freetype.FT_Load_Glyph`, the five remaining Type 1 MM permissiveness cases,
the five malformed Type 1 MM dictionary errors, the two newly added PCF
property/metrics-count cases, and seven newly added malformed BDF face-open
variants. No ID from the post-baseline Batch 300, 301, or 305 selections was
reused.

The CJK inputs are valid public glyph loads that vary only stem geometry,
target mode, and ppem. They target the pinned autofitter's smooth-width
thresholds and strong-snap mode decisions in
`freetype/src/autofit/afcjk.c:1439-1604`, while the composite witness is a
deliberately malformed but public font whose glyph exceeds FreeType's
cumulative outline-point limit. Pinned FreeType rejects that glyph with
`FT_Err_Array_Too_Large` before copying the overflowing component, as checked
against `freetype/src/base/ftgloadr.c:222-291` and
`freetype/src/truetype/ttgload.c:1800-1845`.

The remaining face-open cases preserve the original parser boundaries. Pinned
Type 1 MM rejects empty or mismatched callback arrays but accepts incomplete
blend dictionaries by discarding the blend; the PCF cases exercise bounded
property/metrics-count handling; and the BDF variants remain loadable while
testing the documented malformed metadata. The reviewed Rust routes are
`src/autohint/cjk.rs`, `src/tt/glyf.rs:540-625`, and `src/font.rs:2854-2980`.
These cases use malformed bytes only where the pinned public loader has a
defined success or error result; no unsafe out-of-bounds oracle input was
introduced.

Focused parity passed 50/50 with the normal parallel workers and no unit test
was used. Coverage MCP run `b2fcd1bd-6129-4cee-a429-3295130f1c77` ingested
snapshot `3f8e1a26-2cef-445b-907d-55af8f64c33e` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`. The additive baseline union reported
`covered_regions_delta=984` and 466 newly observed line identities; the
conservative merged line/function/branch covered deltas remained zero. The
selected diff separately reported 54 newly covered lines, 40 branch-state
changes, 5,796 hit-count-only observations, and zero regressions. The
observed denominator increased by 8,853 regions, 126 lines, three functions,
and 18 branches because this was a selected subset rather than a replacement
full snapshot.

The MCP result is explicitly `measurement_scope.kind=selected_subset` with
`complete=false` and `merge.exact=false`; its selected-union percentage is not
a strict project score. Source and measurement metadata still resolve to
historical commit `4c982ce98572420a07922abf120b36ccf82f9061` while execution
used pushed commit `65249cbd14d09611dee1e5b7821950e41a12f2b0`, so source
markers remain bounded reachability evidence rather than exact closure of
current source lines.

### Batch 307: malformed kern, Select_Size, BDF, and native gvar boundaries

This batch exercised the exact 19 concrete public parity IDs added after the
retained full baseline that remained unselected after Batch 306: five
malformed legacy-kern variants, five `FT_Select_Size` negative probes, the
null-property-name BDF query, three BDF bitmap-bpp normalization cases, and
five native-gvar composite-error variants. The focused parity run also included
two already-covered Select_Size control IDs; all 21 cases passed, including all
19 exact post-baseline IDs. No unit test was used.

The legacy-kern inputs are loadable faces with bounded malformed classic-kern
table shapes: truncated data, offset/length inconsistencies, and a reversed
pair order. They were selected because the pinned loader treats these legacy
tables defensively, tolerating or ignoring malformed records rather than
reading past the table. The relevant oracle paths are
`freetype/src/base/ftobjs.c:3603-3675` and
`freetype/src/sfnt/ttkern.c:185-260`; the expected zero/defined result is the
pinned public outcome, not a private-table unit-test assumption.

The five `FT_Select_Size` probes use public faces and strike-table layouts
with no fixed sizes or unusable strike metadata. They exercise the public
validation/no-fixed-size result, with the fixture oracle retaining the pinned
error instead of forcing a successful size selection. The BDF null-property
case passes a null public property-name pointer and records the wrapper's
defined `None` result. These routes were reviewed against
`freetype/src/base/ftobjs.c`'s size-selection validation and
`freetype/src/base/ftbdf.c:62-86`, `freetype/src/bdf/bdflib.c:1763-1772`,
and `freetype/src/sfnt/ttbdf.c:158-180`; the WASM null mapping is in
`fontdone-wasm/src/implementation.rs:8984-8990`.

The three BDF bpp cases use bounded malformed bitmap metadata whose requested
bits-per-pixel values overflow the supported 1/2/4/8 normalization choices.
Pinned BDF parsing normalizes supported values and returns its defined error
for the overflow cases, as implemented through
`freetype/src/bdf/bdflib.c:1082-1109,1364-1397`. The Rust conversion under
test is `src/font.rs:1722-1817`. The five native-gvar cases keep the composite
glyph structurally valid, then provide a runtime-short variation tuple so the
public variable-coordinate load reaches the defined gvar error after
composite parsing. The pinned reference paths are
`freetype/src/truetype/ttgload.c` and `freetype/src/truetype/ttgxvar.c`; the
Rust routes are `src/tables.rs`, `src/tt/glyf.rs`, and `src/tt/gvar.rs`.

Coverage MCP run `58236197-079b-45bd-ac88-ab25ed75b1cd` ingested child
snapshot `3da79a23-92af-4f9c-96f7-2c2e32a41658` against explicit full baseline
`e97404aa-fb4c-43b3-b057-49a0f79b7473`, using four comma-separated
`--migration-coverage-case-ids` argument values. The additive baseline union
reported `covered_regions_delta=1,244` and 612 run-specific newly observed
line identities. The conservative merged line/function/branch covered deltas
remained zero; the selected diff separately reported 71 newly observed line
identities, 54 branch-state changes, 6,501 hit-count-only observations, and
zero regressions. The selected run is explicitly
`measurement_scope.kind=selected_subset` with `complete=false` and
`merge.exact=false`, so it is not a replacement strict full-denominator score.

The source resolver still identifies historical commit
`4c982ce98572420a07922abf120b36ccf82f9061` while the execution recorded the
then-current pushed checkout at commit `1c1546aad4f709e4e9e65d3d4b1f8e42ae055dc0`.
The MCP line markers are therefore retained as bounded reachability evidence,
not exact closure of current source lines. The next required measurement is a
new unfiltered full snapshot from the committed checkout.

### Batch 309: PCF required-table rejection guards

This batch added exactly four distinct malformed PCF variants to the public
`freetype.FT_New_Memory_Face.error_bad_size_or_unknown_format` parity case:
missing `PCF_METRICS`, `PCF_ACCELERATORS`, `PCF_BITMAPS`, and
`PCF_BDF_ENCODINGS`. Each fixture keeps the preceding table directory valid
so face loading reaches the next required-table lookup; each has an exact
public C-oracle error result. The fixtures are generated by
`scripts/font_generation/build_pcf_fixtures.py` and are tracked under
`tests/fixtures/input/fonts/pcf/`.

The expansion is grounded in the pinned FreeType loader. Its
`pcf_seek_to_table_type` helper returns an invalid-format error when a required
table is absent (`freetype/src/pcf/pcfread.c:372-407`), and
`pcf_load_font` requests the required tables in sequence
(`freetype/src/pcf/pcfread.c:1409-1458`). These are therefore valid public
malformed-input cases, not private unit probes. A proposed PCF
`PIXEL_SIZE`-atom mutation was rejected: `src/font.rs:1325` is in the text BDF
property parser, and a PCF atom cannot reach that arm through the PCF driver.

The focused parity run passed all 4/4 comparisons across Rust, C ABI, and
WASM. Coverage MCP run `0b3c8490-6e47-47d3-8aa0-50c90f96705b` ingested child
snapshot `8560a45a-0ffe-416e-8098-3b4a4f26c737` against explicit baseline
`77935a8a-127d-49ba-b925-76e3feb6d0c3`, using the argument-based
`--migration-coverage-case-ids` filter with concrete `case@variant` IDs. The
incremental additive union reported 4 newly covered regions and no new lines
or branches. The selected run is `measurement_scope.kind=selected_subset`
with `complete=false` and is not a replacement for the strict full
denominator; the retained full baseline remains 89,592/93,194 regions
(96.1349%).

### Batch 313: BDF records after `ENDFONT`

This batch adds one public variant to
`freetype.FT_New_Memory_Face.valid_font_bytes`:
`bdf-trailing-bbx-after-endfont`. The generated BDF contains a valid 8-ppem
face followed by a trailing `BBX 1 1 0 0` record. The record is deliberately
malformed only in its position, not in the face that FreeType opens, and the
case uses the fixture's only supported strike size so it remains a successful
public face-open and size-selection result.

The input was selected because the pinned BDF parser switches from
`bdf_parse_glyphs_` to the no-op `bdf_parse_end_` callback at `ENDFONT` and
ignores all later records (`freetype/src/bdf/bdflib.c:725-741,834-855`). The
trailing record therefore reaches the Rust metadata parser's
`current_glyph == 0` guard at `src/font.rs:1287` without being allowed to
alter the accepted face. The constructor pre-validation initially diverged:
it continued scanning the trailing record and returned
`FT_Err_Missing_Encoding_Field`; its `ENDFONT` handling now stops at the same
state transition while preserving `BdfCorruptedFontGlyphs` for an `ENDFONT`
inside an unfinished glyph.

The fixture is generated by
`scripts/font_generation/generate_malformed_bdf_fixtures.py`, recorded in
`tests/fixtures/input/fonts/PROVENANCE.md`, and retained only after the exact
Rust/C-ABI/WASM/pinned-oracle parity case passed 1/1. No unit test was used to
increase coverage. Coverage MCP measurement is run from the pushed checkout
and recorded here only after its snapshot commit matches this change.

### Batch 308: malformed variation-table boundaries and public setter guards

This batch added exactly 50 new IDs to
`tests/fixtures/inputs/public-api/ftmm.FT_Set_Var_Design_Coordinates.json`,
with no removals or duplicate IDs. The cases are split into five ten-case
families: AVAR version/map boundaries, HVAR header/map boundaries, HVAR item
store validation, MVAR header/record boundaries, and malformed FVAR metadata.
Every case stays on the public `FT_Set_Var_Design_Coordinates` parity route;
the malformed rows are retained because the pinned C runtime has a defined
public success or error result for the corresponding face and setter call.

The inputs were generated by the maintained sections of
`scripts/font_generation/build_ftmm_future_variable_fixtures.py`, rather than
by editing generated font bytes in place. AVAR, HVAR, and MVAR variants use
bounded truncation, version, offset, format, cardinality, and index mutations
to reach the optional-table parser decisions while preserving the surrounding
face where FreeType can construct one. FVAR variants target the required-table
validation path with short headers, impossible counts, and inconsistent axis
or instance sizes. The Rust parser paths are `src/tt/avar.rs`,
`src/tt/hvar.rs`, `src/tt/varstore.rs`, `src/tt/mvar.rs`, and `src/tt/fvar.rs`;
the public dispatch is `src/ffi/handles.rs`.

The source review established the reason for each family. FreeType's
`freetype/src/base/ftmm.c:38-61,280-356` dispatches the setter only when the
face exposes the multiple-master service. Its SFNT face construction in
`freetype/src/sfnt/sfobjs.c:601-657` withholds that public flag when required
FVAR validation fails, so a nonzero coordinate request returns
`FT_Err_Invalid_Argument` (6) instead of entering a nonexistent variation
state. The Rust FFI now mirrors that guard while preserving FreeType's
zero-coordinate no-op on a non-variation face. The AVAR/HVAR/MVAR defensive
paths are optional-table behavior: pinned FreeType either ignores a malformed
optional table during face construction or keeps the defined face state, and
the fixture records that public outcome rather than asserting a private parser
invariant. The relevant variation oracle paths are
`freetype/src/truetype/ttgxvar.c:322-472,476-715,3302-3420`.

One AVAR case also exposed a real Rust mismatch. The pinned normal build
accepts AVAR versions 1 and 2 and applies the shared segment maps; the Rust
parser previously rejected version 2. `src/tt/avar.rs` now accepts both
versions and keeps the segment maps active. This batch's 48-byte version-2
fixture intentionally has no optional AVAR item store or axis-map payload, so
it verifies the shared public behavior without claiming the full OpenType 1.9
extension is implemented.

The first focused parity run found 39/50 passes: ten FVAR status mismatches and
one AVAR bitmap mismatch. After the public FFI guard and AVAR version fix, the
same 50 cases passed 50/50. No unit test was used to inflate coverage. The
focused command was:

```bash
export FONTDONE_UNIFIED_CASE_IDS="$(jq -r '.cases[] | select(.case_id=="ftmm.FT_Set_Var_Design_Coordinates.output_changes_for_design_coordinates") | .inputs.variants[] | select(.id|startswith("batch308-")) | "ftmm.FT_Set_Var_Design_Coordinates.output_changes_for_design_coordinates@" + .id' tests/fixtures/inputs/public-api/ftmm.FT_Set_Var_Design_Coordinates.json | paste -sd, -)"
make test-parity
```

Coverage MCP command `e1a26aea-6698-4e1e-934a-d9e6abfde1d0`
(`fontdone-coverage-active-main-20260830-batch242-reachability`) ran Batch
308 incrementally as run `664556e7-5f91-45d7-b0ca-987fcb9443ab`, using explicit
full baseline snapshot `9bb9b10e-52cd-4246-8275-3957023d4495` and producing
child snapshot `c1d2922d-67fc-4761-9c78-98ac7db0ccc0`. The case filter was
argument-based: 50 full IDs were passed as 12 comma-separated values using
repeated `--migration-coverage-case-ids` arguments (5,244 total ID bytes,
each value at most 512 bytes). The run completed successfully and ingested
the child snapshot.

The read-only incremental review reported 214 newly observed line identities
and, in its conservative additive union, +14 covered lines and +1,199
covered regions with zero regressions. The review is explicitly
`measurement_scope.kind=selected_subset`, `complete=false`,
`merge.exact=false`, and had 43,912 `not_observed` identities. Therefore
these are bounded reachability deltas, not a replacement full-denominator
score; the retained full baseline remains 89,593/93,186 regions (96.1443%),
with 3,593 regions still uncovered. The MCP source resolver also continues to
report historical commit `4c982ce98572420a07922abf120b36ccf82f9061` while this
run executed pushed commit `d50d55f4c696de7b73afe939612c2b512c9c29b5`, so its
source markers remain bounded evidence pending the next unfiltered full
snapshot.

## 5. Fixtures and generators

The tracked input boundary is `tests/fixtures/input/`; maintained
non-generated contracts live in `tests/data/`. Generated matrices and raw
oracle outputs remain ignored under `tests/fixtures/*.json` and
`tests/fixtures/outputs/`.

The canonical input tree currently contains 1,230 tracked paths and no symlinks.
The Makefile exposes 26 named font-generation targets plus the deterministic
compressed-payload target, collected by `make font-fixtures`.

The maintained malformed BDF input
`tests/fixtures/input/fixtures/assets/bdf/missing_font_field.bdf` contains a
blank line after `STARTFONT` to exercise the Rust constructor's blank-line
skip. Pinned FreeType's `bdf_readstream_` skips bytes below space before the
parser callback, so the input remains exact-parity safe. Source-bound parity
run `4cf25299-3965-4726-9159-b76561562270` passed 7,560 / 7,560 comparisons with
0 failures; all-lane coverage run `c58c98ad-a53c-4a7c-8da2-2b3bcfa009d2`
includes the maintained malformed format-13 parser matrix.

The PostScript hinting-property matrix also keeps an out-of-range glyph-index
case. It verifies that the CFF, Type 1, and CID routes return the pinned
Invalid_Argument error and preserve that failed load after a rejected property
value; it is a maintained parity input rather than a coverage-only unit test.
It also includes a maintained non-font-byte case for the same three modules;
the face-open failure is compared as the pinned load error with no fabricated
glyph result.

The runtime row additionally sends selector `99` through the WASM
PS-hinting entry point. The wrapper must reject that unknown selector before
reading font bytes, matching the pinned `FT_Err_Invalid_Argument` result from
the equivalent null-module C property call.

The maintained Type 1 parser control
`tests/fixtures/input/fonts/type1/parser-opcodes.pfb` keeps the valid
CharString movement, stem, hint, escape, division, contour, and termination
paths on the public `FT_Load_Glyph` parity route. Its all-zero geometry is
intentional: it isolates parser execution from unrelated C/Rust outline-metric
differences while still comparing the complete slot result with the pinned C
oracle.

The current coverage loop retains 24 Type 1 face-open variants and 48
Type 1 glyph-load variants across the maintained encoding, PFA/PFB, and
Adobe-MM inputs. The 72 focused cases pass on Rust, the C ABI, WASM, and the
pinned oracle. Coverage MCP snapshot
`280f5d12-eeb7-48e7-996b-bca8bfadfac5` records 52,144 / 54,802 covered lines,
10,444 / 12,508 covered branches, 3,523 / 3,855 covered functions, and
71,729 / 75,851 covered regions; compared with the preceding snapshot it adds
38 lines, one branch, and ten regions without a regression. Duplicate rows are
removed only after grouped or source-gap attribution shows no additional
covered line or branch.

Batch 10 was inserted as one 100-row probe across BDF, CID Type 1, Type 42,
Type 1, glyph-CBox, glyph-copy, and cache-manager routes. After the focused
parity run, nine plain-text BDF face/load candidates were removed because the
pinned C oracle returned `FT_Err_Invalid_Stream_Operation`; they were not
Rust divergences. Grouped source attribution then removed the 14 face-open
rows and the zero-yield glyph-copy/CBox/cache probes. The retained 37 load
rows all pass, and `batch10-cbox-017` remains as a regression case for the
CID no-hinting bug it exposed, for 38 focused passes total. The final MCP
snapshot is `522a782b-0f88-4d4f-a965-8998465ae1eb`; it records 52,154 /
54,805 covered lines, 10,447 / 12,510 branches, 3,523 / 3,855 functions,
and 71,740 / 75,855 regions. Against `280f5d12-eeb7-48e7-996b-bca8bfadfac5`,
that is +10 lines, +3 branches, and +11 regions with no regression. The CID
empty-CharString `FT_LOAD_NO_HINTING` path now returns the same empty outline
slot as the default/no-scale paths, matching `t1cid`.

Batch 12 was also inserted as one 100-row probe. Its Type 1 Adobe-MM rows
exposed a real parity divergence in hinted coordinate scaling; the Rust route
now follows the pinned `cf2_getScaleAndHintFlag` decision and uses Adobe
coordinate scaling only when grid-fit metrics are enabled. The focused and
full parity lanes passed after the fix. Grouped Coverage MCP attribution
removed zero-yield rows and retained only the rows that reached new source
lines; the retained batch added 12 lines, 4 branches, and 12 regions over the
preceding snapshot without changing exact outputs.

Batch 13 followed the same one-shot process with 100 concrete rows spanning
BDF construction, static and variation-coordinate routes, glyph CBox and
bitmap conversion, outline rendering, and Type 1/scalable glyph loads. The
focused parity lane passed all 100 rows after correcting 13 direct-outline
case IDs to carry the shape markers used by the pinned C oracle; this was a
fixture/oracle naming issue, not a Rust runtime divergence. The full matrix
then passed 8,177 / 8,177 runnable comparisons with four explicit pending
cases. Its Coverage MCP snapshot added 10 lines, 3 branches, and 14 regions;
the new lines were the no-`fvar` variation-coordinate errors and the static
face blend delegation in `src/font.rs` and `src/ffi/handles.rs`.

Post-attribution pruning removed 98 zero-yield Batch 13 rows and a temporary
BDF candidate whose `BBX` validation returned before the intended missing
`ENCODING` path. The two retained static-variation rows still pass focused
parity and reproduce the complete +10-line, +3-branch, +14-region gain in the
post-prune full coverage snapshot. No additional runtime divergence was found
in Batch 13. This grouped attribution loop is the required fast path for new
coverage batches: add 100, run parity, measure source deltas, remove rows with
no new coverage, and rerun exact parity before the next batch.

The next MCP-directed batch targeted the synthetic `FT_Get_Glyph` output-clear
guard at `fontdone-wasm/src/implementation.rs:3541`. One hundred nonzero
slot-presence variants passed focused parity; Coverage MCP measured +3 lines,
+2 branches, and +2 regions, with the target line hit 100 times and no
regressions. The 99 redundant variants were pruned to one retained witness;
the post-prune snapshot `f38af3b3-c1d6-4e3a-b6c5-208824086d27` preserves the
same gain at 52,776 / 55,320 lines, 10,688 / 12,678 branches, and 72,342 /
76,447 regions. The synthetic oracle row is explicitly modeled because this
WASM boolean facade has no corresponding public `FT_GlyphSlot` C pointer to
pass to the pinned C function. The full non-coverage matrix then passed
8,796 / 8,796 runnable comparisons with four explicit pending cases.

After the malformed-COLRv1 parser correction, the refreshed Coverage MCP
snapshot `d3a3af5a-6e07-427a-8769-3dd5da59a5e5` records 52,175 / 54,817
covered lines, 10,455 / 12,516 covered branches, 3,523 / 3,855 covered
functions, and 71,765 / 75,870 covered regions. Against the pre-Batch-13
baseline it is +9 lines, +3 branches, and +13 regions; the one-line total
change is LLVM's source-line normalization after the parser boundary fix.

The subsequent MCP-directed fast loop is recorded by immutable snapshots so
coverage-only adoption stays separate from runtime parity evidence. The c73
through c80 batches advanced the current proof from
`889e8b7b-ef49-4e3c-94dc-6987aae038da` at 52,801 / 55,332 lines,
10,702 / 12,682 branches, and 72,368 / 76,451 regions to
`886a99a9-7668-4845-af1e-8fc02fe20d3f` at 52,823 / 55,347 lines,
10,714 / 12,688 branches, and 72,382 / 76,457 regions. Each batch was
focused first, then grouped by source attribution; variants with no new line,
branch, or region were removed and the retained witness was rerun. The c81
null-tag probe produced no coverage delta and was fully pruned.

The c82 batch then added 100 WASM `file_base == NULL` safety variants for the
PostScript hinting facade. Focused parity passed 100 / 100; Coverage MCP
measured +4 lines, +1 branch, and +1 region over the c80 proof, and the
post-prune snapshot `b24f29a8-c1cd-4274-9628-e43bd144160b` preserves that gain
with one retained witness. The c83 batch used the next MCP cursor target,
`fontdone-wasm/src/implementation.rs:1921-1922`, and passed 100 / 100 before
pruning. Only the null-output return at line 1922 improved, so 99 variants were
removed; the post-prune snapshot `67fc56d5-5a3e-46f9-9cab-a2c250b2e199`
reproduces +1 line, +1 branch, and +1 region over c82. It records
52,831 / 55,347 covered lines, 10,716 / 12,688 covered branches, and
72,384 / 76,457 covered regions. No runtime divergence was found in either
batch. The next batch must select its source gap from this current MCP proof,
advance the line-history cursor, and skip a line after three unsuccessful
targeted attempts.

The c84 MCP-directed batch added 100 PostScript property/load-flag variants for
the next WASM post-error preservation target at
`fontdone-wasm/src/implementation.rs:2012`. The first focused run exposed
three grouped runtime divergences and was 68 / 100 before the fixes. Generic
auto-hint dispatch was incorrectly used for Type 1 and empty CID Type 1 faces;
CFF light and `NO_AUTOHINT` loads were incorrectly sent through the generic
TrueType paths; and the Type 1 auto-hinter adapter was inventing SFNT width and
blue-zone metrics for a compact non-SFNT face. The runtime now dispatches each
face family at the same boundary as the pinned driver, adapts Type 1 cubic
outlines through the shared auto-hinter without fabricated SFNT metrics, and
keeps CFF Adobe scaling on the native CFF route. Focused parity now passes all
100 c84 variants across Rust FFI, C ABI, and WASM; the full non-coverage parity
gate is the next required check before coverage attribution and pruning.

Batch168 then added exactly 30 valid public `FT_Get_PS_Font_Value` parity
variants using a synthetic Type 1 face whose optional `version`, `Notice`, and
`FullName` FontInfo strings are absent. The focused and managed parity runs
passed all 30 variants across Rust, the C ABI, WASM, and the pinned oracle.
Coverage MCP run `99585ddd-366c-461c-8ac3-270c25a9a097` produced snapshot
`9295ab13-1859-4c4c-8a7d-9fe08cc04343`; against the strict baseline it adds
12 regions, 7 branches, and 6 lines with no function change. The retained
totals are 88,082 / 91,780 regions, 11,656 / 13,426 branches, 63,841 /
65,960 lines, and 3,692 / 3,980 functions. `FamilyName` and `Weight` remain
absent in the asset but are not counted as reachable missing-string witnesses
because the public Type 1 loader derives fallback values for them.

Batch170 then added exactly 30 valid public `FT_Bitmap_Embolden` rows using
gray bitmaps of widths 1 through 15, each with positive and negative pitch.
The focused and managed parity runs passed all 30 rows across Rust, the C ABI,
WASM, and the pinned oracle. Coverage MCP run
`7adf75e6-4a3e-4bcc-9f66-c27982cd2cc2` produced snapshot
`e7550fa2-3ced-497e-a9ae-f660835446a8`; strict comparison adds one covered
branch at `src/ffi/handles.rs:656` while retaining the 88,082 / 91,780 region,
11,657 / 13,426 branch, 63,841 / 65,960 line, and 3,692 / 3,980 function
totals.

Batch171 then added exactly 30 valid public `FT_Get_PS_Font_Value` rows across
the maintained StandardEncoding, ISOLatin1Encoding, and ExpertEncoding Type 1
faces, using ten valid entry indices per encoding with null output pointers for
sizing queries. The focused and managed parity runs passed all 30 rows across
Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP run
`d9602e73-ad6e-4438-9c46-203f306fc980` produced snapshot
`ecdc34c2-a529-4ca0-9db4-2c8515094625`; strict comparison adds one covered
branch at `src/ffi/handles.rs:7370` with no region, line, or function change,
retaining the 88,082 / 91,780 region, 11,658 / 13,426 branch, 63,841 /
65,960 line, and 3,692 / 3,980 function totals.

Batch173 then added exactly 30 valid public `FT_Get_Track_Kerning` rows using
the maintained Type 1 face and attached AFM track at degree 1, with positive
16.16 point sizes from 0.25 through 7.5 points below the track's 8-point
minimum. The focused and managed parity runs passed all 30 rows across Rust,
the C ABI, WASM, and the pinned oracle. Coverage MCP run
`b1361e2f-ff6d-4dd9-b241-a46018fc3c8e` produced snapshot
`3a3f22b5-497d-418c-9796-8a4b3234cdee`; strict comparison adds the covered
region, line, and branch at `src/ffi/handles.rs:13891-13892` with no function
change, retaining the 88,083 / 91,780 region, 11,659 / 13,426 branch, 63,842
/ 65,960 line, and 3,692 / 3,980 function totals.

Batch174 then added exactly 30 valid public `FT_Get_Track_Kerning` rows using
the maintained Type 1 face and attached AFM track at degree 1, with positive
16.16 point sizes from 72.25 through 79.5 points above the track's 72-point
maximum. The focused and managed parity runs passed all 30 rows across Rust,
the C ABI, WASM, and the pinned oracle. Coverage MCP run
`5354cfaf-2a27-4d06-bb9c-1ea61b40fae3` produced snapshot
`d9324627-484e-406f-ba9a-4a4271247017`; strict comparison adds the covered
region, line, and branch at `src/ffi/handles.rs:13893-13894` with no function
change, retaining the 88,084 / 91,780 region, 11,660 / 13,426 branch, 63,843
/ 65,960 line, and 3,692 / 3,980 function totals.

Batch175 then added exactly 30 valid public `FT_Attach_Stream` rows using the
maintained Type 1 face and AFM pair, with equal positive pixel dimensions from
25 through 54 ppem. Each row attached the AFM and queried `FT_Get_Kerning` in
the default, unfitted, and unscaled modes, exercising the at-or-above-25-ppem
branches in `scale_kerning_vector`. The focused and managed parity runs passed
all 30 rows across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP
run `c3c28df0-71a5-4429-8ed4-cce76a2253a5` produced snapshot
`003af6ed-97c4-449c-9d44-fd9bf1d28a09`; strict comparison with the retained
Batch174 snapshot adds two covered regions and two branches at
`src/ffi/handles.rs:13850-13855`, with no line or function change, retaining
the 88,086 / 91,780 region, 11,662 / 13,426 branch, 63,843 / 65,960 line, and
3,692 / 3,980 function totals.

Batch178 then added exactly 30 valid public `FT_Outline_Render` rows using the
maintained synthetic outline catalog, with AA, DIRECT, and CLIP enabled against
zero-height gray targets and an out-of-box clip box. The focused and managed
parity runs passed all 30 rows across Rust, the C ABI, WASM, and the pinned
oracle. Coverage MCP run `5f94bd8c-cec5-4311-a4b8-b13d04b4a6c8` produced
snapshot `26775395-d0a7-4c34-89b5-2a86dc64135a`; strict comparison with the
retained Batch175 snapshot adds one covered branch at `src/grays.rs:402`, with
no region, line, or function change, retaining the 88,086 / 91,780 region,
11,663 / 13,426 branch, 63,843 / 65,960 line, and 3,692 / 3,980 function
totals. The offline oracle generator recognizes the Batch178 variant marker so
these zero-height rows use the direct span-output path without changing runtime
code.

Batch180 then added exactly 30 valid public `FT_Get_PFR_Metrics` rows using
negative face-index probe mode across the maintained PFR font and DejaVu Sans
control font. The rows cover all 15 non-empty subsets of the four optional
output pointers for each font; probe faces retain no active size, so the
identity-scale and PFR/`Unknown_File_Format` contracts are compared exactly.
The focused parity lane and managed Coverage MCP parity run
`7fe05808-36d7-4db5-ba25-1fee8dbb0f54` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`68751d77-6601-4d9e-814c-a3c992de07ac` produced snapshot
`d3879ffd-22bb-4f65-b8d7-fbc532465247`; strict comparison with the retained
Batch178 snapshot adds 13 covered regions, 11 covered branches, and 2 covered
lines with no function change. The probe-face runtime correction adds 8 total
regions, 6 total branches, and 1 total line, retaining the 88,099 / 91,788
region, 11,674 / 13,432 branch, 63,845 / 65,961 line, and 3,692 / 3,980
function totals.

Batch182 then added exactly 30 valid public `FT_Render_Glyph` rows using a
maintained EBLC/EBDT format-1 gray strike. Glyphs 1 through 30 each carry
positive row counts, zero width, zero pitch, and distinct five-byte
small-metrics records, so SDF rendering reaches the public empty-bitmap
short-circuit without using malformed font data. The focused parity lane and
managed Coverage MCP parity run `ba7b6c0d-dd9f-42d9-b1d6-adf0d1799ac0`
passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Coverage MCP run `20c1cb7f-b960-4e15-ba23-3808fde6ba97` produced snapshot
`22a0cf2e-9ec0-42d4-8204-460465d71e2d`; strict comparison with the retained
Batch180 snapshot adds one covered branch at `src/render.rs:617`, with no
region, line, or function change. The retained totals are 88,099 / 91,788
regions, 11,675 / 13,432 branches, 63,845 / 65,961 lines, and 3,692 / 3,980
functions.

Batch190 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained three-glyph Hebrew face. Glyph 2 maps all Hebrew top-blue characters
to a contour with a short extremum, two consecutive off-curve controls, and a
late on-curve extension point; six ppem sizes and five legal force-auto-hint
targets exercise the public long-blue replacement extension branch. The
focused parity lane and managed Coverage MCP parity run
`62aff22f-c193-4653-9eb9-1929f0a77c15` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`dc9ca01c-7e1d-4669-b5f0-167cc1c2d327` produced snapshot
`4acf5499-2dd4-451e-930a-aba05900bf77`; strict comparison with the retained
Batch182 snapshot adds one covered region, one branch, and one line at
`src/autohint/latin.rs:1567`, with no function change. The retained totals are
88,100 / 91,788 regions, 11,676 / 13,432 branches, 63,846 / 65,961 lines, and
3,692 / 3,980 functions.

Batch191 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained three-glyph Hebrew face. Glyph 2 maps all Hebrew top-blue characters
to a valid 120-unit off-curve apex with adjacent on-curve points; six ppem
sizes and five legal force-auto-hint targets exercise the public blue-segment
on-curve fallback branch. The focused parity lane and managed Coverage MCP
parity run `f324ddd8-e7aa-488a-8255-6ebd372d9892` passed all 30 rows across
Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP run
`4c08b048-13b3-4eea-bda1-98f6185e7f5d` produced snapshot
`6a8a90fd-c49c-46d9-834b-fb457ed1a6a7`; strict comparison with the retained
Batch190 snapshot adds one covered region, one branch, and one line at
`src/autohint/latin.rs:1401`, with no function change. The retained totals are
88,101 / 91,788 regions, 11,677 / 13,432 branches, 63,847 / 65,961 lines, and
3,692 / 3,980 functions.

Batch193 then added exactly 30 valid public `FT_Load_Glyph` rows using the
maintained Khmer sub-top overlap face. Glyph 7 is loaded at six ppem sizes
above the primary-zone threshold, with five legal force-auto-hint targets; the
normal-target anisotropic replacements preserve valid distinct inputs where
the existing LIGHT/LCD metric path diverges. The focused parity lane and
managed Coverage MCP parity run `14294208-2fbc-4dc1-8a2f-852c1d56c7ba` passed
all 30 rows across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP
run `e116ba3b-a76a-463d-9b7a-230b2293473a` produced snapshot
`569361c6-181f-494e-a87d-35eed9cf2208`; strict comparison with the retained
Batch191 snapshot adds one covered region and three branches at
`src/autohint/latin.rs:1920-1921`, `1925`, and `1930`, with no line or function
change. The retained totals are 88,102 / 91,788 regions, 11,680 / 13,432
branches, 63,847 / 65,961 lines, and 3,692 / 3,980 functions.

Batch194 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained Khmer sibling face whose glyph 7 has a lowered sub-top rectangle.
Six ppem sizes and five legal force-auto-hint targets exercise the remaining
primary-zone comparison branch while keeping every input valid and distinct.
The focused parity lane and managed Coverage MCP parity run
`ae504772-00ba-40ac-9e00-b49904398aa3` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`76d610ad-9b8f-4f4a-8faf-d4e825ed5f37` produced snapshot
`dd70b253-7b15-4eb9-ac72-6dd719e433a2`; strict comparison with the retained
Batch193 snapshot adds one covered branch at `src/autohint/latin.rs:1922`, with
no region, line, or function change. The retained totals are 88,102 / 91,788
regions, 11,681 / 13,432 branches, 63,847 / 65,961 lines, and 3,692 / 3,980
functions.

Batch196 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained three-glyph Hebrew face. Glyph 2 maps Hebrew top-blue characters to
a valid short near-top contour span with one lower on-curve point; six ppem
sizes and five legal force-auto-hint targets exercise the public Latin
segment-merge path without malformed input. The focused parity lane and
managed Coverage MCP parity run `d4bb95df-6ce7-4310-a622-5886c1b40230` passed
all 30 rows across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP
run `eaf54921-1a2b-41ba-a948-d8ccc72507ee` produced snapshot
`fc2e2033-25a0-43a7-9562-043962f5446f`; strict comparison with the retained
Batch194 snapshot adds one covered region, one branch, and one line at
`src/autohint/latin.rs:3442`, with no function change. The retained totals are
88,103 / 91,788 regions, 11,682 / 13,432 branches, 63,848 / 65,961 lines, and
3,692 / 3,980 functions.

Batch197 then added exactly 30 valid public `FT_Load_Glyph` rows using the
horizontal mirror of the Batch196 Hebrew witness. The mirrored glyph preserves
the valid near-top span while reaching the opposite public segment-merge arm at
six ppem sizes and five legal force-auto-hint targets. The focused parity lane
and managed Coverage MCP parity run `b0fd2e1a-4f44-40ae-9c78-e570e6b6ee4e`
passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Coverage MCP run `bb0ba5f3-30cf-4a28-a690-ebff563219db` produced snapshot
`665961cc-c8e2-4979-9454-68b01194f7d8`; strict comparison with the retained
Batch196 snapshot adds one covered region, one branch, and one line at
`src/autohint/latin.rs:3445`, with no function change. The retained totals are
88,104 / 91,788 regions, 11,683 / 13,432 branches, 63,849 / 65,961 lines, and
3,692 / 3,980 functions.

Batch199 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the script-coverage face. Glyph 80 retains the public
Latin cmap mapping but adds an on-curve point before the quadratic control
minimum; six ppem sizes and five legal force-auto-hint targets exercise the
false arm of the Latin segment-merge flat-threshold comparison. The focused
parity lane and managed Coverage MCP parity run
`7ae83961-bd3e-4e6a-9d89-e2b56966a60a` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`6d91bdec-2d2c-43d2-866a-3206fa8693c2` passed; explicit import produced
snapshot `7afbc60d-55bc-4847-a953-676fe4bd81c6`. Strict comparison with the
retained Batch197 snapshot adds one covered branch at
`src/autohint/latin.rs:3473`, with no region, line, or function change. The
retained totals are 88,104 / 91,788 regions, 11,684 / 13,432 branches,
63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch200 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the script-coverage face. Glyph 64 retains the U+00F1
public cmap mapping and base rectangle while its quadratic tilde contour puts
the middle on-curve point at the contour minimum; six ppem sizes and five
legal force-auto-hint targets exercise the false arm of `pt.y != min_y`. The
focused parity lane and managed Coverage MCP parity run
`85656edb-0f62-4003-8a0a-50d9d507cedc` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`80aa9b3e-900d-4237-8d97-35fdef7e3e8f` passed; explicit import produced
snapshot `a1d488ad-b953-4f43-9675-297d1580f034`. Strict comparison with the
retained Batch199 snapshot adds one covered branch at
`src/autohint/latin.rs:2390`, with no region, line, or function change. The
retained totals are 88,104 / 91,788 regions, 11,685 / 13,432 branches,
63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch201 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the script-coverage face. Glyph 64 retains the U+00F1
public cmap mapping and base rectangle while its tilde contour uses an
on-curve predecessor before the middle on-curve point; six ppem sizes and
five legal force-auto-hint targets exercise the false arm of the neighboring
control-flag comparison. The focused parity lane and managed Coverage MCP
parity run `14358ad0-375f-44cf-8883-9d056f13d912` passed all 30 rows across
Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP run
`e02cab39-4d3f-4dbd-82a6-1815d8fec28a` passed; explicit import produced
snapshot `f557e252-a7f2-4722-9182-7e8ad1ad25b3`. Strict comparison with the
retained Batch200 snapshot adds one covered branch at
`src/autohint/latin.rs:2392`, with no region, line, or function change. The
retained totals are 88,104 / 91,788 regions, 11,686 / 13,432 branches,
63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch202 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the script-coverage face. Glyph 69 retains the
bottom-tilde base contour while its quadratic accent puts the middle on-curve
point at the contour maximum; six ppem sizes and five legal force-auto-hint
targets exercise the false arm of `pt.y != max_y`. The focused parity lane and
managed Coverage MCP parity run `97300709-6b41-4a0a-9b51-b9a2ac88594f` passed
all 30 rows across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP
run `f5d503dc-36e0-4540-81d2-8ec6f6edeba1` passed; explicit import produced
snapshot `7a21e885-fa85-48c7-aa6d-6bfc1c2a29d9`. Strict comparison with the
retained Batch201 snapshot adds one covered branch at
`src/autohint/latin.rs:2463`, with no region, line, or function change. The
retained totals are 88,104 / 91,788 regions, 11,687 / 13,432 branches,
63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch203 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the `latin-small-ignore.ttf` face. Glyph 7 retains the
U+0122 public cmap mapping while three ordered contours share the same lowest
minimum and use decreasing maxima; six ppem sizes and five legal force-auto-
hint targets exercise the lowest-contour tie-break. The focused parity lane
and managed Coverage MCP parity run `33b85663-9455-4d77-bce1-094b050bcfd6`
passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Coverage MCP run `9374cc50-9609-4ca2-8e31-320a985a0920` passed; explicit
import produced snapshot `31a24498-4790-46a4-92d3-c92917836c07`. Strict
comparison with the retained Batch202 snapshot adds covered branches at
`src/autohint/latin.rs:2326` and `:2774`, with no region, line, or function
change. The retained totals are 88,104 / 91,788 regions, 11,689 / 13,432
branches, 63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch204 then added exactly 30 valid public `FT_Load_Glyph` rows using a
maintained sibling of the script-coverage face. Glyph 75 retains the U+1EAD
public cmap mapping and uses a top accent whose right endpoint is outside the
base while its left endpoint overlaps the base; six ppem sizes and five legal
force-auto-hint targets exercise the public horizontal-overlap arm. The
focused parity lane and managed Coverage MCP parity run
`559648d7-1861-4b90-a3d3-2ae4ffccf61a` passed all 30 rows across Rust, the C
ABI, WASM, and the pinned oracle. Coverage MCP run
`78fb1b38-8231-4398-a208-b085a3ac2fcf` passed; explicit import produced
snapshot `8b258f90-2e3b-46da-9586-f7ee8bb89c6a`. Strict comparison with the
retained Batch203 snapshot adds one covered branch at
`src/autohint/latin.rs:2368`, with no region, line, or function change. The
retained totals are 88,104 / 91,788 regions, 11,690 / 13,432 branches,
63,849 / 65,961 lines, and 3,692 / 3,980 functions.

Batch205 then added exactly 30 valid public `FT_Load_Glyph` rows using the
maintained `gvar-scalar-regions.ttf` variable face. Named instance face index
393216 selects instance 6 (`wdth=100`, `wght=800`) and glyph 10; six ppem sizes
and five legal force-auto-hint targets exercise the normalized-coordinate
upper-end arm at `src/tt/gvar.rs:602`. The focused parity lane and managed
Coverage MCP parity run `a521c518-06c1-4630-b88e-f4c62cccd4e6` passed all 30
rows across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP run
`792f03e5-b887-4752-a0d9-690e6d5387ef` passed; explicit import produced
snapshot `7d4b9480-9d07-44cd-a574-58a994d59811`. Strict comparison with the
retained Batch204 snapshot adds one covered branch at
`src/tt/gvar.rs:602`, with no region, line, or function change. The retained
totals are 88,104 / 91,788 regions, 11,691 / 13,432 branches, 63,849 /
65,961 lines, and 3,692 / 3,980 functions.

Batch206 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `pure-cff-below-baseline-no-vmtx.otf` CFF1 face. Glyph 1 is a
valid rectangle below the baseline, and the face intentionally omits `vmtx`
and `vhea`; six ppem sizes and five legal force-auto-hint targets all request
vertical layout. The focused parity lane passed all 30 rows across Rust, the
C ABI, WASM, and the pinned oracle. After fixing the load-flag precedence and
the CFF vertical metric grid-fit route, managed Coverage MCP parity run
`741a8e2b-193c-417c-860e-910b885a3b9f` passed the full parity suite. Coverage
MCP run `8cebca09-edf5-4bc5-bf85-e1891701d840` passed; explicit import
produced snapshot `9fc16ff7-441f-4693-a98c-8c4252bdd5e8`. Strict comparison
with the retained Batch205 snapshot adds 44 covered regions, six branches,
two functions, and 50 lines. The retained totals are 88,148 / 91,839
regions, 11,697 / 13,438 branches, 63,899 / 66,015 lines, and 3,694 /
3,982 functions.

Batch207 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `pure-cff-baseline-touch-no-vmtx.otf` CFF1 face. Glyph 1 is a
valid rectangle below and up to the baseline, and the face omits `vmtx` and
`vhea`; twelve normal ppem sizes plus six each for mono, LCD, and LCD-V all
request vertical layout. The focused parity lane passed all 30 rows across
Rust, the C ABI, WASM, and the pinned oracle. Managed Coverage MCP parity run
`9c99e87e-8ff2-4752-9743-27f68ac36702` passed the full parity suite. Coverage
MCP run `7f08ef71-03cf-4faf-9f8e-12188d40c350` passed; explicit import
produced snapshot `a3929070-1a2f-42b9-b7e9-6d325c2a377e`. Strict comparison
with the retained Batch206 snapshot adds one covered region and one branch at
`src/scaler.rs:1724`, with no denominator change. The retained totals are
88,149 / 91,839 regions, 11,698 / 13,438 branches, 63,899 / 66,015 lines,
and 3,694 / 3,982 functions.

Batch208 then added exactly 30 valid public `FT_Load_Glyph` rows using the
maintained `pure-cff-cubic-vmtx.otf` CFF1 face. Glyph 1 uses native CFF
loading with `FT_LOAD_NO_AUTOHINT`, `FT_LOAD_NO_BITMAP`, and vertical layout;
six ppem sizes and five legal target modes exercise the present-`vmtx`
vertical-bearing arm at `src/font.rs:7199`. The focused parity lane passed
all 30 rows across Rust, the C ABI, WASM, and the pinned oracle. Managed
Coverage MCP parity run `5a819b6a-b846-44ad-b449-3dbe5f43fc08` passed the full
parity suite. Coverage MCP run `9c971427-00ed-4bb5-b10e-e9767fe15132`
passed; explicit import produced snapshot
`19211e66-5e1c-4655-aa16-094d5b8981e8`. Strict comparison with the retained
Batch207 snapshot adds one covered branch at `src/font.rs:7199`, with no
region, line, or function change. The retained totals are 88,149 / 91,839
regions, 11,699 / 13,438 branches, 63,899 / 66,015 lines, and 3,694 /
3,982 functions.

Batch209 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `parser-setcurrentpoint-after-line.pfb` Type 1 face. Its
valid glyph sets a contour-start move, consumes it with `rlineto`, then uses
`setcurrentpoint`, reaching the false `pending_move` arm at
`src/font.rs:2337`; six ppem sizes and five legal public load modes cover the
matrix. The focused parity lane passed all 30 rows across Rust, the C ABI,
WASM, and the pinned oracle. Managed Coverage MCP parity run
`f6f53204-3a0e-482d-92cf-4ea54378bb59` passed with 18,502/18,502 runnable
cases and four documented pending scenarios. Coverage MCP run
`7a28b59e-db5f-458c-9a0a-1a4c4635a6e8` passed and auto-ingested snapshot
`cb5197a7-b72e-40ed-b03a-ba754cd9faf4`. Strict comparison with the retained
Batch208 snapshot adds one covered region and one branch, at
`src/font.rs:2337`, with no denominator change. The retained totals are
88,150 / 91,839 regions, 11,700 / 13,438 branches, 63,899 / 66,015 lines,
and 3,694 / 3,982 functions.

Batch210 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch210-latin-tilde-next-oncurve.ttf` sibling of the
script-coverage face. Glyph 64 preserves the U+00F1 cmap and uses a valid
two-contour top tilde whose middle point has an on-curve successor, reaching
the false neighbor-control arm at `src/autohint/latin.rs:2393`; six ppem sizes
and five legal force-autohint target modes cover the matrix. The focused parity
lane passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Managed Coverage MCP parity run `e21a499a-0daa-4ae2-9dbc-0ac3fa35203f`
passed. Coverage MCP run `564bca58-0e43-4c62-9936-e47415184bf0` passed and
auto-ingested snapshot `1406bb68-4b28-472d-9379-7a5603561127`. Strict
comparison with the retained Batch209 snapshot adds one covered branch at
`src/autohint/latin.rs:2393`, with no region, line, function, or denominator
change. The retained totals are 88,150 / 91,839 regions, 11,701 / 13,438
branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch211 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch211-latin-tilde-crossed-neighbors.ttf` sibling of the
script-coverage face. Glyph 64 preserves the U+00F1 cmap and uses crossed
neighbor heights around the middle top-tilde point, reaching the asymmetric
measurement path at `src/autohint/latin.rs:2403`; six ppem sizes and five
legal force-autohint target modes cover the matrix. The focused parity lane
passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Managed Coverage MCP parity run `81dae3a2-ace1-411f-9588-e50266f4b29c`
passed. Coverage MCP run `7fe0f508-d3d7-4ed2-9033-cc0796ce1fcd` passed and
auto-ingested snapshot `3d70cb63-4171-4b32-9b13-f782928127ef`. Strict
comparison with the retained Batch210 snapshot adds two covered branches at
`src/autohint/latin.rs:2403` and its ordered comparison, with no region,
line, function, or denominator change. The retained totals are 88,150 /
91,839 regions, 11,703 / 13,438 branches, 63,899 / 66,015 lines, and 3,694 /
3,982 functions.

Batch212 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch212-latin-thin-crossed-tilde.ttf` sibling of the
script-coverage face. Glyph 64 preserves the U+00F1 cmap and uses a thin
crossed top tilde whose scaled zero measurement reaches the second
`measurement != 0` conjunct at `src/autohint/latin.rs:2410`; six ppem sizes
and five legal force-autohint target modes cover the matrix. The focused
parity lane passed all 30 rows across Rust, the C ABI, WASM, and the pinned
oracle. Managed Coverage MCP parity run
`5ab825f3-9bc2-4473-8819-f8643f0789d9` passed. Coverage MCP run
`f7ac75cd-cde9-46ce-a3ed-9fba8396b711` passed and auto-ingested snapshot
`542060e4-2871-4d1d-aacb-cbd42493a0e7`. Strict comparison with the retained
Batch211 snapshot adds one covered branch at `src/autohint/latin.rs:2410`,
with no region, line, function, or denominator change. The retained totals
are 88,150 / 91,839 regions, 11,704 / 13,438 branches, 63,899 / 66,015
lines, and 3,694 / 3,982 functions.

Batch213 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch213-latin-bottom-tilde-prev-oncurve.ttf` sibling of
the script-coverage face. Glyph 69 preserves the U+1E1B cmap and uses an
on-curve predecessor before the bottom-tilde middle point, reaching the
previous-control false arm at `src/autohint/latin.rs:2464`; six ppem sizes and
five legal force-autohint target modes cover the matrix. The focused parity
lane passed all 30 rows across Rust, the C ABI, WASM, and the pinned oracle.
Managed Coverage MCP parity run `e4587cdc-421d-460b-9fd3-632ec8bc63b2`
passed. Coverage MCP run `341b73d9-864c-47af-9364-04713ddacf20` passed and
auto-ingested snapshot `4ab0d18d-6667-445c-b460-00dc30f7fad8`. Strict
comparison with the retained Batch212 snapshot adds one covered branch at
`src/autohint/latin.rs:2464`, with no region, line, function, or denominator
change. The retained totals are 88,150 / 91,839 regions, 11,705 / 13,438
branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch214 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch214-latin-bottom-tilde-next-oncurve.ttf` sibling of
the script-coverage face. Glyph 69 preserves the U+1E1B cmap and uses an
on-curve successor after the bottom-tilde middle point, reaching the false
next-control predicate in the Latin auto-hinter. The rows cover six ppem
sizes and the five legal force-autohint target modes.

Focused parity passed all 30 rows. Managed Coverage MCP parity run
`888d1a51-e8b9-426b-a155-f766ace0f6c8` passed. Coverage MCP run
`99dfb518-d06f-4824-af92-6f10d48efb8e` passed and auto-ingested snapshot
`e4cd85d8-004c-4794-8df0-59cd1da6a1ff`. Strict comparison with the retained
Batch213 snapshot adds one covered branch at
`src/autohint/latin.rs:2465`, with no region, line, function, or denominator
change. The retained totals are 88,150 / 91,839 regions, 11,706 / 13,438
branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch215 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch215-latin-bottom-tilde-crossed-neighbors.ttf` sibling
of the script-coverage face. Glyph 69 preserves the U+1E1B cmap and uses
crossed bottom-tilde neighbors so the ordered-neighbor comparisons reach the
false arms at `src/autohint/latin.rs:2475` and `2477`. The rows cover six
ppem sizes and the five legal force-autohint target modes.

Focused parity passed all 30 rows. Managed Coverage MCP parity run
`3c3affd7-ef33-4c46-90f6-c9b3e7ab755c` passed. Coverage MCP run
`f7ffb6a1-86fa-497d-a8f0-ff70fd82f222` passed and auto-ingested snapshot
`fd201abb-5fef-444f-82f3-85dfde37504e`. Strict comparison with the retained
Batch214 snapshot adds two covered branches at
`src/autohint/latin.rs:2475` and `2477`, with no region, line, function, or
denominator change. The retained totals are 88,150 / 91,839 regions, 11,708
/ 13,438 branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch216 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch216-latin-thin-bottom-crossed-tilde.ttf` sibling of
the script-coverage face. Glyph 69 preserves the U+1E1B cmap and uses a thin
crossed bottom-tilde contour whose zero sentinel measurement reaches the
threshold short-circuit at `src/autohint/latin.rs:2482`. The rows cover six
ppem sizes and the five legal force-autohint target modes.

Focused parity passed all 30 rows. Managed Coverage MCP parity run
`6aeb96b1-9891-4d3d-bea3-3447c1a32635` passed. Coverage MCP run
`0556c1a3-e202-489f-8532-1345c5bf57c1` passed and auto-ingested snapshot
`1d8e579d-886e-4713-82c5-1f5962af0699`. Strict comparison with the retained
Batch215 snapshot adds one covered branch at
`src/autohint/latin.rs:2482`, with no region, line, function, or denominator
change. The retained totals are 88,150 / 91,839 regions, 11,709 / 13,438
branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch217 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch217-latin-overlap-sentinel.ttf` sibling of the
maintained latin-small-ignore face. It preserves the base face and adds
glyphs 9 and 10 mapped to U+0069 and U+01D5; one- and two-point helper
contours drive the overlap-helper sentinel at
`src/autohint/latin.rs:2364`. The rows cover 15 ppem/target combinations for
each glyph using the legal NORMAL, MONO, LCD, and LCD_V force-autohint modes.

Focused parity passed all 30 rows. Managed Coverage MCP parity run
`12638eff-72a4-46c2-b12b-3d2b4a675cf7` passed. Coverage MCP run
`ed2e32b5-7632-4c15-8b47-20ecc01757f8` passed and auto-ingested snapshot
`513741f6-400d-4e6d-9866-0ffd440f3773`. Strict comparison with the retained
Batch216 snapshot adds covered branches at
`src/autohint/latin.rs:2364` and `3810`, with no region, line, function, or
denominator change. The retained totals are 88,150 / 91,839 regions, 11,711
/ 13,438 branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

Batch218 then added exactly 30 valid public `FT_Load_Glyph` rows using the
project-authored `batch218-latin-bottom-distance-order.ttf` sibling of the
script-coverage face. Glyph 73 preserves the U+0122 cmap and uses three
overlapping bottom contours with ordered negative distances to reach the false
right-hand comparison at `src/autohint/latin.rs:2747`. The rows cover ten
ppem sizes and the legal NORMAL, MONO, and LCD force-autohint target modes.

Focused parity passed all 30 rows. Managed Coverage MCP parity run
`cf3da35a-7fd5-48d4-a407-835cd6998e0e` passed. Coverage MCP run
`7a495eb9-49fb-42ed-ae64-a2e3e7eea2b5` passed and auto-ingested snapshot
`bd7b7951-ffda-4fee-9f08-8a8ab504aa17`. Strict comparison with the retained
Batch217 snapshot adds one covered branch at
`src/autohint/latin.rs:2747`, with no region, line, function, or denominator
change. The retained totals are 88,150 / 91,839 regions, 11,712 / 13,438
branches, 63,899 / 66,015 lines, and 3,694 / 3,982 functions.

The next source-reviewed target is the stable public case
`ftcache.FTC_Manager.ownership_requester_failure_propagates_through_lookups`.
Its expansion reason is explicit: use a valid `DejaVuSans.ttf`, install the
caller-supplied `counting_memory_face_requester`, make that requester return
`FT_Err_Invalid_Argument`, and repeat face, size, and SBit lookups so the
WASM ownership snapshot reaches each requester-error arm without caching a
failed face. This is not a malformed-font or invented failure: the public
`FTC_Manager_New` requester callback is an intentional FreeType error channel.
The pinned source calls it from `ftc_face_node_init`
(`freetype/src/cache/ftcmanag.c:215-239`), propagates its error through
`FTC_Manager_LookupFace` (`ftcmanag.c:291-320`) and
`FTC_Manager_LookupSize` (`ftcmanag.c:163-191`), and propagates miss
initialization errors through `FTC_MruList_New` (`ftcmru.c:283-295`). The
focused parity lane passed this one case across Rust, the C ABI, WASM, and the
pinned oracle. After commit `d23d226` was pushed to `main`, Coverage MCP
registration `fa2fed1f-95be-41ee-8af0-1de33906a19f` ran
`880dfd73-e55d-46eb-9f2c-3794a6fc45bf` with the explicit baseline
`05c364db-9864-49d8-8dde-b45169061bbc`; it passed and ingested snapshot
`54dc86bb-2f43-4f71-8462-9702a6c7a3d7`. The bounded source review marks the
four WASM requester-error arms at `fontdone-wasm/src/implementation.rs:2351`,
`2356`, `2361`, and `2365` covered. This was a selected incremental subset,
so its supported review is evidence for those target regions only and is not
reported as a new full-denominator percentage.

The next reachability audit reused the three already-maintained CFF face-open
inputs from commit `5441d86`; no duplicate fonts or coverage-only mutations
were added. Their stable concrete IDs and expansion reasons remain explicit:

| Concrete public ID | Why this input is relevant | Pinned FreeType 2.14.3 behavior |
|---|---|---|
| `freetype.FT_New_Memory_Face.error_malformed_cff_table@cff-top-dict-private-operand-missing` | The Top DICT `Private` operator has one operand instead of the required size/offset pair; this targets `src/tt/cff.rs:378-382`. | `cffparse.c:789-815` initializes `Stack_Underflow` and only accepts the operator when `parser->top >= parser->stack + 2`; `cffload.c:1924-1930` propagates the parser error while opening the face. |
| `freetype.FT_New_Memory_Face.error_malformed_cff_table@cff-top-dict-private-negative-size` | The first `Private` operand is a signed negative size; this targets the checked conversion at `src/tt/cff.rs:384-386`. | `cffparse.c:794-801` rejects a negative size with `Invalid_File_Format`, before `cffload.c:1919-1930` can seek or enter the Private DICT. |
| `freetype.FT_New_Memory_Face.error_malformed_cff_table@cff-top-dict-private-negative-offset` | The size is present but the second `Private` operand is a signed negative offset; this targets `src/tt/cff.rs:387-389`. | `cffparse.c:803-810` rejects the negative offset with `Invalid_File_Format`, again propagated by `cffload.c:1924-1930`; FreeType does not silently accept or seek it. |

Focused parity passed all three IDs across the pinned C oracle, Rust FFI, thin
C ABI, and WASM ABI. Coverage MCP run
`634b1e6b-627e-44fe-8793-e16de8c2dea6` used the argument-based selector
`--migration-coverage-case-ids` with those exact comma-separated IDs at pushed
`main` commit `32185a1`, against explicit baseline
`05c364db-9864-49d8-8dde-b45169061bbc`; it passed and ingested snapshot
`53357999-25b1-4f7f-885b-9d9297b1e582`. The bounded source review marks the
Rust CFF underflow and both negative-conversion errors at
`src/tt/cff.rs:382`, `386`, and `389` covered. The selected incremental
measurement is target-region evidence only; it is not a full-denominator
coverage percentage.

The full local WASM post-error state machine was then checked with the existing
public IDs below. Each ID names a distinct first/second-load transition or
public load mode; no new fixture was added and no Rust implementation change
was needed:

| Concrete public ID | Why this input is relevant |
|---|---|
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_failure_batch@c86-ps-error-001` | Invalid glyph `65535` with default load flags: first and second loads both fail, targeting the `second_load.is_err()` true arm. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_failure_batch@c86-ps-error-002` | The same invalid-glyph error/error transition through `FT_LOAD_NO_SCALE`. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_failure_batch@c86-ps-error-003` | The same invalid-glyph error/error transition with rendered output and the Adobe selector. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_failure_batch@c86-ps-error-004` | The same invalid-glyph error/error transition with `FT_LOAD_NO_HINTING | FT_LOAD_RENDER`. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_failure_batch@c86-ps-error-005` | The existing bitmap-font control exercises the invalid-load path for a non-CFF face and keeps the public module matrix honest. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-001` | Valid CFF global-subroutine input whose first load succeeds and post-property reload errors, targeting the `first_slot`/non-equivalent false arm. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-002` | The same success/error transition through Adobe plus `FT_LOAD_NO_SCALE`. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-003` | The same success/error transition after normal rendering. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-004` | The same success/error transition with `NO_HINTING | RENDER`, isolating reload behavior from the auto-hinter. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c101-ps-global-subr-005` | The same success/error transition with `NO_AUTOHINT | RENDER`, covering the alternate public selector. |
| `ftdriver.FT_HINTING_FREETYPE.mcp_wasm_post_error_global_subr_batch@c102-ps-global-subr-first-error-second-success` | Valid CFF input with deterministic first-load error and second-load success, targeting the opposite `second_load.is_err()` arm. |

Pinned `ftobjs.c:907-921` rejects an invalid face/size/slot before dispatch,
`cffgload.c:238-239` rejects an out-of-range CFF glyph, and
`psintrp.c:2241-2258` plus `psobjs.c:2552-2559` provide the valid global-subr
state transition. Focused parity passed C86 5/5 and C101 5/5; the combined
Coverage MCP run `38ad9143-3f0b-4c88-9cdf-9f85046b561f` used three repeated
`--migration-coverage-case-ids` arguments at pushed commit `ed628bf` and
ingested snapshot `8a3e83ba-d244-4a8a-9167-a17922b18f3a`. Its bounded source
review found no red regions from `fontdone-wasm/src/implementation.rs:2079-2089`.

The malformed Type 1 alternating-curve batch was selected from the remaining
`src/font.rs` no-point branch rather than from a guessed face-open guard. Each
case is a compact project-authored `FT_Load_Glyph` input with a stable ID whose
operand count records the reason for expansion:

| Concrete public ID | Target and expansion reason | Pinned FreeType 2.14.3 result |
|---|---|---|
| `freetype.FT_Load_Glyph.default_load@mcp-type1-hvcurveto-single-operand-001` | `src/font.rs:2424`; one `hvcurveto` operand tests whether Rust matches the C stack-bounds error. | `psintrp.c:2841-2890` computes `count=1`, enters the curve loop, and `psstack.c:185-207` records `Stack_Overflow`; public load returns an error. |
| `freetype.FT_Load_Glyph.default_load@mcp-type1-vhcurveto-single-operand-002` | `src/font.rs:2424`; the vertical-first counterpart proves the same guard for opcode 30. | The same masked-count and `cf2_stack_getReal` bounds path returns the public glyph-format error. |
| `freetype.FT_Load_Glyph.default_load@mcp-type1-hvcurveto-two-operands-003` | `src/font.rs:2481`; two operands test the accepted malformed/no-point path instead of over-rejecting it. | `count=0`, `idx=2`, and the C curve loop is skipped, so FreeType accepts the no-op. |
| `freetype.FT_Load_Glyph.default_load@mcp-type1-vhcurveto-two-operands-004` | `src/font.rs:2481`; vertical-first accepted no-op counterpart. | The same `count=0` masked-count behavior is accepted. |
| `freetype.FT_Load_Glyph.default_load@mcp-type1-hvcurveto-three-operands-005` | `src/font.rs:2481`; three operands verify that the second count bit is masked and the no-point arm remains reachable. | `count=1`, `idx=2`, so the loop is skipped and FreeType accepts the no-op; it does not produce the initially hypothesized error. |

The first focused run was intentionally performed before the Rust change: C
rejected the two one-operand cases while Rust returned success, and C accepted
the two- and three-operand cases. The implementation now rejects only
`values.len() == 1`, preserving the C-accepted malformed counts. Focused parity
then passed all five cases across the pinned oracle, Rust FFI, C ABI, and WASM.
The fixture hashes and generator provenance are recorded in
`tests/fixtures/input/fonts/PROVENANCE.md`.

Batch268 then added exactly five public `otsvg.FT_SVG_Document` parity rows to
exercise the post-load renderer-error arm at
`fontdone-wasm/src/implementation.rs:2826`. The input is the existing valid
project-authored `pure-cff-cubic-peak-shifts.otf`, whose glyphs 1 through 5
contain intentionally extreme cubic coordinates. At 20 ppem, pinned
FreeType's `ft_glyphslot_preset_bitmap` rejects each normal smooth-render box
with `FT_Err_Raster_Overflow` (98), before allocating the bitmap. This is a
valid-font boundary case, not a fabricated pointer state; the pinned sources
are `freetype/src/base/ftobjs.c:490-507` and
`freetype/src/smooth/ftsmooth.c:589-598`.

The first focused comparison exposed a real implementation mismatch: pinned C
returned nested status 98, while Rust returned status 0; the C ABI and WASM
already matched C. The Rust renderer now carries the active `x_ppem/y_ppem`
into normal rendering and applies the same dimension, coordinate, and
`10 * ppem` overflow predicate before raster allocation. Face-backed render
paths use the active size, while `FT_Glyph_To_Bitmap` keeps the no-face dummy
slot behavior. Focused parity then passed all five rows across Rust, the C
ABI, WASM, and the pinned oracle.

Coverage MCP run `5e6ae2a3-ed9b-4d99-af0e-da33e92e4095` used the argument-based
five-ID selector against explicit baseline snapshot
`a761e764-3db0-4dde-9ea6-4fff6074c589` and ingested snapshot
`98c98e72-acc6-4b9e-b685-898fec78f36e`. Its bounded incremental review marks
`fontdone-wasm/src/implementation.rs:2826` newly covered. The selected review
is target-region evidence only (`measurement_scope=selected_subset`), so it
does not claim a new full-denominator percentage. The raw ABI null guards at
2785/2788 and the hook-setup error arm at 2801/2803/2808 remain outside this
public parity harness; no public input can supply those invalid raw-pointer or
failed-owned-hook states without changing the contract.

The next SBit-cache probe corrected an initially wrong hypothesis about a
post-load render error. Two five-case groups use the same public
`FTC_SBitCache_Lookup` route with `FT_LOAD_NO_SCALE | FT_LOAD_RENDER`: the CFF
group uses `pure-cff-cubic-peak-shifts.otf`, and the TrueType group uses
`DejaVuSans.ttf`. Both are valid fonts; the distinct sources make the CFF and
TrueType no-scale loaders observable without fabricating a handle or pointer.

Pinned `ftcbasic.c:131-151` adds `FT_LOAD_RENDER` only in the bitmap-family
loader, while `ftobjs.c:932-948` clears that bit when `FT_LOAD_NO_SCALE` is
present. `ftcsbits.c:121-137` then sees the successfully loaded outline,
branches to `BadGlyph`, and publishes the successful width-255 unavailable
SBit sentinel. It does not call `FT_Render_Glyph` a second time. The first
TrueType probe exposed that mismatch: Rust rendered a bitmap while C returned
the sentinel. Both cache implementations now follow the pinned sequence and
classify any successful non-bitmap load directly as unavailable. Focused
parity passed 207/207 for the SBit lookup operation, including the five
TrueType probes.

Coverage MCP run `51338a99-215f-4b9f-9308-88960b0cf294` used the same five
TrueType IDs before their descriptive rename and ingested snapshot
`338f6b92-a9d4-4cf5-b19f-e0b57aa3492d` against baseline snapshot
`a761e764-3db0-4dde-9ea6-4fff6074c589`. Its selected-subset review records
the direct unavailable-SBit path; it is not a full-denominator percentage.
The source-only post-render error branches were removed because they had no
route in pinned FreeType. The remaining public SBit cases are named
`mcp_no_scale_outline_sentinel_cff_batch` and
`mcp_no_scale_outline_sentinel_truetype_batch` to preserve that source-backed
meaning.

Batch276 added ten source-reviewed public `FT_Load_Glyph` rows for CJK strong
stem snapping. The rows use the maintained `cjk-snap-below-standard.ttf` and
`cjk-wide-stem-snap.ttf` inputs with `FT_LOAD_TARGET_MONO` and
`FT_LOAD_TARGET_LCD_V`, varying near/far below-reference stems, an
above-reference stem, and ppem. The expansion was made because Batch260 used
`FT_LOAD_TARGET_NORMAL`, whose snap flags are disabled; these rows therefore
enter the public CJK snapping route rather than merely repeating the smooth
path. Focused parity passed all ten rows across the pinned oracle, Rust FFI,
the C ABI, and WASM.

The first managed run used dotted logical/variant selectors and failed before
test execution because generated concrete IDs use `@variant`. The retry used
the exact ten concrete IDs in two comma-separated
`--migration-coverage-case-ids` arguments, passed, and ingested snapshot
`dd874628-c2df-4640-bfb3-23e5e88b2094` against the strict full baseline
`0e335d97-c1f7-4dc3-9e99-86efbdba2961`. Its supported incremental review is a
selected-subset measurement and reports zero newly covered lines, regions,
branches, or functions; it is not a replacement full-denominator result.

The raw selected profile did reach `src/autohint/cjk.rs:720-734`, including
the upper-side `width >= reference` path, but the lower-side arm at
`:737-738` remained unexecuted. This is expected under the pinned public
algorithm: `af_cjk_metrics_init_widths` initializes `AF_Width.org` only, and
both pinned C (`freetype/src/autofit/afcjk.c:648-742`) and Rust leave
`AF_Width.cur` at zero during metric scaling; `af_cjk_snap_width`
(`afcjk.c:1439-1480`) therefore retains the positive measured width as its
reference. The public stem computation also normalizes negative distances
before snapping (`afcjk.c:1489-1604`), so a public input cannot reach
`width < reference` without changing pinned behavior or invoking a private
unit-only helper. The lower arm is recorded as defensive/unobservable rather
than forcing a non-parity test or mutating the implementation.

Batch310 added one public `FT_New_Memory_Face` row for an AVAR table whose
version is `3.0`, using the generated `avar-version-3.ttf` fixture. Version 2
is accepted by the current implementation, so version 3 is the next distinct
unsupported-version control that reaches `src/tt/avar.rs:55` through the
optional-table face-open path. Pinned FreeType 2.14.3 treats this malformed
optional table as ignorable during `tt_face_load_avar` and still opens the
variable face; the Rust result therefore remains a successful face open. The
fixture metadata records the source references
`freetype/src/truetype/ttload.c:tt_face_load_avar`,
`freetype/src/sfnt/sfobjs.c:sfnt_open_font`, and `src/font.rs:4282-4287`.

Focused parity passed the concrete row
`freetype.FT_New_Memory_Face.success_malformed_optional_tables_ignored@batch310-avar-unsupported-version-3`
across Rust, the C ABI, WASM, and the pinned oracle. Coverage MCP run
`6ce24a6d-bc50-4ab7-8ca4-498df10b14af` used that exact argument-based selector
against explicit full baseline snapshot
`75ae7b04-0fc8-4c8c-ba7e-c74863a9df58` and ingested snapshot
`7f1cdeda-0239-44e6-8205-89c0da5e5991`. Its supported incremental union adds
two regions, one branch, and newly covers `src/tt/avar.rs:55`; this is selected
incremental evidence and not a replacement full-denominator percentage. The
MCP project source metadata still identifies an older commit, so exact source
line interpretation is anchored to the local pushed checkout.

Batch311 added three public `FT_New_Memory_Face` rows for malformed optional
`sbix` strike records. The records deliberately take three distinct parser
shapes: an offset beyond the table, a one-byte record that cannot provide the
ppem field, and a three-byte record that cannot provide the ppi field. These
inputs target `src/tt/sbix.rs:50-51` while preserving the surrounding valid
face. The pinned FreeType 2.14.3 loader rejects each malformed optional `sbix`
record internally, ignores the optional-table failure during `sfnt_open_font`,
and still opens the face; the Rust, C ABI, and WASM results therefore remain
successful face opens. The fixture metadata records the corresponding pinned C
references at `freetype/src/sfnt/ttsbit.c:tt_face_load_sbix` and
`freetype/src/sfnt/sfobjs.c:sfnt_open_font`.

Focused parity passed all three concrete rows across the pinned oracle, Rust
FFI, C ABI, and WASM. Coverage MCP run
`37338e02-91bd-43b5-b083-23b5efc0a849` used those comma-separated concrete
case IDs against explicit full baseline snapshot
`b7d3ee3f-9a0b-485f-bccd-921c1ac84063` and ingested snapshot
`36d1450c-3865-4751-b45f-271b3da7bd54`. Its supported incremental union adds
four regions, moving the baseline from 89,598/93,194 to 89,602/93,194
(96.141382% to 96.145675%); line, branch, and function totals did not change.
The selected-only profile is not a replacement full-denominator result. The
MCP project source metadata still identifies an older commit, so exact source
line interpretation is anchored to the local checkout and pushed commit.

Batch312 added fifty public parity variants for the remaining C-ABI reachability
work. They are grouped into ten five-case families so each expansion has a
specific input reason: callback-backed LZW streams (valid data, short callback,
bad header, null backing storage, and null handles); requester-backed SBit cache
outputs (bitmap, anode, empty, maximum glyph, and monochrome); bitmap-copy
dimension overflow; built-in glyph creation with default renderers removed;
bitmap and SVG glyph advance bounds; SBit output-pointer states; outline render
validation/direct-mode states; direct-span validation; and stroker null,
border, and malformed-outline guards. The concrete IDs are
`batch312-c115-reach-001` through `batch312-c115-reach-050`, with probes
`1931` through `1980` in `ftsystem.FT_Memory`.

The pinned FreeType 2.14.3 comparison established two runtime mismatches rather
than merely adding coverage. `freetype/src/lzw/ftlzw.c:FT_Stream_OpenLZW`
accepts callback-backed streams and validates them through the stream seek/read
callbacks; the Rust opener previously rejected a null `base` before consulting
those callbacks. `freetype/src/base/ftglyph.c:FT_New_Glyph` selects built-in
bitmap, outline, and SVG glyph classes without requiring a renderer lookup; the
Rust wrapper previously gated those formats on renderer availability. The Rust
paths now mirror those public C decisions. The null-base/no-read LZW case stays
as an explicit Rust defensive error because the pinned no-callback stream path
would dereference the backing pointer. The bitmap overflow, advance bounds,
cache-pointer, renderer, direct-span, and stroker cases retain the pinned
public error/sentinel behavior; they are malformed API records or pointer
shapes used to exercise specified validation, not test-only assertions.

Focused public parity passed all fifty rows across the pinned oracle, Rust FFI,
the C ABI, and WASM (`50/50`, `0` pending). Coverage MCP run
`3cd7a447-73d7-4412-bd24-ecd34aa72ff0` used the ten repeated
comma-separated `--migration-coverage-case-ids` arguments against explicit full
baseline snapshot `b7d3ee3f-9a0b-485f-bccd-921c1ac84063` and ingested snapshot
`b8bbbf84-5729-4c16-9d35-b924bd318c30`. Its supported additive union reports
`+2,334` covered regions; the selected run also adds instrumented probe code,
so the MCP report records denominator growth and must not be presented as a
replacement full-snapshot percentage. The MCP source metadata still identifies
the older baseline commit; source interpretation is anchored to this local
checkout.

Confirmed runtime divergences fixed during the coverage loop are documented
next to their implementations and must remain separate from coverage-only
adoption claims:

- `FT_LOAD_FORCE_AUTOHINT` takes precedence over `FT_LOAD_TARGET_LIGHT`, while
  `FT_LOAD_NO_AUTOHINT` still suppresses the target-light route. The CFF
  driver's synthesized vertical metrics are then grid-fitted for
  `FT_LOAD_VERTICAL_LAYOUT`, matching `ftobjs.c` and `cffgload.c`.

- `FTC_CMapCache_Lookup` normalizes negative cmap indexes into the cache key,
  so a negative lookup can reuse an index-zero entry without consulting the
  active charmap, matching `ftccmap.c`.
- `FTC_SBitCache_Lookup` rounds 26.6 advances with the pinned half-pixel
  offset and preserves the gray format/max-grays descriptor for an empty
  rendered bitmap whose public buffer is elided.
- `FT_Outline_Render` accepts a no-AA request for the public MONO target and
  routes it through the black rasterizer, as `ftraster.c` does.
- `FT_Get_PFR_Metrics` reports zero PFR resolutions and identity scales for a
  negative face-index probe, matching the uninitialized driver-private
  metrics state exposed by the pinned C service before a normal face is
  opened.
- `FT_Get_Color_Glyph_ClipBox` preserves FreeType's SFNT-frame zero padding for
  a format-2 ClipBox whose final `VarIndexBase` reaches past the maintained
  table slice; Rust reads the padded zero and returns the scaled box instead
  of dropping the whole ClipList. The external C-contract audit exposed this
  malformed-input divergence.
- BDF constructor state checks, decimal-prefix `BBX` parsing, strike-size
  selection, and vertical auto-hint metrics now follow the pinned parser and
  scaler ordering. The MVAR vertical-metric rounding correction is likewise
  retained in `src/font.rs`.

Two changes are boundary corrections rather than runtime adoption: the SBit
scaler parity harness compares the pinned direct slot shape while still
executing and validating the cache conversion, and generated oracle/harness
rows preserve the pinned zero-sized MONO/SBit and bitmap target descriptors.
Neither boundary adjustment is counted as complete FreeType API adoption.

Maintained glyph-slot and render inputs may set `probe_wasm_bitmap_accessors`.
Those cases keep the normal slot result in the pinned-C comparison while the
WASM route additionally checks the five exported bitmap accessors against its
slot snapshot, including the null-handle contract. This keeps ABI accessor
evidence on an oracle-backed parity input instead of a unit-only coverage call.

Before changing a fixture:

1. read the [font-generation policy](../scripts/font_generation/README.md);
2. prefer a compact project-authored synthetic input;
3. record generator, classification, source, license, transformation, and
   hashes;
4. regenerate only the affected `make font-fixture-*` family;
5. inspect binary and provenance changes;
6. add or update the corresponding `tests/manifest.yaml` case and exact route
   classification when the fixture exercises a new public behavior;
7. run the focused parity lane and `make check-font-fixtures`.

Maintained public-API inputs also pin raw argument behavior.  For example,
the `FT_Load_Char` input for unassigned load-flag bits keeps the numeric bit
`0x02000000` and compares the complete slot result with C; the Rust boundary
must ignore that bit just as the pinned loader does, rather than turning it
into a blanket unsupported-feature error.

The FT_Outline_Render.bitmap_render_matches_c parity case also carries a
30-input monochrome FT_OUTLINE_SINGLE_PASS family. These inputs use only valid
inline outlines and preallocated public bitmap targets, so the single-pass
raster path is covered through the normal C, Rust, C-ABI, and WASM parity
routes.

The standalone external C-ABI audit cannot inspect FreeType's private `TT_Face`
layout. Its invalid COLRv1 layer-iterator probe therefore seeds the iterator
through public paint APIs and applies equivalent invalid cursor offsets; the
result remains compared exactly with the pinned oracle, while the runtime
contract is still exercised through `FT_Get_Paint_Layers`.

Third-party material must have redistribution permission and exact provenance.
The three retained compact control fonts whose exact upstream transformation
was not recoverable may remain unchanged, but must not be used as new generator
bases until that gap is closed. See
[`PROVENANCE.md`](../tests/fixtures/input/fonts/PROVENANCE.md) and
[`THIRD_PARTY_NOTICES.md`](../tests/fixtures/THIRD_PARTY_NOTICES.md).

`scripts/font_generation/` is the only location for code that creates or
modifies font files. `scripts/build_compressed_fixtures.py` is separate because
it wraps project-authored bytes rather than generating a font.

When a behavior is first observed in a unit test, migrate it to a maintained
public-API input whenever the behavior has a representable font, call sequence,
or ABI observation. The public input must run through the pinned C oracle,
Rust FFI, C ABI, and WASM routes with exact comparison; unit-test execution
alone is not parity evidence. Use `make test-case CASE=<case-substring>` for the
focused migration, regenerate the route audit through `make api-abi-check`, and
then rerun `make test-parity` before retaining any unit-only case.

## 6. CI

`.github/workflows/ci.yml` is the public verification contract:

### 6.1 Per-commit gate

Every push to `main`, every pull-request revision targeting `main`, and every
GitHub merge-queue group runs a bounded Ubuntu fast gate plus a separate MSRV
check:

| Job | Evidence |
|---|---|
| Fast gate | The exact `make ci-fast PYTHON=target/font-generation-venv/bin/python` command used by CI: generated contracts, reproducible fixtures, docs, versions, format, Clippy, strict rustdoc, examples, fast workspace tests, external Rust and C consumers, FFI purity, eight-case parity smoke, and benchmark-harness self-test |
| MSRV | the same fast workspace contract on Rust 1.87.0 |

The stable `Commit gate` succeeds only when both jobs succeed. It is the single
check suitable for ordinary branch protection and merge-queue checks. After
creating the pinned font generation environment from section 1.2, run
`make ci-fast PYTHON=target/font-generation-venv/bin/python` locally (`make ci`
remains an alias when the required Python tools are already on `PATH`). Smoke
diagnostics are retained for seven days. This gate is intentionally not a claim
that the complete parity matrix or every consumer/platform lane ran.

### 6.2 Requested thorough gate

The expensive gate runs only through `workflow_dispatch`. In the Actions UI,
choose **CI**, select the pull-request branch in **Run workflow**, and run it
when the change is ready for pre-merge review. The selected branch head is the
measured commit; do not merge a different revision under that result.

The manual run first repeats the fast commit gate, then adds:

| Job | Evidence |
|---|---|
| Exact parity | every currently runnable C/Rust/C-ABI/WASM comparison and retained diagnostics |
| Integrations | downstream Rust, native C, exact exports, and `wasm32-unknown-unknown` under pinned Node 20 |
| Coverage | all-lane line, branch, function, and region totals |
| Performance | ten raw latency/throughput samples, complete-process peak RSS, and exact release-artifact bytes for pinned FreeType versus Fontdone |
| Native C | Linux x86-64, macOS aarch64, and Windows x86-64 |
| Cross C | Linux i686 and powerpc64 executed under QEMU |
| C scorecard | five fresh platform bundles plus current 12-category debt |
| Packages | all three inspected crate archives |
| Supply chain | advisories, dependency, source, and license policy |

The stable `Thorough gate` succeeds only when every requested job produces
valid evidence. It deliberately runs `make c-abi-contract-all-platforms`
instead of pretending the unfinished 12-category contract is complete.
Coverage, performance, platform, contract, package, parity, and integration
artifacts are retained for 30 days (smoke artifacts are retained separately
for seven days).

The workflow pins every external action to an immutable commit and pins the
Rust, Node, coverage, and audit tool versions. Superseded runs on the same
event and ref are cancelled. A manual run is additional evidence; it is not a
globally required PR check unless a maintainer explicitly requests it.

## 7. Performance

The maintained workload is `tests/data/perf_operation_matrix.json`. The Rust
runner is `examples/bench_ops.rs`; C measurement lives in
`scripts/bench_freetype.py` and `scripts/bench_ft_ops.c` and is never runtime
code.

```bash
make bench-self-test
make bench-quick
make bench
make record-performance-baseline
make bench-regression
```

`make bench-quick` is a two-sample smoke test. `make bench` defaults to ten
samples of the `default` workload and builds both workload executables before
timing. It executes those binaries directly, then records:

1. raw and summarized per-operation latency, including median, p90, and p99;
2. per-operation and aggregate operations/second plus the C/Rust ratio;
3. peak RSS for each complete direct benchmark process;
4. exact byte counts and SHA-256 identities for the Rust and C workload
   executables, Fontdone and FreeType shared libraries, and Fontdone WASM;
5. workload weights, output identity, timing boundaries, CPU, OS, toolchains,
   source commit, dirty state, and CI runner identity.

The report paths are `target/fontdone-bench/latest.json` and `latest.md`.
Timing-only rows are explicitly labeled; exact correctness remains the
responsibility of parity tests and a benchmark output mismatch fails before
the report is accepted.

To enter committed evidence, a report must compare C, use the current matrix
and `default` profile, contain at least ten complete samples, and have been
measured from the current clean commit:

```bash
make bench BENCH_SAMPLES=10 BENCH_PROFILE=default
make record-performance-baseline
make check-docs
```

The recorder does not rerun the benchmark. It validates the raw row and
artifact sets, hashes the report and environment identity, and appends a
compact exact ledger to `doc/compatibility_snapshot.json`. Duplicate recording
is idempotent. Reports from different environment identities remain separate.

The matrix policy requires five clean ten-sample runs from one environment
before maintainers review thresholds. Thresholds are never inferred or
activated by the recorder. While the policy is `collecting_baseline`,
`make bench-regression` intentionally fails after producing evidence. Once
reviewed thresholds are active, that target enforces weighted latency,
aggregate throughput, peak-RSS ratio, shared-library size ratio, and maximum
WASM bytes. `make release-verify` includes the strict target; requested
thorough CI continues to collect ten-sample evidence while the baseline is
being established.

## 8. Repository retention

`doc/FILE_RETENTION_INVENTORY.tsv` assigns every tracked or proposed untracked
path a reason, byte count, and digest. Regenerate it after adding, moving,
deleting, or modifying files:

```bash
make repository-inventory
```

`make check-generated` and `make check-font-fixtures` use the generator's
no-write `--check` mode and fail if any path, byte count, digest, classification,
or reason is stale.

<!-- retention-counts:start -->
| Reason | Paths | Retained context |
|---|---:|---|
| R01 | 58 | published pure-Rust runtime |
| R02 | 100 | package, build, release, and facade contracts |
| R03 | 1,754 | executable parity tests and public contracts |
| R04 | 1,230 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 64 | maintained tooling, examples, and benchmarks |
| R07 | 7 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 5 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **3,223** | **all retained paths** |
<!-- retention-counts:end -->

Reason codes are stable categories, not importance rankings:

1. runtime source;
2. package or root contract;
3. test or public contract;
4. fixture input, license, or provenance;
5. required alias;
6. tooling, example, or benchmark;
7. durable documentation;
8. active self-cleaning plan;
9. CI, community, or security policy;
10. generated source needed by offline builds;
11. generated audit.

Delete build output, duplicate narratives, obsolete work logs, unprovenanced
fixtures, and completed plans after moving any durable contract into an
authoritative guide.

## 9. Documentation policy

- Public downstream Rust APIs deny missing documentation.
- Public `Result` functions describe their error conditions.
- Internal-public parser, hinter, rasterizer, outline, scaler, and arithmetic
  modules carry explicit exceptions because they are instrumentation surfaces,
  not the promised integration API.
- C and WASM pointer/layout contracts live in shipped headers, `abi.json`, and
  package guides.
- Generated support, header, WASM, constant, and legal files are changed only
  through their generators.

Run:

```bash
make check-docs
make doc
make doc-test
```

## 10. Source of compatibility truth

The behavioral authority is checksum-pinned FreeType 2.14.3 source and public
headers fetched by `make oracle-fetch`. External format specifications explain
intent; exact tests record the source function or specification section when a
non-obvious rule becomes durable.
