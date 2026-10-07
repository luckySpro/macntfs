#!/bin/bash
# Development-time build only. The resulting installer never runs this script.
set -euo pipefail
cd "$(dirname "$0")/.."
ntfs_project="$PWD"
ntfs_source="$PWD/vendor/build/ntfs-3g-2026.9.28"
ntfs_output="$PWD/vendor/runtime"
export PATH="/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export MACOSX_DEPLOYMENT_TARGET=12.0
export PKG_CONFIG_PATH=/usr/local/lib/pkgconfig
[ -f "$ntfs_source/configure" ] || (cd "$ntfs_source" && ./autogen.sh)
cd "$ntfs_source"
# Apply our documented macOS-only error mapping; preserve hibernation checks.
if patch --dry-run -p1 < "$ntfs_project/scripts/patches/macos-device-permission.patch" >/dev/null 2>&1; then
  patch -p1 < "$ntfs_project/scripts/patches/macos-device-permission.patch"
else
  patch --dry-run -R -p1 < "$ntfs_project/scripts/patches/macos-device-permission.patch" >/dev/null
fi
./configure --prefix='/Library/Application Support/NTFS Desktop/Runtime' \
  --with-fuse=external --disable-crypto --disable-ntfsprogs --disable-plugins \
  --disable-library --disable-shared --enable-static > "$ntfs_project/vendor/build/configure.log" 2>&1
make clean > /dev/null 2>&1
make -j4 > "$ntfs_project/vendor/build/make.log" 2>&1
mkdir -p "$ntfs_output/bin" "$ntfs_output/lib"
cp src/ntfs-3g src/ntfs-3g.probe "$ntfs_output/bin/"
# Keep the vendor-signed library unchanged, in our root-owned runtime directory.
# /usr/local/lib can be user-owned on machines with Homebrew, so never load an
# elevated process from that shared directory.
cp "$ntfs_project/vendor/core-payload/usr/local/lib/libfuse.2.dylib" "$ntfs_output/lib/"
/usr/bin/install_name_tool -change /usr/local/lib/libfuse.2.dylib '@executable_path/../lib/libfuse.2.dylib' "$ntfs_output/bin/ntfs-3g"
# The portable executables depend only on Apple system libraries and macFUSE.
# Reject accidentally picking up any development-machine library.
for ntfs_binary in "$ntfs_output/bin/ntfs-3g" "$ntfs_output/bin/ntfs-3g.probe"; do
  if /usr/bin/otool -L "$ntfs_binary" | /usr/bin/tail -n +2 | /usr/bin/awk '{print $1}' | /usr/bin/grep -Ev '^(/usr/lib/|/System/Library/|@executable_path/../lib/libfuse\.2\.dylib$)' > "$ntfs_project/vendor/build/unexpected-libraries.txt"; then
    cat "$ntfs_project/vendor/build/unexpected-libraries.txt" >&2
    exit 1
  fi
  /usr/bin/codesign --force --sign - "$ntfs_binary"
done
cp COPYING "$ntfs_project/vendor/licenses/ntfs-3g-GPL.txt"
cp COPYING.LIB "$ntfs_project/vendor/licenses/ntfs-3g-LGPL.txt"
