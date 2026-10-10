#!/usr/bin/env python3
"""No root, no physical disks: offline guest roundtrip on a disposable NTFS image.
Uses the debug anylinuxfs build solely for its user-owned development lock.
This verifies Linux driver/VM image; it does NOT certify macOS NFS or physical hotplug.
"""
import json, os, pathlib, shutil, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parents[1]
preview_binary=pathlib.Path('/tmp/macntfs-offline-vm-build/target/debug/anylinuxfs')
if not preview_binary.exists():raise SystemExit('Build the debug anylinuxfs target after scripts/build-microvm.py')
with tempfile.TemporaryDirectory(prefix='macntfs-vm-qa-') as temporary:
    directory=pathlib.Path(temporary)
    package=root/f"dist/macntfs-{(root/'VERSION').read_text().strip()}-arm64.pkg"
    subprocess.run(['/usr/sbin/pkgutil','--expand-full',str(package),str(directory/'unpacked')],check=True)
    payload=directory/'unpacked/OfflineRuntime.pkg/Payload/Library/Application Support/NTFS Desktop/Runtime/MicroVM'
    subprocess.run(['/usr/bin/ditto','--extattr',str(payload),str(directory/'runtime')],check=True)
    for name,value in json.loads((payload/'GUEST-METADATA.json').read_text()).items():
        subprocess.run(['/usr/bin/xattr','-w','user.containers.override_stat',value,str(directory/'runtime/profile/alpine/rootfs'/name)],check=True)
    # Reproduce installed immutability even though this fixture belongs to us.
    guest=directory/'runtime/profile/alpine/rootfs'
    for parent,dirs,names in os.walk(guest):
        for name in dirs+names:
            path=pathlib.Path(parent)/name
            if not path.is_symlink(): path.chmod(0o555 if path.is_dir() else (path.stat().st_mode & 0o111)|0o444)
    binary=directory/'runtime/bin/anylinuxfs'
    binary.unlink();shutil.copy2(preview_binary,binary)
    subprocess.run(['/usr/bin/codesign','--force','--sign','-','--entitlements',str(root/'scripts/microvm.entitlements'),str(binary)],check=True)
    image=directory/'roundtrip.img'
    with image.open('wb') as stream:stream.truncate(32*1024*1024)
    mkntfs='/opt/homebrew/opt/ntfs-3g-mac/sbin/mkntfs'
    subprocess.run([mkntfs,'--fast','--force','-L','MACNTFS-VM-QA',str(image)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    # The host fixture is the only block device assigned to the VM, /dev/vda.
    command='set -eu; uname -m; mkdir -p /tmp/macntfs-config /tmp/macntfs-qa; mount -t tmpfs tmpfs /tmp/macntfs-config; umount /tmp/macntfs-config; ntfs-3g /dev/vda /tmp/macntfs-qa -o rw,norecover; mkdir /tmp/macntfs-qa/nested; echo macntfs-offline-vm > /tmp/macntfs-qa/nested/roundtrip.txt; test "$(cat /tmp/macntfs-qa/nested/roundtrip.txt)" = macntfs-offline-vm; sync; umount /tmp/macntfs-qa; mount -t nfsd nfsd /proc/fs/nfsd; rpcbind -w; rpc.nfsd 2; rpc.nfsd 0; umount /proc/fs/nfsd; echo MACNTFS_VM_ROUNDTRIP_PASSED'
    try:
        result=subprocess.run([str(binary),'shell',str(image),'--no-tsi','--net-helper','gvproxy','--command',command],text=True,capture_output=True,timeout=60)
    finally:
        for parent,dirs,names in os.walk(guest):
            for name in dirs:
                path=pathlib.Path(parent)/name
                if not path.is_symlink(): path.chmod(0o755)
    pathlib.Path('/tmp/macntfs-vm-roundtrip.log').write_text(result.stdout+result.stderr)
    if result.returncode or 'MACNTFS_VM_ROUNDTRIP_PASSED' not in result.stdout:raise SystemExit('Offline guest roundtrip failed; see /tmp/macntfs-vm-roundtrip.log')
    subprocess.run([str(root/'vendor/runtime/bin/ntfs-3g.probe'),'--readwrite',str(image)],check=True)
    print('PASS: offline Linux boot, NTFS nested write/read, sync, guest unmount, host re-probe; no physical disks or root privileges')
