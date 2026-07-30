# Release guide

`fontdone`, `fontdone-c-abi`, and `fontdone-wasm` form one synchronized release
unit. All three currently use `2.14.3-alpha.1`; facade dependencies require
that exact version.

Only the protected GitHub release workflow publishes. Local commands validate
and assemble evidence but do not authorize publication.

Publication is paused during active parity, coverage, and performance work.
Do not dispatch the release workflow, publish a crate, create a tag, or create
a GitHub release until the repository owner explicitly approves publication.

## 1. Release prerequisites

- At least two current crates.io owners exist for each package.
- The GitHub `crates-io` environment requires reviewer approval.
- `CARGO_REGISTRY_TOKEN` exists only as a protected secret.
- The release commit is clean, pushed, and has a successful CI run.
- The exact version has not previously been published or tagged.

Tokens must never appear in command arguments, repository files, logs, or
generated evidence.

## 2. Prepare the release commit

1. Update all three package versions and both exact internal requirements.
2. Update the root README release banner.
3. Run `make test-parity`, then `make record-parity-snapshot`; the second
   command refuses evidence whose source digest does not match the worktree.
4. Replace the C-contract values with evidence from the complete scorecard.
5. Review the generated function map, C headers, WASM schema/declarations, and
   synchronized legal files.
6. Move the changelog entry from “Unreleased” to the release date.
7. Review `Cargo.lock` and every package archive input.
8. Run:

   ```bash
   make check-versions
   make check-generated
   make check-docs
   make package-verify
   ```

`make package-verify` creates all three `.crate` archives, rejects fixture,
font, oracle, test, and tooling leakage, compiles the extracted packages with
exact local dependency substitutions, and writes inventories and SHA-256
digests under `target/release-evidence/`.

## 3. Required CI evidence

The exact release commit must first pass the per-commit
[CI contract](DEVELOPMENT.md#61-per-commit-gate), then a requested
[thorough run](DEVELOPMENT.md#62-requested-thorough-gate). The latter uploads
all five hash-bound C platform bundles and validates the assembled evidence.
Release preflight additionally runs `make c-abi-contract-complete`; unfinished
contract debt therefore cannot be released.

The release preflight locates the successful CI run for the exact commit,
downloads those same platform artifacts, and runs `make release-verify`.
Without assembled bundles, a local `make release-verify` correctly fails the
complete C contract. Use `make ci` and `make c-abi-contract` for ordinary
single-host development, and `make ci-thorough` only when a local exhaustive
audit is requested. The final release step reruns `make check-docs` after the
complete scorecard is generated, so a stale committed compatibility snapshot
blocks publication.

## 4. Trigger and publication order

Dispatch `.github/workflows/release.yml` with the exact synchronized version.
The workflow verifies that input against Cargo metadata, repeats release
preflight, then pauses at the protected `crates-io` environment.

After approval, `scripts/publish_release.py --publish` performs:

1. publish `fontdone`;
2. wait until crates.io serves the exact version;
3. publish `fontdone-c-abi`;
4. wait until crates.io serves the exact version;
5. publish `fontdone-wasm`.

The script names every package, stops at the first failure, requires a clean
tracked worktree, and waits up to ten minutes for each dependency to become
visible. Never run an unqualified `cargo publish` from the workspace root.

For registry-resolution rehearsal after the root version is visible:

```bash
python3 scripts/publish_release.py --dry-run
```

Before publication, `make package-verify` is the reproducible archive-level
equivalent; a facade registry dry-run cannot resolve an unpublished exact root
dependency.

## 5. Tags and release assets

Only after all three publications succeed, the workflow:

1. creates annotated tag `v2.14.3-alpha.1` at the approved commit;
2. pushes the tag over the configured repository connection;
3. creates the GitHub release from generated notes;
4. attaches all three exact `.crate` archives and `SHA256SUMS`.

Never move or recreate a published tag. Attached checksums must describe the
same archives inspected during preflight.

## 6. Failure, retry, and yank

Stop at the first failed publication. Do not skip a package or publish a facade
against a missing root version.

- Retry the same unpublished package after a transient local or network error.
- Published crate contents are immutable.
- If published contents are wrong, explicitly yank the affected version; do
  not delete its tag or reuse its version.
- Normally yank the synchronized facades when yanking their root version.
- Fix the issue and publish a new synchronized prerelease.

Example:

```bash
cargo yank --version 2.14.3-alpha.1 fontdone-wasm
```

## 7. Alpha policy and current evidence

Any Rust API, C ABI, WASM ABI, layout, ownership, error, or behavioral change
may occur only in a new prerelease. An ABI change in one facade increments all
three packages. The `2.14.3` prefix identifies the pinned FreeType target; it
does not claim complete replacement.

| Field | Value |
|---|---|
| Version | `2.14.3-alpha.1` |
| FreeType target | `2.14.3` |
| Last committed evidence | `2026-07-30` |
| Packages | `fontdone`, `fontdone-c-abi`, `fontdone-wasm` |

The machine-readable denominators are in
[`compatibility_snapshot.json`](compatibility_snapshot.json). Generated
package reports, release notes, inventories, archives, and checksums are local
outputs under `target/release-evidence/`.
