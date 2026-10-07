#!/usr/bin/env python3
import json,pathlib,re,sys
root=pathlib.Path(__file__).resolve().parents[1]
version=sys.argv[1]
if not re.fullmatch(r'\d+\.\d+\.\d+',version): raise SystemExit('Use major.minor.patch')
(root/'VERSION').write_text(version+'\n')
for name in ['Cargo.toml','src-tauri/Cargo.toml']:
 p=root/name;p.write_text(re.sub(r'version\s*=\s*"[^"]+"',f'version = "{version}"',p.read_text(),count=1))
for name in ['package.json','src-tauri/tauri.conf.json']:
 p=root/name;d=json.loads(p.read_text());d['version']=version;p.write_text(json.dumps(d,ensure_ascii=False,indent=2)+'\n')
print('Run npm install --package-lock-only and cargo check to refresh lockfiles; tag v'+version+' to publish.')
