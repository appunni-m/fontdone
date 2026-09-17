# fontdone agent guide

`AGENT.md` links to this file. Edit `AGENTS.md` only.

## Scope and ownership

- `src/` owns font parsing, metrics, hinting, geometry, and rendering. Keep the
  core safe Rust and preserve its `#![deny(unsafe_code)]` boundary.
- `fontdone-c-abi/` and `fontdone-wasm/` adapt core behavior. Keep pointer and
  handle validation, memory ownership, record conversion, and ABI exports here;
  keep font algorithms in core. These are packaging targets, not additional
  published Cargo crates.
- Runtime code must not build, link, or load native FreeType. Use the pinned C
  source only as an offline oracle for comparisons and diagnosis.
- Preserve unrelated changes and existing safety/lint checks. Keep library
  diagnostics in `log` macros; do not commit temporary prints or traces.

## Behavior and evidence

- Fix implementation failures without weakening assertions, thresholds, or the
  selected contract. Do not special-case fixture identities in runtime code.
- For a parity mismatch, compare the same font, glyph, size, flags, and endpoint
  against the pinned oracle. Fix the first divergence and preserve subtle
  reference behavior beside the implementation.
- Maintain inputs under `tests/fixtures/input/` and contracts under `tests/data/`.
  Change generated contracts through `make generate-contracts`. Keep downloaded
  `/freetype/`, generated matrices, and raw oracle outputs out of Git.
- Font assets need source, license, transformation, and hash provenance; follow
  [the fixture policy](scripts/font_generation/README.md).
- Keep executed parity, source coverage, and full C compatibility distinct.
  Preserve failing, pending, and unmeasured cases in reports. A declaration or
  passing subset does not establish complete FreeType replacement.
- Update affected public guides when behavior changes. Keep user guides separate
  from contributor procedures, and retain README acknowledgements.

## Verification and references

Use existing Makefile targets; `make help` and `make help-all` list them.
Direct commands are fine for focused diagnostics or a task without a suitable
target. Add a maintained target when introducing a reusable workflow.

Run checks relevant to the change. Documentation edits use
`make docs-lint check-docs`; site changes also use `make docs-test docs-build`.
Runtime changes use focused tests, `make lint`, and the affected parity lanes before
`make test-parity`. ABI or C data-model changes also need their contract and
platform checks. Benchmark changes use `make bench-self-test bench-quick`.
Broader validation is available through `make ci-fast` and `make ci-thorough`.
Report failures and checks that could not run; do not present historical
measurements as current evidence.

Run `make repository-inventory` after editing tracked files; CI checks the
generated file inventory, including byte sizes.

- [Contributing](CONTRIBUTING.md) and [development](doc/DEVELOPMENT.md): setup,
  test selection, coverage, generated files, and platform checks.
- [Evidence](doc/EVIDENCE.md) and [roadmap](doc/ROADMAP.md): measured results and
  remaining compatibility goals.
- [Benchmarking](doc/BENCHMARKING.md): measurement and budget policy.
- [Releasing](doc/RELEASING.md): package boundaries, version synchronization,
  verification, and tag-triggered publishing.
