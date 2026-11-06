import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]


class DataManifestTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "tools").mkdir()
        for name in ("rust_data.py", "compare.py"):
            shutil.copy2(ROOT / "tools" / name, self.root / "tools" / name)
        shutil.copytree(ROOT / "tools/rust-data", self.root / "tools/rust-data")
        (self.root / "upstream").mkdir()
        shutil.copy2(ROOT / "upstream/sources.json", self.root / "upstream/sources.json")
        shutil.copytree(ROOT / "data", self.root / "data")
        self.manifest_path = self.root / "data/manifest.json"
        self.manifest = json.loads(self.manifest_path.read_text())

    def check(self, manifest):
        self.manifest_path.write_text(json.dumps(manifest))
        return subprocess.run(
            [sys.executable, str(self.root / "tools/rust_data.py"), "check"],
            capture_output=True, text=True, timeout=10)

    def assert_rejected(self, result, message):
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("rust data failed:", result.stderr)
        self.assertIn(message, result.stderr)
        self.assertNotIn("Traceback", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_unknown_manifest_version_is_rejected(self):
        self.manifest["schema_version"] = 2
        self.assert_rejected(self.check(self.manifest), "unsupported data manifest schema_version")

    def test_manifest_version_requires_an_integer(self):
        for version in (True, False, 1.0, "1", None):
            with self.subTest(version=version):
                self.manifest["schema_version"] = version
                self.assert_rejected(self.check(self.manifest), "unsupported data manifest schema_version")

    def test_manifest_requires_an_object(self):
        for manifest in ([], None, "manifest", 1, True):
            with self.subTest(manifest=manifest):
                self.assert_rejected(self.check(manifest), "data manifest must be an object")

    def test_current_manifest_is_accepted(self):
        result = self.check(self.manifest)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Rust data checked: 11 files", result.stdout)
        self.assertEqual(result.stderr, "")

    def test_missing_manifest_version_is_rejected(self):
        del self.manifest["schema_version"]
        self.assert_rejected(self.check(self.manifest), "unsupported data manifest schema_version")

    def test_schema_is_checked_before_version_specific_fields(self):
        for version in (0, 2):
            with self.subTest(version=version):
                self.assert_rejected(self.check({"schema_version": version}),
                                     "unsupported data manifest schema_version")

    def test_provenance_mismatches_remain_rejected(self):
        for field, message in (
                ("engine_revision", "data was imported at"),
                ("client_build", "data client build"),
                ("tool_sha256", "tools/rust-data changed")):
            with self.subTest(field=field):
                manifest = dict(self.manifest, **{field: "invalid"})
                self.assert_rejected(self.check(manifest), message)

    def test_changed_data_remains_rejected(self):
        name = next(iter(self.manifest["files"]))
        with (self.root / "data" / name).open("a") as data:
            data.write(" ")
        self.assert_rejected(self.check(self.manifest), "differs from its manifest digest")

    def test_missing_data_remains_rejected(self):
        name = next(iter(self.manifest["files"]))
        (self.root / "data" / name).unlink()
        self.assert_rejected(self.check(self.manifest), f"missing data/{name}")

    def test_unlisted_data_remains_rejected(self):
        (self.root / "data/unlisted.json").write_text("{}")
        self.assert_rejected(self.check(self.manifest), "data/unlisted.json is not in the manifest")


if __name__ == "__main__":
    unittest.main()
