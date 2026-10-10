# Experimental microVM backend: attribution and source availability

macntfs's application code remains MIT. The optional MicroVM backend runs separate
GPL-licensed `anylinuxfs` and Linux components; their original licenses remain in
force. This backend borrows the anylinuxfs/libkrun architecture documented by
[khr898/ntfsmac](https://github.com/khr898/ntfsmac), without copying its GUI or
privileged helper. Our Rust helper retains its own authenticated request boundary.

Pinned inputs:

- anylinuxfs (GPL-3.0): `8aa9ccd6504e64ca26ce769c1623ed1741c6b7d3`.
  Original source archive is included in Resources/Sources. Our deterministic
  profile, private socket, numeric loopback mount and offline-only changes are in
  `scripts/build-microvm.py`, included with macntfs source.
- Guest vmproxy and gvproxy: extracted from khr898/ntfsmac release v3.1,
  source commit `65ea3caabda07f12c1eeb3e06af927c8a4a3585a`.
  The source archive and build scripts identify its anylinuxfs pin and guest build.
  Reference DMG SHA256:
  `d46e3c274250e55427149cf38dfa42f915d74c8b3262630e326d6610032f6ea6`.
- libkrun 1.19.3 (Apache-2.0), and other Rust dependencies: exact versions and
  checksums are in anylinuxfs/Cargo.lock. The corresponding-source release archive includes the vendored host and guest Rust dependencies.
- Linux 6.12.62 (GPL-2.0): kernel files from nohajc/libkrunfw v6.12.62-rev1,
  commit `9fe60c621c3dce85680274262c1be90046dbd6fc`; upstream kernel and its
  corresponding configuration/patches are cached locally for the corresponding-source archive.
- gvproxy / gvisor-tap-vsock 0.8.9 (Apache-2.0), source commit
  `9cfc86f66679ef0feed0f20ba1df558fe2bef5c6`.
- Static libblkid/libuuid: Homebrew util-linux 2.42.4 (component licenses apply).
  Static libintl: Homebrew gettext 1.0 (LGPL); corresponding sources are cached locally and must accompany a public binary release.
- Alpine 3.23.5 base image (per-package licenses): arm64 manifest digest
  `sha256:d858bb5442632a31bd4bca6c5e601dbe6b536fd7942092ea6a08a0a95805693c`.
  The provisioned guest package database is retained in the offline payload.
  Package versions and Alpine build-recipe commits are retained in the guest database.
  The release archive includes all 44 installed package origins, their exact
  aports recipes and SHA-512-verified source inputs.

The runtime contains a fully provisioned root filesystem. No initializer is
shipped or invoked at runtime. A damaged/missing image causes a repair message,
not a network download. Development-time image provisioning needs network access;
package versions are recorded, and installed files are sealed by SHA256 manifests.
Rebuilding against changing package repositories is not claimed to be byte-for-byte
reproducible.

Only external, physical NTFS partitions validated by macntfs can reach this
backend. Other upstream filesystem types, encryption, custom actions, shell
commands and network sharing are not exposed by the macntfs helper.

NFS listeners use explicitly selected loopback addresses, rather than vmnet/PF.
The backend does not change global firewall rules, VPN routes, SIP or startup
security policy. Loopback does not isolate a volume from other local user processes.

Experimental limitations: network-volume semantics, upstream Word compatibility
and large NTFS file-batch I/O issues; a 512 MiB memory limit per mounted partition.
This mode is manual only and never silently replaces the stable kernel backend.
