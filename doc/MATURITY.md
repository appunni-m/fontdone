# Maturity and adoption

Version **2.14.3-alpha.10** is an early application and migration release,
not a complete FreeType replacement. Pick the interface and exact function
set the application requires before evaluating it.

| Status | Functions | Meaning |
| --- | ---: | --- |
| Complete | 52 | Maintained application contract and mapping complete |
| Implemented, mapping incomplete | 5 | Runtime exists; application mapping unfinished |
| Partial | 29 | Only the specified subset is available |
| Planned | 69 | No complete application behavior claimed |
| Intentionally excluded | 63 | Outside the selected product scope |
| Total | 218 | Pinned public inventory |

The [generated function map](FREETYPE_SUPPORT.md) identifies the exact paths.
These counts are declarations of adoption status. The
[measurement evidence](EVIDENCE.md) validates them against the source map and
reports executed outcomes separately.

## Choose a boundary

The compact Rust `Font` API accepts bytes and produces glyph masks and metrics.
Its string helpers process one scalar; they do not shape or lay out text.
Format support through a low-level memory-face route is not automatically
available through the compact constructor.

The safe Rust FreeType-shaped facade, native C ABI, raw WASM ABI, and npm
wrapper have different integration contracts. A wrapper exposes core behavior;
it does not compensate for missing algorithms. C library names and alpha ABI
compatibility are documented in the [C guide](../fontdone-c-abi/README.md).

## What release evidence proves

The retained full run passed 20,357/20,357 runnable comparisons. Three
undefined-C or safety-extension cases remain pending. All 218 function names
have a runtime route, which may be a narrow validation path.

The retained local C scorecard completes 8/12 categories and 13,624/18,100
runtime contract rows. The local platform bundle covers 1/5 targets; the
[alpha.10 tag CI](https://github.com/appunni-m/fontdone/actions/runs/35002119892)
separately passed all five platform jobs. Do not combine these measurements
into one invented completeness percentage. Cross-host equality of unspecified
SBit fields remains unproven.

Historical source coverage and timing reports retain their older source
revisions. Benchmarks remain in `collecting_baseline` with no accepted
regression thresholds. The [public roadmap](ROADMAP.md) lists the open goals.

## Evaluate your application

List the functions, font formats, variation/color behavior, error recovery,
ownership rules, and target platforms you use. Compare representative licensed
inputs with version-matched FreeType. Include malformed input and lifecycle
behavior when those are part of the application contract.

A successful glyph render does not establish shaping, all color fonts,
all variable fonts, every bitmap format, or safe resource use for arbitrary
untrusted files. Apply application limits and report concrete failures through
the project's contribution and security channels.
