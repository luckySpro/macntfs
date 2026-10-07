#!/usr/bin/env python3
"""Package and sign final app bytes; publish manifest for Tauri updater."""
import datetime,hashlib,json,os,pathlib,subprocess,tarfile
root=pathlib.Path(__file__).resolve().parents[1]
version=(root/'VERSION').read_text().strip()
config=json.loads((root/'src-tauri/tauri.conf.json').read_text())
assert config['version']==version
assert json.loads((root/'package.json').read_text())['version']==version
archive=root/f'dist/NTFS-Desktop-{version}-arm64.app.tar.gz'
with tarfile.open(archive,'w:gz') as tar: tar.add(root/'dist/NTFS Desktop.app',arcname='NTFS Desktop.app')
args=['npm','run','tauri','--','signer','sign','--app-version',version,'-p','']
if os.environ.get('NTFS_UPDATE_KEY'): args += ['-f',os.environ['NTFS_UPDATE_KEY']]
subprocess.run([*args,str(archive)],cwd=root,check=True)
signature=archive.with_suffix(archive.suffix+'.sig').read_text().strip()
manifest={'version':version,'notes':'Tauri 界面、离线驱动与改进的 NTFS 错误提示。','pub_date':datetime.datetime.now(datetime.timezone.utc).isoformat(),'platforms':{'darwin-aarch64':{'signature':signature,'url':f'https://github.com/luckySpro/macntfs/releases/download/v{version}/{archive.name}'}}}
(root/'dist/latest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
files=[p for p in (root/'dist').iterdir() if p.is_file() and p.suffix in ['.pkg','.dmg','.gz','.sig','.json']]
(root/'dist/SHA256SUMS').write_text(''.join(f'{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.name}\n' for p in sorted(files)))
