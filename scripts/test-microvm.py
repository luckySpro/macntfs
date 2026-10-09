#!/usr/bin/env python3
"""No root, no physical disks: offline guest roundtrip on a disposable NTFS image.
Uses the debug anylinuxfs build solely for its user-owned development lock.
This verifies Linux driver/VM image; it does NOT certify macOS NFS or physical hotplug.
"""
import os, pathlib, shutil, subprocess, tempfile
root=pathlib.Path(__file__).resolve().parents[1]
preview_binary=pathlib.Path('/tmp/macntfs-offline-vm-build/target/debug/anylinuxfs')
if not preview_binary.exists():raise SystemExit('Build the debug anylinuxfs target after scripts/build-microvm.py')
with tempfile.TemporaryDirectory(prefix='macntfs-vm-qa-') as temporary:
    directory=pathlib.Path(temporary)
    subprocess.run(['/usr/bin/ditto','--extattr',str(root/'vendor/microvm'),str(directory/'runtime')],check=True)
    binary=directory/'runtime/bin/anylinuxfs'
    binary.unlink();shutil.copy2(preview_binary,binary)
    subprocess.run(['/usr/bin/codesign','--force','--sign','-','--entitlements',str(root/'scripts/microvm.entitlements'),str(binary)],check=True)
    image=directory/'roundtrip.img'
    with image.open('wb') as stream:stream.truncate(32*1024*1024)
    mkntfs='/opt/homebrew/opt/ntfs-3g-mac/sbin/mkntfs'
    subprocess.run([mkntfs,'--fast','--force','-L','MACNTFS-VM-QA',str(image)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    # The host fixture is the only block device assigned to the VM, /dev/vda.
    command='set -eu; uname -m; mkdir -p /tmp/macntfs-qa; ntfs-3g /dev/vda /tmp/macntfs-qa -o rw,norecover; mkdir /tmp/macntfs-qa/nested; echo macntfs-offline-vm > /tmp/macntfs-qa/nested/roundtrip.txt; test "$(cat /tmp/macntfs-qa/nested/roundtrip.txt)" = macntfs-offline-vm; sync; umount /tmp/macntfs-qa; echo MACNTFS_VM_ROUNDTRIP_PASSED'
    result=subprocess.run([str(binary),'shell',str(image),'--command',command],text=True,capture_output=True,timeout=60)
    pathlib.Path('/tmp/macntfs-vm-roundtrip.log').write_text(result.stdout+result.stderr)
    if result.returncode or 'MACNTFS_VM_ROUNDTRIP_PASSED' not in result.stdout:raise SystemExit('Offline guest roundtrip failed; see /tmp/macntfs-vm-roundtrip.log')
    subprocess.run([str(root/'vendor/runtime/bin/ntfs-3g.probe'),'--readwrite',str(image)],check=True)
    print('PASS: offline Linux boot, NTFS nested write/read, sync, guest unmount, host re-probe; no physical disks or root privileges')
