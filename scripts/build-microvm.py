#!/usr/bin/env python3
"""Development-only builder. End-user runtime never downloads or initializes images.
Pinned anylinuxfs source is GPL-3.0; our host isolation patch is distributed alongside it.
Build dependencies: Rust, Homebrew util-linux/gettext/llvm, pkg-config.
"""
import hashlib, json, os, pathlib, shutil, subprocess, tarfile
ROOT = pathlib.Path(__file__).resolve().parents[1]
COMMIT = '8aa9ccd6504e64ca26ce769c1623ed1741c6b7d3'
SOURCE_SHA = '27ee90647b6da372a0d052282cb6322c1db73302c4d1d1abb57abcc21f652a61'
REFERENCE_SHA = 'd46e3c274250e55427149cf38dfa42f915d74c8b3262630e326d6610032f6ea6'
ALPINE_DIGEST = 'd858bb5442632a31bd4bca6c5e601dbe6b536fd7942092ea6a08a0a95805693c'
BASE = 'macntfs-build-v1-d858bb544263'
BUILD = pathlib.Path('/tmp/macntfs-offline-vm-build')
OUT = ROOT/'vendor/microvm'

def run(*args, **kw): subprocess.run([str(a) for a in args], check=True, **kw)
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def fetch(name, url, digest):
    path = ROOT/'vendor/downloads'/name
    if not path.exists(): run('/usr/bin/curl', '--fail', '--location', '--proto', '=https', '--tlsv1.2', url, '-o', path)
    if sha(path) != digest: raise SystemExit('Pinned input checksum mismatch: '+name)
    return path

def replace(path, old, new):
    text = path.read_text()
    if text.count(old) != 1: raise SystemExit('Source patch marker changed: '+str(path)+' '+old[:70])
    path.write_text(text.replace(old,new,1))

def main():
    BUILD.mkdir(parents=True, exist_ok=True)
    source = fetch('anylinuxfs-8aa9ccd.tar.gz', f'https://github.com/nohajc/anylinuxfs/archive/{COMMIT}.tar.gz', SOURCE_SHA)
    image = fetch('ntfsmac-reference-v3.1.dmg', 'https://github.com/khr898/ntfsmac/releases/download/v3.1/ntfsmac-gui-v3.1.160926.dmg', REFERENCE_SHA)
    src = BUILD/('anylinuxfs-'+COMMIT)
    if src.exists(): shutil.rmtree(src)
    with tarfile.open(source) as archive:
        for entry in archive.getmembers():
            if entry.issym() or entry.islnk() or not (BUILD/entry.name).resolve().is_relative_to(BUILD.resolve()): raise SystemExit('Unsafe source archive')
        archive.extractall(BUILD)
    # Root-owned application profile; never load the desktop user's config/custom actions.
    main_rs = src/'anylinuxfs/src/main.rs'
    replace(main_rs, 'let profile_path = home_dir.join(".anylinuxfs");', 'let profile_path = prefix_dir.join("profile");')
    replace(main_rs, 'let config_file_path = home_dir.join(".anylinuxfs").join("config.toml");', 'let config_file_path = prefix_dir.join("etc/disabled-user-config.toml");')
    replace(main_rs, 'let log_dir = home_dir.join("Library").join("Logs");', 'let log_dir = profile_path.join("logs");\n    fs::create_dir_all(&log_dir)?;')
    replace(main_rs, 'pub(crate) const LOCK_FILE: &str = "/tmp/anylinuxfs.lock";', '''#[cfg(not(debug_assertions))]
pub(crate) const LOCK_FILE: &str = "/var/run/com.macntfs.microvm.lock";
#[cfg(debug_assertions)]
pub(crate) const LOCK_FILE: &str = "/tmp/com.macntfs.microvm-preview.lock";''')
    cmd_mount = src/'anylinuxfs/src/cmd_mount.rs'
    replace(cmd_mount, 'fs::read_dir("/tmp")', 'fs::read_dir("/var/run")')
    replace(cmd_mount, 'filename.starts_with("anylinuxfs")', 'filename.starts_with("com.macntfs.vm-")')
    replace(cmd_mount, 'format!("/tmp/anylinuxfs-{}.sock", rand_string(8))', 'format!("/var/run/com.macntfs.vm-{}.sock", rand_string(8))')
    replace(cmd_mount, 'mdns::register_vm_record(&config.vm_hostname, net_helper_svc.vm_host_ip)?', 'mdns::register_vm_record(&config.vm_hostname, net_helper_svc.vm_host_ip.clone())?')
    replace(cmd_mount, 'let vm_host_b = vm_host.to_string().into_bytes();', 'let vm_host_b = net_helper_svc.vm_host_ip.to_string().into_bytes();')
    replace(cmd_mount, 'if *changed_to_ro {', 'if *changed_to_ro {\n                    config.nfs_options.push("ro".into());')
    # Refuse automatic provisioning. Missing/damaged offline payload is repaired by a PKG.
    vm_image = src/'anylinuxfs/src/vm_image.rs'
    replace(vm_image, 'host_println!("Initializing VM root filesystem...");', 'anyhow::bail!("Offline microVM image missing or incompatible; reinstall the complete macntfs PKG");\n        #[allow(unreachable_code)]\n        let _offline_only = ();\n        host_println!("Initializing VM root filesystem...");')
    (src/'share/alpine/rootfs.ver').write_text('macntfs-microvm-v1')
    # Only static libblkid dependencies; no Homebrew dylib may reach the package.
    stage = BUILD/'static'
    if stage.exists(): shutil.rmtree(stage)
    stage.mkdir()
    (stage/'lib/pkgconfig').mkdir(parents=True,exist_ok=True)
    brew_util = pathlib.Path(subprocess.check_output(['brew','--prefix','util-linux'],text=True).strip())
    brew_intl = pathlib.Path(subprocess.check_output(['brew','--prefix','gettext'],text=True).strip())
    for name, owner in [('libblkid.a',brew_util),('libuuid.a',brew_util),('libintl.a',brew_intl)]: shutil.copy2(owner/'lib'/name,stage/'lib'/name)
    for name in ['blkid','uuid']: shutil.copytree(brew_util/'include'/name,stage/'include'/name,dirs_exist_ok=True)
    shutil.copy2(brew_intl/'include/libintl.h',stage/'include/libintl.h')
    for name, lib, deps in [('blkid','blkid','uuid intl'),('uuid','uuid',''),('intl','intl','')]:
        (stage/f'lib/pkgconfig/{name}.pc').write_text(f'prefix={stage}\nName: {name}\nDescription: static {name}\nVersion: 2.42\nLibs: -L${{prefix}}/lib -l{lib}\nCflags: -I${{prefix}}/include -I${{prefix}}/include/blkid\nRequires.private: {deps}\nLibs.private: '+('-liconv -framework CoreFoundation' if name=='intl' else '-lpthread')+'\n')
    env = os.environ.copy(); env.update(PKG_CONFIG_PATH=str(stage/'lib/pkgconfig'),PKG_CONFIG_ALL_STATIC='1',CARGO_TARGET_DIR=str(BUILD/'target'),PATH='/opt/homebrew/opt/lld/bin:/opt/homebrew/opt/llvm/bin:'+env.get('PATH',''),LIBCLANG_PATH='/opt/homebrew/opt/llvm/lib')
    run('cargo','build','--release','--locked',cwd=src/'anylinuxfs',env=env)
    import plistlib
    attached = plistlib.loads(subprocess.check_output(['/usr/bin/hdiutil','attach','-readonly','-nobrowse','-plist',str(image)]))
    mount = pathlib.Path(next(e['mount-point'] for e in attached['system-entities'] if 'mount-point' in e))
    try:
        app = mount/'ntfsmac.app'
        run('/usr/bin/codesign','--verify','--deep','--strict',app)
        vendor = app/'Contents/Resources/cli-src/vendor'
        bootstrap = BUILD/'bootstrap'; (bootstrap/'libexec').mkdir(parents=True,exist_ok=True); (bootstrap/'lib').mkdir(exist_ok=True)
        for name in ['init-rootfs','vmproxy']: shutil.copy2(vendor/'bin'/name,bootstrap/'libexec'/name)
        shutil.copy2(vendor/'kernel/Image',bootstrap/'libexec/Image')
        shutil.copy2(vendor/'kernel/modules.squashfs',bootstrap/'lib/modules.squashfs')
        cache = pathlib.Path.home()/'.anylinuxfs'/BASE/'rootfs'
        if not (cache/'usr/sbin/rpc.nfsd').exists():
            run(bootstrap/'libexec/init-rootfs','-base-dir',BASE,'-docker-ref','docker.io/library/alpine@sha256:'+ALPINE_DIGEST)
        if OUT.exists(): shutil.rmtree(OUT)
        (OUT/'bin').mkdir(parents=True); (OUT/'libexec').mkdir(); (OUT/'etc').mkdir(); (OUT/'profile/alpine').mkdir(parents=True)
        shutil.copy2(BUILD/'target/release/anylinuxfs',OUT/'bin/anylinuxfs')
        for name in ['gvproxy','vmproxy']: shutil.copy2(vendor/'bin'/name,OUT/'libexec'/name)
        shutil.copy2(vendor/'kernel/Image',OUT/'libexec/Image')
        shutil.copy2(vendor/'kernel/Image-4K',OUT/'libexec/Image-4K')
        run('/usr/bin/ditto','--extattr',cache,OUT/'profile/alpine/rootfs')
        shutil.copy2(OUT/'libexec/vmproxy',OUT/'profile/alpine/rootfs/vmproxy')
        run('/usr/bin/xattr','-w','user.containers.override_stat','0:0:0755',OUT/'profile/alpine/rootfs/vmproxy')
        (OUT/'profile/alpine/rootfs.ver').write_text('macntfs-microvm-v1')
        (OUT/'etc/anylinuxfs.toml').write_text('[images.offline]\nbase_dir = "alpine"\nos_type = "Linux"\n[images.offline.kernel]\n[linux]\ndefault_image = "offline"\n[network]\nhelper = "gvproxy"\n[krun]\nnum_vcpus = 2\nram_size_mib = 512\n')
        for needed in ['bin/bash','usr/sbin/rpc.nfsd','usr/local/bin/entrypoint.sh','vmproxy','etc/fstab']:
            if not (OUT/'profile/alpine/rootfs'/needed).exists(): raise SystemExit('Offline rootfs incomplete: '+needed)
        # Host-accessible guest files are immutable; guest /tmp is mounted separately.
        for parent, dirs, files in os.walk(OUT):
            for name in dirs+files:
                path=pathlib.Path(parent)/name
                if not path.is_symlink(): path.chmod((path.stat().st_mode & ~0o022) | (0o700 if path.is_dir() else 0o600))
        (OUT/'SOURCE.json').write_text(json.dumps({'anylinuxfs_commit':COMMIT,'reference_release':'khr898/ntfsmac v3.1','reference_sha256':REFERENCE_SHA,'alpine_digest':'sha256:'+ALPINE_DIGEST,'network':'gvproxy-loopback','experimental':True},indent=2)+'\n')
        print('Offline microVM payload ready:',OUT)
    finally: run('/usr/bin/hdiutil','detach',mount)
if __name__=='__main__': main()
