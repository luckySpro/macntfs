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
    expected={'#Core.pkg','#PreferencePane.pkg','#OfflineRuntime.pkg','#Application.pkg'}
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
    assert len(names)==4
    executable=app/'Contents/MacOS/macntfs'
    for binary in [executable,*[payload/name for name in names]]:
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
        _,mode,uid=item.split('\t'); assert uid=='0' and mode=='100755',item
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
        'root_runtime_permissions_verified':True,'vendor_components_unchanged':True,
        'vendor_team_id':'3T5GSNBU6W','runtime_homebrew_library_dependency':False,
        'remote_package_downloads':False,
        'clean_machine_offline_install_tested':False,'developer_id_notarized':False,
        'kernel_image_roundtrip':'Passed separately with scripts/test-offline-runtime.py',
        'fskit':'Experimental, not verified',
    }
    (root/'dist/validation-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
    print('PASS: local installation plan, unchanged upstream components, signatures, payload hashes, root permissions, portable library dependencies')
