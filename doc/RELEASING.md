# Release guide

The `fontdone`, `fontdone-c-abi`, and `fontdone-wasm` Cargo crates plus the
browser npm package `fontdone` form one synchronized release unit. All four
artifacts currently use `2.14.3-alpha.1`; Cargo facade dependencies require
that exact root-crate version.

The root manifest is the publishable `fontdone` crate. A downstream crate may
use a sibling checkout during development, but its dependency must retain the
exact version requirement:

```toml
fontdone = { version = "=2.14.3-alpha.1", path = "../fontdone" }
```

After publication, a registry consumer such as `pillow-rs` must use
`fontdone = { version = "=2.14.3-alpha.1" }`. A path-only declaration is valid
for a local build but Cargo rejects it when packaging the downstream crate.

The first synchronized release is bootstrapped locally from the exact clean
commit. After the registry versions exist, the protected tag-driven GitHub
workflow publishes the three Cargo crates and the browser npm package. Local
commands validate and assemble evidence; they do not create tags or releases.

Publication is paused during active parity, coverage, and performance work.
Do not push a release tag, publish a crate, or create a GitHub release until
the repository owner explicitly approves publication.

## 1. Release prerequisites

- At least two current crates.io owners exist for each package.
- The GitHub `crates-io` environment requires reviewer approval.
- crates.io and npm trusted publishers are configured for the protected
  `crates-io` and `npm` environments; no long-lived registry token is stored in
  the workflow.
- The release commit is clean, pushed, and has a successful CI run.
- The exact version has not previously been published or tagged.
- The `fontdone` npm name is still available to the publishing account, the
  account requires two-factor authentication, and the exact npm version has
  not previously been published.

Tokens must never appear in command arguments, repository files, logs, or
generated evidence.

## 2. Prepare the release commit

1. Update all three Cargo crate versions, the npm package version, and both
   exact internal Cargo requirements.
2. Update the root README release banner.
3. Run `make test-parity`, then `make record-parity-snapshot`; the second
   command refuses evidence whose source digest does not match the worktree.
4. Replace the C-contract values with evidence from the complete scorecard.
5. Review the generated function map, C headers, WASM schema/declarations, and
   synchronized legal files.
6. Move the changelog entry from “Unreleased” to the release date.
7. Review `Cargo.lock` and every Cargo and npm archive input.
8. Run:

   ```bash
   make check-versions
   make check-generated
   make check-docs
   make package-verify
   make npm-package-verify
   ```

`make package-verify` creates all three `.crate` archives, rejects fixture,
font, oracle, test, and tooling leakage, compiles the extracted packages with
exact local dependency substitutions, and writes inventories and SHA-256
digests under `target/release-evidence/`. `make npm-package-verify` builds the
Wasm asset, runs wrapper tests, creates and inspects the exact `.tgz`, installs
it into a temporary dependency consumer, reruns its shipped self-test, and
renders a glyph through the installed package. `make check-versions` also
verifies the root package identity, synchronized workspace members, exact
facade requirements, npm name/version/publish tag, and the versioned path
dependency used by the external Rust consumer.

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

## 4. First local bootstrap

After `make release-verify` passes on a clean, reviewed commit, publish the
Cargo crates one at a time through the maintained dependency-order script:

```bash
cargo login
RELEASE_APPROVED=1 RELEASE_CI_SHA="$(git rev-parse HEAD)" \
  python3 scripts/publish_release.py --publish
cargo logout
```

The script publishes `fontdone`, waits for crates.io visibility, then publishes
`fontdone-c-abi` and `fontdone-wasm`. Build and publish the exact npm artifact
from `make npm-package-verify` separately under the `next` dist-tag:

```bash
npm publish target/npm-package/fontdone-2.14.3-alpha.1.tgz \
  --access public --tag next --provenance
```

The local bootstrap intentionally uses `--publish` because these are new
registry versions. The tag workflow uses `--publish-if-missing` and checks the
npm version first, so retrying the immutable bootstrap tag after a successful
local upload preserves the existing registry artifacts instead of attempting a
second publication.

Do not place either credential in a command, file, or log. Configure the
protected trusted publishers before using the automated path.

## 5. Trigger and publication order

Push an annotated `v<version>` tag for the exact synchronized commit. The
`.github/workflows/release.yml` workflow verifies the tag, waits for successful
CI on that commit, and publishes from the verified bundle.

After approval, `scripts/publish_release.py --publish-if-missing` performs:

1. publish `fontdone` when that exact version is not already visible;
2. wait until crates.io serves the exact version;
3. publish `fontdone-c-abi` when that exact version is not already visible;
4. wait until crates.io serves the exact version;
5. publish `fontdone-wasm` when that exact version is not already visible.

The script names every package, preserves immutable versions already visible,
stops at the first failure, requires a clean tracked worktree, and waits up to
ten minutes for each dependency to become visible. Never run an unqualified
`cargo publish` from the workspace root.

For registry-resolution rehearsal after the root version is visible:

```bash
python3 scripts/publish_release.py --dry-run
```

Before publication, `make package-verify` is the reproducible archive-level
equivalent; a facade registry dry-run cannot resolve an unpublished exact root
dependency.

## 6. Browser npm publication

The verified browser artifact is:

```text
target/npm-package/fontdone-2.14.3-alpha.1.tgz
```

Rehearse the registry command without publishing:

```bash
npm publish --dry-run \
  target/npm-package/fontdone-2.14.3-alpha.1.tgz \
  --access public --tag next
```

After explicit owner approval, authenticate with npm and publish that exact
tarball, not the mutable source directory:

```bash
npm publish target/npm-package/fontdone-2.14.3-alpha.1.tgz \
  --access public --tag next
```

The tag workflow queries `npm view fontdone@<version>` first and skips an
already visible immutable version, which makes a tag retry safe after the
local bootstrap.

The `next` dist-tag prevents this alpha from silently becoming the stable
`latest` release. Immediately verify the immutable version and tag:

```bash
npm view fontdone@2.14.3-alpha.1 version dist.tarball --json
npm view fontdone dist-tags --json
```

Never place an npm token in a command, repository file, npm URL, or captured
log. A registry name check is time-sensitive; rerun it immediately before the
approved publish.

## 7. Tags and release assets

After the maintainer pushes annotated tag `v2.14.3-alpha.1` at the approved
commit and all three Cargo publications succeed, the workflow:

1. verifies the immutable tag and successful CI result;
2. creates the GitHub release from generated notes;
3. attaches all three exact `.crate` archives, the verified npm `.tgz`, and
   `SHA256SUMS`.

Never move or recreate a published tag. Attached checksums must describe the
same archives inspected during preflight.

## 8. Failure, retry, and registry recovery

Stop at the first failed publication. Do not skip a package or publish a facade
against a missing root version.

- Retry the same unpublished package after a transient local or network error.
- Published crate contents are immutable.
- If published contents are wrong, explicitly yank the affected version; do
  not delete its tag or reuse its version.
- Normally yank the synchronized facades when yanking their root version.
- Fix the issue and publish a new synchronized prerelease.

For npm, do not reuse a published version. If package contents are wrong,
deprecate the affected version with a clear replacement message, move `next`
back to the last reviewed version when appropriate, fix forward, and publish a
new synchronized prerelease. Follow npm's current unpublish policy only for an
exception that genuinely requires removal.

Example:

```bash
cargo yank --version 2.14.3-alpha.1 fontdone-wasm
```

## 9. Alpha policy and current evidence

Any Rust API, JavaScript API, C ABI, WASM ABI, layout, ownership, error, or
behavioral change may occur only in a new prerelease. A public change in one
surface increments all three Cargo crates and the npm artifact. The `2.14.3`
prefix identifies the pinned FreeType target; it does not claim complete
replacement.

| Field | Value |
|---|---|
| Version | `2.14.3-alpha.1` |
| FreeType target | `2.14.3` |
| Last committed evidence | `2026-07-30` |
| Cargo crates | `fontdone`, `fontdone-c-abi`, `fontdone-wasm` |
| Browser npm package | `fontdone` |

The machine-readable denominators are in
[`compatibility_snapshot.json`](compatibility_snapshot.json). Generated
package reports, release notes, inventories, archives, and checksums are local
outputs under `target/release-evidence/`.
