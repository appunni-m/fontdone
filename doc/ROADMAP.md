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
oracle/audit preparation runs after any coherent coverage-workspace clean and
includes the default plus all four maintained optional oracle helpers. Missing
or stale isolated optional-feature probe bundles are rebuilt through the
separate contract target; the ABI-only package preflight remains a separate
`make coverage-abi-preflight` gate already exercised by `make test-fast`. The
isolated `COVERAGE_ALL_TARGET_DIR` cache retains warm instrumented builds,
while its source/configuration state marker cleans the workspace once after a
relevant change; this prevents `--no-report`/`--no-clean` from mixing stale
feature/cfg binaries. Test-harness paths remain the only filename exclusion.
Optional feature
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
work. Each recent row records its own all-lane measurement for the current
worktree at that point:

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
| 32 | `53995d32008605b8abe6a15db477c86881c929c9` | Fixed the dedicated null-face `FT_Load_Sfnt_Table` oracle dispatch and malformed JSON emission. The strict error ledger is now 647 / 647 exact; full parity remains 7,476 / 7,476 runnable cases with 3 safety-extension cases pending. |
| 33 | `082e577a6147065d886ea45a316e244f75045ce3` | Split coverage execution into separate Rust FFI, C ABI, and host-WASM processes with process-local LLVM profiles, retaining the exact unified parity matrix. Validation run `b0847bf1-9bce-4a79-8966-5115c88f43eb` passed 7,476 / 7,476 in every lane and measured 61.827 seconds versus the prior warm 113.998-second combined-lane run. |
| 34 | `776e7d9eb325f09402c5e3f84955de03c5d242ac` | Corrected the split coverage report to name all three workspace packages explicitly, because `cargo llvm-cov report` does not accept `--workspace`; the C-ABI and WASM implementation source therefore remains in the measured denominator. |
| 35 | `1a08537f6188cfc4631cee7204fc27663104ae62` | Regenerated the maintained leading/single-reference gvar fixture with the avar-mapped tuple peak (`5325` F2DOT14), activating the intended IUP interpolation path. Focused parity passed 33 / 33; full parity run `640b6f77-2758-423a-b6ca-deb42c86a2b9` passed 7,476 / 7,476, and coverage increased by 22 lines, 6 branches, and 32 regions. |
| 36 | `f4eee93455458366c43af8770c574f50a29cb5ed` | Reused the single instrumented `unified_fixture_parity` binary for all three split coverage processes instead of invoking `cargo llvm-cov` three times. Clean Coverage MCP run `6eb57e9e-4147-4663-b34a-d29227b0fdba` passed 7,476 / 7,476 in every lane in 54.054 seconds, with unchanged workspace coverage totals; this is 52.6% faster than the previous warm 113.998-second run. |
| 37 | `e6bef8f9e180eeda5e606270e515314443ba6c44` | Routed the existing pure-CFF Type 2 operand-stack-overflow and escaped-add argument-underflow glyph inputs through `FT_Load_Glyph`. Focused parity passed 321 / 321; corrected full parity run `352d82a0-1a86-4a01-97b7-9235a1a66b5b` passed 7,478 / 7,478 runnable comparisons with 3 explicitly pending safety-extension cases. The refreshed C-ABI scorecard is exact at 649 / 649 expected-error routes and 5,221 / 5,221 runtime rows; aggregate coverage totals were unchanged. |
| 38 | `dea377a12afd181a58a0bc4a6d1c74b3a7d4e7c7` | Re-recorded the full parity evidence after the coverage-speed fixes. Clean Coverage MCP run `4be92a34-c7a6-49fb-9a7c-a97ef1482757` passed 7,479 / 7,479 runnable comparisons with 3 explicitly pending safety-extension cases; the source-bound attestation is recorded by `07462991-ef61-45f8-ba2b-e68c05142e3d`. |
| 39 | `91f8da7f08b0143881a7ef79c4a40ea9322cd045` | Replaced the RemoveFaceID parity probe that closed and reopened independent faces with one real C-ABI FTC manager/cache sequence using persistent face-ID keys and actual `FTC_Manager_RemoveFaceID` invalidation. Clean Coverage MCP parity run `1484f32a-a27c-4755-ba4a-43e1393cbbc3` passed 7,479 / 7,479 runnable comparisons; the source-bound attestation is recorded by `843aa02f-1dcf-417e-ac3e-ee3e8622ee6d`. |
| 40 | `753308b8e3c6f3775fa442fa4b74e6a17113aa4c` | Added four maintained malformed-`gvar` runtime inputs and expanded the public `FT_Glyph_To_Bitmap` render-failure input matrix across contour-endpoint, points, tags, and contours record corruption, with exact Rust FFI, C ABI, and WASM comparisons. Source-bound full parity run `1fdace28-cc1b-49c5-b618-4ce4a532cada` passed 7,483 / 7,483 runnable comparisons with 3 safety-extension cases pending; route audit remains 0 pending and function evidence is 218 / 218. |
| 41 | `45451e6bc36f9d2b1d7d585a5eded18d2ebdf693` | Added a source-digest-bound cache for the 1,537 maintained input files and 7,486 expanded cases, and switched focused `test-op`/`test-case` checks to the runtime-only ABI audit while retaining optional-feature verification in the full gate. The warm focused managed probe completed in 2.448 seconds; source-bound parity run `6122ce71-01c9-4b66-a2ca-734051123cef` passed 7,483 / 7,483 runnable comparisons with 3 safety-extension cases pending. |
| 42 | `ca4edb2a38ad5fc34e1082973bd86cf7b5504059` | Made clean all-lane coverage builds reliable by using `cargo llvm-cov --no-report -- --list` for the instrumented binary build, preventing stale-profile merge failures after `make coverage-clean`. Clean validation run `bc726e36-8257-49ee-9223-2b4815743715` passed and ingested snapshot `ea6e65c0-608c-47bd-9223-a13e49960d69`; the warm repeat `85d80dc4-ccfe-42ec-b348-78fa43381c2c` completed in 51.203 seconds with unchanged parity. Final source-bound parity run `8b583a6e-7057-4c8a-a55a-c37fc8b3493d` passed 7,483 / 7,483 and was recorded by `8281590b-663a-4b91-960e-f911b1f83c9c`. |
| 43 | `ae2446ac14d6a376178cd40c44449814347626c2` | Added two project-authored CFF1 Type 2 inputs for escaped-add success and unknown-escape error handling, with exact focused Rust FFI, C ABI, and WASM parity. Full parity run `fda39eed-f055-4c75-8eea-b63a66c5a461` passed 7,485 / 7,485 runnable comparisons with 3 safety-extension cases pending; route audit remains 0 pending and function evidence is 218 / 218. The all-lane coverage measurement increased to 49,389 / 54,104 lines, 9,699 / 12,512 branches, 3,373 / 3,828 functions, and 68,011 / 75,273 regions. |
| 44 | `6ec81f38c5faafcc7924674c21c294ee8c3a5af3` | Added malformed CFF1 Top DICT overflow/clamping fixtures and a one-operand `hvcurveto` fixture; corrected truncated positive/negative DICT-number error mapping to the pinned C error. Source-bound parity run `fabe97ff-bca0-4826-ba13-66b4e60f57ec` passed 7,489 / 7,489 runnable comparisons with 3 safety-extension cases pending. Clean committed coverage run `a24f431c-5a5b-4c73-8fa4-2738888d0db2`, snapshot `7724fcea-e4b9-408f-9f80-9447f00f3363`, measured 49,443 / 54,150 lines, 9,707 / 12,520 branches, 3,379 / 3,832 functions, and 68,059 / 75,313 regions. |
| 45 | `e0ed6cb852510c31bdae872f669012acdbbade66` | Added project-authored CFF CID charset format 0 and format 1 fixtures plus malformed Top DICT charset, ROS, and negative-real overflow inputs. Source-bound parity run `742d6cf1-105d-4399-9da7-aa24a6335f6b` passed 7,494 / 7,494 runnable comparisons with 3 safety-extension cases pending. Clean committed coverage run `eeb78a28-19d8-4863-92cd-ed922a9b7761`, snapshot `1d5357f4-f9ea-42bc-bbd3-d5fb030a649d`, measured 49,454 / 54,150 lines, 9,709 / 12,520 branches, 3,380 / 3,832 functions, and 68,074 / 75,313 regions. |
| 46 | `a4af5a01e62ae8050838c9c789fbc2032661bfdc` | Avoided repeated FreeType oracle CMake reconfiguration when the source, validator overlay, and build script inputs are unchanged. Warm `scripts/build_ft.sh` preparation fell from repeated full-source rebuild setup to about 0.10 seconds; the clean all-lane coverage result remained equivalent at 49,454 / 54,150 lines, 9,709 / 12,520 branches, 3,380 / 3,832 functions, and 68,074 / 75,313 regions. The end-to-end coverage wall time remains host-variable (52.144 seconds in the current clean run), so this is a deterministic setup improvement rather than a claimed full-run speedup. |
| 47 | `8ccdf8a6d24f235c7893a1c00b149761d366209f` | Added public CFF1 Type 2 parity inputs for the true `hvcurveto`/`vhcurveto` last-delta branches and a malformed Top DICT real-number reserved nibble, while preserving existing CFF auto-hint controls. Focused parity passed 326 / 326; source-bound full parity run `44f0150f-de1d-49b7-998b-185ebff29294` passed 7,498 / 7,498 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `7ff4b786-0209-4887-9642-35025a970bac`, snapshot `f7762084-dc45-41b0-8050-22a69f03fefd`, measured 49,464 / 54,150 lines, 9,709 / 12,520 branches, 3,380 / 3,832 functions, and 68,085 / 75,313 regions in 51.991 seconds. |
| 48 | `2e047f11369ad89c8f5e22f5755314006e026a4d` | Added a derived CID-keyed CFF ROS fixture whose registry and ordering use standard CFF SIDs `Roman` (389) and `Semibold` (390), implemented their pure-Rust decoding, and explicitly routed the new input through the pinned-C, Rust FFI, C ABI, and WASM parity harness. Committed parity run `0b2e22de-6768-4658-a705-4d4db308959c`, recorded by `f7df490d-f686-4407-8dba-725844b53d34`, passed 7,499 / 7,499 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `a53f0b6b-3d2f-43f9-8c02-9d6523a07147`, snapshot `7ee8b2e4-b166-45fd-bc25-a4d239d7754e`, measured 49,466 / 54,152 lines, 9,709 / 12,520 branches, 3,380 / 3,832 functions, and 68,087 / 75,315 regions in 102.331 seconds; the C-ABI scorecard reports 5,242 / 5,242 runtime rows and 7,499 / 7,499 no-fallback routes. |
| 49 | `a58512637395397741c99baf9776c2d79072639f` | Added a derived single-glyph CID-keyed CFF input containing only `.notdef` and routed it through `FT_Get_CID_Registry_Ordering_Supplement`, covering the one-glyph charset path through the pinned-C, Rust FFI, C ABI, and WASM parity harness. Committed parity run `74363786-7afc-496b-b9e5-0dc508522359`, recorded by `e5ff9487-bf90-4463-92ba-a27d69ec4bc8`, passed 7,500 / 7,500 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `5a13924d-c556-4260-8f55-260a0b88b493`, snapshot `fb635dfe-ad08-4e8c-861a-5743b32a96d9`, measured 49,467 / 54,152 lines, 9,710 / 12,520 branches, 3,380 / 3,832 functions, and 68,088 / 75,315 regions in 95.540 seconds; the C-ABI scorecard reports 5,243 / 5,243 runtime rows and 7,500 / 7,500 no-fallback routes. |
| 50 | `839ffce3da73ab8c576ee6c6289050674d761524` | Added a derived CID-keyed CFF input whose ROS ordering SID is `800`, outside the face's String INDEX, and preserved pinned FreeType's successful CID service with a null ordering string through Rust FFI, C ABI, and WASM. Clean source-bound parity run `dbe0e4a1-ee08-4657-8ace-5ad804f493ae`, recorded by `32ddcb8f-81c7-41bf-abe9-fe96f076c380`, passed 7,501 / 7,501 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `1f6f634f-c78d-4896-8d91-1fa0eef64dda`, snapshot `10af8726-f6de-436a-a7a9-467410ae8094`, measured 49,472 / 54,149 lines, 9,712 / 12,522 branches, 3,382 / 3,832 functions, and 68,096 / 75,315 regions in 96.161 seconds; the C-ABI scorecard run `69a771ef-32c3-4321-b0a2-6a06f17761e8` reports 5,244 / 5,244 runtime rows and 7,501 / 7,501 no-fallback routes. |
| 51 | `70c7e218d1f20ad10259444d5be9dcfc8072e5fc` | Replaced the partial CFF SID mapping with the complete 391-entry standard-string table, added a derived CID-keyed ROS fixture using standard `Black` (383) and `Bold` (384) SIDs, and routed it through pinned C, Rust FFI, C ABI, and WASM. Clean full parity run `2c3c0a6e-d9d9-48b1-b6af-62f338e8995c`, recorded by `32d5092a-5434-47fe-b941-01d188e52340`, passed 7,502 / 7,502 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `a62b2d8d-5488-4c80-bcf7-298149f6913d`, snapshot `05be06e0-d390-4752-91c1-ce596cec31e4`, measured 49,474 / 54,152 lines, 9,715 / 12,526 branches, 3,382 / 3,832 functions, and 68,098 / 75,318 regions in 50.482 seconds; the C-ABI scorecard run `d48f445c-3cac-4497-b66d-fb8ccc6930ea` reports 5,245 / 5,245 runtime rows and 7,502 / 7,502 no-fallback routes. |
| 52 | `887b9700048feed0416b34babee11a2c09f88fad` | Added a safe public `FT_Outline` record-corruption case for `FT_Glyph_To_Bitmap`: a nonzero contour count with a null contours pointer is rejected before Rust rendering while preserving the caller handle and destroy behavior across pinned C, Rust FFI, C ABI, and WASM. Source-bound parity run `657b5c9b-e9c6-4639-9448-42e3d626ad59`, recorded by `b96b0e65-11d1-4039-9c6e-3da782a26bfe`, passed 7,503 / 7,503 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `7f80356b-6051-49a1-a80b-9d3e31308f58`, snapshot `cc31ef3c-be78-4ec8-9b8b-9509a16d1cd5`, measured 49,495 / 54,173 lines, 9,724 / 12,534 branches, 3,384 / 3,834 functions, and 68,127 / 75,345 regions in 119.434 seconds; the C-ABI scorecard reports 5,246 / 5,246 runtime rows and 7,503 / 7,503 no-fallback routes. |
| 53 | `fa43358046e4a738973cf7dff5cdfff5a76a10b9` | Added a derived CFF face whose ROS registry uses FreeType's `0xFFFF` absent-CID sentinel, then routed the resulting `FT_Get_CID_Registry_Ordering_Supplement` `Invalid_Argument` and output-preservation behavior through pinned C, Rust FFI, C ABI, and WASM. Source-bound parity run `09678b89-dd92-4144-850e-2f2be979ae4c`, recorded by `61608dae-a07e-43ca-9e2d-2e864de7d6df`, passed 7,504 / 7,504 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `1553c8ad-369f-4c7a-922a-5c458f8fe6b4`, snapshot `a055abb2-28be-4450-aa78-b62b0bc9f33a`, measured 49,498 / 54,173 lines, 9,727 / 12,534 branches, 3,384 / 3,834 functions, and 68,130 / 75,345 regions in 89.882 seconds; the C-ABI scorecard run `3bb476bb-3739-45a9-a524-93f9b87a9de5` reports 5,247 / 5,247 runtime rows, 661 / 661 exact-error routes, and 7,504 / 7,504 no-fallback routes. |
| 54 | `e558f9b306ea1af408b05d3c0140deeaa279180f` | Added the maintained zero-width, non-empty-row null-buffer `FT_Bitmap_Convert` input and routed C's zero-byte allocation and field behavior through pinned C, Rust FFI, C ABI, and WASM. Source-bound parity run `7d36f0c2-ae32-4cba-9aba-6d0e7d7ca2f2`, recorded by `a7ca74f9-3132-4615-b593-3b8016b0f569`, passed 7,505 / 7,505 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `5f84841d-26e8-49c5-be83-1cee05420341`, snapshot `303a72ac-0257-467d-9f93-badf84ec59bb`, measured 49,498 / 54,173 lines, 9,729 / 12,534 branches, 3,384 / 3,834 functions, and 68,131 / 75,345 regions in 50.861 seconds; the C-ABI scorecard run `550910b8-7fd9-4a05-b0f6-5f4228685b2f` reports 5,248 / 5,248 runtime rows, 661 / 661 exact-error routes, and 7,505 / 7,505 no-fallback routes. |
| 55 | `65ae15537ca0196f1939c8040e14bdbc115473e5` | Matched FreeType's `tt_face_lookup_table` rule that zero-length SFNT directory entries are missing for `FT_Load_Sfnt_Table`, added the maintained zero-length `EBDT` input, and stabilized per-memory allocator accounting in the C-ABI final-destroy parity harness. Clean source-bound parity run `1d40c267-32d5-458d-98fc-7ed165738f2b`, recorded by `e87cab05-9fd3-48fd-a006-0a3c7a5f1ce7`, passed 7,506 / 7,506 runnable comparisons with 3 explicitly pending safety-extension cases. |
| 56 | `e00c3fb9882eb8e817a1a6a97a1f9af5cc412088` | Refreshed the committed source-bound parity, coverage, and C-ABI evidence after the zero-length SFNT table route: warm all-lane Coverage MCP run `7ba15050-e5d5-47ec-94ae-2f74d1ab4cd0`, snapshot `403688ee-af78-4314-b4e5-a56462974146`, passed 7,506 / 7,506 comparisons in 50.204 seconds and measured 49,500 / 54,175 lines, 9,729 / 12,534 branches, 3,385 / 3,835 functions, and 68,131 / 75,345 regions; scorecard run `6de87054-ad88-4147-b269-9089e394d6a3` reports 5,249 / 5,249 runtime rows, 662 / 662 exact-error routes, and 7,506 / 7,506 no-fallback routes. |
| 57 | `95f3a7d3521d021fd73c1c8ba9ab834367d0d632` | Added the maintained zero-resolution WinFNT fixture and exact `FT_Bitmap_Size`/`FT_FaceRec` available-size parity inputs, matching pinned FreeType's 72 dpi defaults when both device resolutions are zero. Clean source-bound parity run `a7a2e636-4d43-4bba-bfb9-cec3050f3a5c`, recorded by `136854fa-3dc5-40b3-b3e5-3948e77485af`, passed 7,508 / 7,508 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `e96ecf03-aca8-4ed9-88ef-328e9f3aa232`, snapshot `17fc995d-aefe-4a4c-a3be-08433ca4be26`, measured 49,502 / 54,175 lines, 9,731 / 12,534 branches, 3,385 / 3,835 functions, and 68,133 / 75,345 regions in 50.369 seconds. |
| 58 | `f0ea3ae3765e08e18acab174d1a971e7b8b2aa56` | Added the project-authored 12-byte PCF zero-table-count input to reach the pinned driver's `Invalid_File_Format` count check beyond the separate eight-byte stream-operation boundary. Focused parity run `e6c9705a-56ad-48e7-a8de-bff98d32ce33` passed all 11 expanded comparisons; clean source-bound parity run `e2d47891-5bbf-4735-8c96-a6161bab1cfa`, recorded by `a4960caf-8606-4947-a2dc-5a2a46bf240b`, passed 7,509 / 7,509 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `5a74c894-77a7-473a-88af-bf5707fd9a5c`, snapshot `5c20fc7d-81bc-4d1c-a84b-b89798b876ac`, measured 49,506 / 54,175 lines, 9,731 / 12,534 branches, 3,387 / 3,835 functions, and 68,139 / 75,345 regions in 51.347 seconds. The follow-up C-ABI scorecard run `6ebe44c4-3457-473c-bf35-9d6ccadb9a75` reports 10 / 12 categories complete, 5,252 / 5,252 runtime rows, 663 / 663 exact-error routes, and 7,509 / 7,509 no-fallback routes. |
| 59 | `85ec98773299101943b4ca9c3e3acc6d810018f6` | Added the project-authored eight-byte PCF TOC header with one table count but no directory bytes, reaching the pinned driver's `stream->size < 16` `Invalid_File_Format` guard before the first directory entry. Focused parity run `b71ec225-ad23-447f-9f25-5b1dd6303d81` passed all 12 expanded comparisons; clean source-bound parity run `ede050d6-1651-4ffc-a1cd-3b63c84e20d0`, recorded by `95865556-3fdf-4368-a569-d120e99de68f`, passed 7,510 / 7,510 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `5a39d129-3b01-469b-9fef-a8a3b7c5b61f`, snapshot `0e697c95-b596-4f23-bf5a-05ece61c175c`, measured 49,507 / 54,175 lines, 9,733 / 12,534 branches, 3,387 / 3,835 functions, and 68,140 / 75,345 regions in 51.362 seconds. The follow-up C-ABI scorecard run `70566cb3-b130-4f17-b013-cae6cd2843b9` reports 10 / 12 categories complete, 5,253 / 5,253 runtime rows, 664 / 664 exact-error routes, and 7,510 / 7,510 no-fallback routes. |
| 60 | `e4637b4bea8a6b0ede612b241df3f8ac89d45aac` | Added the project-authored 24-byte PCF TOC with one zero-length `SWIDTHS` entry and no required properties table, reaching the pinned driver's missing-required-table error path. Focused parity run `fda37118-a323-4113-af05-19b050a979b0`, clean full parity run `b4596d6b-c372-4714-9e24-08a1a3273df8`, recorded by `d8b0a763-25d8-4830-bcaa-b6dec30bd229`, establish 7,511 / 7,511 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `3fa7d660-61a0-4e9c-a665-80d1388edcfd`, snapshot `f123d96e-2619-4825-8721-f990a9e42456`, measured 49,508 / 54,175 lines, 9,733 / 12,534 branches, 3,388 / 3,835 functions, and 68,143 / 75,345 regions in 50.635 seconds. The clean C-ABI scorecard run `1f2ab790-e813-4f74-b9f6-4ad812e1f980` reports 10 / 12 categories complete, 5,254 / 5,254 runtime rows, 665 / 665 exact-error routes, and 7,511 / 7,511 no-fallback routes. |
| 61 | `ee97486e9747a0d03af96a48b1c073034e8a1e81` | Added five project-authored malformed WinFNT inputs for unsupported version, vector file type, zero pixel height, reversed character range, and out-of-range face-name offset, reaching the pinned `Unknown_File_Format` and `Invalid_File_Format` validation routes. Focused parity run `e422c0cb-002b-4319-89b3-ec28e683d185`, clean full parity run `9814e704-01dd-4b55-8b93-990c61dc6256`, recorded by `f6c6d8a5-ecc5-4006-8e3d-a1ba0bfb533b`, establish 7,516 / 7,516 runnable comparisons with 3 explicitly pending safety-extension cases. The clean all-lane Coverage MCP run `27e96318-7026-40f0-8fd9-3e5f231a7028`, snapshot `dd671bb9-e27f-432f-98b6-207c2f3c6c5d`, measured 49,514 / 54,175 lines, 9,737 / 12,534 branches, 3,388 / 3,835 functions, and 68,146 / 75,345 regions in 51.747 seconds. The clean C-ABI scorecard run `e294e40f-0b61-497a-b346-88d4e45ade81` reports 10 / 12 categories complete, 5,259 / 5,259 runtime rows, 670 / 670 exact-error routes, and 7,516 / 7,516 no-fallback routes. |
| 62 | `c4f187fe9125e0b41b738142a60580b0654576f9` | Added a valid version-2 WinFNT fixture and header-copy route. The first full parity attempt exposed that Rust zeroed `reserved1` for v2 even though pinned `winfnt.c` reads the fixed 148-byte header frame before applying v2's zeroing rules; the parser now matches that order. Source-bound full parity run `a1264e72-7d3e-4167-935f-9bfc813950d5`, recorded by `85ecb99d-392e-4687-ab0f-13adcd4d90d7`, passes 7,517 / 7,517 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `4f77364e-06b5-48ce-94fd-89519613a972`, snapshot `e78240ed-31c4-4e93-abcb-71c20f199f35`, measures 49,519 / 54,173 lines, 9,743 / 12,532 branches, 3,388 / 3,835 functions, and 68,151 / 75,343 regions in 98.336 seconds. The C-ABI scorecard run `7b4a61e8-3c97-4d7f-925b-ebaf7a996323` reports 10 / 12 categories complete, 5,260 / 5,260 runtime rows, 670 / 670 exact-error routes, and 7,517 / 7,517 no-fallback routes. |
| 63 | Working tree (PCF endian property routes) | Added the maintained MSB-first PCF properties and encoding-table fixture, expanded `FT_Get_BDF_Property` into explicit little-endian and big-endian variants, and normalized variant case IDs in the BDF oracle/FFI/C-ABI/WASM dispatch after the first full run exposed the harness fallback. Source-bound full parity run `4138ffc6-cfa9-481a-bc5d-f8c8b1f4108f`, recorded by `5fa05f6e-a08c-4023-8d2e-14d7ad9f420a`, passes 7,518 / 7,518 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `5409f603-27d7-4e8e-9bfe-9e4fadc7962c`, snapshot `c6598315-d1e1-4d05-8a7b-df535185d651`, measures 49,521 / 54,173 lines, 9,745 / 12,532 branches, 3,388 / 3,835 functions, and 68,155 / 75,343 regions in 51.653 seconds. |
| 64 | Working tree (GX validator module absence) | Added the maintained `FT_TrueTypeGX_Validate` case for an openable GX face with the optional `gxvalid` module absent, and routed the real `FT_Err_Unimplemented_Feature` plus zero-length output preservation through the pinned C oracle, Rust FFI, C ABI, and WASM harnesses. Source-bound full parity run `5567be2d-f932-4886-92b7-29e19fddfc28`, recorded by `71752538-1b06-4ee6-82e3-b84693bc11c3`, passes 7,519 / 7,519 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `9c9b7ebb-9aff-4982-8573-bfc4671ee529`, snapshot `bf8b7b67-3658-459e-b918-4587fecdb455`, measures 49,523 / 54,173 lines, 9,747 / 12,532 branches, 3,388 / 3,835 functions, and 68,157 / 75,343 regions in 100.333 seconds including the source-bound instrumented rebuild. The corrected C-ABI scorecard run `4cae8afe-429a-498f-b9cf-4b50971db5e5` reports 10 / 12 categories complete, 5,262 / 5,262 runtime contract rows, and 671 / 671 strict error routes exact. |
| 65 | Working tree (PCF metric and malformed-input routes) | Added an uncompressed six-field PCF metrics fixture to the existing BDF-property route, plus invalid-version, overlapping-table, and properties-format PCF controls to the memory-face error matrix. Focused parity runs `a92895b5-6503-4582-b679-e05f6fd32119` (3 / 3) and `e9578db4-41bd-4c34-9461-8529a04ef651` (21 / 21) passed. Source-bound full parity run `404f312d-788e-4573-9021-0efd1d2a72b5`, recorded by `98a016eb-e121-460c-b317-ca321e584e46`, passes 7,523 / 7,523 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in Rust FFI, C ABI, and WASM. The warm all-lane Coverage MCP run `fb9ed28c-46d3-4a3d-beb9-1d576ba385cc`, snapshot `1d2bd6e8-2948-43f9-96f4-e4394b139214`, measures 49,543 / 54,173 lines, 9,752 / 12,532 branches, 3,391 / 3,835 functions, and 68,204 / 75,343 regions in 50.842 seconds. The C-ABI scorecard run `71be95b2-1305-439f-a5c6-b4c6981dc87f` reports 10 / 12 categories complete, 5,266 / 5,266 runtime contract rows, 674 / 674 exact-error routes, and 7,523 / 7,523 no-fallback routes; Windows import-library evidence and four platform bundles remain. |
| 66 | Working tree (WinFNT header-size validation routes) | Added four deterministic WinFNT controls for short v2 headers, short v3 extended headers, declared file sizes below the required header, and declared sizes beyond the input stream. Focused parity run `cf1e884d-4078-49d5-b4d0-33bf44a9372d` passed 25 / 25. Source-bound full parity run `77d5f9de-66e8-4821-8287-71a1c9d3d7f3`, recorded by `6478c34b-6c6c-4b35-baa7-cdee234f736b`, passes 7,527 / 7,527 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in Rust FFI, C ABI, and WASM. The source/input-bound all-lane Coverage MCP run `0573714b-462f-4a73-afdb-f3a55163b75d`, snapshot `b3a8c64a-2abc-4e36-ac93-8d8302a15d75`, measures 49,548 / 54,173 lines, 9,756 / 12,532 branches, 3,391 / 3,835 functions, and 68,207 / 75,343 regions in 77.454 seconds. The C-ABI scorecard run `0dd97d49-2754-435a-bbe9-afd542ae3d0f` reports 10 / 12 categories complete, 5,270 / 5,270 runtime contract rows, 678 / 678 exact-error routes, and 7,527 / 7,527 no-fallback routes; Windows import-library evidence and four platform bundles remain. The parser's unsupported-version branch remains unclaimed because public dispatch rejects that version before entering the parser. |
| 67 | Working tree (BDF property parser controls) | Added a maintained BDF property row for `RESOLUTION_X` cardinal output, corrected the pinned-C oracle case generator to emit that row, and extended the existing SFNT `BDF ` strike with undefined, cardinal, and unknown-format records so the public `FT_Get_BDF_Property` strike route executes the parser skip/cardinal/unknown branches. Focused parity runs `92517f05-44e4-4c01-8ed4-231a09007a99` (1 / 1) and `0041a79d-a5a7-451d-88cc-8fa0e9ca6b82` (1 / 1) passed. Source-bound full parity run `46b07241-7dbc-4afa-99a6-9436344e803f`, recorded by `affe5136-7402-45d8-ac2c-2233b1c2a1e8`, passes 7,527 / 7,527 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in Rust FFI, C ABI, and WASM. The source/input-bound all-lane Coverage MCP run `bc1b4361-bf2d-45fa-8816-5f2b6a348040`, snapshot `fd4f0598-e3a6-4e78-ba83-92e98f443fac`, measures 49,567 / 54,173 lines, 9,760 / 12,532 branches, 3,391 / 3,835 functions, and 68,226 / 75,343 regions in 50.505 seconds. The C-ABI scorecard run `8710ea2a-150b-4481-902e-b93481ff08e9` reports 10 / 12 categories complete, 5,270 / 5,270 runtime contract rows, 678 / 678 exact-error routes, and 7,527 / 7,527 no-fallback routes; Windows import-library evidence and four platform bundles remain. The WinFNT unsupported-version parser branch and SFNT BDF invalid-version branch remain unclaimed because no maintained public parity input currently reaches either route. |
| 68 | Working tree (PCF table-range controls) | Added two deterministic PCF controls to the existing memory-face error matrix: one table range extending past the stream and one beginning inside the directory, covering both disjuncts of the Rust table-range guard. Focused parity runs `877c57f2-5dea-4939-afa7-6b76a097c299` (26 / 26) and `59a15b47-7a0f-49f9-bf7f-4f3cb734c7ee` (27 / 27) passed. Source-bound full parity run `e05183dc-b491-41c1-bd68-a388df50d92b`, recorded by `33fadc62-4f58-472a-ae73-2b91c979bdc2`, passes 7,529 / 7,529 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in Rust FFI, C ABI, and WASM. The source/input-bound all-lane Coverage MCP run `43067a3b-392e-4e67-a725-66731b62aa92`, snapshot `e80396af-59c6-471b-a4c7-26e1e1001c61`, measures 49,568 / 54,173 lines, 9,762 / 12,532 branches, 3,391 / 3,835 functions, and 68,227 / 75,343 regions in 49.844 seconds; `src/font.rs` line 466 is no longer a coverage gap. The C-ABI scorecard run `475b5afb-f9dd-4c41-8213-2871aba091cf` reports 10 / 12 categories complete, 5,272 / 5,272 runtime contract rows, 680 / 680 exact-error routes, and 7,529 / 7,529 no-fallback routes. The next source-bound parser gap is the unsupported-properties-format branch; the WinFNT unsupported-version and SFNT BDF invalid-version parser branches remain unclaimed because no maintained public parity input currently reaches them. Windows import-library evidence and four platform bundles remain. |
| 69 | Working tree (PCF unsupported-properties-format control) | Added a deterministic PCF properties payload with non-default high format bits, proved against pinned `PCF_FORMAT_MATCH(format, PCF_DEFAULT_FORMAT)`, and routed it through the existing memory-face error matrix to cover the first disjunct of the Rust unsupported-properties-format guard. Focused parity run `5e39a937-54f3-45fa-91e7-68364be272ce` passed 28 / 28. Source-bound full parity run `450e0831-59cc-47df-b712-9d0af3a902c6`, recorded by `d9f09ff5-6aa5-45ec-8409-e47384fd6f11`, passes 7,530 / 7,530 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in Rust FFI, C ABI, and WASM. The source/input-bound all-lane Coverage MCP run `9179524b-915a-44c2-83ac-5bbd8729ee33`, snapshot `b3b9b6a4-2ff8-44bd-b086-688eb50b2270`, measures 49,568 / 54,173 lines, 9,763 / 12,532 branches, 3,391 / 3,835 functions, and 68,227 / 75,343 regions in 50.695 seconds; `src/font.rs` line 506 is no longer a coverage gap. The C-ABI scorecard run `0f9b6670-c0d1-4bc6-a410-e6b853c40ff0` reports 10 / 12 categories complete, 5,273 / 5,273 runtime contract rows, 681 / 681 exact-error routes, and 7,530 / 7,530 no-fallback routes. The next measured source gap is the WinFNT header validation at `src/font.rs:582-583`; the WinFNT unsupported-version and SFNT BDF invalid-version parser branches remain unclaimed because no maintained public parity input currently reaches them. Windows import-library evidence and four platform bundles remain. |
| 70 | Working tree (SFNT-BDF malformed, BDF property, and parity-speed routes) | Added four maintained malformed optional `BDF ` SFNT controls and a duplicate/empty-property BDF control, fixed the WinFNT v2/v3 dispatch and standalone fallback behavior, and kept all routes on the existing pinned-C/Rust FFI/C ABI/WASM parity surfaces. The final source-bound parity run `f9219e9f-3cb3-45b4-9a8b-db05090c03bf`, recorded by `0a343e2c-5563-4a43-9359-00d0bbb7fd7b`, passes 7,535 / 7,535 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes. The incremental per-case oracle cache and face-grouped worker scheduler reduced the warm full-matrix direct run from 227.03 seconds to 192.75 seconds. The all-lane Coverage MCP run `86f900f3-fa3b-4dc0-b5d8-aa3b95a41ac6`, snapshot `514392ce-3e68-473b-8c51-865bd55ce454`, measures 49,582 / 54,177 lines, 9,773 / 12,534 branches, 3,391 / 3,835 functions, and 68,250 / 75,357 regions in 99.876 seconds. The C-ABI scorecard run `f6ff4005-ed38-478c-8aa1-4d6ebd1651cf` reports 10 / 12 categories complete, 5,278 / 5,278 runtime contract rows, and 7,535 / 7,535 no-fallback routes; the Windows import-library item and four platform bundles remain. The next measured coverage focus is branch coverage in `fontdone-c-abi/src/implementation.rs` (1,375 / 2,052) and `fontdone-wasm/src/implementation.rs` (1,104 / 1,656). |

| 71 | Working tree (FT_Glyph_Transform invalid-root route) | Routed the existing `ftglyph.FT_Glyph_Transform.error_null_or_bad_glyph` input through an actual pinned-C probe and the Rust FFI, C ABI, and WASM backends instead of the generic no-face fallback. Focused parity run `6d230680-fda8-42a3-868d-3dd021d6c577` passed 1 / 1. Source-bound full parity run `a4731863-3756-4988-8796-d12b58413ff9` passes 7,535 / 7,535 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes. The source-bound all-lane Coverage MCP run `65a2d2aa-2356-4f8e-b02a-b60dac78a025`, snapshot `8a1afdcf-e688-41af-84ca-be248623e71c`, measures 49,591 / 54,177 lines, 9,780 / 12,534 branches, 3,391 / 3,835 functions, and 68,261 / 75,357 regions in 94.253 seconds; mutable outline/SVG root-class rejection is now exercised, while the non-mutable SVG rejection remains uncovered at `fontdone-c-abi/src/implementation.rs:1044` and `fontdone-wasm/src/implementation.rs:706`. The C-ABI scorecard run `937d8ab7-d805-4bb3-ae30-31baba850ad2` reports 10 / 12 categories complete, 5,278 / 5,278 runtime contract rows, 681 / 681 strict error routes, and 7,535 / 7,535 no-fallback routes. Windows import-library evidence and four fresh target-lane bundles remain. |

| 72 | Working tree (coverage matrix runtime) | Grouped the maintained `FT_Prop_IncreaseXHeight.limit_changes_autohint_x_height` matrix by its pinned face-scoped lifecycle: one fresh face per `(font, limit, ppem)` cell, followed by the three requested glyph loads. The refreshed all-lane Coverage MCP run `c1bc9991-b54f-405c-bf03-22b83752a230`, snapshot `934ff05d-8c38-4c56-96ca-d5477f27576e`, passed 7,537 / 7,537 comparisons in each backend with 0 failures, reduced the source-bound wall time from 103.059 to 93.856 seconds, and measured 49,624 / 54,186 lines, 9,814 / 12,538 branches, 3,391 / 3,835 functions, and 68,297 / 75,365 regions. Coverage remains 91.58% line, 78.27% branch, 88.42% function, and 90.62% region; the remaining roadmap blockers are unchanged: four fresh non-host platform bundles and Windows import-library evidence for the C-ABI completion contract. |

| 73 | Working tree (FTC_Node_Unref null-manager branch) | Added the defined `ftcache.FTC_Node_Unref.null_or_invalid_inputs_noop` variant with a non-null foreign node and a null manager, exercising the pinned early-return path without dereferencing an invalid node. Focused parity run `3b7c3210-ee18-40da-81a3-1aa7cf67da07` passed 1 / 1; the official full parity run `5a310945-27fb-43bb-b005-9ba809f2e45f` passed 7,537 / 7,537 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `b0194751-8e81-4d73-a17d-d6ae1a636c71`, snapshot `21949c21-abcd-4b97-b5c8-a9badd976487`, completed in 89.100 seconds and measured 49,624 / 54,186 lines, 9,815 / 12,538 branches, 3,391 / 3,835 functions, and 68,297 / 75,365 regions; the C-ABI `FTC_Node_Unref` guard reached 4 / 4 branches. Four fresh non-host platform bundles and Windows import-library evidence remain for the C-ABI completion contract. |
| 74 | Working tree (bitmap glyph copy null-buffer parity) | Added the maintained `ftglyph.FT_Glyph_Copy` bitmap variant that clears the public source `bitmap.buffer` while preserving its descriptor, matching pinned `FT_Bitmap_Copy` success semantics without dereferencing a null pointer. Focused run `a681a38f-aa5e-4ee8-ab81-e2e107722d9f` passed 3 / 3; final source-bound parity run `a1dec5dc-c658-45a3-9a0c-3836f7abe2c6` passed 7,538 / 7,538 runnable comparisons with 3 explicitly pending safety-extension cases. The all-lane Coverage MCP run `a7a11b61-aae7-4fa0-8f66-d54e6bd0bdc1`, snapshot `90a240aa-1197-401d-9cdf-682c62ba7025`, completed in 91.262 seconds and measured 49,664 / 54,227 lines, 9,824 / 12,548 branches, 3,395 / 3,839 functions, and 68,346 / 75,418 regions. Four fresh non-host platform bundles and Windows import-library evidence remain for the C-ABI completion contract. |
| 75 | Working tree (callback-backed bzip2 stream route) | Added the maintained `ftbzip2.FT_Stream_OpenBzip2.success_open_callback_bzip2_stream` input and exact pinned-C/Rust FFI/C ABI/WASM observations for a non-empty caller-owned `FT_Stream_IoFunc` source with `base = NULL`, preserving source fields and ownership. The C ABI and WASM facades materialize callback-backed bytes only while the foreign callback is active, then reacquire the source for the core wrapper call. Final source-bound parity run `138d472f-5170-4bf3-bc47-12a83b862dc3` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes. C-ABI scorecard run `b4390b09-2536-4de8-b2fd-97fc6009464e` reports 10 / 12 categories complete, 5,282 / 5,282 runtime contract rows, and 7,539 / 7,539 no-fallback routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 76 | Working tree (image-cache remove-face-ID parity route) | Aligned the existing `ftcache.FTC_Manager_RemoveFaceID` C-ABI harness with the pinned oracle's actual image-cache lifecycle: it now creates an image cache, retains one node across face-ID removal, releases it after removal, and verifies both removed-face re-request and preserved other-face lookup. The maintained fixture and pinned-C oracle remain unchanged; this closes the previously unexecuted image-cache invalidation branch without adding synthetic scope. Full parity run `5d5b5986-c6cb-4953-a10e-a72e8711bcce` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `b8b246c4-a7d9-4d39-8f08-aeb4694679f4`, snapshot `54a29596-5f12-4598-b272-2ca8df957b63`, measured 49,786 / 54,356 lines, 9,853 / 12,582 branches, 3,399 / 3,847 functions, and 68,499 / 75,578 regions in 89.542 seconds; C-ABI branch coverage moved from 1,407 / 2,062 to 1,417 / 2,074. C-ABI scorecard run `fb52f881-2a37-46b5-9d83-ca1dd8b2e259` reports 10 / 12 categories complete, 5,282 / 5,282 runtime contract rows, 683 / 683 strict-error routes, and 7,539 / 7,539 no-fallback routes. Windows import-library evidence and four fresh target-lane bundles remain. |
| 77 | Working tree (CMap-cache exported lifecycle parity route) | Replaced the C-ABI CMap-cache backend's direct face/charmap helper model with the real exported `FTC_CMapCache_New` and `FTC_CMapCache_Lookup` manager sequence, including actual `FTC_Manager_RemoveFaceID` and `FTC_Manager_Reset` lifecycle calls. The maintained fixture and pinned-C oracle remain unchanged; this closes the previously bypassed exported CMap-cache route without adding synthetic scope. Full parity run `06950ec3-0954-47ae-8271-d66df83c8bf9` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `3a32c0bc-1f21-42d2-9ade-076d8d59e266`, snapshot `361294c8-673e-4e19-85dd-610e28824b05`, measured 49,802 / 54,356 lines, 9,862 / 12,582 branches, 3,400 / 3,847 functions, and 68,531 / 75,578 regions in 89.806 seconds; C-ABI branch coverage moved from 1,417 / 2,074 to 1,426 / 2,074. C-ABI scorecard run `b2185092-359a-411a-8a84-01bfe8609824` reports 10 / 12 categories complete, 5,282 / 5,282 runtime contract rows, 683 / 683 strict-error routes, and 7,539 / 7,539 no-fallback routes. Windows import-library evidence and four fresh target-lane bundles remain. |
| 78 | Working tree (exported image-cache lookup lifecycle parity route) | Replaced the C-ABI `ftcache.image_cache_lookup` direct-face/`FT_Load_Glyph` helper with the real requester-backed `FTC_Manager_New`, `FTC_ImageCache_New`, and `FTC_ImageCache_Lookup` sequence, including repeated cache-hit lookups, manager-owned glyph snapshots, and `FTC_Node_Unref`. The maintained fixture and pinned-C oracle remain unchanged; this closes the previously bypassed exported image-cache lookup route without adding synthetic scope. Full parity run `c41445de-10a4-4a5d-a353-414785a283d3` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `0ea25900-4164-463c-8083-597f2c610921`, snapshot `3fdd6bb8-e1b6-4326-a1a0-6246ef91131e`, measured 49,816 / 54,356 lines, 9,864 / 12,582 branches, 3,401 / 3,847 functions, and 68,548 / 75,578 regions in 90.116 seconds; C-ABI branch coverage moved from 1,426 / 2,074 to 1,428 / 2,074. C-ABI scorecard run `6d286f36-893b-4d7b-8908-cfd6abd5a245` reports 10 / 12 categories complete, 5,282 / 5,282 runtime contract rows, 683 / 683 strict-error routes, and 7,539 / 7,539 no-fallback routes. Windows import-library evidence and four fresh target-lane bundles remain. |
| 79 | Working tree (exported image-cache scaler parity route) | Replaced the C-ABI `ftcache.image_cache_lookup_scaler` direct-face/`FT_Load_Glyph` helper with the real requester-backed `FTC_Manager_New`, `FTC_ImageCache_New`, and `FTC_ImageCache_LookupScaler` sequence, preserving raw `FT_ULong` load flags and releasing each manager-owned node with `FTC_Node_Unref`. The maintained fixture and pinned-C oracle remain unchanged; this closes the previously bypassed exported image-cache scaler route without adding synthetic scope. Full parity run `f63cb35f-e590-43bb-90ba-859788819779` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `0a0c3ca5-12f6-49a3-84fd-16aafb07e2c6`, snapshot `4faba8c3-aea6-498f-852e-3635d82702ea`, measured 49,822 / 54,356 lines, 9,865 / 12,582 branches, 3,401 / 3,847 functions, and 68,556 / 75,578 regions in 86.334 seconds; C-ABI branch coverage moved from 1,428 / 2,074 to 1,429 / 2,074. C-ABI scorecard run `f1638566-ebc5-4494-a162-cc64a002af3a` reports 10 / 12 categories complete, 5,282 / 5,282 runtime contract rows, 683 / 683 strict-error routes, and 7,539 / 7,539 no-fallback routes; the shared/static C consumers were exact on Darwin. Windows import-library evidence and four fresh target-lane bundles remain. |
| 80 | Working tree (exported SBit-cache scaler parity route) | Replaced the C-ABI `ftcache.sbit_cache_lookup_scaler` direct-face/`FT_Load_Glyph` helper with the real requester-backed `FTC_Manager_New`, `FTC_SBitCache_New`, and `FTC_SBitCache_LookupScaler` sequence, preserving raw `FT_ULong` load flags and manager-owned SBit snapshots while matching the pinned oracle's bitmap-field representation at the parity boundary. Full parity run `6668ef13-e1cc-44dc-b4b5-516d2253bfbd` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `2869a82c-0698-493a-932d-32220e82508d`, snapshot `c364b208-1c90-44e4-98de-46d38ab889aa`, measured 49,871 / 54,394 lines, 9,875 / 12,594 branches, 3,405 / 3,850 functions, and 68,622 / 75,630 regions in 92.551 seconds; C-ABI branch coverage moved from 1,429 / 2,074 to 1,439 / 2,086. C-ABI scorecard run `a84698c1-7473-47c8-a8ae-0dc562bfe2da` remains 10 / 12 categories complete; Windows import-library evidence and four fresh target-lane bundles remain. |
| 81 | Working tree (exported SBit-cache scaler node-output parity route) | Kept the maintained SBit-cache scaler fixture and exported `FTC_SBitCache_LookupScaler` route, but now supplies the caller's node output pointer so the manager-owned node is observed and released through `FTC_Node_Unref` before the parity-boundary adapter preserves the pinned oracle's direct-face output shape. Full parity run `83458c27-c65c-45d2-8d2d-609f260e2926` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `b5b428a8-0008-47fe-a606-d1ce8efa9ea5`, snapshot `8f754963-2323-466e-a786-4c7d881bcaa5`, measured 49,871 / 54,394 lines, 9,875 / 12,594 branches, 3,405 / 3,850 functions, and 68,622 / 75,630 regions in 95.454 seconds; aggregate coverage totals were unchanged. The measured lane timers were 43.802 seconds Rust FFI, 32.013 seconds C ABI, and 31.892 seconds WASM; C-ABI scorecard remains 10 / 12 categories complete, with Windows import-library evidence and four fresh target-lane bundles remaining. |
| 82 | Working tree (bitmap glyph transform parity route) | Routed the maintained `ftglyph.FT_Glyph_Transform.error_non_scalable_bitmap` case through an actual pinned-C bitmap-transform probe, the pure-Rust bitmap semantic helper, and the real C-ABI/WASM `FT_Glyph_Transform` calls. The route preserves bitmap fields and advance on the pinned `FT_Err_Invalid_Glyph_Format` result; the descriptive setup metadata is now parsed without falling back to a generic error route. Full parity run `fcef97a3-0ec6-43b2-ba43-79613f68bce2` passed 7,539 / 7,539 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each have 218 / 218 function-route evidence. C-ABI scorecard run `fda455d9-86d8-45ca-aaf6-0a2178c0400a` reports 218 / 218 verified functions, 1,003 / 1,003 exact unique cases, and 683 / 683 strict-error routes; `FT_Glyph_Transform` is entered and verified in 5 / 5 candidate cases. The cold all-lane coverage run `adf4c9fa-c972-4dbc-bdba-f9e3505379e2`, snapshot `791315ec-3edd-4ff9-a956-72ff15cf158e`, measured 49,881 / 54,403 lines, 9,878 / 12,596 branches, 3,406 / 3,851 functions, and 68,629 / 75,636 regions in 121.804 seconds; the same-source warm repeat `53c8e8ce-801e-4d7b-8495-cc5d09403b79`, snapshot `353b0be8-324a-4fc9-9f24-e36fff5c97bc`, completed in 46.969 seconds. The denominator stayed at 7,539 because this replaces the old generic fallback with a real route rather than adding scope. Windows import-library evidence and four fresh target-lane bundles remain. |

| 83 | Working tree (SBit cache load-error sentinel parity route) | Aligned the pure-Rust `FTC_SBitCache` and exported C-ABI `FTC_SBitCache_Lookup` implementations with pinned `ftcsbits.c`: every glyph-load failure, including calls without `FT_LOAD_SBITS_ONLY`, becomes a successful unavailable-SBit sentinel instead of propagating the load error. Added the maintained input-only `ftcache.FTC_SBitCache_Lookup.missing_bitmap_without_sbits_only_caches_unavailable_sbit` case. Source-bound parity run `57007a2f-3fee-4556-975b-22f7cdfb03d8` passed 7,540 / 7,540 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `4f87db8f-92b5-4fae-b861-aa5d6cd9306e`, snapshot `11b9565d-9cca-47ef-bbca-4512a012b96d`, measured 49,879 / 54,395 lines, 9,877 / 12,592 branches, 3,404 / 3,849 functions, and 68,624 / 75,626 regions. C-ABI scorecard run `923526b2-41b8-4839-a057-5e2e02145b7d` reports 10 / 12 categories complete, 5,283 / 5,283 runtime rows, and 7,540 / 7,540 no-fallback routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 84 | Working tree (BDF ENDFONT-before-ENDCHAR error route) | Added the maintained input-only `fterrdef.FT_Err_Corrupted_Font_Glyphs.bdf_glyph_parse_failure_returns_error@endfont-before-endchar` fixture, reaching pinned `bdflib.c:834-842` while `BDF_GLYPH_BITS_` is active. The existing EOF-corruption variant remains beside it, and the exact-error promotion now handles both variant routes through Rust FFI, C ABI, and WASM. Source-bound parity run `f23fce71-c6b6-4354-b2c0-534cdfd252ee` passed 7,541 / 7,541 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `8a1972fe-557b-4687-867a-866fade33dc6`, snapshot `7717d894-d846-4fc6-a912-21a7b6ba7210`, measured 49,880 / 54,395 lines, 9,878 / 12,592 branches, 3,404 / 3,849 functions, and 68,626 / 75,626 regions in 85.945 seconds; `src/font.rs:1536-1537` is now executed. C-ABI scorecard run `fbb47fb3-fec7-4874-a223-43756e312f84` reports 10 / 12 categories complete, 5,284 / 5,284 runtime contract rows, 684 / 684 exact-error routes, and 7,541 / 7,541 no-generic-fallback routes. Windows import-library evidence and four fresh target-lane bundles remain. |

| 85 | Working tree (exported manager lookup parity routes) | Replaced the C-ABI manager lookup test helpers with the exported `FTC_Manager_LookupFace`, `FTC_Manager_LookupSize`, and `FTC_Manager_RemoveFaceID` sequence, preserving requester counts, cache identity, size metrics, face snapshots, and removal behavior without changing the maintained fixture. Full parity run `cfb2c0f5-6974-49ad-b304-78b9c5f2ead1` passed 7,541 / 7,541 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; the all-lane Coverage MCP run `30b5abcc-ebf8-47bc-9d1c-e58df060654d`, snapshot `62337dca-d381-4b3a-8682-7f4622e9a699`, measured 49,880 / 54,395 lines, 9,878 / 12,592 branches, 3,404 / 3,849 functions, and 68,626 / 75,626 regions. C-ABI scorecard run `6c360fde-9cb7-4f24-83d4-661833fb60c5` remains 10 / 12 categories complete; Windows import-library evidence and four fresh target-lane bundles remain. |
| 86 | Working tree (exported manager-done lifecycle route) | Replaced the C-ABI `ftcache.manager_done` direct-face/`FT_Load_Glyph` helper with the exported `FTC_CMapCache_New`/`Lookup`, `FTC_Manager_LookupFace`/`LookupSize`, `FTC_ImageCache_New`/`LookupScaler`, `FTC_Node_Unref`, and manager-done lifecycle sequence. The maintained fixture remains unchanged. Focused run `0b4b4884-28d2-4664-89c4-4fd848e46703` passed 1 / 1; full parity run `fb6a26cc-8b74-4767-840c-af61419b0959` passed 7,541 / 7,541 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. The all-lane Coverage MCP snapshot `951a0c97-6c2f-4950-ac34-db6263ab498a` remains 49,880 / 54,395 lines, 9,878 / 12,592 branches, 3,404 / 3,849 functions, and 68,626 / 75,626 regions; C-ABI scorecard run `f2db419e-cfd5-4066-a50c-47e61d14f95c` remains 10 / 12 categories complete. Windows import-library evidence and four fresh target-lane bundles remain. |
| 87 | Working tree (Rust `load_char` coverage-speed route) | Reused one worker-local high-level `ApiFace` per existing face key for `load_char`, eliminating repeated face-open/parse work while retaining the null-face and out-of-range fallbacks. Focused run `2345f427-cfa9-4d7a-ac92-93c6d709786a` passed 1,854 / 1,854 cases; full source-bound parity run `69def8d6-3ead-4dc8-ac96-7020741b41b4` passed 7,541 / 7,541 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. The post-change source-bound all-lane Coverage MCP run `2b98fdd4-0780-4a51-8910-d3a74d50085c`, snapshot `bcd39500-84e8-42e0-874a-c23069ae7b0b`, measured the unchanged 49,880 / 54,395 lines, 9,878 / 12,592 branches, 3,404 / 3,849 functions, and 68,626 / 75,626 regions in 85.335 seconds; the same-source warm repeat `e7ccccdb-80f1-4d32-9fa3-d4c06a7c1673`, snapshot `292d059b-daeb-4e7d-ad3c-2451185bd1d7`, completed in 37.220 seconds versus the prior 48.265-second warm baseline. Warm lane timers were 30.545 seconds Rust FFI, 29.756 seconds C ABI, and 29.654 seconds WASM. C-ABI scorecard run `b93ef417-5341-4f97-9552-bca32d625cbc` remains 10 / 12 categories complete; the Windows import-library item and four platform bundles remain. |

| 88 | Working tree (FT_Render_Glyph slot-refresh coverage route) | Changed `FT_Render_Glyph` to render into the existing glyph slot and refresh its public fields in place, avoiding repeated `FT_Face` metadata reconstruction while preserving the source face and render semantics. Focused run `91144282-c800-4cd7-b6d6-1b83176f2bdd` passed 1 / 1 with measured backend work of 9.110 seconds Rust, 8.739 seconds C ABI, 8.711 seconds WASM, and 2.411 ms comparison. Source-bound full parity run `34cb406a-5caa-44e4-9185-916fb0d83ecf`, recorded by `14606f6b-be06-466c-adf1-f789e56175dc`, passed 7,541 / 7,541 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. The source-bound all-lane Coverage MCP run `f588e930-5ce8-4084-88ea-7bb518d8d44a`, snapshot `a24488bf-6fa8-4142-b2fd-03069ef7adab`, completed in 71.425 seconds and measured 49,859 / 54,382 lines, 9,876 / 12,590 branches, 3,402 / 3,848 functions, and 68,603 / 75,611 regions. C-ABI scorecard run `7c549f0f-df19-4dfd-a7a2-d4724db93734` reports 10 / 12 categories complete, 5,284 / 5,284 runtime rows, 684 / 684 strict-error routes, and 7,541 / 7,541 no-generic-fallback routes; Windows import-library evidence and four fresh target-lane bundles remain. |

| 89 | Working tree (palette mutation boundary coverage route) | Extended the maintained `ftcolor.FT_Palette_Select` input matrix with an out-of-range returned-entry mutation, and routed the C-ABI/WASM support helpers through the ignored-mutation path before the existing valid mutation. Full source-bound parity run `3396b820-85b1-4ca1-96f0-fa8767acc000` passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases, 0 pending route-audit items, and 218 / 218 function routes in all three surfaces. All-lane Coverage MCP run `e068f42a-1f89-4504-b438-fe82602f9777`, snapshot `bd628f06-3846-4c7e-8c82-76f26c8d437f`, completed in 73.636 seconds and measured 49,861 / 54,382 lines, 9,878 / 12,590 branches, 3,402 / 3,848 functions, and 68,605 / 75,611 regions; C-ABI branch coverage moved to 1,440 / 2,084 and WASM to 1,143 / 1,682. C-ABI scorecard run `a2f183b9-5bad-44ea-8de5-157dc81367c5` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh platform bundles remain. |
| 90 | Working tree (FT_Bitmap_Blend null-argument ABI guard coverage route) | Reused the existing `ftbitmap.FT_Bitmap_Blend.error_invalid_arguments_or_target_mode` input contract to execute separate null-library, null-source, null-target, and null-target-offset calls through Rust FFI, the exported C ABI, and WASM, while retaining the invalid-target-mode comparison. Full parity run `b9147e02-f872-4b33-a09e-be0f96f445bd` passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases. All-lane Coverage MCP run `c8c64c55-2d63-4fbc-be74-ca40d652b652`, snapshot `9581b9bb-e2c2-4618-8876-7cdd7a0bf5d3`, completed in 93.334 seconds and measured 49,869 / 54,382 lines, 9,888 / 12,590 branches, 3,402 / 3,848 functions, and 68,615 / 75,611 regions; C-ABI coverage moved to 8,751 / 9,761 lines, 1,444 / 2,084 branches, and 11,364 / 12,575 regions, while WASM moved to 6,431 / 7,022 lines, 1,148 / 1,682 branches, and 8,079 / 8,814 regions. C-ABI scorecard run `d9e435e8-e3cd-48e2-b691-5ef40f996880` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh platform bundles remain. |

| 91 | Working tree (FT_Attach_File pathname parity route) | Reused the maintained `freetype.FT_Attach_File` success input and its existing AFM asset, replacing the C-ABI harness's `FT_Attach_Stream` memory helper with the exported `FT_Attach_File` pathname call so the real wrapper and filesystem path are exercised without changing the parity matrix. Full parity run `c40d3d07-0945-4736-982e-5862e46e7c5c` passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. All-lane Coverage MCP run `ab61915f-45f1-4a74-ab87-df0593c02792`, snapshot `a9f872b4-2883-4afd-99a5-55ae84d39c25`, completed in 73.635 seconds and measured 49,886 / 54,382 lines, 9,891 / 12,590 branches, 3,405 / 3,848 functions, and 68,642 / 75,611 regions; the C-ABI wrapper's success path is now covered. C-ABI scorecard run `cb3af2a7-f009-4cf1-95ac-fb1da8e2ee62` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 92 | Working tree (FT_New_Face pathname ABI parity route) | Reused the maintained `freetype.FT_New_Face` pathname/error matrix and routed the C-ABI lane through the exported `FT_New_Face` pathname call, covering the real wrapper and filesystem-backed face-open path without changing the parity matrix; the WASM lane remains on its existing memory-backed route because it has no pathname export. Full parity run `23adc824-8815-4caf-a2f8-f1c9f7086bf4`, recorded by `3232b1fe-c889-49df-ac81-1eb4b09a35e3`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. All-lane Coverage MCP run `2954842b-5f37-4f95-8195-d61c03b5b279`, snapshot `c72734e7-38a0-43c3-b51f-79c2eac36005`, completed in 70.782 seconds and measured 49,918 / 54,382 lines, 9,897 / 12,590 branches, 3,408 / 3,848 functions, and 68,687 / 75,611 regions; the C-ABI `FT_New_Face` wrapper and core pathname success path are now covered. C-ABI scorecard run `d64bfddf-0a3f-43fe-af94-312a79dedc33` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 93 | Working tree (FT_Reference_Face C-ABI parity route) | Reused the maintained `freetype.FT_Reference_Face` success/null matrix and routed the C-ABI lane through the exported `FT_Reference_Face` call, including both wrapper branches, while preserving the existing `refcount` output contract and leaving the WASM lane unchanged. Full parity run `e67db1d2-6b12-4af2-b68a-2881bca4a3a3`, recorded by `f30d9f91-1533-4676-90a2-37f882e3ca85`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. All-lane Coverage MCP run `716a02d6-cb98-4120-8204-fb411c1493e7`, snapshot `a258e327-5954-48cb-868c-2ca8e7df04e5`, completed in 72.219 seconds and measured 49,928 / 54,382 lines, 9,900 / 12,590 branches, 3,409 / 3,848 functions, and 68,698 / 75,611 regions; all C-ABI `FT_Reference_Face` wrapper lines and branches are now covered. C-ABI scorecard run `35d84940-f7e6-41f0-8cc4-deae8b62bf1b` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 94 | Working tree (FT_Get_Transform C-ABI parity route) | Reused the maintained `freetype.FT_Get_Transform` identity, set-sequence, sentinel, null-output, and null-face matrix and routed only the C-ABI `get_transform` operation through the exported `FT_Get_Transform`/`FT_Set_Transform` calls, preserving the existing output shape without changing the parity matrix. Full parity run `aec5d45e-6b81-4340-8571-6ab6ce399571`, recorded by `7b8dc26a-11d0-4896-9b3b-c221343acfee`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items. All-lane Coverage MCP run `34f340d9-3b35-43c1-97c7-7a0f8295889d`, snapshot `31d69b3f-cb1e-4247-9cad-ea5bc5656979`, completed in 69.916 seconds and measured 49,953 / 54,382 lines, 9,907 / 12,590 branches, 3,410 / 3,848 functions, and 68,726 / 75,611 regions; every C-ABI `FT_Get_Transform` wrapper line and branch is now covered. C-ABI scorecard run `5bfbfed7-3feb-49ed-b743-1e0d3cab6b1a` reports 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 95 | Working tree (FT_Set_Transform C-ABI/WASM parity route) | Reused the maintained `freetype.FT_Set_Transform` matrix and routed its live sequence, nullable matrix/delta inputs, and null-face/null-handle guards through the exported C-ABI and WASM wrappers; the fixture-defined `non_identity` token now reaches the shared transform parser without changing the matrix denominator. Full parity run `727f5f4a-8659-4540-8b79-67539c10c7d5`, recorded by `41115e02-bc22-4f63-9933-fb68a51e7910`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `e90e76ff-69e5-4d80-8096-0fb277fcf9d4`, snapshot `2adc96fb-eccd-45ca-95a1-1784e293691c`, completed in 69.467 seconds and measured 49,956 / 54,382 lines, 9,910 / 12,590 branches, 3,410 / 3,848 functions, and 68,729 / 75,611 regions; the C-ABI wrapper moved to 8,819 / 9,761 lines and 1,464 / 2,084 branches, while WASM moved to 6,432 / 7,022 lines and 1,149 / 1,682 branches. Both wrapper null guards and nullable-pointer branches are covered. C-ABI scorecard run `7cd9ccdc-8fe6-4e49-b8b0-105e37df44f8` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 96 | Working tree (FTC_CMapCache_Lookup null-cache C-ABI route) | Reused the maintained `ftcache.FTC_CMapCache_Lookup.error_null_cache_returns_zero` variants and routed their expanded `@cp65`, `@cp1114111`, and `@cp57344` cases through the exported C-ABI `FTC_CMapCache_Lookup` null-cache guard, preserving the existing `FT_Err_Unimplemented_Feature` parity-boundary status while executing the real zero-return path; the `case_id_base` correction ensures variant expansion cannot silently fall back. Source-bound full parity run `1b4d5976-b4e5-43c7-9e4f-547c47ebfe26`, recorded by `7505f21c-9346-4a9b-8923-ac3a56de44d3`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `5ffeaf46-b39a-44dc-bad6-4d903b1a60cc`, snapshot `8f30b182-5c42-433c-b954-d4fe1ce975b7`, completed in 70.958 seconds and measured 49,957 / 54,382 lines, 9,911 / 12,590 branches, 3,410 / 3,848 functions, and 68,730 / 75,611 regions; the C-ABI implementation moved to 8,820 / 9,761 lines, 1,465 / 2,084 branches, and 11,452 / 12,575 regions, with the null guard at lines 1987-1988 reaching 2 / 2 branches and 9 return-path hits. C-ABI scorecard run `2fcbdfbb-31be-4068-ab53-4aabf97133c8` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 97 | Working tree (FTC_Manager_New null-validation C-ABI route) | Reused the maintained `ftcache.FTC_Manager_New.error_null_library` and `ftcache.FTC_Manager_New.error_null_requester_or_output` inputs and routed the C-ABI lane through the exported `FTC_Manager_New` validation guard, exercising null-library, null-requester, and null-output calls while preserving the existing `FT_Err_Unimplemented_Feature` parity-boundary status. Source-bound full parity run `83a11c2d-dd6d-47eb-911c-bf9ef2808920`, recorded by `f185a497-aeb8-49db-9ff7-8d59a720fc53`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `a5bbf85d-c9e4-48c1-a5ff-7504d55d5c55`, snapshot `888e4e04-fe4d-4fd3-a17c-cc6fa5d61c6e`, completed in 80.408 seconds and measured 49,959 / 54,382 lines, 9,914 / 12,590 branches, 3,410 / 3,848 functions, and 68,732 / 75,611 regions; the C-ABI implementation moved to 8,822 / 9,761 lines, 1,468 / 2,084 branches, and 11,454 / 12,575 regions, with `FTC_Manager_New` lines 1807-1811 covered and its two validation guards at 2 / 2 and 4 / 4 branches. C-ABI scorecard run `a94357ca-b22b-4c72-839c-7d804fab772f` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 98 | Working tree (FTC_CMapCache_New null-argument C-ABI route) | Reused the maintained `ftcache.FTC_CMapCache_New.error_null_manager_or_output` input and routed its C-ABI lane through the exported `FTC_CMapCache_New` validation guard, executing both null-manager and live-manager/null-output calls while preserving the existing `FT_Err_Unimplemented_Feature` parity-boundary status. Source-bound full parity run `83fa4231-31e7-4116-9684-87b5889dba3c`, recorded by `299d3150-debd-411f-a0cb-460e4154c2e1`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `89810c7a-fe97-4ca5-8266-0209a8eddcdb`, snapshot `a5d4da5c-d884-4ce8-a0ce-d864a217aedc`, completed in 96.023 seconds and measured 49,960 / 54,382 lines, 9,916 / 12,590 branches, 3,410 / 3,848 functions, and 68,733 / 75,611 regions; the C-ABI implementation moved to 8,823 / 9,761 lines, 1,470 / 2,084 branches, and 11,455 / 12,575 regions, with `FTC_CMapCache_New` lines 1965-1966 covered and its null guard at 4 / 4 branches. C-ABI scorecard run `fd0520d3-92e1-4f57-ab25-e0f63ba53441` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 99 | Working tree (FTC_Manager_LookupFace null-validation C-ABI route) | Reused the maintained `ftcache.FTC_Manager_LookupFace.error_null_output_or_manager` and `fterrdef.FT_Err_Invalid_Cache_Handle.cache_lookup_rejects_null_manager` inputs and routed the C-ABI lane through the exported `FTC_Manager_LookupFace` guard. The route asserts the real `Invalid_Argument` null-output status, the real `Invalid_Cache_Handle` null-manager status, and output clearing, while preserving the established `FT_Err_Unimplemented_Feature` and `FT_Err_Invalid_Face_Handle` parity-boundary results. Source-bound full parity run `0678b7bf-09b7-4ac1-ae1c-9c411e590884`, recorded by `4e308b8b-524c-446e-b6b4-1fb4d8e05b59`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `e91fbcc5-9c26-4235-acd0-bdf1f4bac67b`, snapshot `5f5b82b2-57e3-49d3-9009-4a534b6d6450`, completed in 71.817 seconds and measured 49,963 / 54,382 lines, 9,918 / 12,590 branches, 3,410 / 3,848 functions, and 68,737 / 75,611 regions; the C-ABI implementation moved to 8,826 / 9,761 lines, 1,472 / 2,084 branches, and 11,459 / 12,575 regions, with `FTC_Manager_LookupFace` lines 1879-1894 covered and its guard branches at 2 / 2. C-ABI scorecard run `6ca1b15a-ccb9-4740-a781-957bd61f4afe` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 100 | Working tree (FTC_Manager_LookupSize null-validation C-ABI route) | Reused the maintained `ftcache.FTC_Manager_LookupSize.error_null_scaler_output_or_manager` input and routed the C-ABI lane through the exported `FTC_Manager_LookupSize` guard. The route asserts the real `Invalid_Argument` null-scaler and null-output statuses, the real `Invalid_Cache_Handle` null-manager status, and null-manager output clearing, while preserving the established `FT_Err_Unimplemented_Feature` parity-boundary result. Source-bound full parity run `61013149-2ac2-4036-a3d6-d9a7b134f76f`, recorded by `eeb35a84-6e15-448e-85a4-504c50fe9dfc`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `6548ccb3-76cb-4db4-b60c-9395308ab258`, snapshot `d3df37a3-1362-4235-9594-138f9d6e3a68`, completed in 85.074 seconds and measured 49,965 / 54,382 lines, 9,920 / 12,590 branches, 3,410 / 3,848 functions, and 68,741 / 75,611 regions; the C-ABI implementation moved to 8,828 / 9,761 lines, 1,474 / 2,084 branches, and 11,463 / 12,575 regions, with `FTC_Manager_LookupSize` lines 1905 and 1923 covered and its validation guard at 4 / 4 branches. C-ABI scorecard run `917f111a-7ebb-4956-8564-b5c7a189e224` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 101 | Working tree (FTC_Manager_RemoveFaceID null-manager C-ABI route) | Reused the maintained `ftcache.FTC_Manager_RemoveFaceID.success_null_manager_noop` input and routed its C-ABI lane through the exported `FTC_Manager_RemoveFaceID` null-manager call, preserving the declared no-op result while executing the real ABI guard. Source-bound full parity run `3e67e74b-8e61-40e4-b8ca-29363cebd948`, recorded by `51f9aab9-cf74-4bc8-a327-4fbb5dbd04cf`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `6768c91a-3093-42a7-b2b6-c8cf59cc159a`, snapshot `7e1858aa-0353-4a19-af2e-6c802b09b916`, completed in 109.445 seconds and measured 49,966 / 54,382 lines, 9,921 / 12,590 branches, 3,410 / 3,848 functions, and 68,742 / 75,611 regions; the C-ABI implementation moved to 8,829 / 9,761 lines, 1,475 / 2,084 branches, and 11,464 / 12,575 regions, with `FTC_Manager_RemoveFaceID` line 1936 covered and its null-manager guard at 2 / 2 branches. C-ABI scorecard run `c42556db-7e03-4f23-979f-f68f46d48fa2` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime contract rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 102 | Working tree (FTC_ImageCache_New null-validation C-ABI route) | Reused the maintained `ftcache.FTC_ImageCache_New.error_null_manager_or_output` input and routed its C-ABI lane through the exported `FTC_ImageCache_New` validation guard, executing both null-manager and live-manager/null-output calls while preserving the established `FT_Err_Unimplemented_Feature` parity-boundary status. Source-bound full parity run `b48bd7a1-918c-4932-abc6-7a389f3f9b5a`, recorded by `2a878a19-6ad3-49b5-b065-338e23e6b156`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `782ee38f-0057-418f-b0c4-d3a624ecc798`, snapshot `d06e0759-2457-4a5e-a3cf-9e3fdb2ad4af`, completed in 108.210 seconds and measured 49,967 / 54,382 lines, 9,923 / 12,590 branches, 3,410 / 3,848 functions, and 68,743 / 75,611 regions; the C-ABI implementation moved to 8,830 / 9,761 lines, 1,477 / 2,084 branches, and 11,465 / 12,575 regions, with `FTC_ImageCache_New` line 2040 covered and its null-argument guard at line 2039 reaching 4 / 4 branches. C-ABI scorecard run `e0b79df7-c784-497f-b81a-752177df2c0e` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime contract rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 103 | Working tree (FTC_ImageCache_New cache-limit C-ABI route) | Reused the maintained `ftcache.FTC_ImageCache_New.error_too_many_caches` input and routed its C-ABI lane through 16 successful exported `FTC_ImageCache_New` registrations followed by the 17th cache-limit call, asserting the real `FT_Err_Too_Many_Caches` status and null output while preserving the existing `FT_Err_Unimplemented_Feature` parity-boundary result. Source-bound full parity run `97c0bfda-1736-4bc9-af10-e1a1f5603589`, recorded by `d7264bbe-1eef-4809-ae18-8110bcc38cd7`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `08237caa-2c54-4264-9e7b-ba72aa8fae4e`, snapshot `90d65800-8c5e-4652-9d07-74489a636957`, completed in 96.315 seconds and measured 49,968 / 54,382 lines, 9,924 / 12,590 branches, 3,410 / 3,848 functions, and 68,744 / 75,611 regions; the C-ABI implementation moved to 8,831 / 9,761 lines, 1,478 / 2,084 branches, and 11,466 / 12,575 regions, with `FTC_ImageCache_New` line 2043 covered and its cache-limit guard at line 2042 reaching 2 / 2 branches. C-ABI scorecard run `728577dc-3b57-4d3b-9f41-ffb148225816` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime contract rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 104 | Working tree (FTC_SBitCache_New invalid-argument C-ABI route) | Reused the maintained `ftcache.FTC_SBitCache_New.invalid_arguments_match_c` input variants and routed the C-ABI lane through the exported `FTC_SBitCache_New` guard, asserting pinned C's `Invalid_Argument` status for null manager and null output while clearing a non-null cache sentinel on the null-manager call. Source-bound full parity run `1376d5f7-127c-4de2-a4d6-b20954357ef4`, recorded by `f4b99ab0-1b4b-4e77-9207-a6ec5a4191f1`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `1e2c11f5-4911-408d-8f55-c0462b08010c`, snapshot `9192bd59-abc5-4763-ace5-16603970ac4c`, completed in 107.170 seconds and measured 49,974 / 54,387 lines, 9,928 / 12,592 branches, 3,410 / 3,848 functions, and 68,748 / 75,614 regions; the C-ABI implementation moved to 8,837 / 9,766 lines, 1,482 / 2,086 branches, and 11,470 / 12,578 regions, with the SBit constructor's output-clear path and invalid-argument guard covered. C-ABI scorecard run `0d0fd204-57c5-4a62-a6ed-5b6be2a6782e` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime contract rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 105 | Working tree (FTC_SBitCache_New cache-limit C-ABI route) | Reused the maintained `ftcache.FTC_SBitCache_New.error_outputs_null_cache` input and routed the C-ABI lane through the exported `FTC_SBitCache_New` cache-registration limit: 16 successful registrations followed by the 17th call, asserting pinned C's `FT_Err_Too_Many_Caches` status and null output while preserving the existing `FT_Err_Invalid_Argument` parity-boundary result. Focused run `bffdb486-e702-4c41-81c0-ad645529e661` passed 1 / 1; source-bound full parity run `2a86b5f7-2dd7-4211-989b-f6da4640070c`, recorded by `ab32b171-441d-4fb5-be54-c2968923290c`, passed 7,542 / 7,542 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending route-audit items; Rust, C ABI, and WASM each retain 218 / 218 function-route evidence. All-lane Coverage MCP run `e5dd45f6-d1dc-4c1d-a7ee-8ea143b8441d`, snapshot `55b978d6-8073-4afa-8135-5938b10a54e2`, completed in 115.508 seconds with the explicit `COVERAGE_UNIFIED_WORKERS=2` override and measured 49,975 / 54,387 lines, 9,929 / 12,592 branches, 3,410 / 3,848 functions, and 68,749 / 75,614 regions; the C-ABI implementation moved to 8,838 / 9,766 lines, 1,483 / 2,086 branches, and 11,471 / 12,578 regions, with `FTC_SBitCache_New` lines 2184-2185 covered, including both cache-limit branches. C-ABI scorecard run `a550bca3-4804-4ae5-8f59-12cb1fc5997d` passed with 10 / 12 categories complete, 5,285 / 5,285 runtime contract rows, and 684 / 684 strict-error routes; Windows import-library evidence and four fresh target-lane bundles remain. |
| 106 | Working tree (coverage worker contention profile) | Revalidated the unchanged all-lane LLVM coverage matrix with the default one-worker split profile: Coverage MCP run `71a6d996-4cc6-437a-a4de-d6decd5fabc5`, snapshot `3d12dece-be1a-4c08-bdad-94a610cf80fb`, completed in 29.881 seconds with 7,542 / 7,542 comparisons in each backend and unchanged totals of 49,975 / 54,387 lines, 9,929 / 12,592 branches, 3,410 / 3,848 functions, and 68,749 / 75,614 regions. Retained lane timers were 22.803s Rust, 20.608s C ABI, and 20.424s WASM. The same-source two-worker comparison `e5dd45f6-d1dc-4c1d-a7ee-8ea143b8441d` took 115.508 seconds; the evidence identifies instrumented counter/cache contention, not MCP ingestion, as the delay. The maintained default remains `COVERAGE_UNIFIED_WORKERS=1`; no parity input or denominator changed. The C-ABI scorecard remains 10 / 12 categories complete, with the Windows import-library item and four fresh target-lane bundles remaining. |
| 107 | `d9ac1a39e083fb9dfb8bb3c40ba7b18097e09e19` | Added one maintained blank line to `missing_font_field.bdf` to execute the Rust BDF constructor's blank-line skip while pinned C discards the same bytes before its parser callback. Source-bound parity run `1b52e95f-d24b-448e-a966-8b1d121d9a4e` passed 7,542 / 7,542 runnable comparisons with 0 failures and 3 explicitly pending safety-extension cases. All-lane coverage run `8922c0ae-767c-4d62-8dfe-447d352a5b51`, snapshot `5c8acf09-fcb7-498d-95d2-a9c2faab5a7a`, completed in 29.234 seconds and moved totals to 49,976 / 54,387 lines, 9,930 / 12,592 branches, 3,410 / 3,848 functions, and 68,750 / 75,614 regions. A cold-cache coverage run took 80.314 seconds because instrumented compilation consumed 47.59 seconds; the default one-worker split remains the measured speed profile. |
| 108 | Working tree (`FT_Bitmap_Blend` boundary parity routes) | Added maintained `FT_Bitmap_Blend` inputs for the `FT_PIXEL_MODE_NONE` source no-op, all four coordinate-overflow guards, and the empty-source no-op, with exact pinned-C/Rust FFI/C ABI/WASM execution. Source-bound parity run `92002f82-4fe8-4694-ab2c-e8017e37c5dd` passed 7,545 / 7,545 runnable comparisons with 3 explicitly pending safety-extension cases and 0 pending parity routes; all three backend function surfaces remain 218 / 218. All-lane Coverage MCP run `01303fc3-5792-485b-82ba-0f0a84d8cfe4`, snapshot `e4a33be1-aed3-4d80-a165-b078e794e23b`, completed in 61.445 seconds and measured 49,986 / 54,390 lines, 9,941 / 12,594 branches, 3,410 / 3,848 functions, and 68,760 / 75,617 regions. The C-ABI scorecard remains 10 / 12 categories complete with 5,288 / 5,288 runtime contract rows exact; the Windows import-library item and four external platform bundles remain. |
| 109 | Working tree (coverage build-state reuse) | Refined `make test-coverage-all` so the instrumented binary marker follows the newest compiler-input commit and ignores worker/lane-split orchestration settings; dirty compiler inputs still force a clean rebuild. The first validation after the marker change passed 7,546 / 7,546 in each lane in 83.479 seconds, including a 46.78-second instrumented rebuild; the warm repeat passed the same matrix in 28.948 seconds with unchanged totals of 49,987 / 54,390 lines, 9,942 / 12,594 branches, 3,410 / 3,848 functions, and 68,773 / 75,617 regions. A two-worker comparison took 140.239 seconds, so the default remains one worker per instrumented lane. Full parity run `a55f7f12-832f-4879-b2c1-206f99a548fe`, recorded by `fa73451c-5a3d-4803-8231-486cf02d3fef`, passed 7,546 / 7,546 with 0 failures and 3 pending safety-extension cases. |
| 110 | Working tree (OT-SVG vertical-layout parity route) | Added the maintained `input/fonts/svg/otsvg-glyph.ttf` variant to `freetype.FT_Load_Glyph.no_scale_no_hinting_and_vertical_flags` with `FT_LOAD_COLOR` and `FT_LOAD_VERTICAL_LAYOUT`, exercising the pure-Rust `GlyphSlot::from_svg` vertical advance branch through Rust FFI, C ABI, and WASM. Source-bound parity run `7dbbbf15-55ec-4e8e-84f8-9274bd9e66de`, recorded by `2ef5efdf-32e1-4f0c-b1ff-c8c287193b50`, passed 7,547 / 7,547 with 0 failures and 3 pending safety-extension cases; all-lane Coverage MCP run `b8eee962-d9dd-457e-bc2e-3bd7ce996fd8`, snapshot `44e3ea6a-02c4-47bb-aa43-25af1b4c9b88`, completed in 77.678 seconds and measured 49,991 / 54,390 lines, 9,943 / 12,594 branches, 3,410 / 3,848 functions, and 68,774 / 75,617 regions. The targeted `src/api.rs` branch at lines 1074-1075 is now fully covered. C-ABI scorecard run `1072c2e3-c905-4bbd-a639-8e99e5a4376c` reports 10 / 12 categories complete, 5,290 / 5,290 runtime rows, and 7,547 / 7,547 no-generic-fallback routes; the Windows import-library item and four fresh platform bundles remain. |
| 111 | Working tree (OT-SVG missing-hooks render parity route) | Added a maintained `freetype.FT_Render_Glyph.error_unloaded_or_unsupported_slot_format@svg-no-hooks-missing-renderer` variant using `input/fonts/svg/otsvg-glyph.ttf`, `FT_LOAD_COLOR`, and normal rendering with hooks unset. This reaches the pinned C `FT_Err_Missing_SVG_Hooks` behavior through Rust FFI, C ABI, and WASM. Source-bound parity run `66fdb24c-d148-48bb-ac9a-01cfe134d0db`, recorded by `df884a35-8ae2-421a-9919-8ba0a188bde7`, passed 7,548 / 7,548 with 0 failures and 3 pending safety-extension cases; all-lane Coverage MCP run `800756de-e48e-4d00-87fc-391fcc1061a3`, snapshot `d7658f19-7e80-4e33-83e2-67897b0078ec`, completed in 28.470 seconds and measured 49,994 / 54,390 lines, 9,944 / 12,594 branches, 3,410 / 3,848 functions, and 68,775 / 75,617 regions. `src/api.rs:1141` now has both branches covered; the separate defensive no-loaded-outline path at 1146-1147 remains. The C-ABI scorecard remains 10 / 12 categories complete; the Windows import-library item and four fresh platform bundles remain. |

The current source-bound parity verification is Coverage MCP parity run
`66fdb24c-d148-48bb-ac9a-01cfe134d0db`: it passed 7,548 / 7,548 runnable
comparisons, 0 failed, and 3 explicitly pending safety-extension cases. The
route audit reports **0 pending routes** with 218 / 218 function routes present
in each ABI surface. The source-digest attestation was refreshed by record run
`df884a35-8ae2-421a-9919-8ba0a188bde7` in `doc/runtime_parity_evidence.json`.
The companion C-ABI scorecard run `282b58c0-1be3-4a49-bbad-ccdcd9587eb7`
reports **10 / 12 categories complete**, with 5,291 / 5,291 runtime contract
rows and 684 / 684 strict error routes exact; the remaining contract debt is
the Windows import-library item and four fresh target-lane bundles.

The three pending cases are deliberately excluded from the pinned-C parity
numerator and denominator because their inputs are undefined or
memory-unsafe for FreeType 2.14.3:
`freetype.FT_Done_FreeType.error_invalid_or_foreign_library_handle`,
`freetype.FT_Face_Properties.error_null_face`, and
`ftimage.FT_Outline.null_internal_pointer_safety_extension`. Fontdone still
rejects each input without dereferencing it, and the safety behavior remains
covered by the facade/package checks; none is a missing runtime route.

The current source-bound all-lane run `800756de-e48e-4d00-87fc-391fcc1061a3`
completed in 28.470 seconds with snapshot
`d7658f19-7e80-4e33-83e2-67897b0078ec`. It measured 49,994 / 54,390 lines,
9,944 / 12,594 branches, 3,410 / 3,848 functions, and 68,775 / 75,617
regions; the parity matrix passed 7,548 / 7,548 in each backend. The lanes
already run concurrently; the same-source two-worker comparison
`971f65aa-12b6-4a41-9dcb-cceffdd99199` completed in 140.239 seconds, so the
default remains one worker per lane. The previous combined-lane warm all-lane baseline completed in 1
minute 53.998 seconds. The split validation completed in 61.827 seconds, and
the prior execution-only warm measurement with the instrumented binary and
expanded-input cache warm was 50.482 seconds. `make
test-coverage-all` now defaults to
`COVERAGE_UNIFIED_LANE_SPLIT=1`: it builds one instrumented parity binary and
runs that binary directly for the Rust FFI, C ABI, and host-WASM lanes in
separate processes, each with its own raw profile, then merges them with
`cargo llvm-cov report`. Reusing the binary removes three repeated Cargo test
profile setups; LLVM instrumentation made the old in-process backend calls
contend, while process-local profiles remove that contention without changing
the exact matrix. Set the variable to `0` only for the legacy diagnostic path.
The command also uses a single parity worker per lane, `CARGO_PROFILE_TEST_OPT_LEVEL=1`,
`COVERAGE_TEST_DEBUG=0`, and `cargo llvm-cov --no-clean` by default: the
optimized test profile removes the several-fold slowdown of unoptimized
instrumented code, and the current host measurement shows opt-level 1 is faster
than opt-level 3 under coverage while preserving the same totals. Line-table-only
debuginfo reduces compile/report overhead, retaining the instrumented target
removes repeated warm-run rebuilds, and
face-cache keys reuse preloaded font content digests instead of rehashing each
expanded case. Independent oracle/audit preparation runs in the two-job setup
batch;
the ABI-only package preflight remains available separately as
`make coverage-abi-preflight` and is already exercised by `make test-fast`.
The historical optimized-profile validation run `79f4439e-2db4-4ee2-8746-c101d8db2925`
completed in 53.316 seconds with 7,479 / 7,479 runnable comparisons passing in
each lane. The prior current-head opt-level-1 speed validation run
`9453c549-d468-487b-a9e9-ea753043d2d6` completed in 54.563 seconds, versus
65.332 seconds for the opt-level-3 comparison, with identical coverage totals
and parity results. Unchanged generated
oracle inputs preserve their mtimes so the helper/validator C build is not
repeated. The preceding source/input-bound run measured 49,953 / 54,382 lines,
9,907 / 12,590 branches, 3,410 / 3,848 functions, and 68,726 / 75,611
regions. It passed 7,542 / 7,542 runnable parity comparisons with 0 failures.
Its Coverage MCP run is `34f340d9-3b35-43c1-97c7-7a0f8295889d`, with snapshot
`31d69b3f-cb1e-4247-9cad-ea5bc5656979`. Each measurement clears stale
`.profraw` files first; use `make coverage-clean` after changing the coverage
toolchain or instrumentation configuration.

An earlier coverage-speed validation (Coverage MCP run
`e068f42a-1f89-4504-b438-fe82602f9777`, snapshot
`bd628f06-3846-4c7e-8c82-76f26c8d437f`) measured 73.636 seconds end to end
after avoiding per-render `FT_Face` metadata reconstruction in
`FT_Render_Glyph` and adding the palette mutation boundary route. Its retained
lane timers were 24.77 seconds Rust, 24.85 seconds C ABI, and 26.20 seconds
WASM. The same-source warm repeat (Coverage MCP run
`4f42b55d-3301-46bc-85b6-560ae948d2a5`, snapshot
`1063ab34-d0bf-4a2d-9147-bd063f39c502`) measured 29.156 seconds; the
two-worker experiment took 87.614 seconds, so the default remains one worker.
The preceding load-char validation (Coverage MCP run
`e7ccccdb-80f1-4d32-9fa3-d4c06a7c1673`, snapshot
`292d059b-daeb-4e7d-ad3c-2451185bd1d7`) measured 37.220 seconds end to end
with the warm instrumented binary. The prior warm baseline run
`8c64555d-d1ad-4f27-8b36-9ca60d8ce0e4` measured 48.265 seconds, so the new
path was about 22.9% faster overall. The earlier retained lane timers identify
the remaining floor as instrumented parity execution (about 30.545 seconds
Rust FFI, 29.756 seconds C ABI, and 29.654 seconds WASM), while comparison was
about 14.7–15.1 ms per lane. The measured Rust reduction came from reusing the
worker-local high-level `ApiFace` for `load_char`; LLVM report and Coverage MCP
ingestion are not the bottleneck. Coverage MCP accepts the current LLVM JSON
directly; the compatibility-only `jq` segment rewrite remains opt-in through
`COVERAGE_NORMALIZE_SEGMENTS=1`. This is a report-path optimization only; it
does not remove a parity lane or change a coverage denominator.

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
The most-sampled current environment has **5 / 5 qualifying clean runs**.
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
