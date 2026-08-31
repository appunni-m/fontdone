# Changelog

All notable user-visible changes are recorded here. This project uses one
synchronized version for the `fontdone`, `fontdone-c-abi`, and `fontdone-wasm`
Cargo crates and the `fontdone` browser npm package.

## 2.14.3-alpha.1 (Unreleased)

First standalone alpha targeting FreeType 2.14.3.

### Added

- Pure-Rust font loading, table parsing, scaling, TrueType bytecode hinting,
  multi-script auto-hinting, outlines, metrics, and rasterization.
- Compact Rust masks/metrics API and a safe FreeType-shaped Rust facade.
- Native `fontdone-c-abi` package with C/C++ headers, shared/static artifacts,
  `pkg-config` metadata, install layout, and maintained external-C consumers.
- Browser npm package `fontdone` with a prebuilt Wasm asset, typed ESM
  lifecycle wrapper, package-level tests, and browser and Node examples.
- Low-level `fontdone-wasm` crate with a generated ABI schema, TypeScript
  declarations, and a Node 20 raw consumer.
- Exact C-oracle parity harness spanning Rust, native C, external C, and WASM,
  with runnable, failed, pending, manifest, and route measurements kept
  separate and committed snapshots bound to the exact tested source digest.
- Twelve-category C compatibility scorecard covering declarations, layouts,
  ownership, state, errors, modules, headers, artifacts, and platform behavior.
- Deterministic, license-reviewed font generators and a canonical tracked input
  boundary; generated oracle matrices and outputs remain uncommitted.
- Strict callable-surface rustdoc, rustfmt, Clippy, MSRV, integration, package
  link/provenance, supply-chain, benchmark, repository-retention, and
  synchronized-release automation.

### Current compatibility boundary

- The maintained application adoption map classifies 52 of 218 pinned public
  functions complete, 5 implemented with incomplete mapping, 29 partial, 69
  planned, and 63 intentionally excluded.
- The last committed full runtime evidence passes 7,212 of 7,212 runnable
  comparisons with 0 failures and 95 explicitly pending cases.
- The last measured combined core, C-ABI, and host-compiled WASM run reports
  45,547/50,898 lines (89.49%), 8,986/11,915 branches (75.42%),
  3,112/3,585 functions (86.81%), and 63,052/71,420 regions (88.28%) on
  commit `e554aca48fb3168fa852dd79267f50d06201e1e4`.
- Every pinned function has at least one runtime route, but route evidence does
  not imply every success path or lifecycle is complete.
- The C contract is 9 of 12 categories complete. Its function category is
  blocked by 83 pending runtime contract rows; Windows import-library evidence
  and four of five assembled platform bundles also remain pending.

This prerelease is for compatibility development and controlled evaluation.
It is not an unqualified drop-in FreeType replacement, and no compatibility is
promised between different alpha releases.
