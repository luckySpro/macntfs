#!/usr/bin/env python3
"""Seal corresponding sources, package recipes and binary provenance for release."""
import hashlib
import json
import pathlib
import subprocess
import tarfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
CACHE = ROOT / 'vendor/source-cache'
PINS = {
    'anylinuxfs-8aa9ccd.tar.gz': '27ee90647b6da372a0d052282cb6322c1db73302c4d1d1abb57abcc21f652a61',
    'ntfsmac-65ea3ca.tar.gz': 'ec784070317895d30e48fc15757ffd071a75c929b3472882edd67b5528e7eee3',
    'libkrunfw-9fe60c6.tar.gz': '00c14606a5ce4513d434a87fe4a927a599afb11a6ad4465f801de681224846aa',
    'linux-6.12.62.tar.xz': '13e2c685ac8fab5dd992dd105732554dae514aef350c2a8c7418e7b74eb62c13',
    'util-linux-2.42.4.tar.xz': 'fbd62a100ab7bb8746ba0661255c3c48185b1e9021507c624da01fbc696330ec',
    'gettext-1.0.tar.gz': '85d99b79c981a404874c02e0342176cf75c7698e2b51fe41031cf6526d974f1a',
    'gvisor-tap-vsock-9cfc86f.tar.gz': 'c174f2e14868999d81c9ac0b50d8e6c94666091f393dd5f0e14e66aa0392bf07',
}
URLS = {
    'anylinuxfs-8aa9ccd.tar.gz': 'https://codeload.github.com/nohajc/anylinuxfs/tar.gz/8aa9ccd6504e64ca26ce769c1623ed1741c6b7d3',
    'ntfsmac-65ea3ca.tar.gz': 'https://codeload.github.com/khr898/ntfsmac/tar.gz/65ea3caabda07f12c1eeb3e06af927c8a4a3585a',
    'libkrunfw-9fe60c6.tar.gz': 'https://codeload.github.com/nohajc/libkrunfw/tar.gz/9fe60c621c3dce85680274262c1be90046dbd6fc',
    'linux-6.12.62.tar.xz': 'https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.12.62.tar.xz',
    'util-linux-2.42.4.tar.xz': 'https://mirrors.edge.kernel.org/pub/linux/utils/util-linux/v2.42/util-linux-2.42.4.tar.xz',
    'gettext-1.0.tar.gz': 'https://ftp.gnu.org/gnu/gettext/gettext-1.0.tar.gz',
    'gvisor-tap-vsock-9cfc86f.tar.gz': 'https://codeload.github.com/containers/gvisor-tap-vsock/tar.gz/9cfc86f66679ef0feed0f20ba1df558fe2bef5c6',
}


def build():
    # Load sibling with a hyphenated CLI filename without executing its main.
    import importlib.util
    spec = importlib.util.spec_from_file_location('collector', ROOT / 'scripts/collect-microvm-sources.py')
    collector = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(collector)
    collector.main()
    for name, digest in PINS.items():
        p = ROOT / 'vendor/downloads' / name
        if not p.exists():
            collector.download(URLS[name], p)
        if hashlib.sha256(p.read_bytes()).hexdigest() != digest:
            raise SystemExit('Pinned host source mismatch: ' + name)
    crates = CACHE / 'crates'
    if not any((crates / name).exists() for name in ['libkrun', 'libkrun-1.19.3']):
        src = pathlib.Path('/tmp/macntfs-offline-vm-build/anylinuxfs-8aa9ccd6504e64ca26ce769c1623ed1741c6b7d3')
        config = subprocess.check_output(['cargo', 'vendor', '--locked', '--manifest-path',
            str(src / 'anylinuxfs/Cargo.toml'), '--sync', str(src / 'vmproxy/Cargo.toml'), str(crates)], text=True)
        (CACHE / 'cargo-config.toml').write_text(config)
    for package in crates.iterdir():
        sums = json.loads((package / '.cargo-checksum.json').read_text())
        for name, digest in sums['files'].items():
            p = package / name
            if not p.resolve().is_relative_to(package.resolve()) or hashlib.sha256(p.read_bytes()).hexdigest() != digest:
                raise SystemExit('Vendored dependency changed: ' + str(p))
    version = (ROOT / 'VERSION').read_text().strip()
    manifest = {'version': version, 'source_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                'host_sources': PINS, 'alpine_origins': json.loads((CACHE / 'alpine/MANIFEST.json').read_text()),
                'rust_dependency_count': len(list(crates.iterdir())), 'runtime_binaries': {}}
    manifest['installer_sha256'] = hashlib.sha256((ROOT / f'dist/macntfs-{version}-arm64.pkg').read_bytes()).hexdigest()
    for rel in ['bin/anylinuxfs', 'libexec/gvproxy', 'libexec/vmproxy', 'libexec/Image', 'libexec/Image-4K']:
        manifest['runtime_binaries'][rel] = hashlib.sha256((ROOT / 'vendor/microvm' / rel).read_bytes()).hexdigest()
    (CACHE / 'PROVENANCE.json').write_text(json.dumps(manifest, indent=2) + '\n')
    formulas = CACHE / 'homebrew'
    formulas.mkdir(exist_ok=True)
    for name, ver in [('util-linux', '2.42.4'), ('gettext', '1.0')]:
        prefix = pathlib.Path(subprocess.check_output(['brew', '--prefix', name], text=True).strip()).resolve()
        if prefix.name != ver:
            raise SystemExit('Unexpected static library version: ' + name)
        for p in [prefix / '.brew' / (name + '.rb'), prefix / 'INSTALL_RECEIPT.json']:
            (formulas / (name + '-' + p.name)).write_bytes(p.read_bytes())
    (CACHE / 'README.md').write_text('''# macntfs MicroVM corresponding sources

Unpack archives/anylinuxfs for the full host and guest agent sources. The
macntfs-source.tar.gz scripts/build-microvm.py applies our host changes and
documents static linking. Its original source, crates and Cargo.lock accompany
this archive. Configure cargo with cargo-config.toml, adjusting the vendor path
to this archive's crates directory. vmproxy is built with the upstream ntfsmac
build/build-all.sh patches; its build scripts and guest provisioning scripts
are in archives/ntfsmac. gvproxy sources are in archives/gvisor-tap-vsock.

Linux 6.12.62 and the libkrunfw configuration, patches and kernel build scripts
are in archives/linux and archives/libkrunfw. Homebrew formulae and receipts
record host static library builds; matching util-linux and gettext archives
are included. For each installed Alpine package, alpine/<origin>-<commit>
contains its exact APKBUILD, local patch/install files and all checksum-listed
source inputs. Build using Alpine abuild for aarch64 at these recorded versions.
The APK database records installed subpackages; PROVENANCE.json indexes origins
and binaries. Original licenses remain in their source trees and archives.

This is a source-availability archive, not a byte-for-byte reproducibility claim.
Build toolchains and ordinary system libraries may be obtained separately.
''')
    output = ROOT / f'dist/macntfs-{version}-microvm-sources.tar.gz'
    own = CACHE / 'macntfs-source.tar.gz'
    subprocess.run(['git', 'archive', '--format=tar.gz', '-o', str(own), 'HEAD'], cwd=ROOT, check=True)
    with tarfile.open(output, 'w:gz') as archive:
        for name in PINS:
            archive.add(ROOT / 'vendor/downloads' / name, arcname='archives/' + name)
        for name in ['alpine', 'crates', 'homebrew', 'cargo-config.toml', 'PROVENANCE.json', 'README.md', 'macntfs-source.tar.gz']:
            archive.add(CACHE / name, arcname=name)
        archive.add(ROOT / 'vendor/microvm/profile/alpine/rootfs/lib/apk/db/installed', arcname='installed-apk-database')
    print('PASS: source archive sealed:', output)


if __name__ == '__main__':
    build()
