# Documentation

Start with the [project README](../README.md). It explains what `fontdone` is,
which package to choose, and the current compatibility boundary.

## 1. User guides

| Need | Read |
|---|---|
| Integrate from Rust | [Integration guide](INTEGRATION.md) |
| Integrate from C | [`fontdone-c-abi`](../fontdone-c-abi/README.md) |
| Integrate from Node/WebAssembly | [`fontdone-wasm`](../fontdone-wasm/README.md) |
| Build, test, debug, or add fixtures | [Development guide](DEVELOPMENT.md) |
| Prepare or publish a release | [Release guide](RELEASING.md) |

## 2. Compatibility evidence

These documents answer different questions:

| Evidence | Question answered |
|---|---|
| [Function adoption map](FREETYPE_SUPPORT.md) | Which pinned FreeType functions are application-ready? |
| [Compatibility snapshot](compatibility_snapshot.json) | What are the last committed measured denominators and debts? |
| [C-contract roadmap](ROADMAP.md) | What blocks a complete C replacement claim? |

Runtime reports under `target/` describe the current worktree and supersede the
committed snapshot for that worktree. In particular:

- `target/api-abi-audit/api_abi_audit.{json,md}` inventories declarations;
- `target/api-abi-audit/route_audit.{json,md}` records runtime routes and debt;
- `target/api-abi-audit/c_abi_contract_status.{json,md}` scores all 12 C
  contract categories;
- `target/parity-evidence/runtime_parity.json` binds a full parity result to
  its source tree, toolchain, oracle, and captured log.

## 3. Project and fixture policies

| Topic | Read |
|---|---|
| Contributing | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |
| Security reporting | [`SECURITY.md`](../SECURITY.md) |
| Community conduct | [`CODE_OF_CONDUCT.md`](../CODE_OF_CONDUCT.md) |
| Release history | [`CHANGELOG.md`](../CHANGELOG.md) |
| Package and fixture licensing | [`NOTICE.md`](../NOTICE.md) and [fixture notices](../tests/fixtures/THIRD_PARTY_NOTICES.md) |
| Font provenance | [Fixture provenance](../tests/fixtures/input/fonts/PROVENANCE.md) |
| Generator review | [Font-generation policy](../scripts/font_generation/README.md) |
| Repository retention | [Generated retention inventory](FILE_RETENTION_INVENTORY.tsv) |

## 4. Documentation lifecycle

Every file directly under `doc/` has one declared lifecycle:

| Class | Document | Maintenance rule |
|---|---|---|
| authoritative | [Documentation index](README.md) | Keep every public documentation surface reachable |
| authoritative | [Integration](INTEGRATION.md) | Update when a consumer contract changes |
| authoritative | [Development](DEVELOPMENT.md) | Update when build, test, CI, fixture, or benchmark behavior changes |
| authoritative | [Releasing](RELEASING.md) | Update when packaging or publication changes |
| active-plan | [C-contract roadmap](ROADMAP.md) | Delete after every ledger goal is complete and durable results have moved into authoritative docs |
| generated | [Function adoption map](FREETYPE_SUPPORT.md) | Generate from `tests/data/interface_map.json` |
| generated | [Compatibility snapshot](compatibility_snapshot.json) | Refresh only from a committed passing evidence set |
| generated | [Runtime parity evidence](runtime_parity_evidence.json) | Record with `make record-parity-snapshot` after a complete full-parity run |
| generated | [Retention inventory](FILE_RETENTION_INVENTORY.tsv) | Generate with `make repository-inventory` |

Run `make check-docs` after editing documentation. It checks every tracked
Markdown file, local links and anchors, referenced Make targets, current
project names, lifecycle metadata, snapshot counts, and Rust API documentation.
It does not test the availability of external websites.
