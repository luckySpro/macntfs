#!/usr/bin/env python3
"""Collect pinned recipe files and checksum-verified Alpine source inputs.

Never execute downloaded APKBUILD files. The installed APK database supplies
the exact aports commit; its sha512sums supplies the required source inputs.
"""
import concurrent.futures
import hashlib
import json
import pathlib
import re
import subprocess
import urllib.parse

ROOT = pathlib.Path(__file__).resolve().parents[1]
OUT = ROOT / 'vendor/source-cache/alpine'
DB = ROOT / 'vendor/microvm/profile/alpine/rootfs/lib/apk/db/installed'


def download(url, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + '.partial')
    result = subprocess.run(['curl', '-fLsS', '--retry', '2', '--retry-all-errors', '--connect-timeout', '20',
                             '--max-time', '300', '--proto', '=https', url, '-o', str(tmp)],
                            capture_output=True)
    if result.returncode:
        tmp.unlink(missing_ok=True)
        raise RuntimeError(url + ': ' + result.stderr.decode()[-200:])
    tmp.replace(path)


def collect(item):
    (origin, commit), packages = item
    if not re.fullmatch(r'[a-zA-Z0-9+_.-]+', origin) or not re.fullmatch(r'[a-f0-9]{40}', commit):
        raise RuntimeError('Unpinned package: ' + origin)
    directory = OUT / (origin + '-' + commit)
    directory.mkdir(parents=True, exist_ok=True)
    listing = directory / 'recipe-files.json'
    if listing.exists():
        entries = json.loads(listing.read_text())
    else:
        entries = None
        for section in ['main', 'community']:
            result = subprocess.run(['gh', 'api',
                f'repos/alpinelinux/aports/contents/{section}/{origin}?ref={commit}'],
                capture_output=True)
            if result.returncode == 0:
                entries = json.loads(result.stdout)
                listing.write_text(json.dumps(entries, indent=2) + '\n')
                break
        if not isinstance(entries, list):
            raise RuntimeError('No exact recipe: ' + origin)
    for entry in entries:
        if entry['type'] != 'file':
            raise RuntimeError('Unexpected nested recipe: ' + origin)
        name = entry['name']
        if pathlib.Path(name).name != name:
            raise RuntimeError('Unsafe recipe name')
        target = directory / name
        if not target.exists():
            download(entry['download_url'], target)
        raw = target.read_bytes()
        git_hash = hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()
        if git_hash != entry['sha']:
            raise RuntimeError('Recipe blob mismatch: ' + origin + '/' + name)
    recipe = (directory / 'APKBUILD').read_text()
    sums = re.findall(r'^(?:sha512sums=")?([a-f0-9]{128})\s+([^\s"]+)"?\s*$', recipe, re.M)
    if not sums and 'source=' in recipe and not re.search(r'source=["\']?["\']?\s*\n', recipe):
        raise RuntimeError('Missing source checksums: ' + origin)
    inputs = []
    for checksum, name in sums:
        if pathlib.Path(name).name != name:
            raise RuntimeError('Unsafe source name')
        target = directory / name
        if not target.exists():
            errors = []
            for version in ['v3.23', 'edge', 'v3.22']:
                try:
                    download('https://distfiles.alpinelinux.org/distfiles/' + version + '/' + urllib.parse.quote(name), target)
                    break
                except RuntimeError as error:
                    errors.append(str(error))
            if not target.exists():
                raise RuntimeError('Missing ' + origin + '/' + name + ': ' + '; '.join(errors))
        if hashlib.sha512(target.read_bytes()).hexdigest() != checksum:
            raise RuntimeError('Source checksum mismatch: ' + origin + '/' + name)
        inputs.append({'name': name, 'sha512': checksum})
    manifest = {'origin': origin, 'aports_commit': commit, 'packages': packages, 'inputs': inputs}
    (directory / 'SOURCE.json').write_text(json.dumps(manifest, indent=2) + '\n')
    print('PASS:', origin, len(inputs), 'verified inputs', flush=True)
    return manifest


def main():
    origins = {}
    for block in DB.read_text().split('\n\n'):
        fields = dict(line.split(':', 1) for line in block.splitlines()
                      if len(line) > 2 and line[1] == ':' and line[0] in 'PVoc')
        if 'P' in fields:
            origins.setdefault((fields.get('o', fields['P']), fields.get('c', '')), []).append(
                fields['P'] + '=' + fields['V'])
    failed, manifests = [], []
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        pending = {pool.submit(collect, item): item[0][0] for item in origins.items()}
        for future in concurrent.futures.as_completed(pending):
            try:
                manifests.append(future.result())
            except Exception as error:
                failed.append(str(error))
                print('FAIL:', error, flush=True)
    if failed:
        raise SystemExit('\n'.join(failed))
    (OUT / 'MANIFEST.json').write_text(json.dumps(sorted(manifests, key=lambda x: x['origin']), indent=2) + '\n')
    print('PASS: all', len(manifests), 'installed package origins have exact recipes and verified inputs')


if __name__ == '__main__':
    main()
