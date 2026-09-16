#!/usr/bin/env python3
"""Execute public quickstarts against packages installed from their registries."""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import platform
import tarfile
import urllib.request
import os
import re
from pathlib import Path
import subprocess
import sys
import tempfile

from check_docs_examples import fenced_examples
from docs_release import check_documents, published

ROOT = Path(__file__).resolve().parent.parent


def run(argv: list[str], cwd: Path, env: dict | None = None) -> None:
    subprocess.run(argv, cwd=cwd, env=env, check=True)


def sdk_example(config: dict) -> None:
    if (platform.system(), platform.machine()) != ('Darwin', 'arm64'):
        raise ValueError('The published C SDK example requires macOS ARM64')
    version = published(config)['version']
    asset = f'fontdone-c-abi-{version}-aarch64-apple-darwin.tar.gz'
    base = f'https://github.com/{config["repository"]}/releases/download/v{version}/'
    with urllib.request.urlopen(base + 'SHA256SUMS', timeout=30) as response:
        manifest = response.read().decode()
    expected = [line.split()[0] for line in manifest.splitlines()
                if len(line.split()) == 2 and line.split()[1].removeprefix('./') == asset]
    if len(expected) != 1 or not re.fullmatch('[0-9a-f]{64}', expected[0]):
        raise ValueError('SDK checksum is missing or ambiguous')
    with urllib.request.urlopen(base + asset, timeout=30) as response:
        data = response.read()
    if hashlib.sha256(data).hexdigest() != expected[0]:
        raise ValueError('published SDK checksum mismatch')
    with tempfile.TemporaryDirectory(prefix='docs-sdk-') as directory:
        work = Path(directory)
        with tarfile.open(fileobj=io.BytesIO(data), mode='r:gz') as archive:
            for member in archive.getmembers():
                # Copy regular files only, stripping the archive's fixed root.
                path = Path(member.name)
                if path.is_absolute() or '..' in path.parts or path.parts[0] != asset.removesuffix('.tar.gz'):
                    raise ValueError('unexpected SDK member path')
                if not member.isfile():
                    if member.isdir(): continue
                    raise ValueError('SDK links are not permitted')
                target = work / 'fontdone-sdk' / Path(*path.parts[1:])
                target.parent.mkdir(parents=True, exist_ok=True)
                with archive.extractfile(member) as source:
                    target.write_bytes(source.read())
        code = fenced_examples(ROOT / 'fontdone-c-abi/README.md', 'c')[0]
        (work / 'example.c').write_text(code)
        run(['cc', 'example.c', '-Ifontdone-sdk/include', '-Lfontdone-sdk/lib', '-lfontdone_c_abi',
             '-Wl,-rpath,@loader_path/fontdone-sdk/lib', '-o', 'example'], work)
        run(['./example'], work)
    print(f'Published C SDK quickstart passed: fontdone {version}')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sdk-only', action='store_true')
    args = parser.parse_args()
    config = json.loads((ROOT / 'documentation.json').read_text())
    check_documents(ROOT, config)
    if args.sdk_only:
        sdk_example(config)
        return
    version = published(config)['version']
    name = config['project']
    with tempfile.TemporaryDirectory(prefix='docs-consumer-') as directory:
        work = Path(directory)
        rust = config['rust_example']
        code = fenced_examples(ROOT / rust['source'], 'rust')[rust.get('index', 0)]
        if rust.get('font_input'):
            font = (ROOT / rust['font_input']).resolve()
            code += f'\nfn main() {{ assert!(!render_a(include_bytes!({json.dumps(str(font))})).unwrap().is_empty()); }}\n'
        (work / 'src').mkdir()
        (work / 'src/main.rs').write_text(code)
        (work / 'Cargo.toml').write_text(
            '[package]\nname = "public-docs-consumer"\nversion = "0.0.0"\nedition = "2024"\n'
            f'[dependencies]\n{name} = "={version}"\n')
        toolchain = re.search(r'(?m)^channel\s*=\s*"([^"]+)"', (ROOT / 'rust-toolchain.toml').read_text())[1]
        env = dict(os.environ, RUSTC_WRAPPER='', RUSTUP_TOOLCHAIN=toolchain,
                   CARGO_TARGET_DIR=str(ROOT / 'target/docs-registry-cargo'))
        run(['cargo', 'generate-lockfile'], work, env)
        run(['cargo', 'run', '--locked'], work, env)
        if 'pypi' in config['registries']:
            run([sys.executable, '-m', 'venv', str(work / 'venv')], work)
            python = work / 'venv' / ('Scripts/python.exe' if os.name == 'nt' else 'bin/python')
            run([str(python), '-m', 'pip', 'install', '--only-binary=:all:', f'{name}=={version}'], work)
            for source in config['python_examples']:
                for code in fenced_examples(ROOT / source, 'python'):
                    run([str(python), '-c', code], work)
        if 'npm' in config['registries']:
            run(['npm', 'install', '--ignore-scripts', '--no-audit', '--no-fund', '--save-exact', f'{name}@{version}'], work)
            installed = json.loads((work / 'node_modules' / name / 'package.json').read_text())
            if installed['version'] != version:
                raise ValueError('npm installed a different package version')
            example = config['javascript_example']
            code = fenced_examples(ROOT / example['source'], example['language'])[example.get('index', 0)]
            (work / 'quickstart.mjs').write_text(code)
            argv = ['node', 'quickstart.mjs']
            if rust.get('font_input'):
                argv.append(str((ROOT / rust['font_input']).resolve()))
            run(argv, work)
    print(f'Published-package quickstarts passed: {name} {version} ({", ".join(config["registries"])})')


if __name__ == '__main__':
    main()
