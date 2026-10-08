import sys
import tempfile
import unittest
from pathlib import Path

import weapon_proc_sweep as sweep


class WeaponProcSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_weapon_is_worn_by_several_classes(self):
        classes = {}
        for _, base, *_ in sweep.rows():
            player = base["raid"]["parties"][0]["players"][0]
            for item in player["equipment"]["items"]:
                classes.setdefault(item.get("id"), set()).add(player["class"])
        everything = {"wolfsbane": 267369}
        for table in (sweep.AXES, sweep.DAGGERS, sweep.FISTS, sweep.MACES, sweep.SWORDS, sweep.POLEARMS,
                      sweep.RANGED):
            everything.update(table)
        for weapon, item in everything.items():
            self.assertTrue(classes.get(item), weapon)
        # The weapons any build can wield are worn by at least three classes.
        for weapon in ("masterwork-stormhammer", "stinging-viper", "plaguefang"):
            self.assertGreaterEqual(len(classes[everything[weapon]]), 3, weapon)

    def test_a_row_equips_every_slot_it_names(self):
        for name, base, *_ in sweep.rows():
            items = base["raid"]["parties"][0]["players"][0]["equipment"]["items"]
            self.assertTrue(all(item for item in items), name)

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["weapon_proc_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
