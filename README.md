# fontdone

<div align="center">

**A pure-Rust font engine measured against FreeType 2.14.3.**

[![CI](https://github.com/appunni-m/fontdone/actions/workflows/ci.yml/badge.svg)](https://github.com/appunni-m/fontdone/actions/workflows/ci.yml)
[![License: FTL](https://img.shields.io/badge/license-FTL-blue.svg)](FTL.TXT)
[![MSRV: Rust 1.87](https://img.shields.io/badge/MSRV-1.87-orange.svg)](https://github.com/appunni-m/fontdone/blob/main/rust-toolchain.toml)

</div>

> **Release:** `2.14.3-alpha.1`, not yet published. The project is suitable for
> compatibility development and controlled evaluation, not as an unqualified
> drop-in FreeType replacement.

`fontdone` implements font loading, metrics, hinting, outlines, and
rasterization in Rust. Runtime packages do not build, link, or load FreeType C.
Pinned FreeType source is used only by ignored offline test tooling.

## 1. Choose an integration

| Consumer | Package | Contract | Start here |
|---|---|---|---|
| Rust application | `fontdone` | Compact masks/metrics API | [Rust integration](https://github.com/appunni-m/fontdone/blob/main/doc/INTEGRATION.md#2-compact-rust-api) |
| Rust FreeType migration | `fontdone` | Safe Rust API preserving measured `FT_*` concepts | [Safe migration](https://github.com/appunni-m/fontdone/blob/main/doc/INTEGRATION.md#3-freetype-shaped-safe-rust) |
| C or C-compatible host | `fontdone-c-abi` | Raw pointers, shipped headers, shared/static libraries | [C ABI guide](https://github.com/appunni-m/fontdone/blob/main/fontdone-c-abi/README.md) |
| Node.js host | `fontdone-wasm` | Low-level wasm32 linear-memory ABI | [WASM guide](https://github.com/appunni-m/fontdone/blob/main/fontdone-wasm/README.md) |

The crates are not on crates.io yet. Evaluate from a local checkout while
keeping the version requirement that a publishable downstream package needs:

```toml
[dependencies]
fontdone = { version = "=2.14.3-alpha.1", path = "../fontdone" }
```

After `fontdone` is published, a registry consumer should request the exact
prerelease:

```toml
[dependencies]
fontdone = { version = "=2.14.3-alpha.1" }
```

Cargo requires a version requirement on dependencies of a crate that will be
packaged or published. A Git dependency can be used instead when the exact
release tag is public:

```toml
[dependencies]
fontdone = { git = "https://github.com/appunni-m/fontdone", tag = "v2.14.3-alpha.1" }
```

## 2. Rust quick start

```rust
use fontdone::Font;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read("font.ttf")?;
    let font = Font::truetype(&bytes, 16.0)?;
    let mask = font.getmask("A")?;

    println!(
        "{}x{}, origin=({}, {}), advance={}",
        mask.width, mask.height, mask.xmin, mask.ymin, mask.advance_width
    );
    Ok(())
}
```

The input is copied into owned font data. `GlyphMask::pixels` is owned,
tightly packed, row-major 8-bit coverage. The compact API renders the first
Unicode scalar and does not shape text. See the
[integration guide](https://github.com/appunni-m/fontdone/blob/main/doc/INTEGRATION.md) for formats, units, errors, ownership,
threading, and compiled examples.

## 3. Compatibility status

Compatibility has 3 separate measurements. They must not be combined into one
percentage. Performance is tracked separately and cannot increase a
compatibility score.

### 3.1 Maintained adoption map

The generated [function adoption map](https://github.com/appunni-m/fontdone/blob/main/doc/FREETYPE_SUPPORT.md) classifies all
218 pinned public functions by application-ready contract status:

| Status | Functions | Meaning |
|---|---:|---|
| Complete | 52 | Maintained application behavior and mapping are complete |
| Implemented, mapping incomplete | 5 | Runtime code exists; public mapping is incomplete |
| Partial | 29 | Only the documented behavior is ready |
| Planned | 69 | No application-ready implementation is claimed |
| Intentionally excluded | 63 | Outside the currently declared product surface |
| **Total** | **218** | Pinned FreeType 2.14.3 function inventory |

This is a conservative adoption classification. A planned or excluded function
can still have a declaration, stub, validation route, or focused runtime probe.
That evidence does not make its complete application behavior available.

### 3.2 Last committed runtime evidence

The latest full parity snapshot was recorded on **2026-08-08** (UTC):

| Measurement | Count |
|---|---:|
| Runnable exact-comparison cases | 7,543 |
| Passed cases | 7,543 |
| Failed cases | 0 |
| Explicitly pending cases | 3 |
| Covered manifest cases | 4,193 |
| Validated public API subjects | 1,543 |
| Validated public API input files | 1,537 |
| Logical declared cases | 4,285 |
| Concrete expanded cases | 7,546 |
| Functions with at least one C/Rust/C-ABI/WASM runtime route | 218 / 218 |

`7,543 / 7,543` means every runnable case in that execution matched; the 3
explicitly pending concrete cases are safety-extension exclusions and the route audit still
reports **0 pending parity routes**. Likewise, 218/218 function-route evidence
can be satisfied by a narrow success or null-validation route; it is not
equivalent to complete behavior for every input, state, or platform.

The latest source-bound verification is Coverage MCP parity run
`dd78f134-7fbd-4ea9-a12a-452b193ebfc1`, recorded by
`2be4a36b-a360-489f-8433-14ae5b94c4f2` in
`doc/runtime_parity_evidence.json` after passing 7,543 / 7,543 runnable
comparisons with 0 failures and 3 explicitly pending safety-extension cases.
Its source-bound parity-tree digest is `fc9f242b48c8469abdf9d0676c246e078310a51cb8509c060ec06d241ff5d2a8`.

Run `make test-parity` for current worktree evidence. It writes the full log
and a source-digest-bound report under `target/parity-evidence/`. After a
complete run, `make record-parity-snapshot` copies that report into the
[committed runtime evidence](https://github.com/appunni-m/fontdone/blob/main/doc/runtime_parity_evidence.json)
and updates this table. Recording fails if parity-relevant source changed after
the run. Generated runtime reports under `target/` are newer authority for
their exact worktree than the committed release snapshot.

### 3.3 Last measured combined coverage

The latest source-bound all-lane coverage snapshot was recorded on
**2026-08-08** for commit `d9ac1a39e083fb9dfb8bb3c40ba7b18097e09e19`
(Coverage MCP run `8922c0ae-767c-4d62-8dfe-447d352a5b51`, snapshot
`5c8acf09-fcb7-498d-95d2-a9c2faab5a7a`):

| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 49,976 / 54,387 | 91.89% |
| Branches | 9,930 / 12,592 | 78.86% |
| Functions | 3,410 / 3,848 | 88.62% |
| Regions | 68,750 / 75,614 | 90.92% |

This warm source-bound validation completed in 29.234 seconds with the
default `COVERAGE_UNIFIED_WORKERS=1`. The same-source two-worker comparison
(`e5dd45f6-d1dc-4c1d-a7ee-8ea143b8441d`) took 115.508 seconds, confirming that
extra workers contend inside each instrumented lane instead of reducing the
wall time.

The maintained malformed BDF input
`tests/fixtures/input/fixtures/assets/bdf/missing_font_field.bdf` now includes
one blank line after `STARTFONT`. That exercises Rust's blank-line parser path;
the pinned C reader skips the same byte range before invoking its parser. The
source-bound parity run `1b52e95f-d24b-448e-a966-8b1d121d9a4e` passed 7,542 /
7,542 comparisons with 0 failures, and coverage increased by one line, one
branch, and one region without changing any denominator or function count.
When the instrumented binary cache is cold, the managed run
`90f0239f-93a7-4f66-a758-eb84da5d24b7` took 80.314 seconds, including a
47.59-second instrumented rebuild; its longest lane was 26.45 seconds. This
identifies cache-miss compilation, not MCP ingestion or the C oracle, as the
large delay on cold coverage runs.

The coverage-speed validation on the current worktree set
`COVERAGE_TEST_DEBUG=0`: the cold instrumented profile build fell from about
66 seconds with the previous line-table setting to 49.40 seconds, while the
valid coverage totals and all-lane parity stayed unchanged (Coverage MCP run
`43214315-ba24-44e0-b4f9-fce152052ec5`, snapshot
`f9469846-d77a-4cfa-b80d-011d9ab87456`). A warm confirmation
(`5f1b9ccd-8151-4fc7-9be2-045e3ea7a1e8`, snapshot
`fced0659-ec06-4909-8744-40377010abad`) completed in 29.114 seconds with
lane times of 24.95, 25.04, and 26.49 seconds. The change primarily removes
cold instrumented-link overhead; the three parity lanes remain the dominant
warm cost. LLVM coverage mapping supplies the source locations used by the
report without requiring DWARF line tables.

This is an LLVM branch-coverage measurement across the Rust core, native C
ABI, and host-compiled WASM facade. The 3 explicitly pending cases remain
pending. The maintained `make test-coverage-all` command keeps all workspace
packages in the report but executes only the `unified_fixture_parity`
integration target, whose exact default test name is
`parity_fixture::unified_fixture_parity` and which already drives the Rust,
C-ABI, and host-compiled WASM lanes. This avoids empty root-unit and pipe-trace binaries that can make
LLVM count cfg-dependent FFI source twice without adding a parity input. The
default `COVERAGE_UNIFIED_LANE_SPLIT=1` path builds one instrumented binary,
then runs that binary directly for the Rust FFI, C ABI, and host-WASM lanes in
separate processes with separate raw profile files; the final `cargo llvm-cov
report` merges them. Those raw profiles live under the nested
`COVERAGE_ALL_TARGET_DIR/llvm-cov-target` directory scanned by
`cargo llvm-cov report`; placing them beside that directory would make the
report reuse stale profile data. Reusing the already-built binary avoids reacquiring
Cargo's build lock and repeating the `cargo llvm-cov` test-profile setup three
times. This avoids the LLVM counter contention measured when all three
backends share one instrumented process without changing the parity inputs.
Set
`COVERAGE_UNIFIED_LANE_SPLIT=0` only for the legacy single-process diagnostic
path. The all-lane command uses a dedicated `COVERAGE_ALL_TARGET_DIR` cache and
records a source/configuration state marker; it cleans stale instrumented
workspace artifacts once after a relevant change and retains the fast warm
repeat path afterward.
The
ABI-only package preflight remains available as `make coverage-abi-preflight`,
but the default coverage target does not rerun it because `make test-fast`
already executes that contract before `make ci-thorough`. Optional feature
profiles are verified separately by `make optional-feature-contract`; coverage
also restores all four maintained oracle helpers after a workspace clean and
rebuilds the isolated optional-feature probe bundles only when their artifacts
are missing or stale, so warm runs reuse them.
The newer dirty-worktree validation run `b0194751-8e81-4d73-a17d-d6ae1a636c71`
(`21949c21-abcd-4b97-b5c8-a9badd976487`) passed 7,537 / 7,537 cases in each
backend and measured 49,624 / 54,186 lines, 9,815 / 12,538 branches,
3,391 / 3,835 functions, and 68,297 / 75,365 regions; it is retained in
`doc/DEVELOPMENT.md` and `doc/ROADMAP.md` until a new compatibility snapshot
is deliberately promoted.
The preceding source/input-bound all-lane run `e068f42a-1f89-4504-b438-fe82602f9777`
(`bd628f06-3846-4c7e-8c82-76f26c8d437f`) passed 7,542 / 7,542 comparisons in
each backend and measured 49,861 / 54,382 lines, 9,878 / 12,590 branches,
3,402 / 3,848 functions, and 68,605 / 75,611 regions in 73.636 seconds.
Its exact BDF error branch is recorded at `src/font.rs:1536-1537`; the three-surface
instrumented execution and fresh binary build are the dominant costs; the report
is accepted by Coverage MCP without the compatibility-only segment rewrite. The default
`COVERAGE_NORMALIZE_SEGMENTS=0` therefore skips the measured ~2.9-second `jq`
pass over the 28.6 MB JSON artifact; set it to `1` only for an older LLVM JSON
producer that needs the segment-count clamp. Coverage builds retain the
instrumented target with `cargo llvm-cov --no-clean`, remove stale `.profraw`
files before each measurement, and clear stale workspace artifacts when the
source/configuration state changes. They omit DWARF line tables via
`COVERAGE_TEST_DEBUG=0`; LLVM's coverage mapping remains sufficient for the
report. Face-cache keys now
reuse the preload phase's content digests instead of hashing the same font for
each expanded case, and read-only SFNT table-load/info routes reuse those
content-bound handles; the variation-sequence route remains isolated. Oracle
preparation now also preserves generated-file mtimes
when contents are unchanged, so the C helper and FreeType validator overlay are
not rebuilt on every run; unchanged FreeType CMake configuration is reused as
well. Run `make coverage-clean` after changing coverage
instrumentation or profile configuration.

The preceding source/input-bound coverage run completed in 73.636 seconds,
including the 40.07-second instrumented rebuild; its same-source warm repeat
completed in 29.156 seconds;
the preceding managed warm source-bound coverage run completed in 50.842 seconds;
the preceding source-bound run completed in 100.333 seconds including the
instrumented rebuild; its longest backend execution was 48.24 seconds;
the preceding managed source-bound run completed in 98.336 seconds; the
preceding managed source-bound run completed in 52.387 seconds; the
preceding managed warm source-matched coverage run completed in 51.362 seconds;
the preceding managed run completed in 51.347 seconds, while the first
source-bound run after the code change took 99.254 seconds because it rebuilt
the instrumented binary. The prior execution-only warm measurement with the
instrumented binary and expanded-input cache warm was 50.482 seconds, and the
prior warm committed baseline was 51.991 seconds. The
repository default remains `COVERAGE_TEST_OPT_LEVEL=1`; use an
explicit level-3 override only for comparison. `make coverage-clean` is now
safe before a run: the build-only coverage step uses `--no-report -- --list`
so it cannot attempt to merge profiles before the three backend lanes execute.

Coverage is a code-execution signal, not a compatibility score. These
percentages apply only to the named source commit, suite, and toolchain. Run
`make test-coverage-all` again after source changes. The exact machine-readable
measurement provenance is retained in the compatibility snapshot.

### 3.4 C ABI completion contract

The latest committed scorecard has **10 / 12 categories complete**:

| Category group | Status |
|---|---|
| Functions | 218 / 218 functions without unresolved subject routes; 218 / 218 names, signatures, and traced function routes; 5,285 / 5,285 pinned-C runtime contract rows exact |
| Constants, types, layouts, callbacks | Complete under their blocking scorecard measurements |
| Ownership | Complete under the current scorecard measurements |
| State, modules, headers | Complete under their blocking scorecard measurements |
| Errors | 684 / 684 expected-error routes compare exact error and output results; 7,542 / 7,542 routes have no generic fallback evidence |
| Binary/install artifacts | 7 / 8; Windows import-library evidence pending |
| Platform behavior | 1 / 5 fresh target bundles; Linux x86-64, Windows x86-64, Linux i686, and Linux powerpc64 pending |

Only `make c-abi-contract-complete` is the full-contract pass condition. The
ordinary `make c-abi-contract` command intentionally succeeds while reporting
remaining debt. Unresolved function-subject routes and incomplete expected-error
routes remain even when every bare function name has some traced route. The self-cleaning
[completion roadmap](https://github.com/appunni-m/fontdone/blob/main/doc/ROADMAP.md) defines the exact 12-category goal.

The committed machine-readable snapshot is
[`doc/compatibility_snapshot.json`](https://github.com/appunni-m/fontdone/blob/main/doc/compatibility_snapshot.json).
The latest scorecard run is Coverage MCP run
`a550bca3-4804-4ae5-8f59-12cb1fc5997d`.

### 3.5 Performance baseline

The maintained release-mode benchmark measures per-operation latency and
throughput, complete-process peak RSS, and exact unstripped release-artifact
bytes against pinned FreeType. Correctness mismatches fail before a
measurement can qualify.

<!-- performance-baseline:start -->
The committed ledger contains **5 / 5 clean runs**
for its most-sampled current environment. Five runs from the same environment
are required before regression thresholds can be reviewed.

| Latest clean measurement | Value |
|---|---:|
| Source commit | `38bba241c1d3df4cb9bfcadc681caf6fb1308476` |
| Samples | 10 |
| Weighted latency speedup versus C | 0.324x |
| Total throughput ratio versus C | 0.428x |
| Median peak-RSS ratio versus C | 3.415x |
| Shared-library byte-size ratio versus C | 2.396x |
| Fontdone WASM size | 1,245,397 bytes |

The regression policy is `collecting_baseline`. `make bench-regression`
therefore fails closed until reviewed thresholds become active.
<!-- performance-baseline:end -->

Run `make bench` to generate a ten-sample report under
`target/fontdone-bench/`. From a clean source commit, run
`make record-performance-baseline` to append it to the committed ledger.
Performance evidence is machine- and environment-specific; results from
different environment identities are never pooled toward the five-run
threshold-review minimum.

## 4. What is implemented

The Rust runtime contains:

- SFNT, TrueType, CFF/CFF2, Type 1/Type 42, BDF, PCF, PFR, WinFNT, and
  collection-facing load paths where listed by the adoption map;
- TrueType bytecode hinting and Latin/CJK auto-hinting infrastructure;
- smooth, mono, LCD, LCD-V, SDF, embedded-bitmap, outline, and selected color
  and SVG routes;
- metrics, charmaps, variation data, names, kerning, glyph objects, outlines,
  stroking, caches, streams, validators, and module-state compatibility paths;
- safe Rust, native C, and WebAssembly facades backed by the same Rust core.

This list describes components, not universal format or API completeness.
Always use the adoption map and exact parity cases for a compatibility claim.

## 5. Repository map

```text
src/                  pure-Rust engine and safe APIs
fontdone-c-abi/       native C artifact, headers, and C example
fontdone-wasm/        wasm32 ABI, schema, declarations, and Node example
tests/data/           maintained, non-generated contracts
tests/fixtures/input/ tracked font and auxiliary inputs
scripts/font_generation/
                      deterministic font generators
doc/                  integration, development, release, status, and roadmap
```

Runtime flow:

```text
font bytes -> table parsing -> scaling -> native/auto hinting -> rasterization
                                                        |
                                      Rust / C ABI / WASM observations
```

## 6. Build and verify

Requirements and host support are documented in
[`doc/DEVELOPMENT.md`](https://github.com/appunni-m/fontdone/blob/main/doc/DEVELOPMENT.md).

```bash
make setup       # fetch and build the pinned offline C oracle
make test-fast   # workspace tests that do not need full parity
make test-parity-smoke # eight exact runtime cases across every facade
make test-parity # exact C/Rust/C-ABI/WASM parity
make lint        # rustfmt and Clippy
make doc-test    # compile public Rust examples
make ci-fast     # exact fast per-commit local gate (make ci is an alias)
```

Important complete gates:

| Command | Contract |
|---|---|
| `make api-abi-audit` | Parse the pinned public declarations and local surfaces |
| `make c-abi-contract` | Report every C-contract numerator, denominator, and debt item |
| `make c-abi-contract-all-platforms` | Validate five target bundles and report current C-contract debt |
| `make c-abi-contract-complete` | Fail unless all 12 C-contract categories complete |
| `make test-integrations` | Run downstream Rust, external C, exports, and Node/WASM consumers |
| `make check-docs` | Check every tracked Markdown document, status snapshot, links, commands, and rustdoc policy |
| `make bench-regression` | Fail unless reviewed latency, throughput, memory, and size thresholds all pass |
| `make ci-thorough` | Run the requested local pre-merge full parity, coverage, performance, contract, package, and supply-chain gate |
| `make release-verify` | Run local release gates; requires assembled five-target platform evidence |

`make help` is the maintained command index.

## 7. Fixtures and licensing

Tracked input fixtures remain under `tests/fixtures/input/`. Generated matrices
and oracle outputs are ignored. Font-generation code is isolated under
`scripts/font_generation/` so its deterministic inputs, output classification,
and licensing can be reviewed separately.

Read:

- [fixture notices](https://github.com/appunni-m/fontdone/blob/main/tests/fixtures/THIRD_PARTY_NOTICES.md);
- [font provenance](https://github.com/appunni-m/fontdone/blob/main/tests/fixtures/input/fonts/PROVENANCE.md);
- [font-generation policy](https://github.com/appunni-m/fontdone/blob/main/scripts/font_generation/README.md).

The root FreeType License does not relicense third-party font inputs.

## 8. Project policies

- [Documentation map](https://github.com/appunni-m/fontdone/blob/main/doc/README.md)
- [Contributing](https://github.com/appunni-m/fontdone/blob/main/CONTRIBUTING.md)
- [Security](https://github.com/appunni-m/fontdone/blob/main/SECURITY.md)
- [Code of conduct](https://github.com/appunni-m/fontdone/blob/main/CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

`fontdone` is distributed under the FreeType Project License:
[`LICENSE`](LICENSE), [`FTL.TXT`](FTL.TXT), and [`NOTICE.md`](NOTICE.md).
