# Release guide

The public release unit has one Cargo crate (`fontdone`), one native C SDK
archive, and one browser npm package (`fontdone`). The `fontdone-c-abi` and
`fontdone-wasm` Cargo packages stay in the workspace as internal build targets;
their exact version requirements keep the SDK and npm output synchronized with
the root crate.

The root manifest is the publishable `fontdone` crate. A downstream crate may
use a sibling checkout during development, but its dependency must retain the
exact version requirement:

```toml
fontdone = { version = "=2.14.3-alpha.3", path = "../fontdone" }
```

After publication, a registry consumer such as `pillow-rs` must use
`fontdone = { version = "=2.14.3-alpha.3" }`. A path-only declaration is valid
for a local build but Cargo rejects it when packaging the downstream crate.

The first synchronized release is bootstrapped locally from the exact clean
commit. The public Cargo crate and synchronized `fontdone@2.14.3-alpha.3` npm
package are still unpublished from this checkout. The older alpha.1 npm
artifact is already visible, but its immutable contents predate the current
source. Because npm versions are immutable, the alpha.3 archive must be
published as a new synchronized prerelease; a retry may skip an existing npm
version only after the registry package contents match the reviewed local
archive. Local commands validate and assemble the Cargo, C SDK, and npm
artifacts; they do not create tags or releases.

Publication is paused during active parity, coverage, and performance work.
Do not push a release tag, publish a crate, or create a GitHub release until
the repository owner explicitly approves publication.

## 1. Release prerequisites

- At least two current crates.io owners exist for the `fontdone` package.
- The GitHub `crates-io` environment requires reviewer approval.
- crates.io and npm trusted publishers are configured for the protected
  `crates-io` and `npm` environments; no long-lived registry token is stored in
  the workflow.
- The release commit is clean, pushed, and has a successful CI run.
- The exact Cargo versions have not previously been published or tagged.
- The `fontdone` npm version is checked immediately before release. If it is
  visible, the workflow compares its extracted package contents with the
  reviewed archive and skips only an exact match; a mismatch stops the release
  and requires a new version.

Tokens must never appear in command arguments, repository files, logs, or
generated evidence.

## 2. Prepare the release commit

1. Update the public Cargo crate version, the npm package version, and both
   exact internal Cargo requirements.
2. Update the root README release banner.
3. Run `make test-parity`, then `make record-parity-snapshot`; the second
   command refuses evidence whose source digest does not match the worktree.
4. Run `make c-abi-contract`, then run `make record-c-contract-snapshot` to
   promote the generated C-contract measurements into the committed snapshot.
   This records incomplete debt as well; `make c-abi-contract-complete` is
   still required before publication.
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

`make package-verify` creates and inspects all three workspace `.crate`
archives so the internal facades remain reproducible, but only the root
`fontdone` archive is a public Cargo artifact. It rejects fixture, font,
oracle, test, and tooling leakage, compiles the extracted packages with exact
local dependency substitutions, and writes inventories and SHA-256 digests
under `target/release-evidence/`. `make c-abi-package` assembles the native C
SDK archive from the built library, headers, pkg-config metadata, examples,
and legal files. `make npm-package-verify` builds the Wasm asset, runs wrapper
tests, creates and inspects the exact `.tgz`, installs it into a temporary
dependency consumer, reruns its shipped self-test, and renders a glyph through
the installed package. `make check-versions` also verifies the root package
identity, synchronized workspace members, private facade markers, exact facade
requirements, npm name/version/publish tag, and the versioned path dependency
used by the external Rust consumer.

## 3. Required CI evidence

The exact release commit must first pass the per-commit
[CI contract](DEVELOPMENT.md#61-per-commit-gate), then a requested
[thorough run](DEVELOPMENT.md#62-requested-thorough-gate). The latter uploads
all five hash-bound C platform bundles and validates the assembled evidence.
Release preflight additionally runs `make c-abi-contract-complete`; unfinished
contract debt therefore cannot be released.

The release preflight locates the successful **thorough `workflow_dispatch`**
CI run for the exact tag commit, downloads its five platform-contract artifacts,
and runs `make release-verify`. A push-triggered fast CI run is intentionally
not sufficient because it does not produce the cross-platform contract
bundles.
Without assembled bundles, a local `make release-verify` correctly fails the
complete C contract. Use `make ci` and `make c-abi-contract` for ordinary
single-host development, and `make ci-thorough` only when a local exhaustive
audit is requested. The final release step reruns `make check-docs` after the
complete scorecard is generated, so a stale committed compatibility snapshot
blocks publication.

## 4. First local bootstrap

After `make release-verify` passes on a clean, reviewed commit, publish the
single public Cargo crate through the maintained script:

```bash
cargo login
RELEASE_APPROVED=1 RELEASE_CI_SHA="$(git rev-parse HEAD)" \
  python3 scripts/publish_release.py --publish
cargo logout
```

The script publishes `fontdone`. Build the exact C SDK archive from
`make c-abi-package` and the npm artifact from `make npm-package-verify`; the
tag workflow attaches the C archive and publishes the npm artifact only after
the registry check confirms that the immutable version is missing. For a new
version, publish the exact verified archive:

```bash
version=2.14.3-alpha.3
npm publish "target/npm-package/fontdone-${version}.tgz" \
  --access public --tag next --provenance
```

The local bootstrap intentionally uses `--publish` because the Cargo version is
new. The tag workflow uses `--publish-if-missing` for Cargo and performs an
immutable npm content check before deciding whether to publish. Retrying a tag
after a successful upload therefore preserves an identical registry artifact;
it fails loudly if the visible version came from different source bytes. The
older `2.14.3-alpha.1` npm version remains immutable historical evidence and
is not reused by this checkout.

Do not place either credential in a command, file, or log. Configure the
protected trusted publishers before using the automated path.

## 5. Trigger and publication order

Push an annotated `v<version>` tag for the exact synchronized commit. The
`.github/workflows/release.yml` workflow verifies the tag, waits for successful
CI on that commit, and publishes from the verified bundle.

After approval, `scripts/publish_release.py --publish-if-missing` publishes
`fontdone` when that exact version is not already visible. The C SDK archive is
distributed as a GitHub release asset, and the browser package is published to
npm by the separate workflow job.

The script names the public package, preserves an immutable version already
visible, stops at the first failure, and requires a clean tracked and untracked
worktree. Never run an unqualified `cargo publish` from the workspace root.

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
target/npm-package/fontdone-2.14.3-alpha.3.tgz
```

Rehearse the registry command without publishing:

```bash
npm publish --dry-run \
  target/npm-package/fontdone-2.14.3-alpha.3.tgz \
  --access public --tag next
```

After bumping all synchronized manifests to a new version, and once that
version is missing after explicit owner approval, authenticate with npm and
publish that exact tarball, not the mutable source directory:

```bash
VERSION=2.14.3-alpha.3
npm publish "target/npm-package/fontdone-${VERSION}.tgz" \
  --access public --tag next
```

Before publication, the tag workflow queries npm's immutable `dist.integrity`
for the exact version and compares it with the SHA-512 digest of the tarball in
the release bundle. A missing version is published; an exact archive match is
skipped; any mismatch fails the job and requires a new prerelease. A registry
name check alone is insufficient because it can silently preserve an artifact
built from a different commit.

The `next` dist-tag prevents this alpha from silently becoming the stable
`latest` release. Immediately verify the immutable version and tag:

```bash
npm view fontdone@2.14.3-alpha.3 version dist.tarball --json
npm view fontdone dist-tags --json
```

Never place an npm token in a command, repository file, npm URL, or captured
log. A registry name check is time-sensitive; rerun it immediately before the
approved publish.

## 7. Tags and release assets

After the maintainer pushes annotated tag `v2.14.3-alpha.3` at the approved
commit and the Cargo and npm publications succeed, the workflow:

1. verifies the immutable tag and successful CI result;
2. creates the GitHub release from generated notes;
3. attaches the exact public `.crate`, native C SDK archive, verified npm
   `.tgz`, and `SHA256SUMS`.

Never move or recreate a published tag. Attached checksums must describe the
same archives inspected during preflight.

## 8. Failure, retry, and registry recovery

Stop at the first failed publication. Do not distribute a C SDK or npm
artifact that was built against a different root version.

- Retry the same unpublished package after a transient local or network error.
- Published crate contents are immutable.
- If published contents are wrong, explicitly yank the affected version; do
  not delete its tag or reuse its version.
- Rebuild the synchronized C SDK and npm artifacts when yanking their root
  version; the internal facade packages are not registry releases.
- Fix the issue and publish a new synchronized prerelease.

For npm, do not reuse a published version. If package contents are wrong,
deprecate the affected version with a clear replacement message, move `next`
back to the last reviewed version when appropriate, fix forward, and publish a
new synchronized prerelease. Follow npm's current unpublish policy only for an
exception that genuinely requires removal.

Example:

```bash
cargo yank --version 2.14.3-alpha.3 fontdone
```

## 9. Alpha policy and current evidence

Any Rust API, JavaScript API, C ABI, WASM ABI, layout, ownership, error, or
behavioral change may occur only in a new prerelease. A public change in one
surface increments the public Cargo crate, C SDK, and npm artifact. The `2.14.3`
prefix identifies the pinned FreeType target; it does not claim complete
replacement.

| Field | Value |
|---|---|
| Version | `2.14.3-alpha.3` |
| FreeType target | `2.14.3` |
| Last committed evidence | `2026-07-30` |
| Public Cargo crate | `fontdone` |
| Internal Cargo build targets | `fontdone-c-abi`, `fontdone-wasm` |
| Native C SDK archive | `fontdone-c-abi-<version>-<target>.tar.gz` |
| Browser npm package | `fontdone` |

The machine-readable denominators are in
[`compatibility_snapshot.json`](compatibility_snapshot.json). Generated
package reports, release notes, inventories, archives, and checksums are local
outputs under `target/release-evidence/`.
