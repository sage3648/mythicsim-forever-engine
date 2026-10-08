import json
import sys
import tempfile
import unittest
from pathlib import Path

import item_effect_sweep as sweep


def ids(request):
    return [item.get("id") for item in sweep.entries(request)]


class ItemEffectSweepTests(unittest.TestCase):
    def test_the_variants_are_the_same_every_time(self):
        self.assertEqual(sweep.variants(), sweep.variants())

    def test_every_item_of_a_row_is_worn_once_without_displacing_another_slot(self):
        database = sweep.items_database()
        for group, items, bases, *_ in sweep.ITEM_ROWS:
            for base_name, _ in bases:
                base = sweep.request_of(base_name)
                for name, item in items.items():
                    request = sweep.equipped(base, item, database)
                    held = ids(request)
                    # Replacing keeps the number of entries; only an empty slot adds one.
                    self.assertLessEqual(len(held) - len(ids(base)), 1, (group, base_name, name))
                    self.assertEqual(held.count(item), 1, (group, base_name, name))

    def test_an_enchant_row_enchants_an_item_of_the_type_it_names(self):
        database, enchants = sweep.items_database(), sweep.enchants_database()
        for name, effect, bases, _ in sweep.ENCHANT_ROWS:
            for base_name, _ in bases:
                request = sweep.enchanted(sweep.request_of(base_name), effect, database, enchants)
                enchanted = [item for item in sweep.entries(request) if item.get("enchant") == effect]
                self.assertEqual(len(enchanted), 1, (name, base_name))
                self.assertEqual(database[enchanted[0]["id"]]["type"], enchants[effect]["type"])

    def test_every_row_has_a_request(self):
        for name, base, *_ in sweep.rows():
            self.assertIsNotNone(base, name)
            self.assertTrue(sweep.entries(base), name)

    def test_a_shield_replaces_a_weapon_and_a_trinket_a_ring_or_trinket(self):
        database = sweep.items_database()
        base = sweep.request_of("production-protection-warrior-3-targets")
        before = ids(base)
        changed = [i for i, (a, b) in enumerate(zip(before, ids(sweep.equipped(base, 18825, database)))) if a != b]
        self.assertEqual(len(changed), 1)
        self.assertEqual(database[before[changed[0]]]["type"], "ItemTypeWeapon")
        changed = [i for i, (a, b) in enumerate(zip(before, ids(sweep.equipped(base, 23570, database)))) if a != b]
        self.assertEqual(len(changed), 1)
        self.assertIn(database[before[changed[0]]]["type"], ("ItemTypeFinger", "ItemTypeTrinket"))

    def test_an_empty_slot_is_filled_by_a_new_entry(self):
        database = sweep.items_database()
        base = sweep.request_of("production-warrior")
        sweep.entries(base).clear()
        self.assertEqual(ids(sweep.equipped(base, 17076, database)), [17076])

    def test_the_rows_name_items_go_registers(self):
        registered = set(json.loads((sweep.DATA / "go-tables.json").read_text())["item_effect_ids"])
        for group, items, *_ in sweep.ITEM_ROWS:
            for name, item in items.items():
                self.assertIn(item, registered, (group, name))

    def test_a_verdict_names_the_first_refusal(self):
        self.assertEqual(sweep.verdict((0, ""), (0, json.dumps({"supported": True}))), ("ran", ""))
        refused = json.dumps({"refusal": {"code": "unsupported_effect", "reason": "no"}})
        self.assertEqual(sweep.verdict((sweep.NOT_PREPARED, refused), None), ("unsupported_effect", "no"))
        gate = json.dumps({"supported": False, "refusals": [{"code": "a", "reason": "first"},
                                                            {"code": "b", "reason": "second"}]})
        self.assertEqual(sweep.verdict((0, ""), (0, gate)), ("a", "first; second"))

    def test_the_output_is_written_once(self):
        argv = sys.argv
        with tempfile.TemporaryDirectory() as scratch:
            output = Path(scratch) / "variants"
            try:
                sys.argv = ["item_effect_sweep.py", "variants", "--output", str(output)]
                sweep.main()
                self.assertEqual(len(list(output.glob("*.request.json"))), len(sweep.variants()))
                with self.assertRaises(FileExistsError):
                    sweep.main()
            finally:
                sys.argv = argv


if __name__ == "__main__":
    unittest.main()
