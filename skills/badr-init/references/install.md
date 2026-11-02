# Release installation when BADR is absent

Use the existing checksum-verifying installer. Check the host OS/architecture:
supported release targets are macOS and GNU/Linux, each on ARM64 or x86_64.
Report other environments, including non-GNU Linux, as unsupported by this
installation workflow. A previously working executable may still be reused.

Require Git for BADR, and `sh`, `uname`, `curl`, `tar`, `awk`, `mktemp`, `mkdir`,
`cp`, `chmod`, `mv`, `rm`, and either `sha256sum` or `shasum` for installation.
Report missing tools; leave their installation to the user.

Explain the destination (`~/.local/bin` by default), then use the release
installer within the user's setup authorization and host permissions:

```sh
curl -fsSL https://raw.githubusercontent.com/soheltarir/badr/main/install.sh | sh
```

`BADR_INSTALL_DIR` can select a user-chosen writable directory; `BADR_VERSION`
can select a user-requested version. Do not inherit an unexpected destination
or version silently. Do not use sudo or alter shell startup files. If a binary
already exists at the destination, return to the existing-installation decision
instead of letting the installer replace it automatically.

The installer downloads the matching release, checks SHA-256 and the binary's
reported version, and installs the binary. A missing release, unavailable network,
checksum failure, or failed version check is a blocker; never bypass it or fall
back to a source build/package manager. Verify the installed absolute executable
with `--version` and `--help`. If its directory is outside PATH, use that absolute
path for this session and tell the user where it is.
