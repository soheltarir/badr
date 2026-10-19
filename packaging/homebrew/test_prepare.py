"""Public generator boundary: release artifacts in, publishable formula out."""
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
TARGETS = ("aarch64-apple-darwin", "x86_64-apple-darwin",
           "aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu")


class PrepareTap(unittest.TestCase):
    def test_checks_all_archives_before_emitting_formula(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for target in TARGETS:
                archive = root / f"badr-0.1.0-{target}.tar.gz"
                archive.write_bytes(target.encode())
                digest = hashlib.sha256(archive.read_bytes()).hexdigest()
                archive.with_suffix(".gz.sha256").write_text(f"{digest}  {archive.name}\n")
            command = ["python3", str(ROOT / "scripts/prepare-homebrew.py"), "0.1.0", str(root)]
            good = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(good.returncode, 0, good.stderr)
            for target in TARGETS:
                self.assertIn(f"badr-0.1.0-{target}.tar.gz", good.stdout)
                self.assertIn(hashlib.sha256(target.encode()).hexdigest(), good.stdout)
            (root / "badr-0.1.0-x86_64-unknown-linux-gnu.tar.gz").write_bytes(b"corrupt")
            bad = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(bad.returncode, 0)
            self.assertEqual(bad.stdout, "")
            self.assertIn("checksum", bad.stderr.lower())


    def test_missing_artifacts_and_invalid_versions_emit_no_formula(self):
        with tempfile.TemporaryDirectory() as directory:
            for version in ("0.1.0", '0.1.0"; system("unexpected")'):
                result = subprocess.run(
                    ["python3", str(ROOT / "scripts/prepare-homebrew.py"), version, directory],
                    capture_output=True, text=True,
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")


if __name__ == "__main__":
    unittest.main()
