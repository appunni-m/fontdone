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

The last committed full parity snapshot was recorded on **2026-08-02**:

| Measurement | Count |
|---|---:|
| Runnable exact-comparison cases | 7,476 |
| Passed cases | 7,476 |
| Failed cases | 0 |
| Explicitly pending cases | 3 |
| Covered manifest cases | 4,183 |
| Validated public API subjects | 1,543 |
| Validated public API input files | 1,537 |
| Logical declared cases | 4,268 |
| Concrete expanded cases | 7,479 |
| Functions with at least one C/Rust/C-ABI/WASM runtime route | 218 / 218 |

`7,476 / 7,476` means every runnable case in that execution matched; the 3
explicitly pending concrete cases are safety-extension exclusions and the route audit still
reports **0 pending parity routes**. Likewise, 218/218 function-route evidence
can be satisfied by a narrow success or null-validation route; it is not
equivalent to complete behavior for every input, state, or platform.

The latest source-matched verification is Coverage MCP parity run
`637ad827-5c8b-41ce-8008-797aee2d6a21`, recorded by run
`7faa8303-f766-45ec-9307-c936fafd5308` against the worktree at commit
`53995d32008605b8abe6a15db477c86881c929c9`; its source-bound parity-tree
digest is `7a5ebc1661978c5edc28367b9b23055c9c633634351eff7a74b1f195714c0eb4`.

Run `make test-parity` for current worktree evidence. It writes the full log
and a source-digest-bound report under `target/parity-evidence/`. After a
complete run, `make record-parity-snapshot` copies that report into the
[committed runtime evidence](https://github.com/appunni-m/fontdone/blob/main/doc/runtime_parity_evidence.json)
and updates this table. Recording fails if parity-relevant source changed after
the run. Generated runtime reports under `target/` are newer authority for
their exact worktree than the committed release snapshot.

### 3.3 Last measured combined coverage

The last all-lane coverage run was recorded on **2026-08-02** against
`53995d32008605b8abe6a15db477c86881c929c9`:

| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 49,341 / 54,104 | 91.20% |
| Branches | 9,688 / 12,512 | 77.43% |
| Functions | 3,371 / 3,828 | 88.06% |
| Regions | 67,921 / 75,273 | 90.23% |

This is an LLVM branch-coverage measurement across the Rust core, native C
ABI, and host-compiled WASM facade. The 3 explicitly pending cases remain
pending. The maintained `make test-coverage-all` command keeps all workspace
packages in the report but executes only the `unified_fixture_parity`
integration target, which already drives the Rust, C-ABI, and host-compiled
WASM lanes. This avoids empty root-unit and pipe-trace binaries that can make
LLVM count cfg-dependent FFI source twice without adding a parity input. The
all-lane command uses a dedicated `COVERAGE_ALL_TARGET_DIR` cache, so its
`--no-clean` reuse cannot mix stale binaries from another coverage target. The
ABI-only package preflight remains available as `make coverage-abi-preflight`,
but the default coverage target does not rerun it because `make test-fast`
already executes that contract before `make ci-thorough`. Optional feature
profiles are
verified separately by `make optional-feature-contract`.
The latest Coverage MCP run is
`1a9ca419-0e1f-41a4-8c9f-eba21a2e385b`, with snapshot
`a8ef34a9-b6f4-47ad-ad76-039d7ee9bce6`; the warm run completed in 1 minute
53.998 seconds, with 7,476 exact parity comparisons. Its runtime backend
timings were about 39.52 seconds Rust FFI, 29.74 seconds C ABI, 30.00
seconds WASM, and 0.03 seconds comparison. A cold current-commit run took
2 minutes 20.153 seconds, including a 24.70-second instrumented rebuild. The
separately measured removal of the duplicate ABI preflight from the default
coverage path accounted for 42.627 seconds (18.7%) of the improvement over
the earlier 3:48.279 sample. The remaining wall-time tail is
setup/build/reporting/ingestion and host contention; Coverage MCP does not
currently expose timestamps for those sub-phases. Coverage builds retain the
instrumented target with `cargo llvm-cov --no-clean`, remove stale `.profraw`
files before each measurement, and retain line tables while omitting full test
debuginfo via `COVERAGE_TEST_DEBUG=1`. Face-cache keys now
reuse the preload phase's content digests instead of hashing the same font for
each expanded case, and read-only SFNT table-load/info routes reuse those
content-bound handles; the variation-sequence route remains isolated. Oracle
preparation now also preserves generated-file mtimes
when contents are unchanged, so the C helper and FreeType validator overlay are
not rebuilt on every run. The latest backend timings are about 39.5 seconds
Rust FFI, 29.7 seconds C ABI, and 30.0 seconds WASM; the remaining dominant
cost is the required instrumented three-surface parity execution. Run
`make coverage-clean` after changing coverage instrumentation or profile
configuration.

Coverage is a code-execution signal, not a compatibility score. These
percentages apply only to the named source commit, suite, and toolchain. Run
`make test-coverage-all` again after source changes. The exact machine-readable
measurement provenance is retained in the compatibility snapshot.

### 3.4 C ABI completion contract

The latest committed scorecard has **10 / 12 categories complete**:

| Category group | Status |
|---|---|
| Functions | 218 / 218 functions without unresolved subject routes; 218 / 218 names, signatures, and traced function routes; 5,219 / 5,219 pinned-C runtime contract rows exact |
| Constants, types, layouts, callbacks | Complete under their blocking scorecard measurements |
| Ownership, state, modules, headers | Complete under their blocking scorecard measurements |
| Errors | 647 / 647 expected-error routes compare exact error and output results; 7,476 / 7,476 routes have no generic fallback evidence |
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
`71dbd4b1-c82d-4185-9e03-4ff4cfd4f0d0`.

### 3.5 Performance baseline

The maintained release-mode benchmark measures per-operation latency and
throughput, complete-process peak RSS, and exact unstripped release-artifact
bytes against pinned FreeType. Correctness mismatches fail before a
measurement can qualify.

<!-- performance-baseline:start -->
The committed ledger contains **2 / 5 clean runs**
for its most-sampled current environment. Five runs from the same environment
are required before regression thresholds can be reviewed.

| Latest clean measurement | Value |
|---|---:|
| Source commit | `7bff73b5038121fac5236afdd0f5feb65d54f35c` |
| Samples | 10 |
| Weighted latency speedup versus C | 0.326x |
| Total throughput ratio versus C | 0.446x |
| Median peak-RSS ratio versus C | 3.398x |
| Shared-library byte-size ratio versus C | 2.330x |
| Fontdone WASM size | 1,191,146 bytes |

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
