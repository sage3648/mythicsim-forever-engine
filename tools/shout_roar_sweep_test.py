import sys
import tempfile
import unittest
from pathlib import Path

import shout_roar_sweep as sweep


def cast_spells(request):
    """The spell IDs the request's rotation casts."""
    items = request["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]
    return [item["action"].get("castSpell", {}).get("spellId", {}).get("spellId") for item in items]


class ShoutRoarSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_challenging_shout_row_casts_it(self):
        found = 0
        for name, base, *_ in sweep.rows():
            if "challenging-shout" in name:
                self.assertEqual(base["raid"]["parties"][0]["players"][0]["class"], "ClassWarrior", name)
                self.assertIn(sweep.CHALLENGING_SHOUT, cast_spells(base), name)
                found += 1
        self.assertEqual(found, len(sweep.CHALLENGING_BASES) + len(sweep.SHOUT_FIXTURES))

    def test_the_shout_refresh_rows_ask_the_aura(self):
        found = 0
        for name, base, *_ in sweep.rows():
            if "shout-refresh" not in name:
                continue
            items = base["raid"]["parties"][0]["players"][0]["rotation"]["priorityList"]
            conditions = [item["action"].get("condition") for item in items
                          if item["action"].get("castSpell", {}).get("spellId", {}).get("spellId")
                          == sweep.DEMORALIZING_SHOUT]
            self.assertEqual(len(conditions), 1, name)
            self.assertIn("auraShouldRefresh", conditions[0], name)
            found += 1
        self.assertEqual(found, 6)

    def test_the_shout_rows_reach_one_to_five_targets_tanking_and_not(self):
        counts, tanking = set(), set()
        for name, base, *_ in sweep.rows():
            if "challenging-shout" not in name:
                continue
            counts.add(len(base["encounter"]["targets"]))
            tanking.add(bool(base["raid"].get("tanks")))
        self.assertEqual(counts, {1, 2, 3, 5})
        self.assertEqual(tanking, {True, False})

    def test_the_roar_rows_cover_each_debuff_and_target_count_for_both_forms(self):
        seen = set()
        for name, base, *_ in sweep.rows():
            if name.startswith("fixture-") or "roar" not in name:
                continue
            debuffs = base["raid"]["debuffs"]
            seen.add((name.split("-")[0] + name.split("-")[1], len(base["encounter"]["targets"]),
                      debuffs.get("demoralizingShout", False), debuffs.get("demoralizingRoar", False)))
        for kind in ("beartank", "bearno", "catroar"):
            for targets in (1, 3, 5):
                for shout, roar in ((False, False), (True, False), (False, True)):
                    self.assertIn((kind, targets, shout, roar), seen)

    def test_a_row_with_a_raid_debuff_keeps_it_in_every_variant(self):
        stems = {stem: request for stem, request in sweep.variants()}
        for stem, request in stems.items():
            debuffs = request["raid"]["debuffs"]
            if "over-shout-debuff" in stem:
                self.assertTrue(debuffs.get("demoralizingShout"), stem)
            if "over-roar-debuff" in stem:
                self.assertTrue(debuffs.get("demoralizingRoar"), stem)

    def test_a_bear_without_a_tank_is_not_tanking(self):
        for name, base, *_ in sweep.rows():
            if name.startswith("bear-no-tank"):
                self.assertFalse(base["raid"].get("tanks"), name)
            if name.startswith("bear-tank"):
                self.assertTrue(base["raid"].get("tanks"), name)

    def test_the_druid_rows_cast_the_roar_by_its_refresh_condition(self):
        for name, base, *_ in sweep.rows():
            player = base["raid"]["parties"][0]["players"][0]
            if player["class"] == "ClassDruid" and "roar" in name:
                self.assertIn(sweep.DEMORALIZING_ROAR, cast_spells(base), name)

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["shout_roar_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
