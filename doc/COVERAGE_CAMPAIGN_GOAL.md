# Strict region coverage campaign

Status: **ACTIVE**

Reach 100% of the regions reported by the existing complete all-lane coverage
command. Preserve its test matrix, filters, thresholds, and expected results.
A rounded percentage or a selected-case report is not completion evidence.

## Fixture policy

Coverage gains must come from maintained fixture-based parity tests. Exercise
defensive paths with deterministic malformed or nonstandard public inputs:
truncated tables, corrupt compressed bodies, unusual dimensions, short reads,
and valid caller-owned callback records. Compare errors, modified outputs,
stream positions, and lifecycle observations as well as successful results.
Do not remove or rewrite defensive code merely to reduce the denominator.

Extend a fixture adapter when a public input dimension is not yet represented.
The adapter must actually pass that input through the compared entrypoints;
declaring a route or synthesizing the expected result is not parity. Public
Rust convenience functions can be checked against an existing oracle-backed
operation on the same fixture, retaining the operation's normal comparisons.

If a fixture reveals a behavioral divergence, reduce it to the first differing
stage, inspect the pinned FreeType implementation, fix the cause, and retain
the fixture. Never alter an established expected result to accommodate Rust.
New malformed cases must declare the error semantics established by the C
oracle, including output comparison where state changes on failure.

Fabricated pointers, private state, invalid nonzero handles, undefined
behavior, and uncontrollable allocator failures are not valid parity inputs.
Keep unresolved targets visible when no legitimate input has been established.

## Execution and measurement

The primary agent owns edits and test/coverage execution. Read-only strategy
agents may inspect disjoint source and fixture families within the available
concurrency limit. Keep proposals bounded by exact locations, existing public
routes, input construction, pinned C evidence, and stop conditions. The
historical 100-proposal batches are planning inventories; their size is not
evidence of generated, executed, or covered cases.

Start with the narrowest relevant repository command:

```sh
RUSTC_WRAPPER= make test-case CASE=<unique-fixture-substring>
```

Use the current Coverage MCP `coverage_gaps` tool with an explicit report path
and literal path/function query. Run the existing repository command normally;
Coverage MCP reads and compares its output rather than running the tests:

```sh
RUSTC_WRAPPER= python3 scripts/run_coverage_command.py \
  --migration-coverage-case-ids '<exact-case-id>,<exact-case-id>'
```

Preserve each report before another run overwrites it. `coverage_compare` with
`scope: incremental` is valid only when baseline, source, instrumentation, and
inventory match. A runtime source change requires fresh full measurement.
Selected-case absences are not full-suite regressions. Report source/build
verification and test-status limitations supplied by the tool; do not infer
receipts or claim that an unverified coordinate comparison proves regression
freedom.

Run the complete matrix with no case selectors for completion evidence:

```sh
RUSTC_WRAPPER= python3 scripts/run_coverage_command.py
```

The former managed `project_context`, command-registration, run-polling, and
`coverage_review` interfaces are historical and are not prerequisites when
the installed Coverage MCP exposes report-based `coverage_gaps` and
`coverage_compare` instead.

## Historical queue

`target/coverage/region_campaign.duckdb` is an ignored planning artifact managed
by `scripts/build_coverage_region_queue.py`. Preserve its proposal and attempt
history. The original seed snapshot
`f435e2c3-5b75-43bc-b3fe-e52c9bc9d9c6` contained 3,501 missing regions. The
historical incremental baseline was
`6e397a43-632a-42ef-9105-d9da5f445b29`; these IDs must not be treated as current
source evidence without verification.

Revalidate coordinates after source changes. Use the queue's `reconcile`
command rather than ad hoc status updates. A selected-case hit remains
`hit_pending_full`; only complete matching-source evidence can mark it `done`.
A missed or failed attempt retains its input, target, report, and explanation.
Do not attach current coordinates to old IDs without checking the source.

## Completion gate

Require a fresh complete report with covered regions equal to total regions,
no unresolved reachable target regions, and the proportionate public gates:

```sh
make test-fast
make test-parity
make c-abi-contract
make lint
make check-docs
```

Report the actual numerator and denominator, source revision and dirty state,
report paths, executed fixture counts, behavior fixes, and gate results.
Keep adoption status, executed parity, and the twelve-category C contract
separate. Generated reports, raw outputs, the queue database, and `freetype/`
remain ignored.
