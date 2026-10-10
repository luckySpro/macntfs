#!/usr/bin/env python3
"""Verify the sealed source archive without extracting or executing its contents."""
import hashlib
import json
import pathlib
import subprocess
import tarfile

ROOT = pathlib.Path(__file__).resolve().parents[1]


def main():
    version = (ROOT / 'VERSION').read_text().strip()
    path = ROOT / f'dist/macntfs-{version}-microvm-sources.tar.gz'
    with tarfile.open(path) as archive:
        members = archive.getmembers()
        names = [m.name for m in members]
        if len(names) != len(set(names)):
            raise SystemExit('Duplicate archive paths')
        for m in members:
            p = pathlib.PurePosixPath(m.name)
            if p.is_absolute() or '..' in p.parts or m.isdev():
                raise SystemExit('Unsafe archive path: ' + m.name)
            if m.issym() or m.islnk():
                raise SystemExit('Unexpected source symlink: ' + m.name)
            if p.name.endswith('.key'):
                raise SystemExit('Unexpected key in source archive')
        def read(name):
            return archive.extractfile(name).read()
        def sha(name, algorithm='sha256'):
            h = hashlib.new(algorithm)
            with archive.extractfile(name) as f:
                for chunk in iter(lambda: f.read(1024 * 1024), b''):
                    h.update(chunk)
            return h.hexdigest()
        provenance = json.loads(read('PROVENANCE.json'))
        assert provenance['version'] == version
        pkg = ROOT / f'dist/macntfs-{version}-arm64.pkg'
        assert hashlib.sha256(pkg.read_bytes()).hexdigest() == provenance['installer_sha256']
        expected = set()
        for block in read('installed-apk-database').decode().split('\n\n'):
            d = dict(line.split(':', 1) for line in block.splitlines()
                     if len(line) > 2 and line[1] == ':' and line[0] in 'PVoc')
            if 'P' in d:
                expected.add((d.get('o', d['P']), d['c'], d['P'] + '=' + d['V']))
        actual = set()
        for origin in provenance['alpine_origins']:
            base = 'alpine/' + origin['origin'] + '-' + origin['aports_commit'] + '/'
            recipe = read(base + 'APKBUILD').decode()
            assert recipe
            for package in origin['packages']:
                actual.add((origin['origin'], origin['aports_commit'], package))
            for item in origin['inputs']:
                assert sha(base + item['name'], 'sha512') == item['sha512']
        assert actual == expected
        for name, digest in provenance['host_sources'].items():
            assert sha('archives/' + name) == digest
        crate_sums = [n for n in names if n.startswith('crates/') and n.endswith('/.cargo-checksum.json')]
        assert len(crate_sums) == provenance['rust_dependency_count']
        for name in crate_sums:
            base = name.removesuffix('.cargo-checksum.json')
            for file, digest in json.loads(read(name))['files'].items():
                assert sha(base + file) == digest
        with tarfile.open(fileobj=__import__('io').BytesIO(read('macntfs-source.tar.gz'))) as own:
            assert own.extractfile('scripts/build-microvm.py')
            assert own.extractfile('VERSION').read().decode().strip() == version
        print(f'PASS: sealed source archive; {len(provenance["alpine_origins"])} Alpine origins, {len(crate_sums)} Rust dependencies, pinned host sources and installer binding')


if __name__ == '__main__':
    main()
