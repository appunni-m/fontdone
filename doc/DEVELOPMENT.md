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
| `wasm32-unknown-unknown` on Node 20 | maintained low-level WASM consumer |

Only Ubuntu and macOS are normal pinned-oracle development hosts. Windows and
the cross targets are claimed only to the extent recorded above.

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
| Node.js | 20 or newer | claimed WASM host |

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
| Integrations | `make test-integrations` | Downstream Rust, external C, exports, and Node/WASM |
| C contract | `make c-abi-contract` | Report all 12 categories and remaining debt |
| Five-platform C evidence | `make c-abi-contract-all-platforms` | Validate five assembled bundles and report current debt without claiming completion |
| Complete C contract | `make c-abi-contract-complete` | Fail unless all categories and all five platform bundles complete |
| Rust docs | `make doc` and `make doc-test` | Strict rustdoc and compiled examples |
| Static quality | `make lint` | rustfmt and workspace Clippy policy |
| Per-commit local CI | `make ci` | Fast commit gates suitable for ordinary branch protection |
| Requested local audit | `make ci-thorough` | Fast gates plus full parity, integrations, coverage, performance, contract, package, and supply-chain evidence |

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
route. The strict region campaign uses exactly 30 different valid public input
variants per batch, adds them only to the parity matrix, and measures the full
batch before any pruning. A batch is retained only when its parity run is clean
and Coverage MCP can attribute its covered-line, branch, or region delta to the
new inputs.

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

## 5. Fixtures and generators

The tracked input boundary is `tests/fixtures/input/`; maintained
non-generated contracts live in `tests/data/`. Generated matrices and raw
oracle outputs remain ignored under `tests/fixtures/*.json` and
`tests/fixtures/outputs/`.

The canonical input tree currently contains 1,073 tracked paths and no symlinks.
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
| R02 | 88 | package, build, release, and facade contracts |
| R03 | 1,754 | executable parity tests and public contracts |
| R04 | 1,073 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 63 | maintained tooling, examples, and benchmarks |
| R07 | 7 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 5 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **3,053** | **all retained paths** |
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
