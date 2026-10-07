#!/usr/bin/env python3
"""Roundtrip on a disposable NTFS IMAGE, never a physical disk.
Requires a developer's mkntfs (pass --mkntfs) and previously authorized macFUSE.
Uses the built offline runtime without Homebrew libraries or network requests.
"""
import argparse, os, pathlib, subprocess, tempfile, time
parser=argparse.ArgumentParser()
parser.add_argument('--mkntfs', default='/opt/homebrew/opt/ntfs-3g-mac/sbin/mkntfs')
args=parser.parse_args()
root=pathlib.Path(__file__).resolve().parents[1]
runtime=root/'vendor/runtime/bin'
mount=pathlib.Path(f'/Volumes/macntfs-Test-{os.getpid()}')
if mount.exists(): raise SystemExit('Test mount path already exists')
with tempfile.TemporaryDirectory(prefix='ntfs-desktop-test-') as directory:
    image=pathlib.Path(directory)/'roundtrip.img'
    with image.open('wb') as stream: stream.truncate(32*1024*1024)
    subprocess.run([args.mkntfs,'--fast','--force','-L','NTFS-DESKTOP-TEST',str(image)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
    subprocess.run([str(runtime/'ntfs-3g.probe'),'--readwrite',str(image)],check=True)
    log=pathlib.Path(directory)/'driver.log'
    mounted=False
    with log.open('w') as output:
        child=subprocess.Popen([str(runtime/'ntfs-3g'),str(image),str(mount),'-o',f'rw,norecover,no_detach,windows_names,uid={os.getuid()},gid={os.getgid()}'],stdin=subprocess.DEVNULL,stdout=output,stderr=output)
        try:
            deadline=time.monotonic()+20
            while time.monotonic()<deadline:
                table=subprocess.check_output(['/sbin/mount'],text=True)
                if f'{image} on {mount} ' in table:
                    mounted=True; break
                if child.poll() is not None: break
                time.sleep(0.25)
            if not mounted: raise RuntimeError('No verified mounted filesystem: '+log.read_text())
            test=mount/'offline-roundtrip.txt'
            test.write_text('macntfs offline roundtrip\n')
            if test.read_text()!='macntfs offline roundtrip\n': raise RuntimeError('Readback mismatch')
            subprocess.run(['/usr/sbin/diskutil','unmount',str(mount)],check=True)
            mounted=False
            child.wait(timeout=5)
            subprocess.run([str(runtime/'ntfs-3g.probe'),'--readwrite',str(image)],check=True)
            with image.open('r+b') as stream: stream.write(b'INVALID BOOT SECTOR')
            if subprocess.run([str(runtime/'ntfs-3g.probe'),'--readwrite',str(image)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode==0:
                raise RuntimeError('Invalid NTFS image was accepted')
            print('PASS: probe, mount, write, readback, clean unmount, re-probe, invalid-image rejection')
        finally:
            if mounted:
                result=subprocess.run(['/usr/sbin/diskutil','unmount',str(mount)])
                if result.returncode!=0:
                    # Preserve the active IMAGE and driver instead of interrupting
                    # a writer; no physical user disk is part of this test.
                    raise RuntimeError('Cannot unmount the test image safely')
            if child.poll() is None:
                child.terminate()
                try: child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill();child.wait()  # only our own unmounted disposable IMAGE
