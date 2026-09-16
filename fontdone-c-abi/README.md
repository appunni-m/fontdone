# C and C++ integration

<!-- release:summary -->
**Latest release: [2.14.3-alpha.11](https://github.com/appunni-m/fontdone/releases/tag/v2.14.3-alpha.11).**
<!-- /release:summary -->

Use the prebuilt native SDK from GitHub Releases. It contains headers, a
shared library, a static library, and a C rendering example. This release's
prebuilt SDK targets **macOS ARM64**. Other targets require a
[contributor source build](https://appunni-m.github.io/fontdone/development/).

## Install the SDK

<!-- release:sdk -->
[Download the macOS ARM64 C SDK](https://github.com/appunni-m/fontdone/releases/download/v2.14.3-alpha.11/fontdone-c-abi-2.14.3-alpha.11-aarch64-apple-darwin.tar.gz).
<!-- /release:sdk -->

Extract the downloaded archive into a directory of your choice. The archive
has one top-level directory containing `include/`, `lib/`, and `examples/`.
In the commands below, place its contents in a directory named `fontdone-sdk`.
No Cargo installation is needed to use this prebuilt SDK.

## Open and close a library

Save as `example.c` next to `fontdone-sdk`:

```c
#include <ft2build.h>
#include FT_FREETYPE_H

int main(void) {
    FT_Library library = NULL;
    FT_Error error = FT_Init_FreeType(&library);
    if (error != 0) return (int)error;
    return (int)FT_Done_FreeType(library);
}
```

Compile and run on macOS ARM64:

```sh
cc example.c -Ifontdone-sdk/include -Lfontdone-sdk/lib -lfontdone_c_abi \
  -Wl,-rpath,@loader_path/fontdone-sdk/lib -o example
./example
```

Successful initialization and cleanup exits with code zero. The SDK's
`examples/render_glyph.c` shows memory-face loading, sizing, rendering, and
cleanup; it accepts the path to a font you are licensed to use.

## Compatibility

Use the SDK headers with `fontdone_c_abi`. Do not mix them with system FreeType
headers in the same translation unit. The library retains supported `FT_*`
and `FTC_*` names, but does not use the `libfreetype` linker name.

Read [supported features](https://appunni-m.github.io/fontdone/maturity/) and
the [function reference](https://appunni-m.github.io/fontdone/api-support/)
before replacing a dependency. Header compatibility does not imply every
FreeType behavior. No ABI compatibility is promised between alpha versions.

## Ownership and lifecycle


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



## Distribution and license

Ship the appropriate shared library with your application and preserve its
loader path, or link the static archive with your toolchain's required system
libraries. Follow the bundled `LICENSE`, `FTL.TXT`, and `NOTICE.md` when
redistributing the SDK. Build and verification procedures are in the
[contributor guide](https://appunni-m.github.io/fontdone/development/).
