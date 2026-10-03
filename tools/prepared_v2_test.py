import json
from pathlib import Path
import shutil
import tempfile
import unittest

from prepared_v2 import FAMILY, capture, check, comparable, leaf_differences


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


class ComparableTests(unittest.TestCase):
    def test_lists_are_keyed_by_id_and_repeats_by_occurrence(self):
        result = {"elapsedNs": 5, "logs": "x", "actions": [
            {"id": {"spellId": 2}, "casts": 1}, {"id": {"spellId": 1}, "casts": 2},
            {"id": {"spellId": 1}, "casts": 3}], "empty": [], "nested": {"none": {}}}
        self.assertEqual(comparable(result), {"actions": {
            '{"spellId": 2}': {"casts": 1}, '{"spellId": 1}': {"casts": 2},
            '{"spellId": 1} #2': {"casts": 3}}})

    def test_map_order_does_not_matter(self):
        go = {"actions": [{"id": {"spellId": 1}, "casts": 1}, {"id": {"spellId": 2}, "casts": 2}]}
        rust = {"actions": list(reversed(go["actions"]))}
        self.assertEqual(leaf_differences(comparable(go), comparable(rust)), [])

    def test_deviations_compare_as_variances(self):
        # A nearly constant series: cancellation moves the deviation, not the variance.
        go, rust = {"avg": 31.822, "stdev": 4.0136e-6}, {"avg": 31.822, "stdev": 4.0179e-6}
        self.assertEqual(leaf_differences(go, rust), [])
        self.assertTrue(leaf_differences({"avg": 600.0, "stdev": 50.0}, {"avg": 600.0, "stdev": 50.01}))

    def test_omitted_deviations_are_zero(self):
        # Go's fused subtraction leaves a residue where Rust reaches zero, which protojson omits.
        go = {"uptimeSecondsAvg": 3.000000000033333, "uptimeSecondsStdev": 2.980230374683199e-08}
        rust = {"uptimeSecondsAvg": 3.000000000033333}
        self.assertEqual(leaf_differences(go, rust), [])
        self.assertTrue(leaf_differences({"avg": 600.0, "stdev": 50.0}, {"avg": 600.0}))


if __name__ == "__main__":
    unittest.main()
