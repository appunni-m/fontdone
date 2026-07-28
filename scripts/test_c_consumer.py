#!/usr/bin/env python3
"""Build and run maintained C consumers against shared and static artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shlex
import shutil
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "target" / "external-consumers" / "c"
LEDGER = ROOT / "target" / "api-abi-audit" / "c_consumer_ledger.json"
FONT = ROOT / "tests" / "fixtures" / "input" / "fonts" / "DejaVuSans.ttf"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_program(
    executable: Path,
    system: str,
    library_dir: Path,
    runner: list[str],
    arguments: list[str],
) -> str:
    environment = os.environ.copy()
    if system == "Windows":
        environment["PATH"] = (
            str(library_dir) + os.pathsep + environment.get("PATH", "")
        )
    else:
        loader_key = "DYLD_LIBRARY_PATH" if system == "Darwin" else "LD_LIBRARY_PATH"
        environment[loader_key] = str(library_dir)
    completed = subprocess.run(
        [*runner, str(executable), *arguments],
        cwd=ROOT,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def run_consumer(
    executable: Path,
    system: str,
    library_dir: Path,
    runner: list[str],
) -> str:
    return run_program(
        executable,
        system,
        library_dir,
        runner,
        [str(FONT)],
    )


def windows_native_static_libraries(environment: dict[str, str]) -> list[str]:
    completed = subprocess.run(
        [
            "cargo",
            "rustc",
            "-p",
            "fontdone-c-abi",
            "--release",
            "--locked",
            "--",
            "--print",
            "native-static-libs",
        ],
        cwd=ROOT,
        env=environment,
        check=True,
        capture_output=True,
        text=True,
    )
    match = re.search(
        r"native-static-libs:\s*(.+)",
        completed.stdout + completed.stderr,
    )
    if match is None:
        raise SystemExit("rustc did not report native static libraries")
    libraries = []
    for value in shlex.split(match.group(1)):
        if value.startswith("-l") and len(value) > 2:
            libraries.append(f"{value[2:]}.lib")
        elif value.lower().endswith(".lib"):
            libraries.append(value)
    return libraries


def system_for_target(target: str) -> str:
    if "windows" in target:
        return "Windows"
    if "apple-darwin" in target:
        return "Darwin"
    if "linux" in target:
        return "Linux"
    raise SystemExit(f"C consumer target is unsupported: {target}")


def data_model(pointer_bits: int, long_bits: int, int_bits: int) -> str:
    models = {
        (32, 32, 32): "ILP32",
        (64, 64, 32): "LP64",
        (64, 32, 32): "LLP64",
    }
    model = models.get((pointer_bits, long_bits, int_bits))
    if model is None:
        raise SystemExit(
            "unsupported C data model: "
            f"pointer={pointer_bits}, long={long_bits}, int={int_bits}"
        )
    return model


def platform_probe(
    out_dir: Path,
    compiler: str,
    system: str,
    runner: list[str],
    environment: dict[str, str],
) -> dict:
    source = out_dir / "platform_probe.c"
    executable = out_dir / (
        "platform_probe.exe" if system == "Windows" else "platform_probe"
    )
    source.write_text(
        "\n".join(
            (
                "#include <stdint.h>",
                "#include <stdio.h>",
                "int main(void) {",
                "  const uint32_t value = UINT32_C(0x01020304);",
                "  const unsigned char *bytes = (const unsigned char *)&value;",
                '  printf("pointer_bits=%zu long_bits=%zu int_bits=%zu '
                'endianness=%s\\n",',
                "         sizeof(void *) * 8, sizeof(long) * 8, sizeof(int) * 8,",
                '         bytes[0] == 1 ? "big" : "little");',
                "  return 0;",
                "}",
            )
        )
        + "\n",
        encoding="utf-8",
    )
    if system == "Windows":
        command = [
            compiler,
            "/nologo",
            "/std:c11",
            "/W4",
            "/WX",
            str(source),
            f"/Fe:{executable}",
        ]
    else:
        command = [
            compiler,
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            str(source),
            "-o",
            str(executable),
        ]
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    output = run_program(executable, system, out_dir, runner, [])
    match = re.fullmatch(
        r"pointer_bits=(\d+) long_bits=(\d+) int_bits=(\d+) "
        r"endianness=(little|big)",
        output,
    )
    if match is None:
        raise SystemExit(f"invalid target platform probe output: {output!r}")
    pointer_bits, long_bits, int_bits = (
        int(match.group(index)) for index in range(1, 4)
    )
    return {
        "pointer_bits": pointer_bits,
        "long_bits": long_bits,
        "int_bits": int_bits,
        "data_model": data_model(pointer_bits, long_bits, int_bits),
        "endianness": match.group(4),
        "probe_output": output,
    }


def install_tree(
    stage: Path,
    system: str,
    dynamic_library: Path,
    static_library: Path,
    import_library: Path | None,
) -> tuple[Path, Path, Path, Path, list[str]]:
    installed_root = stage / "usr"
    installed_lib = installed_root / "lib"
    installed_include = installed_root / "include" / "fontdone2"
    installed_pkgconfig = installed_lib / "pkgconfig"
    installed_lib.mkdir(parents=True)
    installed_include.mkdir(parents=True)
    installed_pkgconfig.mkdir(parents=True)
    shutil.copy2(dynamic_library, installed_lib / dynamic_library.name)
    shutil.copy2(static_library, installed_lib / static_library.name)
    if import_library is not None:
        shutil.copy2(import_library, installed_lib / import_library.name)
    source_include = ROOT / "fontdone-c-abi" / "include"
    for source in source_include.rglob("*.h"):
        target = installed_include / source.relative_to(source_include)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, target)
    shutil.copy2(
        ROOT / "fontdone-c-abi" / "fontdone2.pc",
        installed_pkgconfig / "fontdone2.pc",
    )
    expected_headers = sorted(
        path.relative_to(source_include).as_posix()
        for path in source_include.rglob("*.h")
    )
    installed_headers = sorted(
        path.relative_to(installed_include).as_posix()
        for path in installed_include.rglob("*.h")
    )
    if system == "Windows" and import_library is None:
        raise SystemExit("Windows install tree lacks the DLL import library")
    return (
        installed_root,
        installed_lib,
        installed_include,
        installed_pkgconfig / "fontdone2.pc",
        installed_headers,
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target")
    parser.add_argument("--cc")
    parser.add_argument(
        "--runner",
        default="",
        help="Command prefix used to execute target binaries, for example qemu-ppc64 -L SYSROOT",
    )
    args = parser.parse_args()

    native_target = subprocess.run(
        ["rustc", "-vV"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    native_target = next(
        (
            line.split(":", 1)[1].strip()
            for line in native_target.splitlines()
            if line.startswith("host:")
        ),
        "unknown",
    )
    target = args.target or native_target
    system = system_for_target(target)
    if system not in {"Linux", "Darwin", "Windows"}:
        raise SystemExit(
            f"native C consumer is not claimed on {system}; "
            "use Linux, macOS, or Windows"
        )
    if args.target and target != native_target and system != "Linux":
        raise SystemExit("cross C-consumer execution currently supports Linux targets")
    runner = shlex.split(args.runner)
    if target != native_target and not runner:
        raise SystemExit("a target runner is required for a cross C consumer")
    compiler = (
        args.cc
        or os.environ.get("CC")
        or ("cl" if system == "Windows" else "cc")
    )
    if shutil.which(compiler) is None:
        raise SystemExit(f"C compiler not found: {compiler}")

    environment = os.environ.copy()
    if target != native_target:
        linker_key = (
            "CARGO_TARGET_"
            + re.sub(r"[^A-Za-z0-9]", "_", target).upper()
            + "_LINKER"
        )
        environment.setdefault(linker_key, compiler)
    cargo_command = [
        "cargo",
        "build",
        "-p",
        "fontdone-c-abi",
        "--release",
        "--locked",
    ]
    if args.target:
        cargo_command.extend(("--target", target))
    subprocess.run(
        cargo_command,
        cwd=ROOT,
        env=environment,
        check=True,
    )
    out_dir = OUT_DIR / target
    out_dir.mkdir(parents=True, exist_ok=True)
    release = (
        ROOT / "target" / target / "release"
        if args.target
        else ROOT / "target" / "release"
    )
    if system == "Windows":
        dynamic_library = release / "fontdone_c_abi.dll"
        static_library = release / "fontdone_c_abi.lib"
        import_library = release / "fontdone_c_abi.dll.lib"
    else:
        dynamic_library = release / (
            "libfontdone_c_abi.dylib"
            if system == "Darwin"
            else "libfontdone_c_abi.so"
        )
        static_library = release / "libfontdone_c_abi.a"
        import_library = None
    required_libraries = [dynamic_library, static_library]
    if import_library is not None:
        required_libraries.append(import_library)
    for library in required_libraries:
        if not library.is_file():
            raise SystemExit(f"native C artifact is missing: {library}")

    executable_suffix = ".exe" if system == "Windows" else ""
    dynamic_executable = out_dir / f"render_glyph_dynamic{executable_suffix}"
    source = ROOT / "fontdone-c-abi" / "examples" / "render_glyph.c"
    include = ROOT / "fontdone-c-abi" / "include"
    if system == "Windows":
        common = [
            compiler,
            "/nologo",
            "/std:c11",
            "/W4",
            "/WX",
            f"/I{include}",
            str(source),
        ]
        dynamic_command = [
            *common,
            f"/Fe:{dynamic_executable}",
            "/link",
            f"/LIBPATH:{release}",
            import_library.name,
        ]
    else:
        relative_release = os.path.relpath(release, out_dir)
        loader_origin = "@loader_path" if system == "Darwin" else "$ORIGIN"
        rpath = f"{loader_origin}/{relative_release}"
        common = [
            compiler,
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            f"-I{include}",
            str(source),
        ]
        dynamic_command = [
            *common,
            f"-L{release}",
            "-lfontdone_c_abi",
            f"-Wl,-rpath,{rpath}",
            "-o",
            str(dynamic_executable),
        ]
    subprocess.run(
        dynamic_command,
        cwd=ROOT,
        env=environment,
        check=True,
    )

    static_executable = out_dir / f"render_glyph_static{executable_suffix}"
    if system == "Windows":
        static_command = [
            *common,
            str(static_library),
            *windows_native_static_libraries(environment),
            f"/Fe:{static_executable}",
        ]
    else:
        static_system_libraries = (
            [] if system == "Darwin" else ["-lutil", "-lrt", "-lpthread", "-lm", "-ldl"]
        )
        static_command = [
            *common,
            str(static_library),
            *static_system_libraries,
            "-o",
            str(static_executable),
        ]
    subprocess.run(
        static_command,
        cwd=ROOT,
        env=environment,
        check=True,
    )

    dynamic_output = run_consumer(
        dynamic_executable,
        system,
        release,
        runner,
    )
    static_output = run_consumer(
        static_executable,
        system,
        release,
        runner,
    )
    if dynamic_output != static_output:
        raise SystemExit(
            "shared/static C consumer output mismatch:\n"
            f"shared: {dynamic_output}\nstatic: {static_output}"
        )

    abi = platform_probe(
        out_dir,
        compiler,
        system,
        runner,
        environment,
    )
    with tempfile.TemporaryDirectory(
        prefix="fontdone-c-install-", dir=ROOT / "target"
    ) as temporary:
        stage = Path(temporary)
        (
            installed_root,
            installed_lib,
            installed_include,
            installed_pc,
            installed_headers,
        ) = install_tree(
            stage,
            system,
            dynamic_library,
            static_library,
            import_library,
        )
        installed_dynamic = installed_lib / dynamic_library.name
        installed_static = installed_lib / static_library.name
        expected_headers = sorted(
            path.relative_to(ROOT / "fontdone-c-abi" / "include").as_posix()
            for path in (ROOT / "fontdone-c-abi" / "include").rglob("*.h")
        )
        if installed_headers != expected_headers:
            raise SystemExit("installed C header tree differs from the package header tree")
        if sha256(installed_dynamic) != sha256(dynamic_library):
            raise SystemExit("installed shared library differs from the release artifact")
        if sha256(installed_static) != sha256(static_library):
            raise SystemExit("installed static library differs from the release artifact")
        installed_import = (
            installed_lib / import_library.name
            if import_library is not None
            else None
        )
        if (
            installed_import is not None
            and sha256(installed_import) != sha256(import_library)
        ):
            raise SystemExit(
                "installed Windows import library differs from the release artifact"
            )
        pc_text = installed_pc.read_text(encoding="utf-8")
        required_pc_rows = (
            "Name: fontdone",
            "Version: 2.14.3-alpha.1",
            "Libs: -L${libdir} -lfontdone_c_abi",
            "Cflags: -I${includedir}",
        )
        if any(row not in pc_text for row in required_pc_rows):
            raise SystemExit("installed fontdone2.pc lacks required metadata")

        installed_executable = out_dir / f"render_glyph_installed{executable_suffix}"
        if system == "Windows":
            installed_command = [
                compiler,
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                f"/I{installed_include}",
                str(source),
                f"/Fe:{installed_executable}",
                "/link",
                f"/LIBPATH:{installed_lib}",
                import_library.name,
            ]
        else:
            installed_command = [
                compiler,
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                f"-I{installed_include}",
                str(source),
                f"-L{installed_lib}",
                "-lfontdone_c_abi",
                f"-Wl,-rpath,{installed_lib}",
                "-o",
                str(installed_executable),
            ]
        subprocess.run(
            installed_command,
            cwd=ROOT,
            env=environment,
            check=True,
        )
        installed_output = run_consumer(
            installed_executable,
            system,
            installed_lib,
            runner,
        )
        if installed_output != dynamic_output:
            raise SystemExit("installed C consumer output differs from the build-tree output")
        installation = {
            "status": "exact",
            "prefix": "/usr",
            "header_count": len(installed_headers),
            "shared_sha256": sha256(installed_dynamic),
            "static_sha256": sha256(installed_static),
            "pkg_config": "lib/pkgconfig/fontdone2.pc",
            "consumer_output": installed_output,
            "import_library": (
                import_library.name if import_library is not None else None
            ),
            "import_library_sha256": (
                sha256(installed_import) if installed_import is not None else None
            ),
        }

    artifacts = {
        "shared": {
            "path": str(dynamic_library.relative_to(ROOT)),
            "sha256": sha256(dynamic_library),
            "consumer": str(dynamic_executable.relative_to(ROOT)),
            "output": dynamic_output,
            "status": "exact",
        },
        "static": {
            "path": str(static_library.relative_to(ROOT)),
            "sha256": sha256(static_library),
            "consumer": str(static_executable.relative_to(ROOT)),
            "output": static_output,
            "status": "exact",
        },
    }
    if import_library is not None:
        artifacts["import"] = {
            "path": str(import_library.relative_to(ROOT)),
            "sha256": sha256(import_library),
            "consumer": str(dynamic_executable.relative_to(ROOT)),
            "output": dynamic_output,
            "status": "exact",
        }

    ledger = {
        "schema_version": 1,
        "measurement": (
            "The maintained external C source is compiled, linked, and run "
            "separately against the release shared and static artifacts."
        ),
        "platform": {
            "system": system,
            "machine": (
                platform.machine() if target == native_target else target.split("-", 1)[0]
            ),
            "rust_host": target,
            **abi,
            "execution_runner": runner,
        },
        "compiler": compiler,
        "font": str(FONT.relative_to(ROOT)),
        "font_sha256": sha256(FONT),
        "artifacts": artifacts,
        "installation": installation,
    }
    LEDGER.parent.mkdir(parents=True, exist_ok=True)
    LEDGER.write_text(json.dumps(ledger, indent=2) + "\n", encoding="utf-8")
    print(dynamic_output)
    print(f"C consumers: shared and static artifacts compiled and ran on {system}")


if __name__ == "__main__":
    main()
