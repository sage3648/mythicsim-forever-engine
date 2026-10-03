import json
from pathlib import Path
import unittest

from sweep import generate, lower_talents

BASE = json.loads((Path(__file__).resolve().parents[1] / "fixtures" / "mage" / "prepared-v2"
                   / "arcane-reference.request.json").read_text())


class SweepTests(unittest.TestCase):
    def test_same_seed_same_variants(self):
        self.assertEqual(generate(BASE, 7, 3), generate(BASE, 7, 3))
        self.assertNotEqual(generate(BASE, 7, 3), generate(BASE, 8, 3))

    def test_talent_ranks_only_fall(self):
        import random
        rng = random.Random(1)
        base = "055205003100311531-230500001-005"
        for _ in range(50):
            lowered = lower_talents(base, rng)
            self.assertEqual([len(tree) for tree in lowered.split("-")], [len(tree) for tree in base.split("-")])
            self.assertTrue(all(int(a) <= int(b) for a, b in zip(lowered.replace("-", ""), base.replace("-", ""))))

    def test_base_request_is_not_modified(self):
        before = json.dumps(BASE, sort_keys=True)
        generate(BASE, 3, 5)
        self.assertEqual(json.dumps(BASE, sort_keys=True), before)


if __name__ == "__main__":
    unittest.main()
