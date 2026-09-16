# Release guide

Fontdone publishes one Cargo crate, `fontdone`, one npm package, `fontdone`,
and a native C SDK on GitHub Releases. The internal `fontdone-c-abi` and
`fontdone-wasm` Cargo packages have `publish = false`. There is no PyPI package.

Version **2.14.3-alpha.10** is published. The [release workflow](https://github.com/appunni-m/fontdone/actions/runs/35002120087)
and [tag CI](https://github.com/appunni-m/fontdone/actions/runs/35002119892)
passed for `cb90d41a863f8569335d8ed775a4038d23c79dc5`. Cargo and npm provenance
identify that source/run and their archives match GitHub checksums. Subsequent
publication uses this repository's tag-triggered `release.yml` and GitHub OIDC.

## Next candidate

The checkout prepares **2.14.3-alpha.11**. It remains an alpha: the production
C-replacement gate still requires complete contract and benchmark evidence.
The cleanup preserves deprecated FreeType compatibility aliases because they
are part of the pinned oracle's public contract; removing them would break
replacement behavior. No incomplete comparison becomes a pass through cleanup.

## 1. Trusted publisher configuration

Configure these identities on the existing packages:

| Registry | Repository | Workflow filename | GitHub environment |
|---|---|---|---|
| crates.io | `appunni-m/fontdone` | `release.yml` | `crates-io` |
| npm | `appunni-m/fontdone` | `release.yml` | `npm` |

The publish jobs alone receive `id-token: write`. Crates.io authentication uses
`rust-lang/crates-io-auth-action`; npm uses Node 22.14.0 and npm 11.5.1 with
provenance. An environment configured with required reviewers will pause its
job until that review occurs. No repository secret containing a registry token
is needed. Keep publisher settings and workflow environment names identical.

This follows the isolated verification, artifacts, OIDC, and GitHub Release
stages used by [coverage-mcp's release workflow](https://github.com/appunni-m/coverage-mcp/blob/v0.16.0/.github/workflows/release.yml).

## 2. Alpha acceptance policy

Starting with alpha.7, incomplete source coverage and incomplete C-contract
adoption are reported, and do not require 100% completion for an alpha release.
This is a release acceptance change, not a claim of complete FreeType parity.

Every executed test remains required: the fast gate and MSRV, all runnable
exact parity comparisons, Rust/C/WASM/npm consumers, five native or emulated
platform contracts, package checks, dependency audits, and benchmark checks.
Coverage collection must succeed and retain its actual totals. No source files,
inputs, expected results, or uncovered lines are removed from measurement to
meet this policy. Undefined C inputs remain explicitly named pending cases.

`make release-verify` runs the local alpha checks and reports contract debt.
`make release-verify-complete` additionally requires all twelve C-contract
categories. The latter needs five fresh platform bundles and remains the
stricter gate for a complete replacement claim. It also enforces performance
thresholds once the benchmark policy has completed its baseline-review phase.
That policy currently has no thresholds; alpha CI retains ten-sample benchmark
reports without claiming that a regression budget passed.

The [compatibility snapshot](compatibility_snapshot.json) and
[adoption guide](FREETYPE_SUPPORT.md) distinguish completed functionality, measured
runtime parity, historical coverage, and unfinished C-contract requirements.

The alpha.8 local verification on 2026-09-15 passes 20,357/20,357 runnable
cases in each of the Rust, C ABI, and WASM coverage lanes, with three named
undefined-C cases pending. The combined source report measures 66,270/68,525
lines (96.7092%), 12,189/13,836 branches (88.0963%), 3,849/4,137 functions
(93.0384%), and 91,297/94,949 regions (96.1537%). The WinFNT record conversion
is exercised by the retained header inputs. Big-endian-only branches require
the separate PowerPC runtime lane and are not measured by this macOS report.
GitHub produces its own report for the final tag; this local result does not
stand in for a successful publishing job.

## 3. Prepare and verify a version

1. Increment the root Cargo version, both private workspace versions and their
   exact root dependency requirements, and the npm version together. Run
   `make release-lock-update` to synchronize only workspace lockfile entries
   using the already downloaded dependency set.
2. Update the README, changelog, and package documentation. Regenerate derived
   ABI metadata with `make generate-contracts`.
3. Run `make test-parity` and `make record-parity-snapshot`. This binds the
   reported comparison counts to the measured sources. Then refresh the file
   ledger with `make repository-inventory`.
4. Run `make ci-fast` using the pinned font-generation Python environment
   described in [Development](DEVELOPMENT.md). Inspect packages with
   `make release-dry-run`. Use `make release-verify` for a local thorough run.
5. Commit, push to `main`, and require successful CI for that exact commit.
6. Create an annotated, unused `v<version>` tag on that commit and push it.
   Never move a published tag or overwrite an existing registry version.

For the published release, registry consumers use:

```toml
[dependencies]
fontdone = { version = "=2.14.3-alpha.10" }
```

A development path or pinned Git revision may be added, but a publishable
consumer must retain the exact registry version.

## 4. What the tag runs

The tag starts the full CI matrix. Release preflight waits for successful CI
on the exact tag commit; a fast branch run cannot satisfy that gate. It verifies
the annotated remote tag object, synchronized versions, and downloads that
run's checked Cargo/npm packages and platform evidence. It builds the native
C SDK and produces a checksum manifest excluding the manifest itself.

The crates.io job compiles the packaged source before authentication and
compares the resulting archive with the downloaded CI artifact. Only then does
it mint an OIDC token and run `make release-publish-oidc VERIFIED_CRATE=...`.
The helper rejects local invocation, the wrong repository/tag, missing OIDC
context, dirty sources, and archive differences. Cargo's duplicate verification
is skipped only after the identical archive has been compiled before token
minting. The published registry checksum must match the verified archive.

The npm job checks the bundle checksum and publishes the exact tested `.tgz`
with provenance under `next`. Existing versions are accepted only when the
registry integrity matches the candidate; network errors are not treated as
missing versions.

`make npm-package-verify` also stages a one-directory relative archive and runs
the same publish helper in offline dry-run mode. An existing artifact can be
checked with `make release-npm-verify VERIFIED_NPM_ARCHIVE=<path>`; real uploads
use `make release-npm-publish-oidc` in the exact GitHub tag job. Both paths resolve
the local filename before npm sees it, because
[npm package specifiers](https://docs.npmjs.com/cli/v11/using-npm/package-spec/)
otherwise interpret `release-bundle/archive.tgz` as GitHub shorthand. Failed
uploads expose a bounded diagnostic in the public workflow annotations.

Only after both registries succeed does the final job attest the artifacts and
create the GitHub prerelease with compatibility notes, Cargo archive, npm
archive, native SDK, and `SHA256SUMS`. GitHub Release commands specify the
repository explicitly, including jobs without a source checkout.

## 5. Recovery and verification

Inspect the first failed job and its retained diagnostics. A skipped publish
job has not tested OIDC authentication. A publisher identity rejection in a
running publish job is a registry configuration issue; a failed parity,
coverage-collection, or package job is a repository issue.

Rerun a transient failure on the same immutable tag. If source or workflow
changes are needed, make a new version and tag. A partially published version
must not be rebuilt into different bytes. Preserve earlier failed tags and run
records as history.

After publication, check the exact crates.io version and checksum, npm version
and provenance, and the GitHub prerelease assets. Confirm a fresh registry
consumer can install and exercise the released APIs. A green preflight alone
is not evidence that either registry accepted the release.
