#!/bin/sh
# Install a checksum-verified BADR archive without elevated privileges.
set -eu

fail() { printf 'badr installer: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "requires $1"; }

main() {
    archive=''
    case "$#" in
        0) ;;
        2) [ "$1" = --archive ] || fail 'usage: sh install.sh [--archive PATH]'; archive=$2; [ -n "$archive" ] || fail 'archive path must not be empty' ;;
        *) fail 'usage: sh install.sh [--archive PATH]' ;;
    esac
    case "$(uname -s)" in
        Darwin) platform=apple-darwin ;;
        Linux) platform=unknown-linux-gnu ;;
        *) fail 'supported systems are macOS and GNU/Linux' ;;
    esac
    case "$(uname -m)" in
        arm64|aarch64) arch=aarch64 ;;
        x86_64|amd64) arch=x86_64 ;;
        *) fail 'supported architectures are ARM64 and x86_64' ;;
    esac
    target=$arch-$platform
    for tool in tar awk mktemp mkdir cp chmod mv rm; do need "$tool"; done
    if command -v sha256sum >/dev/null 2>&1; then
        checksum=sha256sum
    else
        need shasum
        checksum=shasum
    fi
    tmp=$(mktemp -d)
    staged=''
    trap 'rm -rf "$tmp"; [ -z "$staged" ] || rm -f "$staged"' 0
    trap 'exit 1' HUP INT TERM
    if [ -n "$archive" ]; then
        name=${archive##*/}
        name=${name%.tar.gz}
        cp "$archive" "$tmp/archive.tar.gz"
        cp "$archive.sha256" "$tmp/checksum"
        version=${name#badr-}
        version=${version%-$target}
    else
        need curl
        version=${BADR_VERSION:-}
        if [ -z "$version" ]; then
            release=$(curl -fsSLI --retry 3 --connect-timeout 10 --max-time 30 \
                -o /dev/null -w '%{url_effective}' https://github.com/soheltarir/badr/releases/latest) \
                || fail 'no public release available; use --archive PATH for a reviewed local candidate'
            version=${release##*/}
        fi
        version=${version#v}
    fi
    case "$version" in
        ''|*[!0-9A-Za-z.+-]*) fail 'invalid release version or archive name' ;;
    esac
    name=badr-$version-$target
    if [ -n "$archive" ]; then
        [ "${archive##*/}" = "$name.tar.gz" ] || fail "expected archive $name.tar.gz for this host"
    else
        url=https://github.com/soheltarir/badr/releases/download/v$version/$name.tar.gz
        curl -fsSL --retry 3 --connect-timeout 10 --max-time 120 "$url" -o "$tmp/archive.tar.gz" \
            || fail 'archive download failed; releases may not be published yet'
        curl -fsSL --retry 3 --connect-timeout 10 --max-time 30 "$url.sha256" -o "$tmp/checksum" \
            || fail 'checksum download failed'
    fi
    # Parse only the digest; never trust a downloaded checksum's filename as a path.
    expected=$(awk 'NR == 1 { print $1 }' "$tmp/checksum")
    case "$expected" in ''|*[!0-9a-f]*) fail 'invalid SHA-256 checksum' ;; esac
    [ "${#expected}" -eq 64 ] || fail 'invalid SHA-256 checksum'
    case "$checksum" in
        sha256sum) actual=$(sha256sum < "$tmp/archive.tar.gz" | awk '{print $1}') ;;
        shasum) actual=$(shasum -a 256 < "$tmp/archive.tar.gz" | awk '{print $1}') ;;
    esac
    [ "$actual" = "$expected" ] || fail 'archive checksum mismatch'
    # Extract only the binary to stdout, so archive paths cannot write elsewhere.
    tar -xzOf "$tmp/archive.tar.gz" "$name/badr" > "$tmp/badr"
    chmod 755 "$tmp/badr"
    [ "$("$tmp/badr" --version)" = "badr $version" ] || fail 'binary version check failed'
    install_dir=${BADR_INSTALL_DIR:-$HOME/.local/bin}
    mkdir -p "$install_dir"
    [ ! -d "$install_dir/badr" ] && [ ! -L "$install_dir/badr" ] \
        || fail 'destination badr is a directory or symlink; remove it explicitly first'
    # Stage on the destination filesystem so replacement is one rename.
    staged=$(mktemp "$install_dir/.badr.XXXXXXXX")
    cp "$tmp/badr" "$staged"
    chmod 755 "$staged"
    mv -f "$staged" "$install_dir/badr"
    staged=''
    printf 'Installed badr %s to %s/badr\n' "$version" "$install_dir"
    case ":$PATH:" in
        *":$install_dir:"*) ;;
        *) printf 'Add %s to PATH to run badr.\n' "$install_dir" ;;
    esac
}

# Keep piped installation from running a partially downloaded script.
main "$@"
