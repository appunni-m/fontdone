# Top-Level Font Fixture Provenance

These files are compact, modified test controls. The hashes below identify the
reviewed bytes; they are not hashes of upstream full-family releases.

| File | SHA-256 | Embedded copyright | License |
|---|---|---|---|
| `DejaVuSans.ttf` | `4c44206cbb0238b4a07bac4c557a9254a2dc87941f6ebebfa1147c32dd73b29a` | Bitstream, Inc.; Tavmjong Bah; DejaVu changes public domain | `../../licenses/DEJAVU.txt` |
| `LiberationSerif-Regular.ttf` | `2cf2f480ecb644a0702cfd687cc0dec3025b3601edd57bf11ba0c5fce9014a6e` | Google Corporation; Red Hat, Inc. | `../../licenses/OFL-1.1.txt` |
| `NotoSans-Regular.ttf` | `c950ae3feb035f6ebfbd5cf10c6c4de7e541edec43d636e3b6b9f38bd3209adc` | Google LLC | `../../licenses/OFL-1.1.txt` |

All three fonts retain readable copyright and license records in their
OpenType `name` tables. The repository history available during the 2026-07-28
review did not identify the exact upstream release and subsetting command for
these already-compact files. Therefore:

- the present reviewed bytes may remain as licensed test fixtures;
- they must not be regenerated, further modified, or used as a new generator
  base without first recording the exact upstream release and deterministic
  transformation;
- new fixtures should prefer project-authored synthetic fonts;
- replacement work should record the upstream URL, release or commit, original
  hash, command, output hash, and any Reserved Font Name handling.

The applicable license texts are retained in `tests/fixtures/licenses/`.

## 1. Project-authored generated input

| File | Generator | Bytes | SHA-256 | Classification and license |
|---|---|---:|---|---|
| `cff2/fontinfo-invalid-argument.otf` | `scripts/font_generation/build_cff_fixtures.py` via `make font-fixture-cff` | 804 | `6f457efaafee3496f42e8e2dd977acf39730a86252066135076050563949c4e3` | Synthetic; project-authored SFNT metadata, CFF2 Top DICT, FDArray, and Type2 charstrings; no third-party font material or license notice required. |
| `cid/fontinfo-populated.cid` | `scripts/font_generation/build_type1_fixtures.py` via `make font-fixture-type1` | 900 | `8bad30fd383566771d5498ba12bbf04f941e876dc9538c50290ee46fae2c5f2e` | Synthetic; project-authored CIDFont dictionaries, FontInfo, FDArray, and CIDMap; no third-party font material or license notice required. |
| `pfr/basic-metrics-and-kerning.pfr` | `scripts/font_generation/build_pfr_fixtures.py` via `make font-fixture-pfr` | 146 | `c15dc9fcfe1066ee91c149cbedc49f64cc788fe946c551d2794cd6e926eb5e41` | Synthetic; project-authored PFR v4 header, logical and physical font records, two character advances, and two kerning pairs; no third-party font material or license notice required. |
