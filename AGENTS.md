# fontdone agent guide

`AGENT.md` is a symlink to this file. Edit this file only.

## Goal

Replace pinned FreeType 2.14.3 C runtime behavior with pure Rust while
preserving measured functions, constants, records, lifecycle behavior, errors,
metrics, geometry, and rendered bytes.

## Rules

- Runtime code must not build, link, or load FreeType C. C is an offline oracle
  for fixtures, diagnostics, and comparison only.
- Fetch ignored oracle source with `make oracle-fetch`; never commit
  `/freetype/`.
- Track maintained inputs under `tests/fixtures/input/` and contracts under
  `tests/data/`. Generated matrices and raw outputs remain ignored.
- Never weaken a test, fixture matrix, expected result, threshold, or filter to
  obtain a pass. Fix the first behavioral divergence.
- Keep permanent diagnostics behind `log::trace!`; do not commit temporary
  prints.
- Do not equate a declaration, stub, validation route, or compact helper with
  complete FreeType behavior. Report adoption status, executed parity, and the
  12-category C contract separately.
- Regenerate derived contracts with `make generate-contracts`; do not edit
  generated support, header, WASM, or license files by hand.
- Update the relevant guide whenever a public contract, command, fixture,
  benchmark, CI lane, or release step changes.

## Workflow

Start with the narrowest relevant test. Before handing off a change, run the
proportionate public gates:

```bash
make test-fast
make test-parity
make c-abi-contract
make lint
make check-docs
```

`make ci-fast` is the local equivalent of the required per-commit GitHub gate
(`make ci` is an alias). Run `make ci-thorough` only for a requested pre-merge audit; it adds full
coverage, C-contract, package, supply-chain, and ten-sample benchmark evidence.
Use `make c-abi-contract-complete` only after all five platform bundles have
been assembled. Benchmark changes also require `make bench-self-test` and
`make bench-quick`.

For a parity failure: reduce to one font, glyph, size, and endpoint; compare C
and Rust at the same pipeline stages; find the first divergence; read the
pinned C implementation; fix the Rust cause; rerun the focused lane and then
the full matrix.

Put durable implementation nuance beside the relevant code. Put integration,
development, and release procedure in `doc/`, not in temporary status notes.
