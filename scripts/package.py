#!/usr/bin/env python3
"""Create an offline macOS product installer and DMG; no network is used."""
import hashlib, html, json, os, pathlib, plistlib, shutil, subprocess, tarfile, xml.etree.ElementTree as ET
os.environ['COPYFILE_DISABLE'] = '1'
root = pathlib.Path(__file__).resolve().parents[1]
dist = root/'dist'
build = dist/'package-build'
version = (root/'VERSION').read_text().strip()
if build.exists(): shutil.rmtree(build)
build.mkdir(parents=True)

def run(*args): subprocess.run([str(x) for x in args], check=True)
def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def readable_app_resources(app):
    # Source crates may carry 0640 notices. Installed bundles must be readable to
    # normal users after pkgbuild changes ownership to root; keep execute bits.
    for parent, dirs, files in os.walk(app):
        for name in dirs+files:
            path=pathlib.Path(parent)/name
            if not path.is_symlink(): path.chmod((path.stat().st_mode & ~0o022) | (0o555 if path.is_dir() else 0o444))

manifest = json.loads((root/'vendor/manifest.json').read_text())
for name, entry in [('macfuse-5.4.0.dmg',manifest['macfuse']),('ntfs-3g-2026.9.28.tar.gz',manifest['ntfs3g'])]:
    if digest(root/'vendor/downloads'/name) != entry['sha256']: raise SystemExit('Upstream asset checksum mismatch')
# Runtime signatures are established before the manifest is generated.
runtime = build/'runtime-root/Library/Application Support/NTFS Desktop/Runtime'
shutil.copytree(root/'vendor/runtime',runtime)
shutil.copy2(root/'target/release/ntfs-helper',runtime/'bin/ntfs-helper')
identity = os.environ.get('NTFS_APPLICATION_SIGN_IDENTITY', '-')
for binary in [runtime/'bin/ntfs-3g',runtime/'bin/ntfs-3g.probe',runtime/'bin/ntfs-helper']:
    options = ['--options','runtime','--timestamp'] if identity != '-' else []
    if identity != '-' and binary.name == 'ntfs-3g':
        options += ['--entitlements',str(root/'scripts/driver.entitlements')]
    run('/usr/bin/codesign','--force','--sign',identity,*options,binary)
# Preserve libfuse's vendor signature. Do not re-sign or modify its contents.
run('/usr/bin/codesign','--verify','--strict',runtime/'lib/libfuse.2.dylib')
# Both backends are bundled; MicroVM mounting remains manual in this preview.
microvm = runtime/'MicroVM'
if not (root/'vendor/microvm/bin/anylinuxfs').is_file(): raise SystemExit('Build the offline microVM payload with scripts/build-microvm.py first')
run('/usr/bin/ditto','--extattr',root/'vendor/microvm',microvm)
# Installer does not reliably retain every Linux override_stat attribute (notably
# setuid mount). Store guest metadata as ordinary, integrity-checked payload data.
guest_metadata={}
guest=microvm/'profile/alpine/rootfs'
for parent,dirs,names in os.walk(guest):
    for name in dirs+names:
        path=pathlib.Path(parent)/name
        if path.is_symlink(): continue
        result=subprocess.run(['/usr/bin/xattr','-p','user.containers.override_stat',str(path)],capture_output=True,text=True)
        if result.returncode==0: guest_metadata[path.relative_to(guest).as_posix()]=result.stdout.strip()
(microvm/'GUEST-METADATA.json').write_text(json.dumps(guest_metadata,sort_keys=True)+'\n')
run('/usr/bin/codesign','--force','--sign',identity,'--options','runtime','--entitlements',root/'scripts/microvm.entitlements',microvm/'bin/anylinuxfs')
run('/usr/bin/codesign','--force','--sign',identity,'--options','runtime',microvm/'libexec/gvproxy')
# Linux files retain guest symlinks; hash links themselves rather than following them.
vm_entries=[]
for parent,dirs,names in os.walk(microvm):
    for name in dirs+names:
        path=pathlib.Path(parent)/name
        rel=path.relative_to(microvm).as_posix()
        if rel=='SHA256SUMS' or rel.startswith('profile/logs/'): continue
        if path.is_symlink(): vm_entries.append((rel,'L',hashlib.sha256(os.fsencode(os.readlink(path))).hexdigest()))
        elif path.is_file(): vm_entries.append((rel,'F',digest(path)))
(microvm/'SHA256SUMS').write_text(''.join(f'{sha}  {kind} {name}\n' for name,kind,sha in sorted(vm_entries)))
files = ['bin/ntfs-3g','bin/ntfs-3g.probe','bin/ntfs-helper','lib/libfuse.2.dylib','MicroVM/SHA256SUMS']
(runtime/'VERSION').write_text(version+'\n')
(runtime/'SHA256SUMS').write_text(''.join(f'{digest(runtime/f)}  {f}\n' for f in files))
packages = build/'packages'; packages.mkdir()
scripts = build/'runtime-scripts'; scripts.mkdir()
shutil.copy2(root/'scripts/runtime-preinstall',scripts/'preinstall')
shutil.copy2(root/'scripts/runtime-postinstall',scripts/'postinstall')
daemons = build/'runtime-root/Library/LaunchDaemons'; daemons.mkdir(parents=True)
with (daemons/'com.macntfs.helper.plist').open('wb') as stream:
    plistlib.dump({'Label':'com.macntfs.helper', 'ProgramArguments':['/Library/Application Support/NTFS Desktop/Runtime/bin/ntfs-helper','serve'], 'RunAtLoad':True, 'KeepAlive':True, 'AbandonProcessGroup':True, 'AssociatedBundleIdentifiers':['com.yuntu.ntfs-desktop'], 'ProcessType':'Background', 'ThrottleInterval':10, 'StandardErrorPath':'/Library/Application Support/NTFS Desktop/Runtime/service.log', 'StandardOutPath':'/Library/Application Support/NTFS Desktop/Runtime/service.log'},stream)
run('/usr/bin/pkgbuild','--root',build/'runtime-root','--identifier','com.yuntu.ntfs-desktop.runtime','--version',version,'--ownership','recommended','--scripts',scripts,packages/'OfflineRuntime.pkg')
app = dist/'macntfs.app'
if app.exists(): shutil.rmtree(app)
(app/'Contents/MacOS').mkdir(parents=True)
resources = app/'Contents/Resources'; resources.mkdir()
shutil.copy2(root/'target/release/macntfs',app/'Contents/MacOS/macntfs')
# Guest absolute symlinks belong only in the installed runtime, never in a signed app.
shutil.copytree(runtime,resources/'Runtime',ignore=shutil.ignore_patterns('MicroVM'))
(resources/'Runtime/MicroVM').mkdir()
shutil.copy2(microvm/'SHA256SUMS',resources/'Runtime/MicroVM/SHA256SUMS')
(resources/'Installers').mkdir()
shutil.copy2(packages/'OfflineRuntime.pkg',resources/'Installers/OfflineRuntime.pkg')
shutil.copy2(root/'vendor/downloads/Install macFUSE.pkg',resources/'Installers/Install macFUSE.pkg')
shutil.copytree(root/'vendor/licenses',resources/'Licenses')
shutil.copy2(root/'vendor/macfuse-expanded/Resources/License.rtf',resources/'Licenses/macFUSE-Installer-License.rtf')
(resources/'Sources').mkdir()
for name in ['ntfs-3g-2026.9.28.tar.gz','macfuse-library-7a6cdd2.tar.gz','anylinuxfs-8aa9ccd.tar.gz']:
    shutil.copy2(root/'vendor/downloads'/name,resources/'Sources'/name)
shutil.copy2(root/'vendor/manifest.json',resources/'Sources/manifest.json')
# Collect notices for compiled Rust dependencies from locally cached crates.
metadata = json.loads(subprocess.check_output(['cargo','metadata','--offline','--locked','--format-version','1','--filter-platform','aarch64-apple-darwin'],cwd=root))
notices = resources/'Licenses/Rust-Dependencies'; notices.mkdir()
for package in metadata['packages']:
    if package.get('source','') and str(package['source']).startswith('registry+'):
        crate = pathlib.Path(package['manifest_path']).parent
        folder = notices/f"{package['name']}-{package['version']}"
        for license_file in crate.glob('*'):
            if license_file.is_file() and license_file.name.lower().startswith(('license','copying','notice')):
                folder.mkdir(exist_ok=True); shutil.copy2(license_file,folder/license_file.name)
        if not folder.exists():
            folder.mkdir(); (folder/'NOTICE.txt').write_text(f"{package['name']} {package['version']}\nDeclared license: {package.get('license')}\nSource: {package.get('repository')}\n")
# Native icon, generated from vector drawing code in our source tree.
run('/usr/bin/swift',root/'scripts/make-icon.swift',root/'assets')
shutil.copy2(root/'assets/AppIcon.icns',resources/'AppIcon.icns')
info = {
    'CFBundleExecutable':'macntfs','CFBundleIdentifier':'com.yuntu.ntfs-desktop',
    'CFBundleName':'macntfs','CFBundleDisplayName':'macntfs','CFBundlePackageType':'APPL',
    'CFBundleShortVersionString':version,'CFBundleVersion':version,'CFBundleIconFile':'AppIcon',
    'LSMinimumSystemVersion':'12.0','NSHighResolutionCapable':True,
    'NSRemovableVolumesUsageDescription':'识别和挂载用户选择的外置 NTFS 磁盘。'
}
with (app/'Contents/Info.plist').open('wb') as f: plistlib.dump(info,f)
options = ['--options','runtime'] + (['--timestamp'] if identity != '-' else [])
readable_app_resources(app)
run('/usr/bin/codesign','--force','--sign',identity,*options,app)
run('/usr/bin/codesign','--verify','--deep','--strict',app)
app_root = build/'app-root/Applications'; app_root.mkdir(parents=True)
run('/usr/bin/ditto','--extattr',app,app_root/app.name)
# Include current application source and reproducible driver build instructions.
with tarfile.open(resources/f'Sources/macntfs-{version}-source.tar.gz','w:gz') as archive:
    for path in ['frontend','src-tauri','package.json','package-lock.json','vite.config.js','VERSION','src','scripts','THIRD_PARTY_MICROVM.md','Cargo.toml','Cargo.lock','README.md','VALIDATION.md','LICENSE','vendor/manifest.json','vendor/licenses']:
        archive.add(root/path,arcname=f'macntfs/{path}')
# Re-sign after adding the source archive and recreate the staged copy.
readable_app_resources(app)
run('/usr/bin/codesign','--force','--sign',identity,*options,app)
shutil.rmtree(app_root/app.name); run('/usr/bin/ditto','--extattr',app,app_root/app.name)
# Bundle relocation is disabled: the privileged helper trusts a fixed root path.
components = [{'RootRelativeBundlePath':'Applications/macntfs.app','BundleIsRelocatable':False,'BundleIsVersionChecked':True,'BundleHasStrictIdentifier':True,'BundleOverwriteAction':'upgrade'}]
with (build/'components.plist').open('wb') as f: plistlib.dump(components,f)
app_scripts = build/'app-scripts'; app_scripts.mkdir()
(app_scripts/'preinstall').write_text('#!/bin/bash\nset -euo pipefail\nif [ -L /Applications ] || [ -L /Applications/macntfs.app ]; then echo "Unsafe application path" >&2; exit 1; fi\nif [ -d /Applications/macntfs.app ] && /usr/bin/find /Applications/macntfs.app -type l -print -quit | /usr/bin/grep -q .; then echo "Application bundle contains symbolic links" >&2; exit 1; fi\n')
(app_scripts/'preinstall').chmod(0o755)
run('/usr/bin/pkgbuild','--scripts',app_scripts,'--root',build/'app-root','--component-plist',build/'components.plist','--identifier','com.yuntu.ntfs-desktop.application','--version',version,'--ownership','recommended',packages/'Application.pkg')
# The guard executes before vendor components, closing the manual-install race.
guard_scripts = build/'guard-scripts'; guard_scripts.mkdir()
preflight = (root/'scripts/runtime-preinstall').read_text().replace('/bin/launchctl bootout system/com.macntfs.helper >/dev/null 2>&1 || true', '')
(guard_scripts/'preinstall').write_text(preflight); (guard_scripts/'preinstall').chmod(0o755)
run('/usr/bin/pkgbuild','--nopayload','--scripts',guard_scripts,'--identifier','com.yuntu.ntfs-desktop.update-guard','--version',version,packages/'UpdateGuard.pkg')
# Combine the official macFUSE components without changing their code-signed
# payload or scripts. The wrapper itself needs our Developer ID Installer for
# public distribution; the complete original vendor-signed PKG is also kept.
for name in ['Core.pkg','PreferencePane.pkg']:
    run('/usr/sbin/pkgutil','--flatten',root/'vendor/macfuse-expanded'/name,packages/name)
installer_resources = build/'installer-resources'
shutil.copytree(root/'vendor/macfuse-expanded/Resources',installer_resources)
welcome = '<html><meta charset="utf-8"><body style="font-family:-apple-system,sans-serif;padding:20px"><h1>macntfs · 两种读写模式</h1><p>Apple Silicon、macOS 13+ 新安装默认不安装 macFUSE，使用内置 Linux 微虚拟机（实验性）。无需内核扩展、恢复模式或降低启动安全性；仍需管理员安装授权与助手磁盘访问权限。</p><p>需要成熟的稳定模式时，在下一页勾选「稳定模式组件 · macFUSE」。首次启用可能需要授权内核扩展并重启。macOS 12 必须安装稳定模式组件。</p><p>已安装 macFUSE 的用户默认保留稳定组件更新；取消勾选不会卸载已有驱动。应用保留已保存的模式。</p><p>更新前请安全推出磁盘，再从菜单栏退出 macntfs。全部组件已离线提供，无需 Homebrew 或联网。开发测试包尚未签名公证。</p></body></html>'
(installer_resources/'Welcome.html').write_text(welcome)
license_text = '\n\n'.join((root/'vendor/licenses'/f).read_text() for f in ['macfuse-LICENSE.txt','ntfs-3g-GPL.txt','ntfs-3g-LGPL.txt'])
(installer_resources/'License.html').write_text('<html><meta charset="utf-8"><body><h2>macntfs · 免费开源</h2><p>应用代码采用 MIT 许可。驱动保留各自许可，源码随包附带。macFUSE 的商业捆绑需另行取得授权。</p><pre style="white-space:pre-wrap;font-size:11px">'+html.escape(license_text)+'</pre></body></html>')
conclusion = '<html><meta charset="utf-8"><body style="font-family:-apple-system,sans-serif;padding:20px"><h2>已安装 macntfs</h2><p>从「应用程序」打开 macntfs，在「设置与更新 → 读写模式」确认模式。免内核扩展模式目前仅手动挂载；稳定模式支持插入后自动读写。</p><p>首次使用按向导完成助手磁盘访问授权。免内核扩展模式跳过 macFUSE 授权步骤；稳定模式按系统提示授权驱动。更新助手后，可能需要移除旧 ntfs-helper 权限条目并重新添加。</p><p>切换模式前先安全推出磁盘，切换后重新连接。取消安装稳定组件不会卸载已有 macFUSE。</p></body></html>'
(installer_resources/'Conclusion.html').write_text(conclusion)
tree = ET.parse(root/'vendor/macfuse-expanded/Distribution'); distribution = tree.getroot()
distribution.find('title').text = 'macntfs'
distribution.find('welcome').set('file','Welcome.html')
distribution.find('license').set('file','License.html')
ET.SubElement(distribution,'conclusion',{'file':'Conclusion.html'})
distribution.find('options').set('hostArchitectures','arm64')
distribution.find('options').set('customize','always')
# Do not downgrade an existing newer macFUSE installation.
# UpdateGuard preinstall performs privileged checks before driver installation.
distribution.find('script').text += '''
function hasNewerMacFUSE() {
    var receipt = my.target.receiptForIdentifier('io.macfuse.installer.components.core');
    return receipt != null && system.compareVersions(receipt.version, '5.4.0') > 0;
}
function needsStableComponents() {
    return system.compareVersions(system.version.ProductVersion, '13.0') < 0;
}
function defaultStableComponents() {
    return !hasNewerMacFUSE() && (needsStableComponents() || my.target.receiptForIdentifier('io.macfuse.installer.components.core') != null);
}
'''
outline = distribution.find('choices-outline')
for choice in list(distribution.findall('choice')):
    distribution.remove(choice)
for line in list(outline): outline.remove(line)
stable_id='com.macntfs.optional-macfuse'
stable_choice=ET.SubElement(distribution,'choice',{
    'id':stable_id,'title':'稳定模式组件 · macFUSE（可选）',
    'description':'免内核模式不需要此项。稳定模式需要驱动授权；取消勾选不会卸载已有驱动。macOS 12 必选。',
    'start_selected':'defaultStableComponents()',
    'enabled':'!hasNewerMacFUSE() && !needsStableComponents()',
    'visible':'true',
})
for identifier in ['io.macfuse.installer.components.core','io.macfuse.installer.components.preferencepane']:
    ET.SubElement(stable_choice,'pkg-ref',{'id':identifier})
ET.SubElement(outline,'line',{'choice':stable_id})
guard_id='com.yuntu.ntfs-desktop.update-guard'
guard_choice=ET.SubElement(distribution,'choice',{'id':guard_id,'visible':'false','start_selected':'true','start_enabled':'false'})
ET.SubElement(guard_choice,'pkg-ref',{'id':guard_id})
outline.insert(0,ET.Element('line',{'choice':guard_id}))
ET.SubElement(distribution,'pkg-ref',{'id':guard_id,'version':version,'auth':'root'}).text='#UpdateGuard.pkg'
for identifier,name in [('com.yuntu.ntfs-desktop.runtime','OfflineRuntime.pkg'),('com.yuntu.ntfs-desktop.application','Application.pkg')]:
    choice = ET.SubElement(distribution,'choice',{'id':identifier,'title':identifier,'visible':'false','start_selected':'true','start_enabled':'false'})
    ET.SubElement(choice,'pkg-ref',{'id':identifier})
    ET.SubElement(outline,'line',{'choice':identifier})
    reference = ET.SubElement(distribution,'pkg-ref',{'id':identifier,'version':version,'auth':'root'}); reference.text = '#'+name
# No network URLs or package download locations appear in the distribution.
tree.write(build/'Distribution',encoding='utf-8',xml_declaration=True)
installer = dist/f'macntfs-{version}-arm64.pkg'
arguments = ['/usr/bin/productbuild','--distribution',build/'Distribution','--resources',installer_resources,'--package-path',packages]
if os.environ.get('NTFS_INSTALLER_SIGN_IDENTITY'): arguments += ['--sign',os.environ['NTFS_INSTALLER_SIGN_IDENTITY']]
run(*arguments,installer)
# Compact download image contains one Installer PKG and a short offline guide.
dmg_root = build/'dmg-root'; dmg_root.mkdir()
shutil.copy2(installer,dmg_root/installer.name)
(dmg_root/'开始使用.txt').write_text(f'macntfs {version}（Apple Silicon）\n\n双击 PKG，按 macOS 安装器提示完成安装。\n打开「应用程序」中的 macntfs。\n本包包含全部读写组件，不需要 Homebrew 或网络。\n首次使用的系统授权不可跳过。\n本开发包尚未通过 Apple Developer ID 签名公证，不能视为公开发行版。\n')
dmg = dist/f'macntfs-{version}-arm64.dmg'
if dmg.exists(): dmg.unlink()
run('/usr/bin/hdiutil','create','-fs','HFS+','-volname','macntfs','-srcfolder',dmg_root,'-format','UDZO',dmg)
(dist/'SHA256SUMS').write_text(''.join(f'{digest(p)}  {p.name}\n' for p in [installer,dmg]))
print(f'Offline installer: {installer}\nDownload image: {dmg}\nLocal app: {app}')
