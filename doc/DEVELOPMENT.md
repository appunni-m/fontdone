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

## 5. Fixtures and generators

The tracked input boundary is `tests/fixtures/input/`; maintained
non-generated contracts live in `tests/data/`. Generated matrices and raw
oracle outputs remain ignored under `tests/fixtures/*.json` and
`tests/fixtures/outputs/`.

The canonical input tree currently contains 926 tracked paths and no symlinks.
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

Confirmed runtime divergences fixed during the coverage loop are documented
next to their implementations and must remain separate from coverage-only
adoption claims:

- `FTC_CMapCache_Lookup` normalizes negative cmap indexes into the cache key,
  so a negative lookup can reuse an index-zero entry without consulting the
  active charmap, matching `ftccmap.c`.
- `FTC_SBitCache_Lookup` rounds 26.6 advances with the pinned half-pixel
  offset and preserves the gray format/max-grays descriptor for an empty
  rendered bitmap whose public buffer is elided.
- `FT_Outline_Render` accepts a no-AA request for the public MONO target and
  routes it through the black rasterizer, as `ftraster.c` does.
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
| R02 | 86 | package, build, release, and facade contracts |
| R03 | 1,664 | executable parity tests and public contracts |
| R04 | 917 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 61 | maintained tooling, examples, and benchmarks |
| R07 | 7 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 5 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **2,803** | **all retained paths** |
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
