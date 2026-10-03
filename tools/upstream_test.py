import json
from pathlib import Path
import shutil
import tempfile
import unittest

from upstream import LEDGER, check


class UpstreamLedgerTests(unittest.TestCase):
    def copy(self, directory):
        target = Path(directory) / "upstream"
        shutil.copytree(LEDGER, target)
        return target

    def test_accepted_ledger_passes(self):
        ledger, mapping = check()
        self.assertTrue(ledger["changes"] and mapping["mechanics"])

    def test_change_must_link_back_from_its_mechanic(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.copy(directory)
            mapping = json.loads((target / "mechanics-map.json").read_text())
            for entry in mapping["mechanics"]:
                if entry["id"] == "rotation_interpreter":
                    entry["upstream_changes"] = []
            (target / "mechanics-map.json").write_text(json.dumps(mapping))
            with self.assertRaisesRegex(ValueError, "does not link back"):
                check(target)

    def test_applicable_change_needs_a_regression(self):
        with tempfile.TemporaryDirectory() as directory:
            target = self.copy(directory)
            ledger = json.loads((target / "changes.json").read_text())
            for change in ledger["changes"]:
                if change["disposition"] == "applicable":
                    del change["regression"]
            (target / "changes.json").write_text(json.dumps(ledger))
            with self.assertRaisesRegex(ValueError, "needs a regression"):
                check(target)


if __name__ == "__main__":
    unittest.main()
