# Documentation index

Start with the [project overview](../README.md), then choose [Rust](INTEGRATION.md),
[C/C++](../fontdone-c-abi/README.md), [Node/browser](../fontdone-wasm/npm/README.md),
or the [raw WASM ABI](../fontdone-wasm/README.md).

The [maturity guide](MATURITY.md) identifies application limits. The
[evidence guide](EVIDENCE.md) separates declarations from measured behavior.
Use [Development](DEVELOPMENT.md) and [Contributing](../CONTRIBUTING.md) for changes.

## Maintained public documents

Every file under `doc/` has a declared lifecycle. Generated files change through
their maintained recorder or generator, with source identity intact.

| Class | Document | Maintenance rule |
| --- | --- | --- |
| authoritative | [Documentation index](README.md) | Keep public contract current |
| authoritative | [Rust integration](INTEGRATION.md) | Keep public contract current |
| authoritative | [Development and commands](DEVELOPMENT.md) | Keep public contract current |
| authoritative | [Maturity](MATURITY.md) | Keep public contract current |
| authoritative | [Measurement evidence](EVIDENCE.md) | Keep public contract current |
| authoritative | [Benchmark protocol](BENCHMARKING.md) | Keep public contract current |
| authoritative | [Documentation maintenance](DOCUMENTATION.md) | Keep public contract current |
| authoritative | [Releasing](RELEASING.md) | Keep public contract current |
| active-plan | [Public roadmap](ROADMAP.md) | Retain open goals until the completion gates pass |
| generated | [Function adoption map](FREETYPE_SUPPORT.md) | Regenerate and validate evidence |
| generated | [Compatibility ledger](compatibility_snapshot.json) | Regenerate and validate evidence |
| generated | [Source-bound parity receipt](runtime_parity_evidence.json) | Regenerate and validate evidence |
| generated | [Repository retention inventory](FILE_RETENTION_INVENTORY.tsv) | Regenerate and validate evidence |

Legal and fixture provenance remain in [NOTICE.md](../NOTICE.md),
[fixture notices](../tests/fixtures/THIRD_PARTY_NOTICES.md), and
[font provenance](../tests/fixtures/input/fonts/PROVENANCE.md).
`make check-docs` validates the public documentation and source-bound ledgers.
