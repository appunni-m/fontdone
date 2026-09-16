# Benchmark methodology

The [benchmark results](https://appunni-m.github.io/fontdone/benchmarks/) are
operation-specific observations against the pinned FreeType oracle. Latency,
throughput, complete-process memory, and artifact size have different boundaries
and must not be combined into a compatibility score.

## Reproduce

```sh
make bench-self-test
make bench-quick
make bench BENCH_SAMPLES=10 BENCH_PROFILE=default
```

Run from the repository root with the pinned Rust/C toolchains and oracle.
`bench-quick` uses two samples for diagnostics. A qualifying baseline needs
the maintained default profile, at least ten samples, a clean source checkout,
and complete matrix/identity evidence. Raw reports live under
`target/fontdone-bench/`.

The matrix is [perf_operation_matrix.json](../tests/data/perf_operation_matrix.json).
Each operation retains its own inputs and repeat count. Equality-qualified
comparisons and `timing_only` rows remain distinct; timing-only data cannot
establish output parity.

## Compare like with like

Use the same machine, power conditions, compiler versions, build profile,
matrix hash, and workload policy. Interleave implementations where the runner
does so, retain all samples, and inspect spread and outliers. Do not silently
discard slow runs or pool different hosts toward a baseline requirement.

The public view reports per-operation median and P90 in microseconds. P90 is
not a confidence interval. Process RSS includes process/runtime overhead;
unstripped artifact bytes are not download sizes or incremental memory use.

The historical snapshot measured source
`e0c6b793de33f2b62bc79a0f7923dc52000b75ff` on 2026-09-08. It must not be
presented as performance of every alpha release. The ledger's aggregate ratios
are weighted summaries of that specific matrix, not general speedup claims.

## Baseline and regression policy

The retained ledger has seven qualifying clean runs in its most-sampled
environment. Five are needed before threshold review. The policy is still
`collecting_baseline`, with no accepted thresholds.
`make bench-regression` fails closed until thresholds become active.

`make record-performance-baseline` appends a qualifying current-source report
to the committed evidence ledger. It refuses dirty, incomplete, stale, or
incompatible reports. Review the measurements before accepting a budget;
do not change a threshold to hide a regression.

## Publish evidence

```sh
make docs-benchmark
make docs-build
```

The default export reads the committed compatibility ledger. For a fresh
qualifying report, select `DOCS_BENCHMARK_SOURCE=target/fontdone-bench/latest.json`.
The exporter uses the existing benchmark's strict baseline validator before
projecting public rows. The Benchmark workflow retains full artifacts and
exports the public snapshot for this repository's Pages workflow.

The presentation preserves the source hash and observations. It is not the
full benchmark or coverage receipt. Missing measurements stay unavailable.

These practices follow the [Rust Performance Book](https://nnethercote.github.io/perf-book/benchmarking.html)
and [Criterion's analysis guidance](https://bheisler.github.io/criterion.rs/book/analysis.html):
measure realistic work, separate setup, retain distributions, and compare
controlled experiments.
