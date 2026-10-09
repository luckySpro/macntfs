#!/usr/bin/env python3
"""Read-only package inspection; never installs or writes to a real disk."""
import hashlib, json, pathlib, plistlib, subprocess, tempfile, xml.etree.ElementTree as ET
root=pathlib.Path(__file__).resolve().parents[1]
package=root/f"dist/macntfs-{(root/'VERSION').read_text().strip()}-arm64.pkg"
app=root/'dist/macntfs.app'
def command(*args): return subprocess.check_output([str(x) for x in args],stderr=subprocess.STDOUT)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
with tempfile.TemporaryDirectory(prefix='ntfs-package-verify-') as temporary:
    directory=pathlib.Path(temporary)
    expanded=directory/'expanded'
    command('/usr/sbin/pkgutil','--expand',package,expanded)
    distribution=ET.parse(expanded/'Distribution').getroot()
    refs={node.text for node in distribution.findall('pkg-ref') if node.text and node.text.strip()}
    expected={'#Core.pkg','#PreferencePane.pkg','#OfflineRuntime.pkg','#Application.pkg','#UpdateGuard.pkg'}
    assert refs==expected,refs
    assert distribution.find('options').get('hostArchitectures')=='arm64'
    for component in ['Core.pkg','PreferencePane.pkg']:
        vendor=root/'vendor/macfuse-expanded'/component
        for name in ['Payload','Bom','PackageInfo']:
            assert sha(expanded/component/name)==sha(vendor/name),(component,name)
        for path in (vendor/'Scripts').rglob('*') if (vendor/'Scripts').exists() else []:
            if path.is_file(): assert sha(path)==sha(expanded/component/'Scripts'/path.relative_to(vendor/'Scripts'))
    payload=app/'Contents/Resources/Runtime'
    names=[]
    for line in (payload/'SHA256SUMS').read_text().splitlines():
        digest,name=line.split('  ',1); assert sha(payload/name)==digest,name; names.append(name)
    assert len(names)==5
    vm=root/'dist/package-build/runtime-root/Library/Application Support/NTFS Desktop/Runtime/MicroVM'
    for line in (vm/'SHA256SUMS').read_text().splitlines():
        digest,entry=line.split('  ',1);kind,name=entry.split(' ',1);path=vm/name
        actual=hashlib.sha256(str(path.readlink()).encode()).hexdigest() if kind=='L' else sha(path)
        assert actual==digest,name
    for name in ['bin/anylinuxfs','libexec/gvproxy']:
        for line in command('/usr/bin/otool','-L',vm/name).decode().splitlines()[1:]:
            assert line.strip().split(' (')[0].startswith(('/usr/lib/','/System/Library/')),line
    entitlements=plistlib.loads(subprocess.check_output(['/usr/bin/codesign','-d','--entitlements',':-',str(vm/'bin/anylinuxfs')],stderr=subprocess.DEVNULL))
    assert entitlements.get('com.apple.security.hypervisor') is True
    assert not (vm/'libexec/init-rootfs').exists(),'Runtime must never provision or download images'
    assert (vm/'profile/alpine/rootfs.ver').read_text()=='macntfs-microvm-v1'
    full=directory/'full'
    command('/usr/sbin/pkgutil','--expand-full',package,full)
    guest=full/'OfflineRuntime.pkg/Payload/Library/Application Support/NTFS Desktop/Runtime/MicroVM/profile/alpine/rootfs'
    for name in ['bin/busybox','vmproxy','usr/sbin/rpc.nfsd']:
        override=command('/usr/bin/xattr','-p','user.containers.override_stat',guest/name).decode().strip()
        assert override.startswith('0:0:'),(name,override)
    assert distribution.find('choices-outline')[0].get('choice')=='com.yuntu.ntfs-desktop.update-guard'
    assert (expanded/'UpdateGuard.pkg/Scripts/preinstall').is_file()
    service_plist=root/'dist/package-build/runtime-root/Library/LaunchDaemons/com.macntfs.helper.plist'
    service=plistlib.loads(service_plist.read_bytes())
    assert service['ProgramArguments']==['/Library/Application Support/NTFS Desktop/Runtime/bin/ntfs-helper','serve']
    assert service['AbandonProcessGroup'] is True and service['RunAtLoad'] is True
    service_bom=command('/usr/bin/lsbom','-p','fmu',expanded/'OfflineRuntime.pkg/Bom').decode().splitlines()
    service_item=next(line for line in service_bom if line.split('\t')[0].endswith('/LaunchDaemons/com.macntfs.helper.plist'))
    assert service_item.split('\t')[1:]==['100644','0'],service_item
    assert (expanded/'OfflineRuntime.pkg/Scripts/postinstall').is_file()
    flags=subprocess.run(['/usr/bin/codesign','-dvv',str(app)],capture_output=True,text=True,check=True).stderr
    assert any('flags=' in line and 'runtime' in line for line in flags.splitlines()),'GUI must enable hardened runtime before privileged IPC'

    executable=app/'Contents/MacOS/macntfs'
    for binary in [executable,*[payload/name for name in names if name != "MicroVM/SHA256SUMS"],vm/"bin/anylinuxfs",vm/"libexec/gvproxy"]:
        command('/usr/bin/codesign','--verify','--strict',binary)
    command('/usr/bin/codesign','--verify','--deep','--strict',app)
    vendor_package=app/'Contents/Resources/Installers/Install macFUSE.pkg'
    signing=command('/usr/sbin/pkgutil','--check-signature',vendor_package).decode()
    assert '3T5GSNBU6W' in signing
    for name in ['bin/ntfs-3g','bin/ntfs-3g.probe']:
        lines=command('/usr/bin/otool','-L',payload/name).decode().splitlines()[1:]
        for line in lines:
            library=line.strip().split(' (')[0]
            assert library.startswith(('/usr/lib/','/System/Library/','@executable_path/../lib/libfuse.2.dylib')),library
    bom=command('/usr/bin/lsbom','-p','fmu',expanded/'OfflineRuntime.pkg/Bom').decode().splitlines()
    for name in names:
        item=next(line for line in bom if line.split('\t')[0].endswith('/Runtime/'+name))
        _,mode,uid=item.split('\t'); assert uid=='0' and mode==('100644' if name=='MicroVM/SHA256SUMS' else '100755'),item
    app_bom=command('/usr/bin/lsbom','-p','fmu',expanded/'Application.pkg/Bom').decode().splitlines()
    for suffix in ['/macntfs.app','/macntfs.app/Contents','/macntfs.app/Contents/MacOS','/macntfs.app/Contents/MacOS/macntfs']:
        item=next(line for line in app_bom if line.split('\t')[0].endswith(suffix))
        _,mode,uid=item.split('\t'); assert uid=='0' and int(mode,8) & 0o022 == 0,item
    choices=plistlib.loads(command('/usr/sbin/installer','-showChoicesXML','-pkg',package,'-target','/'))
    def flatten(items):
        for item in items:
            yield item
            yield from flatten(item.get('childItems',[]))
    for choice in flatten(choices):
        for active in choice.get('pathsOfActivePackagesInChoice',[]):
            assert active.startswith('file://'),active
    report={
        'package':package.name,'sha256':sha(package),'size_bytes':package.stat().st_size,
        'embedded_components':sorted(refs),'payload_hashes_verified':names,
        'launchdaemon_and_gui_hardened_runtime_verified':True,'root_runtime_permissions_verified':True,'vendor_components_unchanged':True,
        'vendor_team_id':'3T5GSNBU6W','runtime_homebrew_library_dependency':False,
        'remote_package_downloads':False,
        'clean_machine_offline_install_tested':False,'developer_id_notarized':False,
        'kernel_image_roundtrip':'Passed separately with scripts/test-offline-runtime.py',
        'fskit':'Experimental, not verified',
        'microvm':'Experimental: offline payload and boot verified separately; physical disk/NFS roundtrip not certified',
    }
    (root/'dist/validation-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('PASS: local installation plan, unchanged upstream components, signatures, payload hashes, root permissions, portable library dependencies')
