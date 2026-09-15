"""Regression checks for native artifact build orchestration."""

from contextlib import redirect_stdout
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_c_exports


class CrossExportBuildTests(unittest.TestCase):
    def test_export_inspection_builds_with_the_selected_cross_linker(self):
        for target, linker in (
            ("i686-unknown-linux-gnu", "i686-linux-gnu-gcc"),
            ("powerpc64-unknown-linux-gnu", "powerpc64-linux-gnu-gcc"),
        ):
            with self.subTest(target=target), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                library = root / "library"
                library.write_bytes(b"inspected native archive")
                with (
                    patch.object(check_c_exports, "ROOT", root),
                    patch.object(check_c_exports, "LEDGER", root / "ledger.json"),
                    patch.object(check_c_exports, "header_functions", return_value={"FT_Init_FreeType"}),
                    patch.object(check_c_exports, "binary_exports", return_value=(library, {"FT_Init_FreeType"})),
                    patch.object(check_c_exports.shutil, "which", return_value="/usr/bin/nm"),
                    patch.object(check_c_exports.subprocess, "run", side_effect=[
                        subprocess.CompletedProcess(["rustc"], 0, stdout="host: x86_64-unknown-linux-gnu\n"),
                        subprocess.CompletedProcess(["cargo"], 0),
                    ]) as run,
                    patch("sys.argv", ["check_c_exports.py", "--target", target, "--linker", linker]),
                    patch.dict(os.environ, {"CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER": "host-cc"}),
                    redirect_stdout(io.StringIO()),
                ):
                    check_c_exports.main()
                cargo = run.call_args_list[1]
                self.assertEqual(cargo.args[0][-2:], ["--target", target])
                self.assertEqual(cargo.kwargs["env"]["CARGO_TARGET_" + target.upper().replace("-", "_") + "_LINKER"], linker)
                self.assertEqual(cargo.kwargs["env"]["CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"], "host-cc")


if __name__ == "__main__":
    unittest.main()
