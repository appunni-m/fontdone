# Compatibility and measurement evidence

These are dated observations with separate denominators. Historical release
measurements below include 2.14.3-alpha.10. The [user guide](INTEGRATION.md)
identifies the latest installable release; a documentation edit does not
remeasure an older runtime.

## Independent compatibility measures

Compatibility has 3 separate measurements. They must not be combined into one
percentage. Performance is tracked separately and cannot increase a
compatibility score.

### Maintained adoption map

The generated [function adoption map](https://github.com/appunni-m/fontdone/blob/main/doc/FREETYPE_SUPPORT.md) classifies all
218 pinned public functions by application-ready contract status:

| Status | Functions | Meaning |
|---|---:|---|
| Complete | 52 | Maintained application behavior and mapping are complete |
| Implemented, mapping incomplete | 5 | Runtime code exists; public mapping is incomplete |
| Partial | 29 | Only the documented behavior is ready |
| Planned | 69 | No application-ready implementation is claimed |
| Intentionally excluded | 63 | Outside the currently declared product surface |
| **Total** | **218** | Pinned FreeType 2.14.3 function inventory |

This is a conservative adoption classification. A planned or excluded function
can still have a declaration, stub, validation route, or focused runtime probe.
That evidence does not make its complete application behavior available.

### Last committed runtime evidence

The last committed full parity snapshot was recorded on **2026-09-16**:

| Measurement | Count |
|---|---:|
| Runnable exact-comparison cases | 20,357 |
| Passed cases | 20,357 |
| Failed cases | 0 |
| Explicitly pending cases | 3 |
| Covered manifest cases | 4,420 |
| Validated public API subjects | 1,543 |
| Validated public API input files | 1,537 |
| Logical declared cases | 4,574 |
| Concrete expanded cases | 20,360 |
| Functions with at least one C/Rust/C-ABI/WASM runtime route | 218 / 218 |

`20,357 / 20,357` means every runnable case in that execution matched; the 3
explicitly pending concrete cases are safety-extension or undefined-input
scenarios and the route audit still reports **0 pending parity routes**. Likewise, 218/218 function-route evidence
can be satisfied by a narrow success or null-validation route; it is not
equivalent to complete behavior for every input, state, or platform.

The latest worktree verification is the full parity snapshot recorded in
`doc/runtime_parity_evidence.json` by `make record-parity-snapshot` after
20,357 / 20,357 runnable comparisons with 0 failures and 3 explicitly pending
concrete cases. The source digest, compiler versions, and execution counts are
recorded in the [release's parity receipt](https://github.com/appunni-m/fontdone/blob/v2.14.3-alpha.10/doc/runtime_parity_evidence.json).

Run `make test-parity` for current worktree evidence. It writes the full log
and a source-digest-bound report under `target/parity-evidence/`. After a
complete run, `make record-parity-snapshot` copies that report into the
[committed runtime evidence](https://github.com/appunni-m/fontdone/blob/main/doc/runtime_parity_evidence.json)
and updates this table. Recording fails if parity-relevant source changed after
the run. Generated runtime reports under `target/` are newer authority for
their exact worktree than the committed release snapshot.

## Retained historical combined coverage

The retained historical all-lane coverage snapshot was recorded on
**2026-08-23** for the worktree based at commit
`e8c51cb6dba42fd524d94940673fe6b380411d46`
by managed Coverage MCP 0.10.0 (registered command with the local sccache
wrapper disabled):


| Metric | Covered / total | Coverage |
|---|---:|---:|
| Lines | 63,740 / 65,887 | 96.74% |
| Branches | 11,580 / 13,412 | 86.34% |
| Functions | 3,689 / 3,976 | 92.78% |
| Regions | 87,946 / 91,700 | 95.91% |

This report is historical and is not current-release coverage. Its source
identity and totals remain in the machine-readable ledger. Fresh collection
uses `make test-coverage-all`; shader or foreign-target execution is not implied.

## C ABI completion contract

The retained local scorecard from `make c-abi-contract` in the measured
worktree (regenerated 2026-09-15) has **8 / 12 categories complete**:

| Category group | Status |
|---|---|
| Functions | 183 / 218 functions without unresolved subject routes; 218 / 218 names, signatures, and traced function routes; 13,624 / 18,100 pinned-C runtime contract rows exact; 4,476 pending |
| Constants, types, layouts, callbacks | Complete under their blocking scorecard measurements |
| Ownership | 23 / 23 ownership rules have exact runtime evidence |
| State, modules, headers | 20 / 20 state transitions and 7 / 7 optional public components have exact evidence; headers are complete |
| Errors | 2,756 / 3,689 expected-error routes compare exact error and output results; the current ledger has no strict mismatches and 933 unresolved routes; 15,879 / 20,355 routes have no generic fallback evidence |
| Binary/install artifacts | 7 / 8; Windows import-library evidence pending |
| Platform behavior | 1 / 5 fresh target bundles; Linux x86-64, Windows x86-64, Linux i686, and Linux powerpc64 remain pending |

Tag CI independently requires all five platform jobs and validates their
combined scorecard. Its `c-contract-scorecard` artifact records the release's
platform totals; the table above records the local macOS worktree measurement.

Only `make c-abi-contract-complete` is the full-contract pass condition. The
ordinary `make c-abi-contract` command intentionally succeeds while reporting
remaining debt. Unresolved function-subject routes and incomplete expected-error
routes remain even when every bare function name has some traced route. The self-cleaning
[completion roadmap](https://github.com/appunni-m/fontdone/blob/main/doc/ROADMAP.md) defines the exact 12-category goal.

The committed machine-readable snapshot is
[`doc/compatibility_snapshot.json`](https://github.com/appunni-m/fontdone/blob/main/doc/compatibility_snapshot.json).
The scorecard is generated from the pinned C headers, current route audit,
contract inventory, and available platform artifacts. Incomplete categories
remain blockers for complete replacement; the generated report under `target/api-abi-audit/`
is the detailed authority for the current worktree.

## Performance baseline

The maintained release-mode benchmark measures per-operation latency and
throughput, complete-process peak RSS, and exact unstripped release-artifact
bytes against pinned FreeType. Correctness mismatches fail before a
measurement can qualify.

<!-- performance-baseline:start -->
The committed ledger contains **7 / 5 clean runs**
for its most-sampled current environment. Five runs from the same environment
are required before regression thresholds can be reviewed.

| Latest clean measurement | Value |
|---|---:|
| Source commit | `e0c6b793de33f2b62bc79a0f7923dc52000b75ff` |
| Samples | 10 |
| Weighted latency speedup versus C | 0.293x |
| Total throughput ratio versus C | 0.378x |
| Median peak-RSS ratio versus C | 3.822x |
| Shared-library byte-size ratio versus C | 2.487x |
| Fontdone WASM size | 1,281,884 bytes |

The regression policy is `collecting_baseline`. `make bench-regression`
therefore fails closed until reviewed thresholds become active.
<!-- performance-baseline:end -->

Run `make bench` to generate a ten-sample report under
`target/fontdone-bench/`. From a clean source commit, run
`make record-performance-baseline` to append it to the committed ledger.
Performance evidence is machine- and environment-specific; results from
different environment identities are never pooled toward the five-run
threshold-review minimum.
