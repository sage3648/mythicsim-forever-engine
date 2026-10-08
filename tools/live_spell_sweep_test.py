import sys
import tempfile
import unittest
from pathlib import Path

import live_spell_sweep as sweep


class LiveSpellSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_rank_of_holy_nova_is_cast(self):
        cast = set()
        for _, base, *_ in sweep.rows():
            player = base["raid"]["parties"][0]["players"][0]
            if player["class"] != "ClassPriest":
                continue
            for item in player["rotation"]["priorityList"]:
                spell = item["action"].get("castSpell", {}).get("spellId", {}).get("spellId")
                if spell in sweep.HOLY_NOVA_RANKS:
                    cast.add(spell)
        self.assertEqual(cast, set(sweep.HOLY_NOVA_RANKS))

    def test_a_priest_keeps_the_holy_nova_talent(self):
        for stem, request in sweep.variants():
            player = request["raid"]["parties"][0]["players"][0]
            if player["class"] == "ClassPriest":
                self.assertEqual(player["talentsString"].split("-")[1][5], "1", stem)

    def test_a_warrior_casts_the_shout(self):
        for name, base, *_ in sweep.rows():
            player = base["raid"]["parties"][0]["players"][0]
            if player["class"] != "ClassWarrior":
                continue
            spells = [item["action"].get("castSpell", {}).get("spellId", {}).get("spellId")
                      for item in player["rotation"]["priorityList"]]
            self.assertIn(11556, spells, name)

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["live_spell_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
