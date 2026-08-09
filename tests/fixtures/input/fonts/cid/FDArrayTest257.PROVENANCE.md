# FDArrayTest257 CID fixture provenance

Stored fixture: `ot-cff-cid-keyed.otf`

- Upstream repository: <https://github.com/adobe-fonts/fdarray-test>
- Upstream file: `FDArrayTest257.otf`
- Upstream commit: `e0b4382dee1625833b5f9b214eac0676d8ec7334`
- License: SIL Open Font License 1.1, copied in `FDArrayTest257.LICENSE.txt`
- SHA-256: `211f9ecb8b8064931f860e84bfe6e746e926273ef924990887ff2df13e6fede7`

Why this fixture is used:

- The upstream README describes `FDArrayTest257.otf` as a special-purpose
  CID-keyed OpenType/CFF font based on Adobe-Identity-0 ROS.
- The fixture is small enough for the repo and exercises the SFNT-wrapped CID
  service path needed by `FT_Get_CID_From_Glyph_Index` and
  `FT_Get_CID_Is_Internally_CID_Keyed`.
- Non-SFNT Type 1 CID rows remain pending; this fixture must not be used to
  satisfy those separate `cid_keyed_font` cases.

Derived maintained fixture: `ot-cff-cid-keyed-format2.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `f63d0db1c34ef09855d1270ecaae1f7bf503049de06319c4705896b69863baa5`
- The generator adds one real CID (`cid00257`) using the source's final
  charstring and FD assignment. The resulting contiguous CID range has
  `nLeft = 256`, so CFF serialization uses charset format 2 and exercises the
  corresponding parser branch. It remains distributed under the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-format0.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `f3da5cd3a5e0787bcb9bb3e071819f173dd338c39ab1f2926c4402dfda3c443c`
- The generator renames the source glyphs to alternating odd CIDs, making
  CFF charset format 0 the compact representation. It remains distributed
  under the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-format1.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `b9ac4a6d07c0c6d5fd39ab0086b588f32aef2a874676f8575e3dfa757ebb8734`
- The generator renames the source glyphs into two contiguous CID ranges,
  each within the one-byte `nLeft` limit, making CFF charset format 1 the
  compact representation. It remains distributed under the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-missing-charset.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `7f8f63d7286f5255c2b332e5c69091645a1aa89873c01f170e27ed91119a4f1b`
- The generator replaces the CFF Top DICT `charset` operator with an unrelated
  one-byte dictionary operator, leaving the CID face without a charset
  mapping. This reaches the pinned missing-CID-charset rejection while
  retaining the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-predefined-charset.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `be52582cd078e194d5980ec4fdda84ff5bdee3cb5e49933ac2f155f50725330d`
- The generator changes the Top DICT `charset` offset to the CFF predefined
  charset range `0`, which cannot supply the CID mapping required by this
  ROS face. It retains the source OFL and reaches the pinned predefined-
  charset rejection.

Derived maintained fixture: `ot-cff-cid-keyed-unsupported-charset-format.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `5d37292b7f16ba5065ff4e797e31a648655b3056951869a2593e4a6c712ede39`
- The generator changes the first byte at the maintained charset offset to
  format `3`, reaching the pinned unsupported-CFF-charset-format rejection.
  It remains distributed under the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-truncated-charset-range.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `bbd19f8d6bfc686b651f852587ae72c317ba369332cab4476162d73edfa4ba13`
- The generator moves the CFF table to the end of the SFNT, points `charset` at
  its final three bytes, and writes a format-1 range prefix without its
  required `nLeft` field. This reaches the pinned stream-limit rejection and
  retains the source OFL.

Derived maintained fixture: `ot-cff-cid-keyed-zero-glyph.otf`

- Generator: `scripts/font_generation/build_cff_fixtures.py`
- Source: the stored `ot-cff-cid-keyed.otf` above
- SHA-256: `5393b6912e613c238740dcf68a3792b5577c812e459608bd276aa9c395e41466`
- The generator sets the CFF CharStrings INDEX count and SFNT `maxp.numGlyphs`
  field to zero while preserving the CFF CID data. The pinned loader accepts
  the face and reaches its zero-glyph charset short-circuit; it retains the
  source OFL.
