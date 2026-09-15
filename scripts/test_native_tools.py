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
import build_unified_oracle
import test_c_consumer


class OptionalOracleBuildTests(unittest.TestCase):
    def test_bzip2_enabled_oracle_rejects_a_missing_bzip2_dependency(self):
        pinned_source = Path(__file__).resolve().parents[1] / "freetype"
        real_run = subprocess.run
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "freetype"
            source.mkdir()
            # Exercise the pinned upstream CMake dependency lookup, with an
            # unavailable package even on hosts whose SDK provides libbz2.
            (source / "CMakeLists.txt").write_text(
                "cmake_minimum_required(VERSION 3.16)\n"
                "project(fontdone_missing_bzip2 NONE)\n"
                "set(CMAKE_DISABLE_FIND_PACKAGE_BZip2 TRUE)\n"
                f'add_subdirectory("{pinned_source.as_posix()}" upstream)\n'
            )

            def configure_only(command, **kwargs):
                if command[:2] == ["cmake", "--build"]:
                    return subprocess.CompletedProcess(command, 0)
                return real_run(command, **kwargs, capture_output=True, text=True)

            with patch.object(build_unified_oracle.subprocess, "run", side_effect=configure_only):
                with self.assertRaises(subprocess.CalledProcessError) as failure:
                    build_unified_oracle.configure_bzip2_enabled_freetype(root)
            self.assertIn("BZip2", failure.exception.stderr)
            self.assertIn("REQUIRED", failure.exception.stderr)


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


class WindowsExportTests(unittest.TestCase):
    def test_dll_alias_annotations_preserve_the_exported_name(self):
        output = """ordinal hint RVA      name
          1    0 00001000 FT_Bitmap_Init
          2    1 00001000 FT_Bitmap_New = FT_Bitmap_Init
          3    2 00002000 FT_Get_X11_Font_Format = FT_Get_Font_Format
          4    3 00003000 FT_Undocumented_Endpoint = FT_Not_An_Export
          5    4 00004000 _ZN4core3ptr73drop_in_place$LT$fontdone..ffi..FT_Var_Axis$GT$17h0123456789abcdefE
"""
        with tempfile.TemporaryDirectory() as directory:
            release = Path(directory)
            (release / "fontdone_c_abi.dll").touch()
            with patch.object(check_c_exports.subprocess, "run", return_value=subprocess.CompletedProcess(
                ["dumpbin"], 0, stdout=output, stderr="",
            )):
                _, symbols = check_c_exports.binary_exports("shared", "Windows", release, "nm")
        self.assertEqual(symbols, {
            "FT_Bitmap_Init", "FT_Bitmap_New", "FT_Get_X11_Font_Format",
            "FT_Undocumented_Endpoint",
        })

    def test_static_symbols_do_not_promote_mangled_rust_type_names_to_c_exports(self):
        output = """Archive member name at 8: /
    5 public symbols
    00001000 FT_Init_FreeType
    00002000 _ZN4core3ptr80drop_in_place$LT$fontdone..ffi..FT_OutlineSnapshot$GT$17h0123456789abcdefE
    00003000 _ZN4core3ptr73drop_in_place$LT$fontdone..ffi..FT_Var_Axis$GT$17h0123456789abcdefE
    00004000 _ZN4core3ptr80drop_in_place$LT$fontdone..ffi..FT_Var_Named_Style$GT$17h0123456789abcdefE
    00005000 FT_Undocumented_Endpoint
"""
        with tempfile.TemporaryDirectory() as directory:
            release = Path(directory)
            (release / "fontdone_c_abi.lib").touch()
            with patch.object(check_c_exports.subprocess, "run", return_value=subprocess.CompletedProcess(
                ["dumpbin"], 0, stdout=output, stderr="",
            )):
                _, symbols = check_c_exports.binary_exports("static", "Windows", release, "nm")
        self.assertEqual(symbols, {"FT_Init_FreeType", "FT_Undocumented_Endpoint"})


class WindowsStaticLibraryProbeTests(unittest.TestCase):
    def test_linker_library_probe_preserves_the_measured_dll(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            consumer_library = root / "target" / "release" / "fontdone_c_abi.dll"
            consumer_library.parent.mkdir(parents=True)
            consumer_library.write_bytes(b"DLL measured by the C consumer")

            def cargo_rustc(command, **kwargs):
                build_target = (
                    Path(command[command.index("--target-dir") + 1])
                    if "--target-dir" in command else root / "target"
                )
                probe_library = build_target / "release" / "fontdone_c_abi.dll"
                probe_library.parent.mkdir(parents=True, exist_ok=True)
                probe_library.write_bytes(b"DLL relinked by the informational probe")
                return subprocess.CompletedProcess(
                    command, 0, stdout="", stderr="note: native-static-libs: kernel32.lib -ladvapi32\n",
                )

            with (
                patch.object(test_c_consumer, "ROOT", root),
                patch.object(test_c_consumer.subprocess, "run", side_effect=cargo_rustc),
            ):
                libraries = test_c_consumer.windows_native_static_libraries({})
            self.assertEqual(libraries, ["kernel32.lib", "advapi32.lib"])
            self.assertEqual(consumer_library.read_bytes(), b"DLL measured by the C consumer")


if __name__ == "__main__":
    unittest.main()
