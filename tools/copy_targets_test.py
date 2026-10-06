import json
from pathlib import Path
import tempfile
import unittest

from copy_targets import copied, tank_assigned, write

FAMILY = Path(__file__).resolve().parents[1] / "fixtures" / "mage" / "prepared-v2"
TANK = FAMILY / "production-protection-paladin.request.json"
UNTANKED = FAMILY / "arcane-reference.request.json"


class CopyTargetsTests(unittest.TestCase):
    def test_the_boss_is_copied_and_nothing_else_changes(self):
        request = json.loads(TANK.read_text())
        several = copied(request, 3)
        targets = several["encounter"]["targets"]
        self.assertEqual(len(targets), 3)
        self.assertTrue(all(target == request["encounter"]["targets"][0] for target in targets))
        self.assertIsNot(targets[0], targets[1])
        several["encounter"]["targets"] = request["encounter"]["targets"]
        self.assertEqual(several, request)

    def test_tank_assignments_are_picked_out(self):
        self.assertTrue(tank_assigned(json.loads(TANK.read_text())))
        self.assertFalse(tank_assigned(json.loads(UNTANKED.read_text())))

    def test_files_follow_the_reference_record_format(self):
        recorded = Path(__file__).resolve().parents[1] / "validation" / "2026-10-06-multi-target-paladin-druid-references"
        with tempfile.TemporaryDirectory() as scratch:
            written = write([TANK, UNTANKED], [2, 3], Path(scratch), tanks_only=True)
            self.assertEqual([path.name for path in written],
                             ["production-protection-paladin-2-targets.request.json",
                              "production-protection-paladin-3-targets.request.json"])
            same = recorded / "protection-paladin-2-targets.request.json"
            self.assertEqual(written[0].read_text(), same.read_text())
            plain = write([UNTANKED], [2], Path(scratch) / "plain")
            self.assertEqual(len(plain), 1)

    def test_a_request_with_several_targets_is_skipped(self):
        with tempfile.TemporaryDirectory() as scratch:
            first = write([TANK], [2], Path(scratch) / "first")
            self.assertEqual(write(first, [3], Path(scratch) / "second"), [])


if __name__ == "__main__":
    unittest.main()
