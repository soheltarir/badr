#!/bin/sh
# Exercise a disposable local tap; never contact or publish a remote tap.
set -eu
cd "$(dirname "$0")/.."
version=${1:?Usage: sh scripts/verify-homebrew.sh VERSION ARCHIVE_DIRECTORY}
archives=$(cd "${2:?Archive directory required}" && pwd)
export HOMEBREW_NO_AUTO_UPDATE=1
export HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_INSTALL_CLEANUP=1
if [ -d "$(brew --cellar)/badr" ]; then
  echo 'Refusing to replace an existing BADR installation' >&2; exit 1
fi
work=$(mktemp -d)
tap="badr-local/verification-$$"
formula="$tap/badr"
cleanup() {
  brew uninstall --force --formula "$formula" >/dev/null 2>&1 || :
  brew untap "$tap" >/dev/null 2>&1 || :
  rm -rf "$work"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
mkdir -p "$work/tap/Formula"
python3 scripts/prepare-homebrew.py "$version" "$archives" > "$work/public.rb"
# Only the disposable test copy uses file URLs. The publication copy stays HTTPS.
python3 - "$work/public.rb" "$archives" "$work/tap/Formula/badr.rb" <<'PY'
from pathlib import Path
import re
import sys
source, archives, output = map(Path, sys.argv[1:])
text = re.sub(r'https://github.com/soheltarir/badr/releases/download/v[^/]+',
              lambda _: archives.as_uri(), source.read_text())
output.write_text(text)
PY
git -c init.defaultBranch=main init "$work/tap"
git -C "$work/tap" add Formula/badr.rb
git -C "$work/tap" -c user.name=Test -c user.email=test@example.invalid commit -m 'Local verification tap'
brew tap "$tap" "$work/tap"
local_formula="$(brew --repository "$tap")/Formula/badr.rb"
# A wrong formula checksum must be rejected by Homebrew itself.
cp "$local_formula" "$work/good.rb"
sed 's/sha256 "[0-9a-f]*"/sha256 "0000000000000000000000000000000000000000000000000000000000000000"/' \
  "$work/good.rb" > "$local_formula"
if brew fetch --force "$formula" > "$work/rejected.log" 2>&1; then
  echo 'Homebrew unexpectedly accepted an incorrect checksum' >&2; exit 1
fi
cat "$work/rejected.log"
grep -Ei 'SHA.?256 mismatch|Formula reports different checksum' "$work/rejected.log"
cp "$work/good.rb" "$local_formula"
brew install --formula "$formula"
installed="$(brew --prefix "$formula")/bin/badr"
test "$("$installed" --version)" = "badr $version"
brew test "$formula"
BADR_TEST_BINARY="$installed" cargo test --locked --test runtime
# Revision-only upgrade uses the same real archive, without inventing a release.
sed '/^  version /a\
  revision 1
' "$work/good.rb" > "$local_formula"
brew upgrade --formula "$formula"
test "$(basename "$(cd "$(brew --prefix "$formula")" && pwd -P)")" = "${version}_1"
test "$("$installed" --version)" = "badr $version"
brew test "$formula"
BADR_TEST_BINARY="$installed" cargo test --locked --test runtime
printf 'Homebrew installation, checksum refusal, revision upgrade, and CLI smoke passed.\n'
uname -a
brew --version
