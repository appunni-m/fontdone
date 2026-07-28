# Contributing

Thank you for improving `fontdone`. Correctness claims must be reproducible
against pinned FreeType 2.14.3; a smaller diff is preferable to an unexplained
compatibility shortcut.

## 1. Start

Read the [development guide](doc/DEVELOPMENT.md), then:

```bash
make setup
make test-fast
```

Rust 1.87 is the MSRV. `rust-toolchain.toml` pins the repository toolchain used
by primary CI.

## 2. Runtime boundary

- Runtime packages are implemented in Rust. The core `fontdone` crate forbids
  unsafe Rust; the native C and WebAssembly boundary crates isolate the unsafe
  pointer and linear-memory operations required by their raw ABIs.
- FreeType C is an ignored offline oracle only.
- Runtime source must not add native FreeType linking/build hooks, system-font
  discovery, subprocess execution, network access, or hidden C dependencies.
  The compact API remains memory-based; the FreeType-shaped facade may perform
  only its documented path and environment operations.
- Internal engine modules and compact conveniences are not automatically
  FreeType replacement APIs.

## 3. Make a behavioral change

For a mismatch:

1. reduce it to one font, glyph, size, flag set, and endpoint;
2. compare C and Rust at identical pipeline stages;
3. fix the first divergence;
4. run the focused command;
5. run `make test-parity`.

Never remove a case, narrow a matrix, weaken an assertion, edit oracle output,
or special-case a fixture to create a pass. Include before/after parity
denominators in the pull request.

## 4. Change fixtures or generators

Read the [font-generation policy](scripts/font_generation/README.md). Commit
only reviewed inputs, deterministic generators, provenance/license updates,
and maintained contracts. Generated matrices and raw oracle outputs stay
ignored.

Run the affected `make font-fixture-*` target and:

```bash
make check-font-fixtures
```

Do not add or modify third-party fonts without exact source, license,
transformation, and hash evidence.

## 5. Change APIs, docs, CI, or performance

- Public behavior changes update rustdoc, the relevant guide, and
  `CHANGELOG.md`.
- Generated support/header/WASM/legal files change through their generator.
- CI or Makefile changes update `doc/DEVELOPMENT.md`.
- Release changes update `doc/RELEASING.md`.
- Benchmark changes run `make bench-self-test` and `make bench-quick`, and
  report raw samples plus machine/toolchain metadata.

## 6. Before requesting review

Run the gates proportionate to the change:

```bash
make lint
make check-docs
make test-fast
make test-parity
```

Use `make ci` for the complete platform-independent local gate. Explain what
changed, why it is correct, which commands ran, and any remaining debt.
Publication is maintainer-only and follows the
[release guide](doc/RELEASING.md).
