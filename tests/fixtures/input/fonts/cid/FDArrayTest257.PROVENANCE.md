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
