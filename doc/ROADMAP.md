# FreeType 2.14.3 C-contract completion roadmap

Status: **ACTIVE**

Owner: repository maintainers

Open goals: **1**

Deletion condition: goal G01 is complete, its durable contract data and release
evidence remain reachable from maintained documentation, this file is removed
from `doc/README.md`, and this file is deleted. The repository-retention audit
rejects a retained roadmap with zero open goals.

This is the repository's one active implementation plan. It does not claim
completion from source-code coverage, a familiar function name, a passing null
check, or a route that silently executes another frontend.

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

## 8. Completion gates

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

## 9. Evidence ledger

| Goal | State | Evidence | Verification |
|---|---|---|---|
| G01 | OPEN | pinned audit, route audit, external C consumers, generated 12-category scorecard, release evidence | `make c-abi-contract-complete && make release-verify` |

When G01 completes, preserve the executable ledgers and release evidence,
update durable integration/release documentation, remove this roadmap from the
documentation index, and delete this file. Completed implementation history
belongs in Git history and release notes.
