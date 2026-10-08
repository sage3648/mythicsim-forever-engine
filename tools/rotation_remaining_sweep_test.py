import copy
import json
import random
import sys
import tempfile
import unittest
from pathlib import Path

import rotation_remaining_sweep as sweep


def rotation_text(request):
    return json.dumps(sweep.player_of(request)["rotation"])


def request_with(items, prepull=()):
    return {"raid": {"parties": [{"players": [{"equipment": {"items": []}, "rotation": {
        "priorityList": items, "prepullActions": list(prepull)}}]}]}}


class RotationRemainingSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_row_uses_the_construct_it_is_named_for(self):
        marks = {"moves": '"move', "speed": '"move', "units": '"AllTargets"|"AllPlayers"|"Player"',
                 "reaction": "includeReactionTime", "refresh": "auraShouldRefresh",
                 "groups": '"groups"|"valueVariables"'}
        seen = set()
        for name, base, *_ in sweep.rows():
            prefix = name.split("-")[0]
            if prefix not in marks:
                continue
            seen.add(prefix)
            text = rotation_text(base)
            self.assertTrue(any(mark in text for mark in marks[prefix].split("|")), name)
        self.assertEqual(seen, set(marks))

    def test_a_timeline_moves_in_the_priority_list_and_before_the_pull(self):
        base = request_with([sweep.cast(1)])
        moved, count = sweep.move_timeline(base, random.Random(1))
        rotation = sweep.player_of(moved)["rotation"]
        self.assertGreaterEqual(count, 3)
        self.assertEqual(len(rotation["priorityList"]), count + 1)
        for item in rotation["priorityList"][:-1]:
            self.assertTrue(("move" in item["action"]) != ("moveDuration" in item["action"]))
            self.assertIn("condition", item["action"])
        self.assertEqual(rotation["priorityList"][-1], sweep.cast(1))
        # The base is left as it was.
        self.assertEqual(len(sweep.player_of(base)["rotation"]["priorityList"]), 1)

    def test_a_cast_at_no_unit_is_added_in_every_form(self):
        base = request_with([sweep.cast(5)])
        changed, count = sweep.units(base, random.Random(3))
        rotation = sweep.player_of(changed)["rotation"]
        kinds = sorted(next(iter(item["action"].keys() - {"condition"})) for item in rotation["priorityList"][:-1])
        self.assertEqual(count, 6)
        self.assertEqual(kinds, sorted(["castSpell", "castFriendlySpell", "channelSpell", "sequence",
                                        "strictSequence", "castSpell"]))
        self.assertEqual(len(rotation["prepullActions"]), 1)

    def test_a_rotation_without_a_cast_adds_no_unit_cast(self):
        changed, count = sweep.units(request_with([{"action": {"autocastOtherCooldowns": {}}}]),
                                     random.Random(3))
        self.assertEqual(count, 0)

    def test_reaction_time_reads_the_auras_of_both_units(self):
        prepared = {"player": {"auras": [
            {"label": "A", "action_id": {"spell_id": 2, "tag": -1}, "max_stacks": 3,
             "exclusive_memberships": [{"category": "X"}]},
            {"label": "B", "action_id": {"spell_id": 4}, "max_stacks": 0, "exclusive_memberships": []}]},
            "target": {"auras": [{"label": "C", "action_id": {"spell_id": 6}, "max_stacks": 5,
                                  "exclusive_memberships": [{"category": "Y"}]}]}}
        changed, count = sweep.reaction_time(request_with([sweep.cast(1)]), prepared, random.Random(4))
        self.assertEqual(count, 3)
        text = rotation_text(changed)
        self.assertIn('"includeReactionTime": true', text)
        self.assertIn('{"spellId": 2, "tag": -1}', text)
        self.assertNotIn('"spellId": 4}', text.replace('"spellId": 4,', ""))

    def test_aura_should_refresh_names_auras_in_a_category(self):
        prepared = {"player": {"auras": [{"label": "A", "action_id": {"spell_id": 2}, "max_stacks": 0,
                                          "exclusive_memberships": [{"category": "X"}]}]},
                    "target": {"auras": []}}
        changed, count = sweep.aura_should_refresh(request_with([sweep.cast(1)]), prepared, random.Random(2))
        self.assertEqual(count, 1)
        read = sweep.player_of(changed)["rotation"]["priorityList"][0]["action"]["condition"]["auraShouldRefresh"]
        self.assertEqual(read["auraId"], {"spellId": 2})

    def test_groups_hold_the_items_they_cut_out(self):
        items = [sweep.cast(spell) for spell in (1, 2, 3, 4, 5)]
        shapes = set()
        for seed in range(40):
            changed, count = sweep.groups(request_with(copy.deepcopy(items)), random.Random(seed))
            rotation = sweep.player_of(changed)["rotation"]
            self.assertEqual(count, 1)
            if rotation.get("groups"):
                names = {group["name"] for group in rotation["groups"]}
                references = [
                    item["action"]["groupReference"]["groupName"]
                    for item in rotation["priorityList"] if "groupReference" in item["action"]]
                shapes.add(tuple(sorted(names)))
                self.assertTrue(set(references) <= names | {"nowhere"}, (references, names))
            else:
                shapes.add(("variable",))
        # Each shape of the draw appears: plain, nested, unreferenced and the variable.
        self.assertIn(("g",), shapes)
        self.assertIn(("inner", "outer"), shapes)
        self.assertIn(("g", "unused"), shapes)

    def test_the_speed_rows_enchant_the_boots(self):
        base = request_with([sweep.cast(1)])
        player = sweep.player_of(base)
        player["equipment"]["items"] = [{}] * 8 + [{"id": 15065, "enchant": 1506}]
        enchanted = sweep.speed_enchant(base, random.Random(1))
        self.assertIn(sweep.player_of(enchanted)["equipment"]["items"][8]["enchant"], sweep.SPEED_ENCHANTS)
        self.assertEqual(player["equipment"]["items"][8]["enchant"], 1506)

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
                sys.argv = ["rotation_remaining_sweep.py", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
