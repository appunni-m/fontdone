# Security policy

## Reporting

Do not open a public issue for an undisclosed vulnerability. Submit a private
[GitHub security advisory](https://github.com/appunni-m/fontdone/security/advisories/new)
with:

- affected version or commit;
- impact and expected attacker capability;
- operating system, target, and Rust version;
- the smallest reproduction you can safely provide;
- whether the input may be redistributed.

Do not attach a malicious or unlicensed font to a public issue. If private
reporting is temporarily unavailable, contact the repository owner privately
through the account linked from the repository and provide metadata first,
not the font payload.

Maintainers will acknowledge a report, reproduce it in an isolated fixture,
coordinate a fix and advisory, and credit the reporter unless anonymity is
requested. No fixed response deadline is promised during this alpha.

## Scope

Security-sensitive areas include untrusted font parsing, composite expansion,
bytecode execution, allocation/bounds checks, rasterization, raw C pointers,
WASM linear memory, and oracle/generator tooling. The core `fontdone` crate
forbids unsafe code; the C and WASM crates isolate their required raw boundary
operations. No runtime package may call FreeType C. Do not bypass those
boundaries as a fix.

Only the current prerelease is supported. Security fixes are released as a new
synchronized version of all three Cargo crates and the browser npm package.
