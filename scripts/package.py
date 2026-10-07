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
files = ['bin/ntfs-3g','bin/ntfs-3g.probe','bin/ntfs-helper','lib/libfuse.2.dylib']
(runtime/'VERSION').write_text(version+'\n')
(runtime/'SHA256SUMS').write_text(''.join(f'{digest(runtime/f)}  {f}\n' for f in files))
packages = build/'packages'; packages.mkdir()
scripts = build/'runtime-scripts'; scripts.mkdir()
shutil.copy2(root/'scripts/runtime-preinstall',scripts/'preinstall')
run('/usr/bin/pkgbuild','--root',build/'runtime-root','--identifier','com.yuntu.ntfs-desktop.runtime','--version',version,'--ownership','recommended','--scripts',scripts,packages/'OfflineRuntime.pkg')
app = dist/'macntfs.app'
if app.exists(): shutil.rmtree(app)
(app/'Contents/MacOS').mkdir(parents=True)
resources = app/'Contents/Resources'; resources.mkdir()
shutil.copy2(root/'target/release/macntfs',app/'Contents/MacOS/macntfs')
shutil.copytree(runtime,resources/'Runtime')
(resources/'Installers').mkdir()
shutil.copy2(packages/'OfflineRuntime.pkg',resources/'Installers/OfflineRuntime.pkg')
shutil.copy2(root/'vendor/downloads/Install macFUSE.pkg',resources/'Installers/Install macFUSE.pkg')
shutil.copytree(root/'vendor/licenses',resources/'Licenses')
shutil.copy2(root/'vendor/macfuse-expanded/Resources/License.rtf',resources/'Licenses/macFUSE-Installer-License.rtf')
(resources/'Sources').mkdir()
for name in ['ntfs-3g-2026.9.28.tar.gz','macfuse-library-7a6cdd2.tar.gz']:
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
options = ['--options','runtime','--timestamp'] if identity != '-' else []
run('/usr/bin/codesign','--force','--sign',identity,*options,app)
run('/usr/bin/codesign','--verify','--deep','--strict',app)
app_root = build/'app-root/Applications'; app_root.mkdir(parents=True)
shutil.copytree(app,app_root/app.name)
# Include current application source and reproducible driver build instructions.
with tarfile.open(resources/f'Sources/macntfs-{version}-source.tar.gz','w:gz') as archive:
    for path in ['frontend','src-tauri','package.json','package-lock.json','vite.config.js','VERSION','src','scripts','Cargo.toml','Cargo.lock','README.md','VALIDATION.md','LICENSE','vendor/manifest.json','vendor/licenses']:
        archive.add(root/path,arcname=f'macntfs/{path}')
# Re-sign after adding the source archive and recreate the staged copy.
run('/usr/bin/codesign','--force','--sign',identity,*options,app)
shutil.rmtree(app_root/app.name); shutil.copytree(app,app_root/app.name)
# Bundle relocation is disabled: the privileged helper trusts a fixed root path.
components = [{'RootRelativeBundlePath':'Applications/macntfs.app','BundleIsRelocatable':False,'BundleIsVersionChecked':True,'BundleHasStrictIdentifier':True,'BundleOverwriteAction':'upgrade'}]
with (build/'components.plist').open('wb') as f: plistlib.dump(components,f)
run('/usr/bin/pkgbuild','--root',build/'app-root','--component-plist',build/'components.plist','--identifier','com.yuntu.ntfs-desktop.application','--version',version,'--ownership','recommended',packages/'Application.pkg')
# Combine the official macFUSE components without changing their code-signed
# payload or scripts. The wrapper itself needs our Developer ID Installer for
# public distribution; the complete original vendor-signed PKG is also kept.
for name in ['Core.pkg','PreferencePane.pkg']:
    run('/usr/sbin/pkgutil','--flatten',root/'vendor/macfuse-expanded'/name,packages/name)
installer_resources = build/'installer-resources'
shutil.copytree(root/'vendor/macfuse-expanded/Resources',installer_resources)
welcome = '<html><meta charset="utf-8"><body style="font-family:-apple-system,sans-serif;padding:20px"><h1>macntfs</h1><p>免费的 macOS NTFS 读写工具</p><p>此离线安装包包含应用、NTFS-3G 读写引擎、权限助手和 macFUSE。安装和使用均无需 Homebrew 或联网下载。</p><p>首次使用仍需按 macOS 提示授权。稳定模式可能需要允许系统扩展并重启；macOS 15.4 及以上可手动选择实验性 FSKit 后端。</p><p>当前包适用于 Apple Silicon，macOS 12–15、26、27。未签名公证的开发测试包，尚未完成全系统兼容测试。</p></body></html>'
(installer_resources/'Welcome.html').write_text(welcome)
license_text = '\n\n'.join((root/'vendor/licenses'/f).read_text() for f in ['macfuse-LICENSE.txt','ntfs-3g-GPL.txt','ntfs-3g-LGPL.txt'])
(installer_resources/'License.html').write_text('<html><meta charset="utf-8"><body><h2>macntfs · 免费开源</h2><p>应用代码采用 MIT 许可。驱动保留各自许可，源码随包附带。macFUSE 的商业捆绑需另行取得授权。</p><pre style="white-space:pre-wrap;font-size:11px">'+html.escape(license_text)+'</pre></body></html>')
conclusion = '<html><meta charset="utf-8"><body style="font-family:-apple-system,sans-serif;padding:20px"><h2>已安装 macntfs</h2><p>从「应用程序」打开 macntfs，连接外置 NTFS 磁盘并选择「开启读写」。</p><p>如 macOS 提示允许文件系统扩展或重启，请先完成系统步骤。</p></body></html>'
(installer_resources/'Conclusion.html').write_text(conclusion)
tree = ET.parse(root/'vendor/macfuse-expanded/Distribution'); distribution = tree.getroot()
distribution.find('title').text = 'macntfs'
distribution.find('welcome').set('file','Welcome.html')
distribution.find('license').set('file','License.html')
ET.SubElement(distribution,'conclusion',{'file':'Conclusion.html'})
distribution.find('options').set('hostArchitectures','arm64')
distribution.find('options').set('customize','never')
# Do not downgrade an existing newer macFUSE installation.
distribution.find('script').text += '''
function hasNewerMacFUSE() {
    var receipt = my.target.receiptForIdentifier('io.macfuse.installer.components.core');
    return receipt != null && system.compareVersions(receipt.version, '5.4.0') > 0;
}
'''
for choice in distribution.findall('choice'):
    choice.set('start_selected','!hasNewerMacFUSE()'); choice.set('start_enabled','false')
outline = distribution.find('choices-outline')
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
