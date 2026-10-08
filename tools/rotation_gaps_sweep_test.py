import json
import sys
import tempfile
import unittest
from pathlib import Path

import rotation_gaps_sweep as sweep


def rotation_text(request):
    return json.dumps(sweep.player_of(request)["rotation"])


class RotationGapsSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_row_uses_the_gap_it_is_named_for(self):
        marks = {"reaction-time": "includeReactionTime", "dot-base": "dotBaseDuration",
                 "all-targets": "AllTargets", "weave": '"move"', "timeline": '"move"'}
        seen = set()
        for name, base, *_ in sweep.rows():
            for prefix, mark in marks.items():
                if name.startswith(prefix + "-"):
                    seen.add(prefix)
                    self.assertIn(mark, rotation_text(base), name)
        self.assertEqual(seen, set(marks))

    def test_reaction_time_reaches_only_the_players_own_auras(self):
        request = {"raid": {"parties": [{"players": [{"rotation": {"priorityList": [
            {"action": {"castSpell": {"spellId": {"spellId": 1}}, "condition": {"and": {"vals": [
                {"auraIsActive": {"auraId": {"spellId": 2}}},
                {"auraIsInactive": {"auraId": {"spellId": 3}, "sourceUnit": {"type": "CurrentTarget"}}},
                {"auraIsInactive": {"auraId": {"spellId": 4}, "sourceUnit": {"type": "Self"}}}]}}}}]}}]}]}}
        changed, count = sweep.reaction_time(request)
        self.assertEqual(count, 2)
        vals = sweep.player_of(changed)["rotation"]["priorityList"][0]["action"]["condition"]["and"]["vals"]
        self.assertTrue(vals[0]["auraIsActive"]["includeReactionTime"])
        self.assertNotIn("includeReactionTime", vals[1]["auraIsInactive"])
        self.assertTrue(vals[2]["auraIsInactive"]["includeReactionTime"])

    def test_a_dot_cast_waits_for_the_fight_to_outlast_the_dot(self):
        def cast(spell, condition=None):
            action = {"castSpell": {"spellId": {"spellId": spell}}}
            if condition:
                action["condition"] = condition
            return {"action": action}
        read = {"dotIsActive": {"spellId": {"spellId": 7}}}
        request = {"raid": {"parties": [{"players": [{"rotation": {"priorityList": [
            cast(7, {"not": {"val": read}}), cast(8)]}}]}]}}
        changed, count = sweep.dot_base_duration(request)
        self.assertEqual(count, 1)
        items = sweep.player_of(changed)["rotation"]["priorityList"]
        terms = items[0]["action"]["condition"]["and"]["vals"]
        self.assertEqual(terms[0], {"not": {"val": read}})
        self.assertEqual(terms[1]["cmp"]["rhs"], {"dotBaseDuration": {"spellId": {"spellId": 7}}})
        self.assertNotIn("condition", items[1]["action"])

    def test_every_other_read_names_a_set_of_units(self):
        dot = lambda: {"dotIsActive": {"spellId": {"spellId": 7}}}
        request = {"raid": {"parties": [{"players": [{"rotation": {"priorityList": [
            {"action": {"castSpell": {"spellId": {"spellId": 1}}, "condition": {"and": {"vals": [
                dot(), dot(), dot(), {"auraIsActive": {"auraId": {"spellId": 2}}},
                {"auraIsActive": {"auraId": {"spellId": 2}}}]}}}}]}}]}]}}
        changed, count = sweep.all_targets(request)
        vals = sweep.player_of(changed)["rotation"]["priorityList"][0]["action"]["condition"]["and"]["vals"]
        self.assertEqual([("targetUnit" in v["dotIsActive"]) for v in vals[:3]], [True, False, True])
        self.assertEqual(vals[3]["auraIsActive"]["sourceUnit"], {"type": "AllTargets"})
        self.assertNotIn("sourceUnit", vals[4]["auraIsActive"])
        self.assertEqual(count, 3)

    def test_a_hunter_row_moves_before_and_during_the_fight(self):
        for name, base, *_ in sweep.rows():
            if name.startswith(("weave-", "timeline-")):
                rotation = sweep.player_of(base)["rotation"]
                self.assertTrue(any("move" in prepull["action"] for prepull in rotation["prepullActions"]), name)
                self.assertTrue(any("move" in item["action"] for item in rotation["priorityList"]), name)

    def test_the_variants_keep_the_weapons_of_their_base(self):
        bases = {name: base for name, base, *_ in sweep.rows()}
        for stem, request in sweep.variants():
            base = bases[stem.rsplit("-sweep-", 1)[0]]
            items = sweep.player_of(request)["equipment"]["items"]
            kept = sweep.player_of(base)["equipment"]["items"]
            for slot in sweep.WEAPON_SLOTS:
                if slot < len(kept):
                    self.assertEqual(items[slot], kept[slot], stem)

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["rotation_gaps_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
