# Public compatibility roadmap

Owner: fontdone maintainers

Open goals: **3**

Deletion condition: all three goals below are complete, their evidence remains
reachable in public guides, and this page and its index entry are removed.
This roadmap describes product gaps, not instructions for an agent session.

## Complete the C replacement contract

The target is the full pinned FreeType 2.14.3 contract: functions, constants,
types, layouts, callbacks, ownership, state, errors, modules, headers, artifacts,
and platform behavior. The current measured subset is in [Evidence](EVIDENCE.md).

Completion requires all 12 categories, every required denominator, exact public
observations, and external C/C++ consumers against shipped artifacts on all
supported targets. A declaration or validation-only route is insufficient.
`make c-abi-contract-complete` is the gate; ordinary measurement keeps debt visible.

## Complete source coverage

Reach 100% of lines, branches, functions, and regions in source-bound all-lane
coverage. Preserve the full test/input matrix and production denominator. Add
legitimate public parity inputs for missing behavior. Undefined C behavior is
not an acceptable oracle. See [Development](DEVELOPMENT.md#source-coverage).

## Accept performance budgets

The benchmark measures operation latency, throughput, complete-process RSS,
and exact unstripped artifact sizes. Record qualifying clean ten-sample runs
on the same environment before reviewing regression thresholds.

<!-- performance-roadmap:start -->
The most-sampled current environment has **7 / 5 qualifying clean runs**.
<!-- performance-roadmap:end -->

The policy remains `collecting_baseline`. Existing samples do not automatically
approve thresholds. `make bench-regression` fails closed until reviewed budgets
are active. Follow [Benchmarking](BENCHMARKING.md).

## Evidence ledger

| Goal | State | Evidence | Verification |
| --- | --- | --- | --- |
| G01 | OPEN | Full C contract and platform scorecard | `make c-abi-contract-complete` |
| G02 | OPEN | Source-bound all-lane coverage | `make test-coverage-all`; all four metrics exactly 100% |
| G03 | OPEN | C/Rust samples, memory, sizes, and reviewed thresholds | `make bench-regression` |

An alpha package release does not close these goals. The ordinary release
policy requires passing runnable comparisons and honest incomplete evidence.
