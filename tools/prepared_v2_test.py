import json
from pathlib import Path
import shutil
import tempfile
import unittest

from prepared_v2 import FAMILY, capture, check


class PreparedFixtureTests(unittest.TestCase):
    def copy_family(self, directory):
        family = Path(directory) / "family"
        shutil.copytree(FAMILY, family)
        return family

    def test_accepted_family_passes(self):
        self.assertTrue(check()["cases"])

    def test_changed_prepared_file_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            family = self.copy_family(directory)
            path = family / "frost-reference.prepared.json"
            prepared = json.loads(path.read_text())
            prepared["player"]["cast_speed"] = 0.9
            path.write_text(json.dumps(prepared))
            with self.assertRaisesRegex(ValueError, "changed prepared file"):
                check(family)

    def test_capture_refuses_accepted_storage(self):
        with self.assertRaisesRegex(ValueError, "scratch storage"):
            capture(Path("unused-cache"), "unused-source", FAMILY / "recapture")


if __name__ == "__main__":
    unittest.main()
