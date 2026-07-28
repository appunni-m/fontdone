# Third-Party Fixture Notices

The FreeType License in the repository root applies to `fontdone`; it does not
replace the licenses of fonts used as test fixtures. Fixture fonts are excluded
from the crates.io package.

## 1. Canonical active inputs

The active corpus under `tests/fixtures/input/` contains compact generated
fonts, modified/subset fonts, and a small number of third-party controls.
The [development guide](../../doc/DEVELOPMENT.md) defines the tracked boundary
and review workflow; [font provenance](input/fonts/PROVENANCE.md) and the
[generator policy](../../scripts/font_generation/README.md) record the durable
source and transformation contracts.

The following upstream-derived controls retain their copyright and license
records in each font's OpenType `name` table:

| Fixture family | Embedded attribution | License |
|---|---|---|
| DejaVu-derived fixtures | Bitstream, Inc.; Tavmjong Bah; DejaVu changes are public domain | Bitstream Vera/Arev font terms embedded as name ID 13 |
| Liberation-derived fixtures | Google Corporation; Red Hat, Inc. | SIL Open Font License 1.1 |
| Noto-derived fixtures | Google LLC | SIL Open Font License 1.1 |
| Ubuntu Sans variable-derived fixture | Google LLC | SIL Open Font License 1.1 |
| `FDArrayTest257` CID fixture | Adobe Systems Incorporated | SIL Open Font License 1.1; see `input/fonts/cid/FDArrayTest257.LICENSE.txt` |

The DejaVu license is stored at `licenses/DEJAVU.txt`; the SIL Open Font
License 1.1 is stored at `licenses/OFL-1.1.txt`. Top-level control-font hashes
and the limits of the recovered provenance record are documented in
`input/fonts/PROVENANCE.md`. Those compact controls may not be regenerated or
modified until their exact upstream release and transformation are recorded.

Generated fixtures that do not derive outlines or font software from a
third-party font use project-owned synthetic data. Their generators and
licensing rules are documented in `scripts/font_generation/README.md`.

## 2. Removed corpus

The former `tests/fixtures/deprecated/` corpus was deleted on 2026-07-28. It
included fonts from several upstream families and lacked a complete,
machine-checkable per-file provenance record. It must not be restored, fetched,
or regenerated. A needed behavior must instead receive a minimal focused
fixture whose source, license, modifications, and hashes satisfy the
redistribution checklist below.

## 3. Redistribution checklist

- Preserve embedded copyright and license records when subsetting a font.
- Keep every required standalone license text beside the fixture or in a
  repository-level fixture-license directory.
- Record the upstream source URL, version/commit, original filename, content
  hash, modifications, and resulting fixture hash.
- Do not assume the root FTL covers third-party font data.
- Do not add a font with unknown or non-redistributable terms.
