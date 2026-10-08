import json
import sys
import tempfile
import unittest
from pathlib import Path

import external_cooldown_sweep as sweep


def cast_spells(request):
    """The spell IDs the request's rotation casts, in order."""
    return [item["action"].get("castSpell", {}).get("spellId", {}).get("spellId")
            for item in sweep.rotation_of(request)]


def stem_variants():
    return dict(sweep.variants())


class ExternalCooldownSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_class_is_innervated_and_given_a_mana_tide(self):
        classes = set()
        for name, base, *_ in sweep.rows():
            if name.endswith("-innervate") or name.endswith("-mana-tide"):
                classes.add(sweep.player_of(base)["class"])
        self.assertEqual(classes, {"ClassDruid", "ClassPriest", "ClassMage", "ClassWarlock", "ClassHunter",
                                   "ClassPaladin", "ClassShaman", "ClassWarrior", "ClassRogue"})

    def test_each_row_draws_one_two_and_three_sources(self):
        variants = stem_variants()
        for name, _, count, *_ in sweep.rows():
            if name.startswith("fixture-") or name.endswith("-taunts"):
                continue
            sources = []
            for number in range(count):
                request = variants[f"{name}-sweep-{number:02d}"]
                if name.endswith("-innervate"):
                    sources.append(sweep.player_of(request)["buffs"]["innervates"])
                else:
                    sources.append(request["raid"]["parties"][0]["buffs"]["manaTideTotems"])
            self.assertEqual(set(sources), {1, 2, 3}, name)

    def test_an_innervate_variant_runs_long_and_half_of_them_lose_their_mana_buffs(self):
        variants = stem_variants()
        starved = 0
        for stem, request in variants.items():
            if "-innervate-sweep-" not in stem or stem.startswith("fixture-"):
                continue
            self.assertGreaterEqual(request["encounter"]["duration"], sweep.INNERVATE_DURATION, stem)
            self.assertLess(request["encounter"]["durationVariation"], request["encounter"]["duration"], stem)
            player = sweep.player_of(request)
            if "greaterBlessingOfWisdom" not in player.get("buffs", {}):
                starved += 1
        self.assertGreater(starved, len(sweep.CLASS_BASES) * 2)

    def test_a_row_that_names_a_race_draws_it_from_its_class(self):
        for stem, request in sweep.variants():
            if stem.startswith("balance-druid"):
                self.assertIn(sweep.player_of(request)["race"], sweep.DRUID_RACES, stem)

    def test_the_taunt_rows_cast_both_spells_at_a_target_they_have(self):
        variants = stem_variants()
        counts, tanking = set(), set()
        found = 0
        for name, base, count, *_ in sweep.rows():
            if not name.endswith("-taunts"):
                continue
            counts.add(len(base["encounter"]["targets"]))
            tanking.add(bool(base["raid"].get("tanks")))
            for number in range(count):
                request = variants[f"{name}-sweep-{number:02d}"]
                spells = cast_spells(request)
                self.assertIn(sweep.TAUNT, spells, name)
                self.assertIn(sweep.INTIMIDATING_SHOUT, spells, name)
                # Defensive Stance comes before Taunt, which needs it.
                self.assertLess(spells.index(sweep.DEFENSIVE_STANCE), spells.index(sweep.TAUNT), name)
                for item in sweep.rotation_of(request):
                    target = item["action"].get("castSpell", {}).get("target", {}).get("index", 0)
                    self.assertLess(target, len(request["encounter"]["targets"]), name)
                found += 1
        self.assertEqual(found, sum(count for name, _, count, *_ in sweep.rows() if name.endswith("-taunts")))
        self.assertEqual(counts, {1, 2, 3, 5})
        self.assertEqual(tanking, {True, False})

    def test_a_taunting_variant_can_aim_past_the_first_target(self):
        aimed = set()
        for stem, request in sweep.variants():
            if "-taunts-sweep-" in stem:
                for item in sweep.rotation_of(request):
                    cast = item["action"].get("castSpell", {})
                    if cast.get("spellId", {}).get("spellId") == sweep.TAUNT:
                        aimed.add(cast.get("target", {}).get("index", 0))
        self.assertTrue(aimed - {0})

    def test_the_tide_talent_row_adds_the_totem_to_the_talents(self):
        found = 0
        for name, base, *_ in sweep.rows():
            if name.endswith("-tide-talent"):
                restoration = sweep.player_of(base)["talentsString"].split("-")[2]
                self.assertEqual(restoration[sweep.MANA_TIDE_TALENT], "1")
                found += 1
        self.assertEqual(found, len(sweep.TIDE_TALENT_BASES))

    def test_every_row_is_built_from_an_accepted_request(self):
        for name in [base for _, base, *_ in sweep.CLASS_BASES] + sweep.FIXTURES_OF_THE_SWEEP:
            self.assertTrue((sweep.FIXTURES / f"{name}.request.json").is_file(), name)
        for _, name, *_ in sweep.TAUNT_BASES:
            self.assertTrue((sweep.FIXTURES / f"{name}.request.json").is_file(), name)

    def test_the_fixtures_of_the_sweep_are_accepted_cases(self):
        manifest = json.loads((sweep.FIXTURES / "manifest.json").read_text())
        accepted = {case["id"] for case in manifest["cases"]}
        for name in sweep.FIXTURES_OF_THE_SWEEP:
            self.assertIn(name, accepted)

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["external_cooldown_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
