# fontdone-c-abi

`fontdone-c-abi` is the native C boundary for the pure-Rust `fontdone` engine.
It exports a measured FreeType-shaped subset from a library named
`fontdone_c_abi`; it never links, loads, or builds C FreeType at runtime.

Version `2.14.3-alpha.1` requires exactly `fontdone = 2.14.3-alpha.1`.
The crates are not published yet; build this package from the workspace. The
alpha is not ABI-stable.

Commands beginning with `make` or `python3 scripts/` are repository verification
commands. They require a complete `fontdone` checkout and are not installed by
the `.crate` archive. `cargo build -p fontdone-c-abi` is the package build
command available from an unpacked archive.

## 1. Compatibility boundary

There are three distinct boundaries:

1. **Declarations:** all 47 pinned public header paths compile as C and C++,
   all 218 function signatures are inventoried, and 78 public record layouts
   are compared on each measured target.
2. **Application behavior:** [`API_SUPPORT.md`](API_SUPPORT.md) classifies
   whether each function has a maintained application-ready mapping. An export
   or runtime probe alone is not complete behavior.
3. **Replacement contract:** `make c-abi-contract` measures functions,
   constants, types, layouts, callbacks, ownership, state, errors, modules,
   headers, artifacts, and platforms. The committed snapshot is 9/12
   categories complete: 4,964/5,047 pinned-C runtime contract rows are exact
   with 83 pending, binary artifacts are 7/8, and fresh platform bundles are
   1/5.

The package is not link-name-compatible with `libfreetype`: the artifact is
deliberately named `fontdone_c_abi`. Exported compatible functions retain
their `FT_*`/`FTC_*` names because renaming them would defeat the migration
surface.

Do not replace a system FreeType dependency solely because a header compiles.
Review the exact function rows, ownership contract, artifacts, and target
scorecard required by the application.

## 2. Build and artifacts

Debug:

```bash
cargo build -p fontdone-c-abi --locked
```

Release:

```bash
cargo build -p fontdone-c-abi --release --locked
```

The default `bzip2`, `lzw`, and `color-layers` Cargo features implement the
optional compressed-stream adapters and palette APIs. A deliberate
`--no-default-features` build retains all 5 symbols but returns
`FT_Err_Unimplemented_Feature` before inspecting arguments, matching the
corresponding pinned FreeType builds. `make optional-feature-contract`
automatically builds and compares pinned C, Rust FFI, separately linked C, and
WASM-host lanes for all 3 disabled feature groups and both unavailable LCD
filter setters. The same gate separately enables `subpixel-rendering` and
proves all 7 LCD setter routes, including the stored five-byte filter state and
custom-weight copy boundary.

| Host | Dynamic artifact | Static artifact | Support |
|---|---|---|---|
| Linux | `target/{debug,release}/libfontdone_c_abi.so` | `target/{debug,release}/libfontdone_c_abi.a` | Claimed |
| macOS | `target/{debug,release}/libfontdone_c_abi.dylib` | `target/{debug,release}/libfontdone_c_abi.a` | Claimed |
| Windows MSVC | `target\\{debug,release}\\fontdone_c_abi.dll` plus import `.lib` | `fontdone_c_abi.lib` | CI contract; committed cross-platform bundle still pending |

The Rust `rlib` in `target/{debug,release}/deps/` is for workspace verification,
not a C linker input.

From a repository checkout, install the release libraries, the complete
`fontdone2` include tree, and `fontdone2.pc` under a chosen prefix:

```bash
make c-abi-install PREFIX=/usr/local
make c-abi-install-check
```

Use `DESTDIR` for packaging or a staged install. The shared artifact carries a
relocatable ELF SONAME or Mach-O `@rpath` install-name.

## 3. Maintained C consumer

Build and run the repository example dynamically on Linux:

```bash
cargo build -p fontdone-c-abi --release --locked
cc -std=c11 -Wall -Wextra -Werror \
  -Ifontdone-c-abi/include fontdone-c-abi/examples/render_glyph.c \
  -Ltarget/release -lfontdone_c_abi \
  -Wl,-rpath,'$ORIGIN/release' \
  -o target/fontdone-c-example
LD_LIBRARY_PATH=target/release \
  target/fontdone-c-example tests/fixtures/input/fonts/DejaVuSans.ttf
```

On macOS:

```bash
cargo build -p fontdone-c-abi --release --locked
cc -std=c11 -Wall -Wextra -Werror \
  -Ifontdone-c-abi/include fontdone-c-abi/examples/render_glyph.c \
  -Ltarget/release -lfontdone_c_abi \
  -Wl,-rpath,@loader_path/release \
  -o target/fontdone-c-example
DYLD_LIBRARY_PATH=target/release \
  target/fontdone-c-example tests/fixtures/input/fonts/DejaVuSans.ttf
```

The example checks every error, opens a memory face, selects a pixel size,
renders `A`, validates the bitmap, and releases the face, library, input buffer,
and process allocations. Run the portable maintained wrapper instead of
copying host detection. It links and runs the same source against both the
shared library and static archive and requires identical output:

```bash
make test-c-consumer
```

## 4. Static linking

Start with the static archive:

```bash
cc -std=c11 -Wall -Wextra -Werror \
  -Ifontdone-c-abi/include fontdone-c-abi/examples/render_glyph.c \
  target/release/libfontdone_c_abi.a NATIVE_STATIC_LIBS \
  -o target/fontdone-c-example-static
```

Rust's required transitive system libraries vary by target and toolchain.
Obtain the exact current list instead of hard-coding a different host's list:

```bash
cargo rustc -p fontdone-c-abi --release --locked -- \
  --print native-static-libs
```

Replace `NATIVE_STATIC_LIBS` with the printed `-l`/framework arguments.
Static linking has no runtime library search path. Dynamic consumers must
install the shared library in the platform loader path, embed an rpath
appropriate for their deployment, or set `LD_LIBRARY_PATH`/`DYLD_LIBRARY_PATH`
for a development run. Environment-variable lookup is not recommended as a
production installation strategy.

## 5. Ownership and lifecycle

| Object or memory | Created by | Owner and validity | Release |
|---|---|---|---|
| `FT_Library` | `FT_Init_FreeType` | Caller owns one live handle | `FT_Done_FreeType` exactly once |
| `FT_Face` | `FT_New_Memory_Face` | Caller owns; references its library contractually | `FT_Done_Face` before the library |
| input font bytes | Caller | Copied during successful memory-face open; caller may release immediately afterward | Caller's allocator |
| default `FT_Size` | face open | Face-owned; valid until face teardown | Released with face |
| additional `FT_Size` | `FT_New_Size` | Caller-managed handle associated with the face | `FT_Done_Size` before face teardown |
| `face->glyph` slot | face/load call | Borrowed face-owned record; invalidated by the next load/render mutation or face teardown | Never free directly |
| slot bitmap buffer | load/render call | Borrowed with the slot; length is `abs(pitch) * rows` | Never free directly |
| standalone `FT_Bitmap` allocation | bitmap functions | Owned by the bitmap record after successful allocating calls | `FT_Bitmap_Done` |
| face strings, charmaps, table pointers | query | Borrowed; valid until the documented next mutation or face teardown | Never free directly |
| callbacks and callback user data | caller | Invoked synchronously unless the declaring call documents a stored stream-close callback; caller keeps captured data valid | Caller-defined |
| `FT_MM_Var` and other explicitly allocated result objects | corresponding getter | Caller-owned only on success | Matching `FT_Done_*` function named by the header/support contract |

Null, aliasing, length, and output-pointer rules follow the header signatures
and the measured FreeType row. Every non-null pointer must address the complete
declared object; a `(pointer, length)` pair must describe readable or writable
memory for the duration of the call. Passing an arbitrary pointer is undefined
behavior at the C boundary even when the function reports `FT_Error` for
recognized null or range errors.

## 6. Header and symbol verification

The package includes:

- the full `include/` public header tree;
- `fontdone2.pc`;
- `examples/render_glyph.c`;
- generated `API_SUPPORT.md`;
- `LICENSE`, `FTL.TXT`, and `NOTICE.md`.

Verify the compiled exports against the maintained header and inspect the
package boundary:

```bash
make check-c-exports
cargo package -p fontdone-c-abi --list
```

Run the complete automated C-contract measurement with:

```bash
make c-abi-contract
```

It executes the parity, independent external-C, ownership, state, error,
module, header, artifact, and platform evidence mechanisms and writes exact
numerators, denominators, and remaining debt to
`target/api-abi-audit/c_abi_contract_status.{json,md}`. Use
`make c-abi-contract-complete` when a release or CI job must fail unless all 12
contract categories are complete. The complete target requires all five
platform bundles assembled from CI; an ordinary one-host run is expected to
report platform debt.

The header is maintained for this package. Do not include system FreeType
headers in the same translation unit and assume record identity.

## 7. Alpha ABI and version policy

- All three packages always carry the same version.
- Every internal package dependency uses that exact version.
- No ABI compatibility is promised between different `alpha.N` releases.
- An exported symbol removal, signature change, record layout change,
  ownership change, or constant correction requires a new prerelease.
- A published crate version is immutable and is never replaced.
- Stable ABI policy will be declared only in a future non-prerelease release.

The release gate compares both artifact export sets, package files, versions,
generated contracts, and both running C consumers before publication.

## 8. License

This package is distributed under the FreeType Project License. The crate
archive contains `LICENSE`, `FTL.TXT`, and `NOTICE.md`. No test fonts or oracle
source are included.
