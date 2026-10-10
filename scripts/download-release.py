#!/usr/bin/env python3
"""Download a specific release ID and verify GitHub and published SHA-256 values."""
import hashlib
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[1]
release_id = sys.argv[1]
assert release_id.isdecimal()
repo = 'repos/luckySpro/macntfs'
release = json.loads(subprocess.check_output(['gh', 'api', f'{repo}/releases/{release_id}']))
version = (ROOT / 'VERSION').read_text().strip()
assert release['tag_name'] == 'v' + version
for asset in release['assets']:
    name = asset['name']
    assert pathlib.Path(name).name == name
    path = ROOT / 'dist' / name
    with path.open('wb') as f:
        subprocess.run(['gh', 'api', '-H', 'Accept: application/octet-stream',
                        f'{repo}/releases/assets/{asset["id"]}'], stdout=f, check=True)
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    assert asset['size'] == path.stat().st_size
    assert asset['digest'] == 'sha256:' + digest
for line in (ROOT / 'dist/SHA256SUMS').read_text().splitlines():
    digest, name = line.split('  ', 1)
    assert pathlib.Path(name).name == name
    assert hashlib.sha256((ROOT / 'dist' / name).read_bytes()).hexdigest() == digest
print('PASS: downloaded release assets match GitHub digests and published SHA256SUMS')
