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

The latest source-bound all-lane Coverage MCP run is
`13fa70b0-31dc-4d3d-8d0e-ff7bb7ab2530` (snapshot
`be3f43da-74b3-44bc-863b-25b3f5060bd6`). It completed in 60.630 seconds and
passed 7,582 / 7,582 runnable parity comparisons in the three split backends.
The overall report is 50,180 / 54,346 lines, 10,025 / 12,585 branches,
3,433 / 3,824 functions, and 68,937 / 75,490 regions. The dominant cold-run cost remains
instrumented compilation; warm shard execution and report ingestion are much
smaller.

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

The current managed run completed in 60.630 seconds. Shard timers run
concurrently, so their sum is not wall time; report finalization and artifact
ingestion are included in the wall time but are not separately exposed by
Coverage MCP. Instrumented compilation remains the dominant cold component.

| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 50,180 / 54,346 | 92.33% |
| Branches | 10,025 / 12,585 | 79.66% |
| Functions | 3,433 / 3,824 | 89.78% |
| Regions | 68,937 / 75,490 | 91.32% |

That latest run passed all 7,582 runnable parity comparisons with 0 failures;
3 cases remained explicitly pending. Its immutable coverage snapshot is
`be3f43da-74b3-44bc-863b-25b3f5060bd6`. Coverage MCP accepts the current LLVM
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

The canonical input tree currently contains 659 tracked paths and no symlinks.
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
| R03 | 1,639 | executable parity tests and public contracts |
| R04 | 658 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 61 | maintained tooling, examples, and benchmarks |
| R07 | 7 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 5 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **2,519** | **all retained paths** |
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
