#!/usr/bin/env python3
"""Compare write sizes on an 8 GiB sparse NTFS image; never use a physical disk.
Results include fsync and SHA-256 readback, but remain host-cache-sensitive.
Requires authorized macFUSE and a developer mkntfs installation.
"""
import os, pathlib, tempfile, subprocess, time, hashlib, json, shutil
root=pathlib.Path(__file__).resolve().parents[1]
d=pathlib.Path(tempfile.mkdtemp(prefix='macntfs-perf-image-')); image=d/'test.img'; mount=pathlib.Path(f'/Volumes/macntfs-Perf-{os.getpid()}'); results=[]
if mount.exists(): raise SystemExit('Test mount path already exists')
child=None
with image.open('xb') as f:f.truncate(8*1024**3)
subprocess.run(['/opt/homebrew/opt/ntfs-3g-mac/sbin/mkntfs','--fast','--force',str(image)],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,check=True)
try:
 for label,extra in [(f'{mode}_{i}',extra) for i in range(3) for mode,extra in [('baseline',''),('large_io',',big_writes,iosize=1048576')]]:
  with (d/(label+'.log')).open('w') as log:
   child=subprocess.Popen([str(root/'vendor/runtime/bin/ntfs-3g'),str(image),str(mount),'-o',f'rw,norecover,no_detach,windows_names,uid={os.getuid()},gid={os.getgid()}{extra}'],stdout=log,stderr=log)
   try:
    deadline=time.monotonic()+20
    while f'{image} on {mount} ' not in subprocess.check_output(['/sbin/mount'],text=True):
     if child.poll() is not None or time.monotonic()>deadline:raise RuntimeError((d/(label+'.log')).read_text())
     time.sleep(.25)
    for block in [1048576]:
     data=os.urandom(block);size=0;expected=hashlib.sha256();p=mount/'perf.bin';start=time.monotonic()
     with p.open('wb',buffering=0) as f:
      while size<1024*1048576 and time.monotonic()-start<10:
       v=memoryview(data)
       while v:
        count=f.write(v);expected.update(v[:count]);size+=count;v=v[count:]
      write=time.monotonic()-start;os.fsync(f.fileno());total=time.monotonic()-start
     actual=hashlib.sha256()
     with p.open('rb') as f:
      for chunk in iter(lambda:f.read(1048576),b''):actual.update(chunk)
     assert actual.digest()==expected.digest()
     p.unlink();r={'mode':label,'block_KiB':block//1024,'MiB':size/1048576,'write_MiBs':round(size/1048576/write,2),'with_fsync_MiBs':round(size/1048576/total,2),'verified':True};results.append(r);print(r,flush=True)
   finally:
    if f'{image} on {mount} ' in subprocess.check_output(['/sbin/mount'],text=True):subprocess.run(['/usr/sbin/diskutil','unmount',str(mount)],check=True)
    child.wait(timeout=10)
  subprocess.run([str(root/'vendor/runtime/bin/ntfs-3g.probe'),'--readwrite',str(image)],check=True)
 print(json.dumps({'results':results},indent=2),flush=True)
finally:
 if (child is None or child.poll() is not None) and f'{image} on ' not in subprocess.check_output(['/sbin/mount'],text=True):shutil.rmtree(d)
 else:print('Preserved active fixture:',d)
