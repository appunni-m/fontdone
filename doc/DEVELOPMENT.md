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
multi-worker behavior. It also sets `CARGO_PROFILE_TEST_OPT_LEVEL=3` through
`COVERAGE_TEST_OPT_LEVEL`; this was the fastest measured warm all-lane profile
(53.316 seconds versus 55.474 seconds at opt-level 1) while preserving coverage
instrumentation and assertions. Set the coverage profile, worker, and
`cargo llvm-cov` flag variables only for an explicitly measured instrumented
profile; the historical opt-level-1 setting remains useful when reproducing the
older committed coverage snapshot. Coverage uses the lightweight
`api-abi-runtime-check`; the optional-feature build contract is intentionally
kept in `make optional-feature-contract` and is not rebuilt on every coverage
run. The coverage recipes retain the instrumented Cargo target with
`cargo llvm-cov --no-clean` and remove only stale `.profraw` files before each
measurement, so repeated local runs reuse the compiled coverage binary without
merging prior execution data. The all-lane target keeps workspace report scope
for the C-ABI and host-compiled WASM facades but selects only the
`unified_fixture_parity` integration binary; the workspace's empty unit and
pipe-trace targets add no parity inputs and can duplicate cfg-dependent FFI
coverage. Its `COVERAGE_ALL_TARGET_DIR` cache is separate from other coverage
profiles, so `--no-clean` cannot reuse stale binaries from a different target
selection. Run `make coverage-clean` after changing the coverage toolchain,
profile flags, or coverage instrumentation configuration. The ABI-only package
preflight remains available as `make coverage-abi-preflight`, but the default
all-lane coverage command does not rerun it: `make test-fast` already executes
the same test-support contract, and `make ci-thorough` runs that gate before
coverage. Set `COVERAGE_ABI_PREFLIGHT=1` when an isolated coverage invocation
also needs the extra preflight.

By default, `COVERAGE_UNIFIED_LANE_SPLIT=1` builds one instrumented
`unified_fixture_parity` binary, then runs that binary directly for the Rust
FFI, C ABI, and host-WASM comparisons in three separate processes.
`FONTDONE_UNIFIED_BACKEND` selects the single backend for each process, and
each process writes a distinct `LLVM_PROFILE_FILE`; the final `cargo llvm-cov
report` merges those raw profiles. Reusing the binary avoids reacquiring
Cargo's build lock and repeating the `cargo llvm-cov` test-profile setup three
times. LLVM source-based coverage counters are process-local, so this removes
the cross-backend counter contention without changing the input matrix or
oracle comparison. Set `COVERAGE_UNIFIED_LANE_SPLIT=0` only to reproduce the
legacy single-process diagnostic path. The clean committed validation was
Coverage MCP run `cce7fb2b-ee6d-43cd-970f-e0dbe7a2b106`: all three processes
passed 7,478 / 7,478 cases, and the end-to-end run took 57.821 seconds versus
the previous warm 113.998-second measurement.

The report names `fontdone`, `fontdone-c-abi`, and `fontdone-wasm` explicitly
because `cargo llvm-cov report` does not accept the workspace flag; this keeps
the C-ABI and WASM source in the measured denominator.

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

```bash
make test-coverage
make test-coverage-all
```

The focused command writes core Rust JSON. The all-lane command schedules the
independent oracle/audit preparation, then uses nightly branch coverage for
every non-ignored root unit and integration target under the default feature
profile, including the complete parity matrix. The split path builds and
instruments the core, native C ABI, and host-compiled WASM facade once, executes
each backend lane in its own process, and merges the three raw profiles into one
report. Compiling a facade again under a second feature set would make LLVM
attribute two object variants to the same source path. Optional feature profiles
are verified by `make optional-feature-contract`; mixing them into the default
parity process would compare different runtime contracts. The coverage command
writes `target/coverage/unified-runtime-all-lanes.json`; test-harness paths are
the only filename exclusion in the final report.

The all-lane run is still intentionally expensive, but repeated local runs
reuse the instrumented target and binary. The optimized-profile path measured
53.316 seconds end-to-end in Coverage MCP with a warm oracle cache; allow
roughly 2 minutes for host variation and roughly 4–6 minutes after a cache
reset.
`COVERAGE_TEST_DEBUG=1` keeps line
tables while omitting full test debuginfo; this reduces the measured end-to-end
run without changing the coverage totals. Face-cache keys also reuse preloaded
font content digests instead of rehashing every expanded case, and the
read-only SFNT table-load/info routes reuse those content-bound handles while
keeping variation-sequence cases isolated. Oracle
preparation also preserves the mtime of unchanged generated constants and
validator overlay sources, avoiding a needless helper rebuild and relink. It
runs in requested thorough CI, not on every commit. The latest source-bound
full-scope run took 57.821 seconds end-to-end against commit
`e6bef8f9e180eeda5e606270e515314443ba6c44`; single-run wall time varies with
compilation and host load. Its instrumentation timers were approximately
44.89 seconds Rust FFI, 34.31 seconds C ABI, 34.08 seconds WASM, and 0.02
seconds comparison. The remaining wall-time tail is setup, process/report
merging, and Coverage MCP ingestion rather than another parity route. Coverage
MCP does not expose timestamps for those sub-phases yet:

| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 49,363 / 54,104 | 91.24% |
| Branches | 9,694 / 12,512 | 77.48% |
| Functions | 3,371 / 3,828 | 88.06% |
| Regions | 67,953 / 75,273 | 90.28% |

That managed run passed all 7,478 runnable parity comparisons with 0 failures;
3 cases remained explicitly pending. Its Coverage MCP run ID is
`cce7fb2b-ee6d-43cd-970f-e0dbe7a2b106`, and its immutable snapshot ID is
`1fab8ecd-b4c0-491d-abcc-110f66101193`. That required three-surface
instrumented execution remains the dominant measured test cost, while the
latest wall-time tail is outside the test body. The percentages apply only to the named
source commit, suite, and toolchain. They are not a FreeType-parity percentage,
and a covered line or branch does not prove an exact result.
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

The canonical input tree currently contains 599 tracked paths and no symlinks.
The Makefile exposes 26 named font-generation targets plus the deterministic
compressed-payload target, collected by `make font-fixtures`.

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
| R03 | 1,638 | executable parity tests and public contracts |
| R04 | 599 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 61 | maintained tooling, examples, and benchmarks |
| R07 | 7 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 5 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **2,459** | **all retained paths** |
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
