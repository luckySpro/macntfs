#!/usr/bin/env python3
"""Inject EPERM on a disposable file only; never access a physical disk."""
import os,pathlib,subprocess,tempfile
root=pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='ntfs-probe-permission-') as temporary:
    folder=pathlib.Path(temporary)
    source=folder/'deny.c'
    source.write_text(r'''
#include <errno.h>
#include <fcntl.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static int denied_open(const char *path, int flags, ...) {
    const char *denied = getenv("NTFS_TEST_DENIED_PATH");
    if (denied && strcmp(path, denied) == 0) { errno = EPERM; return -1; }
    mode_t mode = 0;
    if (flags & O_CREAT) { va_list args; va_start(args, flags); mode = va_arg(args, int); va_end(args); }
    return open(path, flags, mode);
}
__attribute__((used)) static struct { const void *replacement; const void *original; }
interpose __attribute__((section("__DATA,__interpose"))) = { (const void *)denied_open, (const void *)open };
''')
    library=folder/'deny.dylib'
    subprocess.run(['/usr/bin/xcrun','clang','-dynamiclib',str(source),'-o',str(library)],check=True)
    image=folder/'disposable.img';image.write_bytes(bytes(4096))
    probe=root/'vendor/runtime/bin/ntfs-3g.probe'
    control=subprocess.run([str(probe),'--readwrite',str(image)],capture_output=True,text=True)
    assert control.returncode==12,control.stderr
    env=dict(os.environ,DYLD_INSERT_LIBRARIES=str(library),NTFS_TEST_DENIED_PATH=str(image))
    denied=subprocess.run([str(probe),'--readwrite',str(image)],env=env,capture_output=True,text=True)
    assert denied.returncode==19,(denied.returncode,denied.stderr)
    assert 'Operation not permitted' in denied.stderr,denied.stderr
    print('PASS: raw-open EPERM maps to access failure (19), not hibernation (14)')
