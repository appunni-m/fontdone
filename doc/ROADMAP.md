# FreeType 2.14.3 parity, coverage, and performance roadmap

Status: **ACTIVE**

Owner: repository maintainers

Open goals: **3**

Deletion condition: goals G01 through G03 are complete, their durable evidence
remains reachable from maintained documentation, this file is removed from
`doc/README.md`, and this file is deleted. The repository-retention audit
rejects a retained roadmap with zero open goals.

This is the repository's one active implementation plan. It does not claim
completion from source-code coverage, a familiar function name, a passing null
check, or a route that silently executes another frontend.

## 0. Copy/paste execution prompt

> Complete fontdone's pinned FreeType 2.14.3 contract. Treat
> `target/api-abi-audit/route_audit.json` as authoritative and close every
> pending route. For each route, read the pinned C declaration/implementation,
> add only reviewed and licensed inputs when necessary, implement the pure-Rust
> behavior, and compare one identical input through pinned C, Rust FFI,
> `fontdone-c-abi`, and WASM. Keep the route Make-executable; run the focused
> case, `api-abi-check`, `c-abi-contract`, `fmt`, `clippy`, and `check-docs`,
> commit the milestone, then rerun full `make test-parity`. After pending
> routes reach zero, run `make test-coverage-all` and raise lines, branches,
> functions, and regions to 100%; run `make record-performance-baseline` and
> `make bench-regression` to record and enforce the performance baseline.
> Finish only when all runnable parity cases pass, all 218 functions have
> four-lane evidence, all 12 C-contract categories are exact,
> `make c-abi-contract-complete` passes, evidence is source-bound, and the
> worktree is clean. Never use C at runtime, weaken or hide expectations,
> publish packages, or create issues. Report exact counts, commands, commits,
> and evidence IDs after every milestone.

## 1. Goal G01: complete the pinned C contract

State: `OPEN`

`fontdone-c-abi` is complete only when all functions, constants, types,
layouts, callbacks, ownership rules, state transitions, errors, supported
modules, headers, binary artifacts, and platform behaviors match the pinned
FreeType 2.14.3 public C contract.

The package may retain the `fontdone-c-abi` name. The compatibility target is
strict: an external C or C++ consumer must be able to compile, link, and run
against the shipped headers and artifacts without FreeType C at runtime.

## 2. Automated verifier

Run:

```bash
make c-abi-contract
```

That target performs 7 stages:

1. regenerates the pinned public API/ABI and route audits and validates the
   fixed contract inventories;
2. compiles the shipped C declarations against pinned declarations and probes
   the actual Rust `repr(C)` records with `size_of`, `align_of`, and
   `offset_of`;
3. builds the same maintained C oracle source once against pinned FreeType and
   once against the real `fontdone-c-abi` release library;
4. runs deterministic same-input cases through both executables and records
   per-call trace evidence, so a synthetic result cannot count as execution of
   its subject function;
5. compiles, links, and runs the maintained external C consumer independently
   against the shared and static artifacts and compares both export sets;
6. measures every fixed ownership, state, module, optional-component,
   artifact, and target-lane item without an open denominator;
7. writes the exact category scorecard to
   `target/api-abi-audit/c_abi_contract_status.json` and
   `target/api-abi-audit/c_abi_contract_status.md`.

The focused inventory-structure gate is:

```bash
make check-c-contract-inventory
```

It rejects missing or duplicate inventory identities, route references that do
not exist, default-module drift from pinned `ftmodule.h`, unknown artifact
probes, and incomplete target-lane definitions. Its maintained authority is
`tests/data/c_contract_inventory.json`; generated pass/fail evidence stays
under `target/api-abi-audit/`.

The platform mechanism is also a Make contract:

```bash
make platform-contract
make check-platform-contract
```

`platform-contract` measures the active native target and writes
`target/api-abi-audit/platform-contract/<rust-target>.json`. The bundle contains
the observed OS, integer data model, endianness, exact 78-record Rust/C layout
result, independently executed shared/static/install-tree C consumers, and
exact shared/static 218-symbol export ledgers. Its source digest and artifact
hashes make copied or stale evidence fail closed. `check-platform-contract`
scans downloaded or locally produced bundles and succeeds only when every one
of the 5 fixed target lanes has one fresh, non-duplicate, exact bundle and all
5 CI markers remain configured. The Windows bundle must also contain exact,
hash-bound DLL import-library evidence from both build-tree and staged-install
consumer execution. Native Linux, macOS, and Windows use `platform-contract`;
Linux i686 ILP32 and powerpc64 big-endian use `platform-contract-cross` with
target GCC, target `nm`, a target sysroot, and QEMU. The target C consumer and
Rust record-layout probe must execute under that runner; a successful
cross-build alone cannot produce a bundle.

The independently runnable stage is:

```bash
make external-c-abi-audit
```

It writes
`target/api-abi-audit/external_c_function_ledger.json`. A function earns C01.4
evidence only when a maintained function-subject case or an explicit direct ABI
probe enters that exact symbol through the C executable linked to Fontdone and
the normalized result exactly matches the same input run through pinned
FreeType. The Make target also fails unless every selected unique case and
every explicit probe is exact; a different passing case for the same function
cannot conceal a mismatch or crash. The ledger records the source, executable,
and linked-library hashes and is rejected when any identity is stale. Candidate
selection is deterministic, deduplicates identical argv, prefers real
non-error parity rows, and retains at most 12 candidates per function by
default. This bound measures function-entry evidence only; it neither narrows
`make test-parity` nor substitutes for full semantic-route coverage.

Compile-time feature behavior is also executable evidence. For example:

```bash
make optional-feature-contract
```

That target builds isolated pinned-FreeType, Rust FFI, native C artifact, and
WASM-host variants with LZW, bzip2, and color-layer support disabled. It also
builds a separate subpixel-rendering-enabled configuration and verifies all 7
LCD setter routes, including stored presets, error-state preservation, the
five-byte copy boundary, and the geometry API's mutually exclusive feature
branch. The default configuration separately proves both unavailable
LCD-filter setters across 5 live/null-library and valid/null-weight scenarios.
It compares 4/4 lanes and records build, header, library, executable, and
normalized-output hashes in generated ledgers. The color-layer probe also
executes all 3 palette stubs against a CPAL-backed face and proves
caller-output preservation. The ordinary API/ABI and aggregate contract
targets depend on it, so a result from the normal build cannot satisfy an
alternate-configuration proof.

The status command succeeds when its evidence is internally consistent, even
while work remains. The release-strength command is:

```bash
make c-abi-contract-complete
```

It fails unless all 12 categories are complete. A category is complete only
when every blocking measurement has a closed, non-zero denominator and its
numerator equals its denominator. `? / ?` is therefore a failure, not a
wildcard. C01.7 additionally requires every non-compile pinned-C runtime
contract row to have exact route evidence, so pending record, constant, and
composite-operation behavior cannot hide behind 218/218 bare function routes.
CI assembles the five platform bundles and invokes this same Make target;
`release-verify` also depends on it.

## 3. Initial measured baseline

Baseline date: **2026-07-29**

Baseline command: `make c-abi-contract`

Baseline result: **0 / 12 categories complete**

This table records the initial debt. Generated JSON from Section 2 is the
current authority after work begins.

| ID | Contract category | Initial automated measurement | Required result |
|---|---|---:|---:|
| C01 | Functions | 28 / 218 semantically complete; 188 / 218 header names; signatures and independent external-C execution unmeasured | every function measurement 218 / 218 and every maintained pinned-C runtime row exact |
| C02 | Constants | 3 / 891 macro names; 9 / 158 enum value names; C-expression equivalence unmeasured | 891 / 891 macros, 158 / 158 enum values, and 1,049 / 1,049 equivalent values |
| C03 | Types | 36 / 62 typedef names; 1 / 20 enum type names; source/ABI equivalence unmeasured | 62 / 62 typedefs, 20 / 20 enums, and 82 / 82 equivalent definitions |
| C04 | Layouts | 50 / 78 records match field names/order; size, alignment, and offsets unmeasured | 78 / 78 on every supported ABI |
| C05 | Callbacks | 11 / 39 directly parsed callback names; exact signatures and final callback denominator unmeasured | closed inventory and every callback exact |
| C06 | Ownership rules | 35 ownership-labelled routes discovered; denominator unclosed | closed inventory and every rule exact |
| C07 | State transitions | 45 lifecycle/state-labelled routes discovered; denominator unclosed | closed inventory and every transition exact |
| C08 | Errors | 1 / 119 header names; 465 / 631 exact error-and-output routes; 7,296 / 7,298 routes free of generic fallback | 119 / 119 names, 631 / 631 exact routes, and 7,298 / 7,298 fallback-free routes |
| C09 | Supported modules | 4 / 32 interface-map paths have all functions complete; authoritative module denominator unclosed | closed inventory and every supported module exact |
| C10 | Headers | 0 / 47 drop-in header paths; all-header C/C++ compilation unmeasured | 47 / 47 paths compile in C and C++ |
| C11 | Binary artifacts | 2 / 2 library kinds configured; 0 / 2 are drop-in complete; install/link/package denominator unclosed | every artifact and packaging contract exact |
| C12 | Platform behaviors | 2 / 3 named native OS families in C-consumer CI; target/ABI matrix unclosed | 3 / 3 named OS families and every inventoried ABI exact |

The directly parsed callback count is a discovery count, not the final
denominator. Callback aliases and macro-exposed callback names must be resolved
by the authoritative Clang-based inventory before C05 can complete.

Current generated evidence after the initial verifier-closure work is not
maintained in this plan; read the scorecard and external function ledger
produced by the commands above. This keeps the self-cleaning roadmap from
becoming a second, stale status authority.

## 4. Fixed inventory denominators

The pinned audit currently identifies these public declarations:

| ID | Subject kind | Denominator |
|---|---|---:|
| I01 | functions | 218 |
| I02 | macros | 891 |
| I03 | non-record typedefs | 62 |
| I04 | callback typedefs in the authoritative Clang inventory | 39 |
| I05 | records | 78 |
| I06 | enums | 20 |
| I07 | enum variants | 158 |
| I08 | error codes | 119 |
| I09 | public header paths | 47 |
| I10 | ownership rules | 23 |
| I11 | state transitions | 20 |
| I12 | default modules, drivers, and renderers | 19 |
| I13 | optional public components | 7 |
| I14 | binary and installation artifact items | 8 |
| I15 | target, data-model, and endianness lanes | 5 |

Changing a pinned denominator requires 3 pieces of evidence:

1. the exact FreeType 2.14.3 header declaration that was omitted or
   double-counted;
2. a parser regression test;
3. regenerated audit output showing the corrected inventory.

Reducing a denominator because behavior is hard, deprecated, optional,
platform-specific, or currently excluded is forbidden.

## 5. Measurement rules

Every blocking measurement uses the same 5 states:

1. `unmeasured` — denominator or evidence mechanism is absent;
2. `missing` — no shipped declaration, implementation, or artifact exists;
3. `partial` — some behavior or configurations match;
4. `mismatch` — evidence differs from pinned C;
5. `complete` — the whole declared contract has direct exact evidence.

Only state 5 contributes to a completion numerator. The following never count
as complete:

1. compile-only evidence for runtime behavior;
2. a null-validation route standing in for non-null success behavior;
3. a Rust FFI call standing in for execution through the built C artifact;
4. a same-named type without size, alignment, field-offset, and calling
   convention evidence;
5. a code-coverage hit without contract assertions;
6. an intentional exclusion, generic fallback, placeholder, skipped platform,
   or build-dependent expectation.

Inputs for which pinned FreeType has no defined result are not invented as C
error contracts. A Fontdone hardening behavior must declare
`contract_scope: fontdone_safety_extension`, cite the exact pinned C source
that establishes the undefined boundary, use the `abi_safety_result` schema,
and have a direct package test. The route audit reports these extensions
separately; `make c-abi-contract` runs their tests, and the C-contract
numerators and denominators exclude them.

## 6. Measurement closure work

Before implementation progress can complete a category, the verifier must
close its missing evidence:

1. C01: compare every function signature through Clang and execute every
   applicable route through an independently compiled external C consumer
   linked to the actual shared and static artifacts.
2. C02: compile and evaluate every macro and enum value in pinned and shipped
   headers, including aliases and expression types.
3. C03: compare canonical Clang types, signedness, widths, qualifiers, pointer
   depth, and enum underlying behavior.
4. C04: compare size, alignment, and every public field offset on each platform
   ABI.
5. C05: keep the closed 39-typedef and 16-alias Clang inventory exact; callback
   invocation, nullability, lifetime, ownership, and error behavior remain
   blocking runtime work under C06 through C08.
6. C06: complete the fixed ownership ledger for caller-owned, library-owned,
   borrowed, transferred, retained, copied, released, and callback-managed
   storage.
7. C07: complete the fixed state-machine ledger for libraries, faces, sizes,
   slots, glyphs, streams, modules, caches, validators, and renderers.
8. C08: require exact error code, output initialization, side effects, and
   cleanup for every failure route; remove all pending and fallback evidence.
9. C09: complete all pinned modules, drivers, renderers, validators, cache
   services, stream adapters, and compile-time feature behavior in the fixed
   inventory.
10. C10: ship the public include tree and compile every header independently
    and in supported combinations as C and C++.
11. C11: complete verification of shared/static consumers and exports, SONAME
    or install-name, import libraries, pkg-config metadata, and installation
    layout.
12. C12: execute every fixed OS/architecture/data-model lane, including
    Windows LLP64, 32-bit ILP32, and big-endian behavior.

## 7. Implementation order

Work in this order because later proof depends on earlier surfaces:

1. repair the verifier and close every `? / ?` denominator;
2. complete headers, names, signatures, types, layouts, and callbacks;
3. implement all 218 functions with no excluded subset;
4. close success, error, ownership, and state-transition behavior;
5. close modules and optional build configurations;
6. close binary packaging and external consumers;
7. close the platform matrix;
8. run every required gate and the strict completion target.

For each batch:

1. select one concrete incomplete scorecard item;
2. add or strengthen direct evidence before claiming progress;
3. compare one input through pinned C and the real Fontdone artifact;
4. locate and fix the first semantic divergence in pure Rust;
5. run the narrow case;
6. run `make c-abi-contract`;
7. report exact before/after numerators and denominators;
8. run the full required gates when the batch is green.

## 8. Goal G02: reach 100% production-code coverage

State: `OPEN`

The maintained all-lane coverage suite measures reachable production source in
the Rust core, C ABI, and host-compiled WASM facade. It keeps all workspace
packages in the report but executes only the `unified_fixture_parity`
integration target under one default-profile build; that target drives the
complete parity matrix through all three surfaces. Empty root-unit and
`pipe_trace` targets add no parity inputs and can duplicate cfg-dependent FFI
source in LLVM's report, so they are intentionally not executed. Independent
oracle/audit preparation runs in the setup batch before the coherent coverage
build; the ABI-only package preflight remains a separate
`make coverage-abi-preflight` gate already exercised by `make test-fast`. The isolated
`COVERAGE_ALL_TARGET_DIR` cache keeps `--no-clean` from mixing stale binaries,
and test-harness paths remain the only filename exclusion. Optional feature
profiles remain a separate `make optional-feature-contract` gate so coverage
never compares a feature-enabled implementation with a default-profile oracle.
Executing code does not prove FreeType parity, so G02 cannot satisfy any G01
item.

Baseline snapshot `5a122fcc-aa76-4503-82d3-e8bbb564f349`, produced from commit
`09110a488bcc53c96def8ccf7e3d6c4e6418737f`, records:

| Metric | Covered / total | Coverage | Pending |
|---|---:|---:|---:|
| Lines | 46,028 / 51,219 | 89.87% | 5,191 |
| Branches | 9,036 / 11,907 | 75.89% | 2,871 |
| Functions | 3,189 / 3,639 | 87.63% | 450 |
| Regions | 63,823 / 71,957 | 88.70% | 8,134 |

Against the preceding snapshot, covered counts increased by 481 lines, 50
branches, 77 functions, and 771 regions. Pending counts decreased by 160 lines,
58 branches, 23 functions, and 234 regions despite the expanded source
denominators. The same run passed 7,212 / 7,212 runnable parity comparisons
with 0 failures; 95 cases remained explicitly pending.

### 8.1 Verified work after the baseline

The following numbered batches are retained as completed implementation/test
work. The latest all-lane measurement below covers the current worktree; no
per-batch coverage increase is claimed:

| Batch | Source commit | Verified change |
|---:|---|---|
| 1 | `ee43406610252d368a2b437746b6b99a76779c4d` | Fixed C-shaped bitmap-copy ownership so replacing the target allocation cannot unregister or invalidate the source buffer. |
| 2 | `b2b7af666cc026ef402323c661ba6cb6e38037e1` | Added deterministic PFR parsing and bitmap lifecycle/state/validation tests, including packed gray, mono, LCD, negative-pitch, blend, conversion, and cleanup routes. |
| 3 | `2fb2d5d044ff39ecd876fb04a906547c6b2e03a7` | Added CFF1, CFF2, glyf-cache, active/inactive gvar, malformed SVG range, gzip, primitive-read, and glyph-lookup tests; removed only mathematically unreachable SVG offset/count failures on the supported 32/64-bit targets. |
| 4 | `e87d704dc269683338faa12cf2a89b0af0c1dc02` | Added root integration contracts for C ABI and WASM bitmap initialization, null validation, deep/self-copy, conversion, cleanup, stale-library rejection, and post-copy ownership independence. |
| 5 | `9eebbe6aed0f4051630e750aa87c08a09f97e90e` | Added null/error contracts for C cache constructors/lookups, face and outline entry points, transform/lang-tag outputs, logging/property controls, plus WASM allocation, face-open, bitmap-accessor, and cleanup routes. |
| 6 | `e63d25a0d954a472648918b9a3da9822ff517313` | Added a valid requester-backed C cache manager lifecycle covering face/size lookup, CMap/image/SBit cache construction, glyph/node ownership, face removal, reset, and teardown. |
| 7 | `fe7210517b9f9ba31384a64370b6c33e4928267f` | Promoted existing Type1 MM/private, FontInfo, ForceBold, and Standard/ISOLatin1/Expert/custom-encoding fixtures from required-future status after all four lanes matched pinned C; removed 16 pending runtime rows. |
| 8 | `890b67f3c12aadeaf4a7737cdd3a6e9fd897e812` | Promoted tracked Adobe custom/platform and Apple full-Unicode charmap fixtures; added Type 1 synthetic charmaps and Adobe encoding tags to every ABI facade; removed two stale runtime-skip guards and added the missing `freetype.enumerate_charmaps` dispatch. |
| 9 | `c58dfaa8a0276c22455dd1f942d070f552b2f033` | Removed unused missing-font declarations from scalar TrueType interpreter-property fixtures, documented that the remaining glyph-output rows still require maintained inputs, and revalidated the unchanged parity denominator. |
| 10 | `15577d60d17a3dfe1b17081083029832d932e249` | Consolidated the per-commit GitHub checks into the exact `make ci-fast` gate plus MSRV, retained smoke diagnostics, and kept full parity, coverage, platform, performance, package, and supply-chain work manual for requested pre-merge audits. |
| 11 | `e41acf4f5e2c01f6b6f4caafd2229d4de04abc4d` | Promoted the maintained eight-byte PCF control route for `FT_HAS_HORIZONTAL` from pending to exact parity, preserving pinned `FT_Err_Invalid_Stream_Operation` across Rust, C ABI, and WASM. |
| 12 | `adf44e4fb451cc91399bde9bb9af46b00f38b7c9` | Routed the maintained pinned-C raster callback lifecycle through the New, Reset, and Done callback rows across Rust FFI, C ABI, and WASM; the failure and set-mode matrices remain explicitly pending. |
| 13 | `466799d94680e8707f7f665dec4da98b31ff051a` | Replaced the `FT_Raster_New_Func` out-of-memory placeholder with a pinned-C callback failure probe; Rust FFI, C ABI, and WASM now agree on `FT_Err_Out_Of_Memory`, the `raster_new` event, and no installed module. |
| 14 | `e29819f5812ac521fd78b1e90e52f080cd17124a` | Routed the maintained `FT_Raster_Set_Mode_Func` matrix through pinned-C callbacks; mode tags, null/non-null payloads, callback status propagation, and callback invocation now agree across Rust FFI, C ABI, and WASM. |
| 15 | `8eed8f2f22288e53429503bed169d2b8608cc10e` | Routed the maintained `FT_Raster_Funcs` callback-slot matrix through live pinned-C class tables and Rust FFI, C ABI, and WASM observations for standard, grays, SDF, and bitmap-SDF renderers. |
| 16 | `bc4513a6e712d53b047e4fdc21f0814f8f83d0be` | Routed the maintained `FT_GLYPH_FORMAT_PLOTTER` source-emitter inventory through pinned-C, Rust FFI, C ABI, and WASM renderer lookup; the pinned module set has no runtime plotter emitter. |
| 17 | `514cca075fc5181b9bbd1005e4d53f6c3462381c` | Routed the maintained SVG glyph-load contract through pinned-C, Rust FFI, C ABI, and WASM observations for enabled SVG document success and `FT_LOAD_SVG_ONLY` error behavior; removed two pending runtime rows. |
| 18 | `c16e21ea68b6aecb2cdd2c7258266326ae958e62` | Added the explicit GitHub `merge_group`/`checks_requested` trigger so the same bounded commit gate reports required status checks for merge-queue revisions; registered `make ci-fast` run `e3277efd-1af9-4a4e-9ccb-912a16b629ab` passed on the resulting clean SHA; kept full parity, coverage, performance, platform, package, and supply-chain jobs manual. |
| 19 | `a51f6c4a8fcd7ac60120d793fc13c0111a98bbff` | Refreshed the exact C-ABI scorecard in run `a1472533-552d-43de-b9ca-a0c3f5adbfe0`: 9 / 12 categories complete; C01 runtime rows 4,997 / 5,047 with 50 pending; C11 binary artifacts 7 / 8; C12 platform bundles 1 / 5; all other blocking categories complete. |
| 20 | `c7d6b3758d0229011c39c170c95719fa0502bc04` | Promoted the two SVG build-feature classification rows through an actual pinned-C oracle, Rust FFI, C ABI, and WASM route. Clean full parity run `3369e0ab-624f-4e46-8539-b862a9cc97b4` passed 7,247 / 7,247 runnable comparisons with 60 explicitly pending concrete cases; the refreshed C-ABI scorecard run `66bc46c3-185a-469c-8573-f28a53f85920` reports C01 runtime rows 4,999 / 5,047 with 48 pending. |
| 21 | `a7db2c44a60c581298601697b536b976284c7d61` | Added project-authored deterministic AAT/GX inputs and routed eight `FT_TrueTypeGX_Validate` table-length/index rows through pinned C, Rust FFI, C ABI, and WASM. Clean full parity run `a5b5421a-ad05-4d3f-9082-bcccda2cc1c8` passed 7,255 / 7,255 runnable comparisons with 52 explicitly pending concrete cases; clean C-ABI scorecard run `7f7133e5-4cf2-4c9c-b238-4beb5a9ba5b8` reports C01 runtime rows 5,007 / 5,047 with 40 pending. |
| 22 | `2f05af7f3f3fd767ecc04679fce58abc9781af2b` | Routed `FT_GLYPH_FORMAT_SVG.unsupported_svg_build_classification` through a same-input pinned-C SVG feature/load probe and exact Rust FFI, C ABI, and WASM observations. Clean full parity run `ef1090a4-3ed3-493f-aaa5-0d83d9853078` passed 7,256 / 7,256 runnable comparisons with 51 explicitly pending concrete cases; clean C-ABI scorecard run `90455200-661a-4be1-b6f3-994b5a70724e` reports C01 runtime rows 5,008 / 5,047 with 39 pending. |
| 23 | `abb16646c6bbb93be42b4b967037a38ad90fbb07` | Routed `FT_LOAD_SVG_ONLY.svg_only_behavior` with the project-authored SVG/non-SVG pair fixture through a same-input pinned-C oracle and exact Rust FFI, C ABI, and WASM observations: SVG glyph success includes slot format/document hash, while the non-SVG glyph preserves the public rejection error. Clean full parity run `3f342b8e-f106-4054-a33e-6fab40d29ce6` passed 7,257 / 7,257 runnable comparisons with 50 explicitly pending concrete cases; clean C-ABI scorecard run `e54a80df-e416-46d2-ac15-9eb4374f9c23` reports C01 runtime rows 5,009 / 5,047 with 38 pending. |
| 24 | `74f6e87388b5cb31f397449db4d17d0b276b2772` | Routed both `FT_SVG_DocumentRec` payload/metrics rows through the project-authored OT-SVG fixture and exact pinned-C, Rust FFI, C ABI, and WASM observations; the follow-up Clippy fix keeps the parity harness warning-free. Focused run `0277b598-09cc-4413-b62c-5f8c5acc6ad5` passed all three selected cases; clean full parity run `0fae0ab3-1df2-4aa8-aaa6-cc5a09945209` passed 7,259 / 7,259 runnable comparisons with 48 explicitly pending concrete cases; clean C-ABI scorecard run `1ea13e0e-c422-4c53-a2ab-1e10192322e0` reports C01 runtime rows 5,011 / 5,047 with 36 pending. |
| 25 | `25846d907293f04d4337a60a5ddf28a824065baa` | Promoted the nine GX/classic-kern semantic rows (`FT_VALIDATE_GX`, `FT_VALIDATE_MS`, and the seven table-slot validators) with deterministic valid, absent, truncated, and invalid-header SFNT controls. Focused GX and external-C gates passed; clean full parity run `660ffb48-3bc8-4995-ace8-525dbb51c468` passed 7,269 / 7,269 runnable comparisons with 38 explicitly pending concrete cases; clean C-ABI scorecard run `c5c5ccf7-79cc-4b62-8bfb-312012e8f281` reports C01 runtime rows 5,021 / 5,047 with 26 pending. |
| 26 | SVG renderer hooks | Routed `otsvg.FT_SVG_Document.renderer_callback_observes_document` through the pinned-C four-hook `ot-svg:svg-hooks` flow and exact Rust FFI, C ABI, and WASM observations: the renderer callback receives the same document pointer class, glyph ID, and lifetime fields (`svg_document_length`, units, glyph range, transform, delta) as pinned C, with the missing-hooks state classified explicitly as unsupported. The route closes the last pending parity route: route audit reports 0 pending routes, clean full parity run `d217ea7be88f6bfd4367b752999520ed6b785a76fb10cd1f6d6da573bc7ebaf7` passed 7,296 / 7,296 runnable comparisons with 12 explicitly pending concrete cases (safety extensions and unresolved stroker assets); C-ABI scorecard is 8 / 12 categories complete (C01.7 5,040 / 5,048, C08.3 7,297 / 7,305). |
| 27 | Generic-fallback elimination | Promoted the eight generic-fallback rows to exact four-lane routes: PS hinting-engine property set/get/string/load across CFF/Type1/t1cid, `FT_PARAM_TAG_IGNORE_SBIX` outline and bitmap-only open-face dispatch, `FT_Parameter` tag/data variant dispatch, and `FT_PARAM_TAG_STEM_DARKENING` state toggle with preserved public output. Route audit: **0 pending routes, 0 generic-fallback rows**, 5,039 real-parity concrete cases; C-ABI scorecard is 10 / 12 categories complete with C01.7 5,048 / 5,048 and C08.3 7,305 / 7,305 (binary artifacts and platform bundles remain). |
| 28 | Working tree (`ad6c489963b2797ab39e226efaa6a4690faa63ef`) | Added 13 maintained input-only SBIT cases for index formats 2/4/5 and grayscale, mono, packed-gray, and BGRA compound bitmaps. Traced the BGRA mismatch to pinned `FT_Bitmap_Convert`, implemented the exact premultiplied-sRGB flattening rule at the SBIT boundary, and passed full parity run `d9ebd2d0-6f84-4e8e-bd5c-f8f9d86cdcb7` with 7,468 / 7,468 runnable comparisons and 3 explicitly pending safety-extension cases. |
| 29 | Working tree (avar normalization) | Added an input-only variable-font case whose `gvar` interpolation exposed the missing `avar` normalization stage. Implemented pinned-compatible `avar` parsing and design/named-instance coordinate mapping; focused parity passed 33 / 33 and full parity run `61dddda3-5866-43ea-bd80-e84cd1d4c5b9` passed 7,469 / 7,469 runnable comparisons with 3 explicitly pending safety-extension cases. |
| 30 | Working tree (SVG glyph-copy zero-length source) | Added an input-only real SVG glyph case for `FT_Glyph_Copy` with a zero public document length, routed the pinned `FT_Err_Invalid_Slot_Handle` and partial-target cleanup through Rust FFI, C ABI, and WASM, and fixed WASM target clearing on class-copy errors. Focused run `9086a87e-aee1-4ef8-a479-276a357623c9` passed 1 / 1; full parity run `4cec1eda-461a-4e04-9009-f7109556e845` passed 7,475 / 7,475 runnable comparisons with 3 explicitly pending safety-extension cases. |
| 31 | Working tree (SBIT transparent BGRA flattening) | Added the input-only `sbit-bgra-format1-grayscale-sbits-only` variant to exercise the transparent-pixel branch of the existing BGRA-to-grayscale load path. Focused parity passed 1 / 1; full parity run `09cd72c3-5638-4b1d-af11-ce3ac712e199` passed 7,476 / 7,476 runnable comparisons with 3 explicitly pending safety-extension cases, and the all-lane coverage snapshot moved to 49,341 / 54,104 lines, 9,688 / 12,512 branches, and 67,921 / 75,273 regions. |

The latest source-bound parity verification is Coverage MCP parity run
`09cd72c3-5638-4b1d-af11-ce3ac712e199`, recorded by run
`709da7f3-b6c1-4841-a4cb-dd8881e75108` against the worktree at commit
`4a13a92e70800aa9c00671bc7f1784dd2cc6dc4e`: it passed 7,476 / 7,476 runnable
comparisons, 0 failed, and 3 explicitly pending safety-extension cases. The
route audit reports **0 pending routes** with 218 / 218 function routes present
in each ABI surface. The committed source-digest attestation is
`doc/runtime_parity_evidence.json`. The companion C-ABI scorecard, Coverage
MCP run `542d64f3-0c97-4a32-b590-fbd385ca1927`, reports 8 / 12 categories
complete, with 5,219 / 5,219 runtime contract rows and 646 / 647 strict error
routes exact; the remaining strict-error gap is the known `FT_Load_Sfnt_Table`
null-face case, while binary artifacts and platform bundles remain the other
tracked gaps.

The latest all-lane coverage run completed in 1 minute 51.746 seconds. Its
runtime backend timings were about 39.11 seconds Rust FFI, 29.25 seconds C ABI,
29.13 seconds WASM, and 0.03 seconds comparison. The separately measured
removal of the duplicate ABI preflight from the default coverage path reduced
the earlier 3:48.279 sample by 42.627 seconds (18.7%); the remaining wall-time
tail is outside backend execution, in setup/build/reporting/ingestion and host
contention. `make test-coverage-all`
uses a single parity worker, `CARGO_PROFILE_TEST_OPT_LEVEL=1`,
`COVERAGE_TEST_DEBUG=1`, and `cargo llvm-cov --no-clean` by default: LLVM
instrumentation made parallel backend calls contend, the optimized test
profile removed the several-fold slowdown of unoptimized instrumented code,
line-table-only debuginfo reduced compile/report overhead, retaining the
instrumented target removes the repeated rebuild on warm runs, and face-cache
keys reuse preloaded font content digests instead of rehashing each expanded
case. Independent oracle/audit preparation runs in the two-job setup batch;
the ABI-only package preflight remains available separately as
`make coverage-abi-preflight` and is already exercised by `make test-fast`.
Unchanged generated
oracle inputs preserve their mtimes so the helper/validator C build is not
repeated. The current run measured 49,341 / 54,104 lines, 9,688 / 12,512
branches, 3,371 / 3,828 functions, and 67,921 / 75,273 regions. It passed
7,476 / 7,476 runnable parity comparisons with 0 failures. Its Coverage MCP run
is `e391ae9e-9591-4b68-b936-8db1a3177144`, with snapshot
`30d2ac67-a15d-4f9a-ac1c-7e727d4af8ec`. Each measurement clears stale
`.profraw` files first; use `make coverage-clean` after changing the coverage
toolchain or instrumentation configuration.

Run:

```bash
make test-coverage-all
```

G02 is complete only when a clean, source-matched run reports:

1. lines equal total lines with a non-zero denominator;
2. branches equal total branches with a non-zero denominator;
3. functions equal total functions with a non-zero denominator;
4. regions equal total regions with a non-zero denominator;
5. no reachable production path is ignored, filtered, removed, or padded to
   alter a metric;
6. the same run passes every runnable parity case and produces a retained
   machine-readable coverage artifact.

## 9. Goal G03: establish enforceable performance baselines

State: `OPEN`

The maintained matrix compares release-mode Fontdone with the pinned C oracle.
The harness builds before timing, executes both workload binaries directly,
and records raw samples, environment identity, output identity, operation
weights, peak process memory, and exact artifact hashes and byte counts.
Requested thorough CI records ten samples and retains both JSON and Markdown
evidence.

<!-- performance-roadmap:start -->
The most-sampled current environment has **2 / 5 qualifying clean runs**.
<!-- performance-roadmap:end -->

| Measurement | Current state | Completion evidence |
|---|---|---|
| Latency | per-operation mean, median, p90, and p99 implemented | clean source-bound ledger plus reviewed weighted-latency threshold |
| Throughput | per-operation, group, and aggregate operations/second implemented | clean source-bound ledger plus aggregate C/Rust threshold |
| Peak memory | complete direct-process peak RSS implemented for Rust and C | clean source-bound ledger plus C/Rust peak-RSS threshold |
| Binary size | five unstripped release artifacts measured by exact bytes and SHA-256 | clean source-bound ledger plus shared-library ratio and WASM byte thresholds |
| Regression thresholds | `collecting_baseline`; strict Make target implemented and intentionally failing closed | active reviewed thresholds passing through `make bench-regression` and requested thorough CI |

Run and record one qualifying measurement with:

```bash
make bench BENCH_SAMPLES=10 BENCH_PROFILE=default
make record-performance-baseline
```

`make record-performance-baseline` accepts only a clean current source commit,
the maintained matrix and profile, at least ten complete C/Rust samples, exact
matrix row coverage, positive memory observations, and all five artifact
identities. It appends compact evidence to
`doc/compatibility_snapshot.json`; it never sets thresholds.

Before setting thresholds, collect at least five clean ten-sample runs for one
runner image and CPU model. The accepted baseline and its raw run identities
must be reviewed together; results from different CPU models remain separate.
Performance correctness mismatches fail before timing can count.

G03 is complete only when latency, throughput, peak memory, and binary size all
have reproducible C-vs-Rust measurements, reviewed machine-readable thresholds,
and a failing regression gate exposed through `make` and requested thorough CI.

## 10. CI evidence levels

The required per-commit `Commit gate` runs fast tests, MSRV, format, Clippy,
strict documentation, generated contracts and fixtures, independent Rust and
C consumers, and a fixed exact runtime/API smoke matrix across the Rust, C ABI,
and WASM routes for pushes, pull requests, and merge-queue groups. The
manually dispatched `Thorough gate` repeats that gate and adds the full runnable parity
matrix, downstream consumers, all-lane coverage, the ten-sample performance
baseline, five platform bundles, the C scorecard, packages, and supply-chain
audits.

The thorough C scorecard uses `make c-abi-contract-all-platforms`, which
validates every available measurement while preserving incomplete numerators.
Only `make c-abi-contract-complete` may assert 12 / 12 completion.

Crate publication, GitHub releases, and public issue creation remain outside
this active development plan and require explicit approval.

## 11. Completion gates

G01 can change to `COMPLETE` only when all commands pass:

```bash
make setup
make test-parity
make test-ffi
make api-abi-audit
make c-abi-contract-complete
make fmt
make clippy
make doc
make doc-test
make test-integrations
make release-verify
```

The final generated scorecard must show:

1. **12 / 12 categories complete**;
2. **0** unmeasured denominators;
3. **0** missing, partial, mismatch, pending, placeholder, fallback, excluded,
   or skipped required items;
4. **0** C-ABI routes implemented by Rust-frontend fallback;
5. every external C/C++ consumer compiling, linking, and running the actual
   release artifact on every supported platform.

Code coverage remains a separate quality signal. It may identify unexecuted
Rust, C-ABI, or Wasm lines, but it cannot satisfy any completion item above.

The roadmap can be deleted only after G01, G02, and G03 are all complete and
the requested thorough CI run for the same clean commit passes.

## 12. Evidence ledger

| Goal | State | Evidence | Verification |
|---|---|---|---|
| G01 | OPEN | pinned audit, route audit, external C consumers, generated 12-category scorecard, release evidence | `make c-abi-contract-complete && make release-verify` |
| G02 | OPEN | source-bound all-lane LLVM coverage snapshot | `make test-coverage-all`; lines, branches, functions, and regions each exactly 100% |
| G03 | OPEN | raw C/Rust timing, throughput, memory, size, and threshold ledgers | requested `make ci-thorough`; every performance regression gate passes |

When all three goals complete, preserve their executable ledgers and durable
evidence, update integration and development documentation, remove this
roadmap from the documentation index, and delete this file. Completed
implementation history belongs in Git history and release notes.
