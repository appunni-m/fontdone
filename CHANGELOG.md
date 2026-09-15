# Changelog

All notable user-visible changes are recorded here. This project uses one
synchronized version for the public `fontdone` Cargo crate, the native C SDK,
and the `fontdone` JavaScript npm package. The C and raw-WASM Cargo packages are
internal workspace build targets.

## 2.14.3-alpha.9 (2026-09-15)

- Carry the alpha.8 native C-width fixes through the full platform release gates.
- Preserve target linkers and host proc-macro search paths in cross-platform
  export and record-layout audits.
- Normalize Windows header paths, preserve strict Clang diagnostics, and keep
  source bytes consistent across checkouts while retaining exact font inputs.
- Compare complete Windows symbol names so Rust type names inside mangled
  symbols cannot be mistaken for undocumented C exports; keep DLL export
  names distinct from their optional implementation-alias annotations.
- Isolate the Windows static-linker library probe so it cannot replace the
  native DLL after the C consumer records its artifact hash.
- Execute and aggregate all five native/QEMU contracts on main before tagging.
- Restore platform evidence at its archive root in CI and release preflight,
  and retain case IDs and value differences for aggregate scorecard failures.
- Publish the one Cargo crate, native SDK, and npm package only through GitHub OIDC.

## 2.14.3-alpha.8 (Superseded; unpublished)

- Fix facade conversions between platform-width C `long` and the engine's
  64-bit arithmetic. This removes 32-bit Linux and Windows compilation failures
  without changing the public C record widths.
- Use FreeType's signed `FT_Char` for the internal stem-darkening sentinel,
  including PowerPC targets where plain C `char` is unsigned.
- Preserve WinFNT reserved record bytes across native word widths and byte orders.
- Require i686, Windows MSVC, and PowerPC64 C ABI compilation on every main CI
  run, in addition to the existing executed platform contract matrix.
- Run instrumented coverage on the canonical macOS oracle host, matching full
  parity; retain all inputs and disclose the unavailable-SBit field limitation.

## 2.14.3-alpha.7 (Superseded; unpublished)

- Release Cargo and npm exclusively through GitHub OIDC after successful exact
  tag CI, using checksums to bind the published artifacts to verified packages.
- Accept measured but incomplete C-contract and source coverage for alpha
  releases. All executed parity, consumer, platform, and package checks remain
  required; the complete-contract gate stays available for the full replacement.
- Include the post-alpha.6 cache-sentinel and malformed COLR/C30 parity fixes,
  optional-feature coverage preparation, and portable Windows fixture paths.
- Reuse CI package evidence without repeating the entire test/coverage matrix
  in the publishing job; preserve old tags as immutable failure history.

## 2.14.3-alpha.6 (Superseded; unpublished)

This superseded candidate kept the complete tag-triggered
parity, coverage, performance, and five-platform C-contract matrix while making
hosted dependency preparation, cross-target diagnostics, Windows checkout, and
release-tag validation deterministic. It verifies the immutable tag through the
GitHub ref API before publishing synchronized Cargo, native SDK, and npm
artifacts, and only after every required gate succeeds.

## 2.14.3-alpha.5 (Superseded; unpublished)

The alpha.5 tag verified the source and synchronized versions locally, but its
release preflight still assumed that the hosted checkout materialized an
annotated tag ref. The tag-triggered CI run remains immutable diagnostic history;
no synchronized release artifacts were published.

## 2.14.3-alpha.4 (Superseded; unpublished)

The alpha.4 tag exposed hosted-runner issues in the release preflight,
dependency cache warm-up, cross-target contract jobs, all-lane coverage, and
Windows checkout. No synchronized release artifacts were published from that
candidate; its tag and CI records remain immutable diagnostic history.

## 2.14.3-alpha.3 (Published Cargo; npm pending)

Previous release candidate containing the post-alpha.2 parity, cache-ownership,
sbix error, generated-inventory, and release-evidence fixes. It keeps one
public Cargo crate; the C ABI and raw-WASM packages remain internal build
targets and ship through the native SDK and JavaScript npm package.

## 2.14.3-alpha.2 (Superseded local candidate)

First standalone alpha targeting FreeType 2.14.3.

### Added

- Pure-Rust font loading, table parsing, scaling, TrueType bytecode hinting,
  multi-script auto-hinting, outlines, metrics, and rasterization.
- Compact Rust masks/metrics API and a safe FreeType-shaped Rust facade.
- Native C SDK archive built from the internal `fontdone-c-abi` target, with
  C/C++ headers, shared/static artifacts, `pkg-config` metadata, install
  layout, and maintained external-C consumers.
- JavaScript npm package `fontdone` with a prebuilt Wasm asset, typed ESM
  lifecycle wrapper, package-level tests, and browser and Node examples.
- Raw `fontdone-wasm` build target with a generated ABI schema, TypeScript
  declarations, and a Node 20 consumer for the public npm package.
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
- The latest committed full runtime evidence passes 20,355 of 20,355 runnable
  comparisons with 0 failures and 3 explicitly pending safety-extension cases.
- The last committed combined core, C-ABI, and host-compiled WASM coverage
  snapshot reports 63,740/65,887 lines (96.74%), 11,580/13,412 branches
  (86.34%), 3,689/3,976 functions (92.78%), and 87,946/91,700 regions
  (95.91%) on source commit `e8c51cb6dba42fd524d94940673fe6b380411d46`.
- Every pinned function has at least one runtime route, but route evidence does
  not imply every success path or lifecycle is complete.
- The C contract is 6 of 12 categories complete. Its runtime contract has
  13,572 of 18,098 rows complete; the Windows import-library evidence and four
  of five assembled platform bundles also remain pending.

This prerelease is for compatibility development and controlled evaluation.
It is not an unqualified drop-in FreeType replacement, and no compatibility is
promised between different alpha releases.
