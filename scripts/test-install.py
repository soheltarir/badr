"""Exercise the installer process with disposable archives and mocked downloads."""
import hashlib
import io
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class Installer(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.bin = self.root / 'tools'
        self.bin.mkdir()
        self.dest = self.root / 'install with spaces'
        self.env = dict(os.environ, PATH=f'{self.bin}:{os.environ["PATH"]}',
                        BADR_INSTALL_DIR=str(self.dest), BADR_VERSION='0.1.0',
                        TMPDIR=str(self.root))
        self.tool('uname', 'case "$1" in -s) echo Darwin;; -m) echo arm64;; esac')

    def tool(self, name, body):
        path = self.bin / name
        path.write_text('#!/bin/sh\nset -eu\n' + body + '\n')
        path.chmod(0o755)

    def archive(self, target='aarch64-apple-darwin', binary=b'#!/bin/sh\necho "badr 0.1.0"\n'):
        name = f'badr-0.1.0-{target}'
        archive = self.root / f'{name}.tar.gz'
        with tarfile.open(archive, 'w:gz') as tar:
            info = tarfile.TarInfo(f'{name}/badr')
            info.size = len(binary)
            info.mode = 0o755
            tar.addfile(info, io.BytesIO(binary))
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        Path(f'{archive}.sha256').write_text(f'{digest}  {archive.name}\n')
        return archive

    def run_installer(self, *args):
        return subprocess.run(['sh', str(ROOT / 'install.sh'), *map(str, args)],
                              env=self.env, text=True, capture_output=True)

    def test_local_install_and_upgrade(self):
        archive = self.archive()
        self.dest.mkdir()
        (self.dest / 'badr').write_text('old binary')
        result = self.run_installer('--archive', archive)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(subprocess.check_output([self.dest / 'badr', '--version'], text=True),
                         'badr 0.1.0\n')
        self.assertIn('PATH', result.stdout)
        self.assertEqual(list(self.dest.glob('.badr.*')), [])

    def test_failed_verification_preserves_existing_binary(self):
        self.dest.mkdir()
        installed = self.dest / 'badr'
        installed.write_text('old binary')
        for mode in ('corrupt', 'missing-checksum', 'invalid-checksum', 'wrong-version', 'missing-binary'):
            with self.subTest(mode=mode):
                archive = self.archive(binary=b'#!/bin/sh\necho wrong\n' if mode == 'wrong-version'
                                       else b'#!/bin/sh\necho "badr 0.1.0"\n')
                checksum = Path(f'{archive}.sha256')
                if mode == 'corrupt':
                    archive.write_bytes(b'corrupt')
                elif mode == 'missing-checksum':
                    checksum.unlink()
                elif mode == 'invalid-checksum':
                    checksum.write_text('invalid\n')
                elif mode == 'missing-binary':
                    with tarfile.open(archive, 'w:gz'):
                        pass
                    checksum.write_text(hashlib.sha256(archive.read_bytes()).hexdigest() + '\n')
                result = self.run_installer('--archive', archive)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(installed.read_text(), 'old binary')
                self.assertEqual(list(self.dest.glob('.badr.*')), [])
                self.assertFalse(any(p.is_dir() and p.name.startswith('tmp.') for p in self.root.iterdir()))

    def test_download_platform_matrix_and_latest(self):
        self.tool('curl', '''output=''
url=''
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o) output=$2; shift;;
        https://*) url=$1;;
    esac
    shift
done
case "$url" in
    */latest) printf 'https://github.com/soheltarir/badr/releases/tag/v0.1.0';;
    */v0.1.0/badr-0.1.0-*.tar.gz.sha256) cp "$FIXTURE.sha256" "$output";;
    */v0.1.0/badr-0.1.0-*.tar.gz) cp "$FIXTURE" "$output";;
    *) exit 1;;
esac
printf '%s\\n' "$url" >> "$URL_LOG"
''')
        for system, machine, target in (
            ('Darwin', 'arm64', 'aarch64-apple-darwin'),
            ('Darwin', 'x86_64', 'x86_64-apple-darwin'),
            ('Linux', 'aarch64', 'aarch64-unknown-linux-gnu'),
            ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'),
        ):
            with self.subTest(target=target):
                self.tool('uname', f'case "$1" in -s) echo {system};; -m) echo {machine};; esac')
                self.env['FIXTURE'] = str(self.archive(target))
                self.env['URL_LOG'] = str(self.root / 'urls')
                self.env['BADR_VERSION'] = ''
                result = self.run_installer()
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn(f'/v0.1.0/badr-0.1.0-{target}.tar.gz\n',
                              (self.root / 'urls').read_text())
        self.env['BADR_VERSION'] = 'v0.1.0'
        (self.root / 'urls').unlink()
        self.assertEqual(self.run_installer().returncode, 0)
        self.assertNotIn('/latest', (self.root / 'urls').read_text())
        self.tool('curl', 'exit 22')
        self.assertNotEqual(self.run_installer().returncode, 0)
        self.assertTrue((self.dest / 'badr').is_file())

    def test_refuses_unsupported_host_arguments_and_unsafe_destination(self):
        archive = self.archive()
        for args in (('--unknown',), ('--archive',), ('--archive', ''), ('--archive', self.root / 'wrong.tar.gz')):
            self.assertNotEqual(self.run_installer(*args).returncode, 0)
        self.dest.mkdir()
        protected = self.root / 'protected'
        protected.write_text('keep')
        (self.dest / 'badr').symlink_to(protected)
        self.assertNotEqual(self.run_installer('--archive', archive).returncode, 0)
        self.assertEqual(protected.read_text(), 'keep')
        (self.dest / 'badr').unlink()
        (self.dest / 'badr').mkdir()
        self.assertNotEqual(self.run_installer('--archive', archive).returncode, 0)
        for response in ('FreeBSD', 'riscv64'):
            self.tool('uname', f'echo {response}')
            self.assertNotEqual(self.run_installer('--archive', archive).returncode, 0)


if __name__ == '__main__':
    unittest.main()
