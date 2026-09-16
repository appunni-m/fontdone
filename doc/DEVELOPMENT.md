# Development and command reference

Use the repository Makefile for repeatable development. `make help` lists common
commands; `make help-all` exposes specialized parity, fixture, and coverage lanes.

## 1. Environment

### 1.1 Host evidence

| Host or target | What CI proves |
|---|---|
| Ubuntu 24.04 x86-64 | Rust, smoke parity, native C, packaging, and supply chain |
| macOS 14 Apple Silicon | full exact parity, external C scorecard, and instrumented coverage using the canonical oracle host |
| macOS 15 Apple Silicon | fresh checkout, native C, layout, exports, and install tree |
| Windows Server 2025 x86-64 MSVC | native C, LLP64 layout, DLL/import library, exports, and install tree |
| Linux i686 | cross-built and QEMU-executed C consumer/layout contract |
| Linux powerpc64 | cross-built and QEMU-executed big-endian C consumer/layout contract |
| `wasm32-unknown-unknown` on Node 22.14.0 | raw WASM consumer plus packed and installed `fontdone` npm consumer |

Only Ubuntu and macOS are normal pinned-oracle development hosts. Windows and
the cross targets are claimed only to the extent recorded above.
The full source coverage job uses the same macOS host family as exact parity.
FreeType's `ftcsbits.c` allocates SBit nodes with `FT_QNEW` and initializes only
width, height, and buffer for unavailable glyphs. Linux allocation contents
produced 77 comparisons of unspecified fields in alpha.7's coverage run. Those
values are not a portable C contract; the alpha evidence is limited to the
canonical host and does not establish cross-host equality for those fields.
The alpha.9 external C audit independently reproduced three such SBit
comparisons on Ubuntu: uninitialized `format`, `max_grays`, and advance fields
differed while the documented unavailable-bitmap sentinel matched. The
aggregate scorecard therefore uses this same canonical host and still requires
fresh evidence from all five native/QEMU jobs. Cross-host equality for those
unspecified fields remains unproven.
CI validates the browser entry point through Node's standards-compatible ESM,
fetch, and WebAssembly APIs; it does not claim a named-browser version matrix.
Release evidence should add a real-browser run of the maintained HTML example
when browser behavior or asset loading changes.

### 1.2 Required tools

| Tool | Version | Used for |
|---|---:|---|
| Rust | public crate MSRV 1.87; private C/WASM facade floor 1.90; repository toolchain 1.96.1 | runtime and packages |
| Rust nightly | 2026-07-16 in CI | LLVM branch and region coverage |
| cargo-llvm-cov | 0.8.7 | combined coverage report |
| cargo-deny / cargo-audit | 0.20.2 / 0.22.2 | supply-chain policy |
| GNU Make | 3.81 or newer | maintained command interface |
| Python | 3.9 or newer | audits, fixtures, consumers, releases |
| CMake | 3.20 or newer | offline FreeType oracle |
| Clang/GCC/MSVC | C11-capable | oracle and C consumers |
| Git, curl, tar, XZ | maintained OS versions | source and archive handling |
| Node.js and npm | Node 22.14.0 / npm 11.5.1 | raw Wasm, browser-wrapper, and npm archive verification |

Install the pinned supply-chain tools with `make setup-tools` and the coverage
frontend with `make setup-coverage-tools`. Font generators use one pinned
Python environment:

```bash
make setup-font-tools
```

## 2. Build boundary and generated state

Runtime packages are pure Rust. They do not build, link, or dynamically load
FreeType C. `make setup` downloads checksum-pinned FreeType 2.14.3 into ignored
`freetype/`, builds the offline oracle, and refreshes generated public
constants.

| Command | Purpose | Persistent output |
|---|---|---|
| `make build` | Build the root runtime | `target/` |
| `cargo build --workspace --locked` | Build the one public crate and two internal facade targets | `target/` |
| `make setup` | Fetch/build the pinned oracle and constants | ignored `freetype/`, `target/`; generated constants |
| `make generate-contracts` | Regenerate support, C header, WASM, and legal derivatives | tracked generated files |
| `make check-generated` | Reject generated drift | no intended writes |
| `make npm-package-verify` | Build, inspect, install, and execute the JavaScript npm tarball | `target/npm-package/`, release evidence |
| `make fresh-checkout-check` | Exercise non-parity checkout contracts | `target/` |

The first oracle fetch and uncached Cargo dependency resolution need network
access. The compact `Font` API performs no file, network, environment, or
process I/O: callers supply font bytes. The FreeType-shaped safe facade and its
native wrapper deliberately implement path-based `FT_New_Face`/`FT_Attach_File`
and read `FREETYPE_PROPERTIES` for the corresponding compatibility routes.
Runtime packages perform no network access or subprocess execution.

## Validation

| Purpose | Command | Meaning |
| --- | --- | --- |
| Fast tests | `make test-fast` | Rust tests, build checks, release/native tooling |
| Full runtime parity | `make test-parity` | All runnable inputs; pending cases remain named |
| Native FFI guard | `make test-ffi` | No runtime C FreeType shortcut |
| C scorecard | `make c-abi-contract` | Measures all categories and reports unfinished scope |
| Complete C contract | `make c-abi-contract-complete` | Requires full contract and all platform evidence |
| Format / lint | `make fmt` / `make clippy` | Rust source quality |
| Rust documentation | `make doc` / `make doc-test` | Strict rustdoc and executable examples |
| Per-commit CI | `make ci-fast` | Required fast validation |
| Thorough review | `make ci-thorough` | Adds full parity, coverage, benchmarks, packages and supply-chain checks |
| Public site | `make docs-setup`, `make docs-build` | Install locked tools, then build checked HTML |
| Documentation | `make check-docs` | Source, contracts, links, names, and evidence consistency |

Run the narrow case first, then the relevant full gate. Ordinary C-contract
measurement may succeed with unfinished categories; only its complete target
asserts replacement readiness. See [maturity](MATURITY.md).

## Diagnose a parity difference

Reduce to one font, glyph, size, entry point, and operation sequence. Compare
C and Rust at the same pipeline stage. Check input normalization, size scaling,
load flags, metrics, outline geometry, then raster bytes. Preserve error codes
and observable output mutations as well as successful output.

Inspect the pinned C source as a read-only oracle. Fix the first Rust divergence;
never special-case a fixture, narrow the matrix, or edit expected values. Put
subtle reference behavior in a comment beside the implementation so the reason
survives future refactors. Permanent tracing must be guarded `log::trace!`.

## Source coverage

`make test-coverage-all` measures the combined Rust, C-ABI, and host-compiled WASM
lanes with source/configuration-bound build state. It merges distinct raw
profiles and preserves all production-source totals. Preparation and instrumented
build caches have separate identities. A changed source or configuration must
invalidate the relevant cache. `make coverage-clean` removes these states.

Use legitimate public inputs to reach missing regions, including malformed files
where the pinned oracle defines behavior. Fabricated pointers, invalid handles,
undefined C behavior, and uncontrollable allocator failures cannot prove parity.
Do not remove defensive code simply to reduce a coverage denominator.

Collect a fresh report after the source change and retain its raw profiles and
identity. A selected-case hit count is not complete coverage. The complete goal
still requires 100% lines, branches, functions, and regions; alpha release policy
reports the actual incomplete totals. [Evidence](EVIDENCE.md) identifies retained
historical measurements without rebinding them to current source.

## Fixtures and provenance

The canonical input tree currently contains 1,336 tracked paths and no symlinks.
The Makefile exposes 26 named font-generation targets plus the deterministic
compressed-payload target, collected by `make font-fixtures`.

Inputs live in `tests/fixtures/input/`; maintained contracts live in `tests/data/`.
Generated matrices and raw oracle output are ignored and regeneratable. Preserve
licensed input bytes, transformations, hashes, and [font provenance](../tests/fixtures/input/fonts/PROVENANCE.md).
Follow the [generator policy](../scripts/font_generation/README.md).

Run the affected generator target and `make check-font-fixtures` after changing
a generator or input. A new case needs a defined public oracle path and exact
comparison; signature-only or fabricated output checks do not count.

## Repository retention

Keep source, public contracts, deterministic generators, legal/provenance files,
and public guides. Superseded session logs and implementation plans are retained
in Git history. The public roadmap keeps open goals visible.

`make repository-inventory` regenerates the complete path, size, hash, and reason
ledger. `make check-generated` rejects drift. The generated summary below tracks
files, not implementation coverage:

<!-- retention-counts:start -->
| Reason | Paths | Retained context |
|---|---:|---|
| R01 | 58 | published pure-Rust runtime |
| R02 | 110 | package, build, release, and facade contracts |
| R03 | 1,761 | executable parity tests and public contracts |
| R04 | 1,336 | licensed canonical fixture inputs |
| R05 | 1 | required repository tooling alias |
| R06 | 76 | maintained tooling, examples, and benchmarks |
| R07 | 11 | durable project documentation |
| R08 | 1 | active self-cleaning roadmap |
| R09 | 7 | CI, community, and security policy |
| R10 | 2 | generated source required for offline builds |
| R11 | 1 | generated exhaustive inventory |
| **Total** | **3,364** | **all retained paths** |
<!-- retention-counts:end -->

Read [benchmark methodology](BENCHMARKING.md) and [releasing](RELEASING.md) for
measurement and publication workflows.
