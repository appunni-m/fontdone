#!/usr/bin/env python3
"""Build compact project-authored GX/AAT validator fixtures."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont
from fontTools.ttLib.tables.DefaultTable import DefaultTable


ROOT = Path(__file__).resolve().parents[2]
BASE_FONT = (
    ROOT
    / "tests"
    / "fixtures"
    / "input"
    / "fonts"
    / "glyf"
    / "hinter-control-matrix.ttf"
)
TYPE1_BASE = (
    ROOT / "tests" / "fixtures" / "input" / "fonts" / "type1" / "simple-type1.pfb"
)
FONT_ROOT = ROOT / "tests" / "fixtures" / "input" / "fonts"


def raw_table(tag: str, data: bytes) -> DefaultTable:
    table = DefaultTable(tag)
    table.data = data
    return table


def save_sfnt(path: Path, tables: dict[str, bytes]) -> None:
    font = TTFont(BASE_FONT, recalcTimestamp=False)
    for tag, data in tables.items():
        font[tag] = raw_table(tag, data)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists() or path.is_symlink():
        path.unlink()
    font.save(path, reorderTables=True)


EMPTY_LOOKUP_FORMAT_8 = b"\x00\x08\x00\x00\x00\x00"

# Each table is the smallest non-malformed instance accepted by the pinned
# FreeType 2.14.3 gxvalid top-level validator.  Zero record/chain counts keep
# the fixture focused on public selection, copying, length, and ownership.
GX_TABLES = {
    "feat": b"\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00",
    "mort": b"\x00\x01\x00\x00\x00\x00\x00\x00",
    "morx": b"\x00\x02\x00\x00\x00\x00\x00\x00",
    "bsln": b"\x00\x01\x00\x00\x00\x00\x00\x00" + bytes(64),
    "just": b"\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00",
    "kern": b"\x00\x00\x00\x00",
    "opbd": b"\x00\x01\x00\x00\x00\x00" + EMPTY_LOOKUP_FORMAT_8,
    "trak": b"\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00",
    "prop": b"\x00\x01\x00\x00\x00\x00\x00\x00",
    "lcar": b"\x00\x01\x00\x00\x00\x00" + EMPTY_LOOKUP_FORMAT_8,
}


def main() -> None:
    aat_dir = FONT_ROOT / "aat"
    kern_dir = FONT_ROOT / "kern"
    truetype_dir = FONT_ROOT / "truetype"
    type1_dir = FONT_ROOT / "type1"

    save_sfnt(
        aat_dir / "feat-opbd-trak-prop.ttf",
        {tag: GX_TABLES[tag] for tag in ("feat", "opbd", "trak", "prop")},
    )
    save_sfnt(aat_dir / "full-gx-valid.ttf", GX_TABLES)
    # The public-api matrix names these focused inputs separately so each
    # validator's output-slot index can be exercised without silently
    # substituting a different table family.  They all derive from the same
    # project-authored minimal table bytes above.
    generated_aat_dir = ROOT / "tests" / "fixtures" / "input" / "generated" / "fonts" / "aat-gx"
    save_sfnt(generated_aat_dir / "all-validation-tables.ttf", GX_TABLES)
    save_sfnt(
        generated_aat_dir / "valid-feat-morx-kern-lcar.ttf",
        {tag: GX_TABLES[tag] for tag in ("feat", "morx", "kern", "lcar")},
    )
    for tag, filename in (
        ("bsln", "valid-bsln.ttf"),
        ("feat", "valid-feat.ttf"),
        ("just", "valid-just.ttf"),
        ("kern", "valid-gx-kern.ttf"),
        ("lcar", "valid-lcar.ttf"),
        ("mort", "valid-mort.ttf"),
        ("morx", "valid-morx.ttf"),
    ):
        save_sfnt(generated_aat_dir / filename, {tag: GX_TABLES[tag]})

    # The public-contract matrix exercises both the selected table output and
    # the validator's absent/malformed controls.  Keep those controls as
    # deterministic SFNTs rather than pointing at ad-hoc files or silently
    # reusing a different table family.  The truncated and invalid-header
    # variants are intentionally small: every pinned validator rejects them
    # before table-specific payload interpretation, which lets the parity
    # route compare the exact public error and sentinel state in all lanes.
    for tag in GX_TABLES:
        save_sfnt(generated_aat_dir / f"no-{tag}.ttf", {})
        save_sfnt(
            generated_aat_dir / f"malformed-{tag}-truncated.ttf",
            {tag: b"\x00"},
        )
        save_sfnt(
            generated_aat_dir / f"malformed-{tag}-header.ttf",
            {tag: b"\xff\xff\xff\xff\xff\xff\xff\xff"},
        )
    save_sfnt(aat_dir / "opbd-valid.ttf", {"opbd": GX_TABLES["opbd"]})
    save_sfnt(aat_dir / "trak-valid.ttf", {"trak": GX_TABLES["trak"]})
    save_sfnt(aat_dir / "prop-valid.ttf", {"prop": GX_TABLES["prop"]})
    save_sfnt(aat_dir / "malformed-gx-table.ttf", {"feat": b"\0"})
    save_sfnt(truetype_dir / "no-gx-tables.ttf", {})
    save_sfnt(truetype_dir / "no-kern.ttf", {})

    for name in (
        "classic-ms-kern.ttf",
        "ms-classic-kern.ttf",
        "classic-apple-kern.ttf",
        "apple-classic-kern.ttf",
    ):
        save_sfnt(kern_dir / name, {"kern": GX_TABLES["kern"]})
    for name in (
        "malformed-classic-kern.ttf",
        "malformed-apple-classic-kern.ttf",
        "malformed-classic-kern-length.ttf",
        "malformed-classic-kern-offset.ttf",
        "malformed-classic-kern-pair-order.ttf",
    ):
        save_sfnt(kern_dir / name, {"kern": b"\0"})

    type1_dir.mkdir(parents=True, exist_ok=True)
    (type1_dir / "no-gx-service.pfa").write_bytes(TYPE1_BASE.read_bytes())


if __name__ == "__main__":
    main()
