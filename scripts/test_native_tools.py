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
import audit_api_abi


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


class SourceByteIdentityTests(unittest.TestCase):
    def test_windows_checkout_preserves_source_and_font_bytes(self):
        root = Path(__file__).resolve().parents[1]
        for path in (
            "Makefile", "Cargo.toml", "src/ffi/handles.rs",
            "scripts/fetch_ft.sh", "tests/fixtures/input/fonts/DejaVuSans.ttf",
        ):
            with self.subTest(path=path):
                committed = subprocess.check_output(["git", "show", f"HEAD:{path}"], cwd=root)
                checkout = subprocess.check_output([
                    "git", "-c", "core.autocrlf=true", "cat-file", "--filters", f"HEAD:{path}",
                ], cwd=root)
                self.assertEqual(committed, checkout, f"checkout changes measured bytes: {path}")


class ClangHeaderTests(unittest.TestCase):
    def test_windows_header_paths_obey_the_portable_exclusions(self):
        inventory = {bucket: {} for bucket in (
            "functions", "macros", "typedefs", "callbacks", "structs",
            "enums", "enum_variants", "error_codes",
        )}
        inventory["functions"] = {
            "portable": {"file": r"freetype\freetype.h"},
            "mac_only": {"file": r"freetype\ftmac.h"},
            "reinclude_only": {"file": r"freetype\fterrdef.h"},
        }
        with patch.object(audit_api_abi.shutil, "which", return_value="clang"):
            command = audit_api_abi.clang_base_command(inventory, local=False)
        includes = [command[index + 1] for index, value in enumerate(command) if value == "-include"]
        self.assertEqual(includes, ["ft2build.h", "freetype/freetype.h"])
        self.assertIn("-Werror", command)

    def test_ast_failure_reports_the_compiler_error(self):
        with (
            patch.object(audit_api_abi, "clang_base_command", return_value=["clang", "-Werror"]),
            patch.object(audit_api_abi.subprocess, "run", side_effect=subprocess.CalledProcessError(
                1, ["clang"], stderr="fatal error: missing native SDK header",
            )),
        ):
            with self.assertRaisesRegex(SystemExit, "missing native SDK header"):
                audit_api_abi.clang_ast({}, local=False)


if __name__ == "__main__":
    unittest.main()
