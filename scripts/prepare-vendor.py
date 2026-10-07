#!/usr/bin/env python3
"""Fetch pinned upstream inputs at DEVELOPMENT time; never used by the app."""
import hashlib, pathlib, subprocess, tarfile, plistlib, shutil
root = pathlib.Path(__file__).resolve().parents[1]
assets = [
    ('macfuse-5.4.0.dmg', 'https://github.com/macfuse/macfuse/releases/download/macfuse-5.4.0/macfuse-5.4.0.dmg', '861814f0ac7fa8f6547ea40cdd49a36ac84bcc7d34f38a1fa74e8cf68b0401c5'),
    ('macfuse-library-7a6cdd2.tar.gz', 'https://github.com/macfuse/library/archive/7a6cdd2b6e11071a706f4295aa931c65223ba953.tar.gz', '3d6c946d3775a7915f2c974b75612dc509e9f094864b9c1d729afd85c73936c8'),
    ('ntfs-3g-2026.9.28.tar.gz', 'https://github.com/tuxera/ntfs-3g/archive/refs/tags/2026.9.28.tar.gz', '8a75293f07bf40df5701d296de32a4cb0e46d6ae135abb4f41b438df9868a390'),
]
downloads = root / 'vendor/downloads'
downloads.mkdir(parents=True, exist_ok=True)
for name, url, digest in assets:
    path = downloads / name
    if not path.exists():
        temporary = path.with_suffix(path.suffix+'.part')
        subprocess.run(['/usr/bin/curl', '--fail', '--location', '--proto', '=https', '--tlsv1.2', '--retry', '2', url, '-o', str(temporary)], check=True)
        if hashlib.sha256(temporary.read_bytes()).hexdigest() != digest:
            temporary.unlink(); raise SystemExit(f'Checksum mismatch: {name}')
        temporary.replace(path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise SystemExit(f'Checksum mismatch: {name}')
source = root / 'vendor/build/ntfs-3g-2026.9.28'
if not source.exists():
    source.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(downloads / 'ntfs-3g-2026.9.28.tar.gz') as archive:
        # The archive is SHA-pinned; reject unsafe paths on Python 3.9 too.
        for member in archive.getmembers():
            destination = (source.parent/member.name).resolve()
            if not destination.is_relative_to(source.parent.resolve()) or member.issym() or member.islnk():
                raise SystemExit('Unsafe source archive entry')
        archive.extractall(source.parent)
expanded = root / 'vendor/macfuse-expanded'
if not expanded.exists():
    result = subprocess.check_output(['/usr/bin/hdiutil', 'attach', '-nobrowse', '-readonly', '-plist', str(downloads/assets[0][0])])
    mounted = next(pathlib.Path(e['mount-point']) for e in plistlib.loads(result)['system-entities'] if 'mount-point' in e)
    try:
        package = mounted / 'Install macFUSE.pkg'
        signature = subprocess.check_output(['/usr/sbin/pkgutil','--check-signature',str(package)], text=True)
        if '3T5GSNBU6W' not in signature: raise SystemExit('Unexpected macFUSE installer signing identity')
        shutil.copy2(package, downloads/'Install macFUSE.pkg')
        subprocess.run(['/usr/sbin/pkgutil', '--expand', str(package), str(expanded)], check=True)
    finally:
        subprocess.run(['/usr/bin/hdiutil', 'detach', str(mounted)], check=True)
core_payload = root / 'vendor/core-payload'
library = core_payload / 'usr/local/lib/libfuse.2.dylib'
if not library.exists():
    core_payload.mkdir(parents=True, exist_ok=True)
    with subprocess.Popen(['/usr/bin/gzip', '-dc', str(expanded/'Core.pkg/Payload')], stdout=subprocess.PIPE) as unzip:
        subprocess.run(['/usr/bin/cpio', '-id', '--quiet', './usr/local/lib/libfuse.2.dylib'], cwd=core_payload, stdin=unzip.stdout, check=True)
        unzip.stdout.close()
        if unzip.wait() != 0: raise SystemExit('Cannot extract vendor library')
subprocess.run(['/usr/bin/codesign','--verify','--strict',str(library)], check=True)
print('Pinned driver inputs verified. Runtime packaging is fully offline.')
