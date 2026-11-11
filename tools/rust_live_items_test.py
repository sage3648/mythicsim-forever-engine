from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))

import rust_live_items as live


def tooltip(*lines):
    body = "<br>".join(lines)
    return {"name": "Test Item", "tooltip": f"<table><tr><td>{body}</td></tr></table>"}


def row(stats, **extra):
    return {
        "id": 1,
        "name": "Test Item",
        "type": "ItemTypeFinger",
        "scaling_options": {"0": {"stats": {str(k): v for k, v in stats.items()}, "ilvl": 60}},
        **extra,
    }


class ReadTooltipTest(unittest.TestCase):
    def test_reads_base_stats_resistances_and_equip_lines(self):
        stated = live.read_tooltip(
            tooltip(
                "Item Level 61",
                "+10 Agility",
                "+7 to All Resistances",
                "+5 Ranged Attack Power",
                "+3 Holy Resistance",
                "Equip: +12 Attack Power.",
                "Equip: Improves your chance to hit by 1.0%.",
                "(2) Set : +4 All Resistances.",
            )
        )
        self.assertEqual(stated["ilvl"], 61)
        self.assertEqual(
            stated["stats"],
            {1: 10, 35: 7, 36: 7, 37: 7, 38: 7, 39: 7, 18: 5, 17: 12, 20: 10},
        )

    def test_reads_the_use_cooldown(self):
        stated = live.read_tooltip(
            tooltip("Item Level 65", "Use: Increases Spirit by 72 for 15 sec. (6 Min Cooldown)")
        )
        self.assertEqual(stated["use_cooldowns"], [360000])
        stated = live.read_tooltip(tooltip("Item Level 65", "Use: Restores 375 mana. (30 Sec Cooldown)"))
        self.assertEqual(stated["use_cooldowns"], [30000])

    def test_no_tooltip_is_none(self):
        self.assertIsNone(live.read_tooltip({"missing": True}))
        self.assertIsNone(live.read_tooltip({"error": "Entity not found"}))

    def test_unknown_stat_line_is_unreadable(self):
        with self.assertRaises(live.client.Unreadable):
            live.read_tooltip(tooltip("Item Level 10", "+3 Luck"))


class ChangesTest(unittest.TestCase):
    def test_generic_attack_power_moves_ranged_with_it(self):
        stated = live.read_tooltip(tooltip("Item Level 58", "Equip: +12 Attack Power."))
        self.assertEqual(live.changes(row({17: 20, 18: 20}), stated), {"stats": {17: 12, 18: 12}, "ilvl": 58})

    def test_attack_power_alone_stays_alone(self):
        # Preparation gives an item's attack power to ranged too, so a row that holds attack
        # power alone keeps it alone (Devilsaur Gauntlets).
        stated = live.read_tooltip(tooltip("Item Level 58", "Equip: +28 Attack Power."))
        self.assertEqual(stated["stats"], {17: 28})
        self.assertEqual(live.changes(row({17: 28}), stated), {})

    def test_the_use_cooldown_replaces_the_rows(self):
        effect = {"buff_id": 1291103, "on_use": {"cooldown_ms": 90000, "category_id": 1141, "category_cooldown_ms": 15000}}
        reference = row({}, item_effects=[effect])
        stated = live.read_tooltip(tooltip("Item Level 60", "Use: Increases Spirit by 72 for 15 sec. (6 Min Cooldown)"))
        change = live.changes(reference, stated)
        self.assertEqual(change, {"cooldown_ms": 360000})
        out = live.live_row(reference, change)
        self.assertEqual(
            out["item_effects"][0]["on_use"],
            {"cooldown_ms": 360000, "category_id": 1141, "category_cooldown_ms": 15000},
        )
        self.assertEqual(reference["item_effects"][0]["on_use"]["cooldown_ms"], 90000)
        stated = live.read_tooltip(tooltip("Item Level 60", "Use: Increases Spirit by 72 for 15 sec. (1 Min Cooldown)"))
        self.assertEqual(live.changes(reference, stated), {"cooldown_ms": 60000})
        stated = live.read_tooltip(tooltip("Item Level 60", "Use: Increases Spirit by 72 for 15 sec. (90 Sec Cooldown)"))
        self.assertEqual(live.changes(reference, stated), {})

    def test_drops_a_named_stat_only_when_the_tooltip_never_names_it(self):
        stated = live.read_tooltip(tooltip("Item Level 60", "+5 Stamina"))
        self.assertEqual(live.changes(row({2: 5, 16: 4}), stated), {"stats": {16: None}})
        stated = live.read_tooltip(tooltip("Item Level 60", "+5 Stamina", "Equip: Spirit of the wild."))
        self.assertEqual(live.changes(row({2: 5, 16: 4}), stated), {})

    def test_a_stat_the_row_keeps_as_an_effect_is_not_added(self):
        effect = {"scaling_options": {"0": {"stats": {"17": 30}}}}
        stated = live.read_tooltip(tooltip("Item Level 60", "+5 Stamina", "Equip: +30 Attack Power."))
        self.assertEqual(live.changes(row({2: 5}, item_effects=[effect]), stated), {})

    def test_hit_within_a_rating_point_agrees(self):
        stated = live.read_tooltip(tooltip("Item Level 60", "Equip: Improves your chance to hit by 1.0%."))
        self.assertEqual(live.changes(row({20: 9}), stated), {})

    def test_a_rating_the_row_keeps_as_a_percent_pseudo_stat_is_not_added(self):
        pseudo = [0] * 42
        pseudo[15] = 1
        stated = live.read_tooltip(
            tooltip("Item Level 63", "Equip: Improves your chance to get a critical strike with spells by 1.0%.")
        )
        self.assertEqual(live.changes(row({}, pseudo_stats=pseudo), stated), {})

    def test_spell_penetration_reads_in_either_wording(self):
        # Elemental Mage Staff, 9 and 11 November 2026.
        old = live.read_tooltip(tooltip("Item Level 61", "+26 Intellect", "+8 Spell Penetration"))
        new = live.read_tooltip(
            tooltip("Item Level 61", "+26 Intellect", "Equip: Your spells pierce 8 Magical Resistance.")
        )
        self.assertEqual(old["stats"], {3: 26, 15: 8})
        self.assertEqual(new["stats"], {3: 26, 15: 8})
        self.assertEqual(live.changes(row({3: 26, 15: 8}), new), {})

    def test_weapon_skills_follow_the_tooltip(self):
        # Edgemaster's Handguards: +7 Axes, Swords and Daggers in the reference row, +1 each live.
        pseudo = [0] * 42
        pseudo[28] = pseudo[29] = pseudo[31] = 7
        pseudo[15] = 1
        stated = live.read_tooltip(
            tooltip(
                "Item Level 49",
                "Equip: Increased Axes +1.",
                "Equip: Increased Daggers +1.",
                "Equip: Increased Swords +1.",
            )
        )
        self.assertEqual(stated["skills"], {28: 1, 29: 1, 31: 1})
        change = live.changes(row({}, pseudo_stats=pseudo), stated)
        expected = [0] * 42
        expected[28] = expected[29] = expected[31] = 1
        expected[15] = 1
        self.assertEqual(change["pseudo_stats"], expected)
        self.assertEqual(live.live_row(row({}, pseudo_stats=pseudo), change)["pseudo_stats"], expected)

    def test_agreeing_weapon_skills_change_nothing(self):
        pseudo = [0] * 42
        pseudo[31] = 4
        stated = live.read_tooltip(tooltip("Item Level 60", "Equip: Increased Daggers +4."))
        self.assertEqual(live.changes(row({}, pseudo_stats=pseudo), stated), {})

    def test_a_weapon_skill_the_tooltip_drops_is_dropped(self):
        pseudo = [0] * 42
        pseudo[31] = 5
        stated = live.read_tooltip(tooltip("Item Level 60", "+10 Agility"))
        change = live.changes(row({1: 10}, pseudo_stats=pseudo), stated)
        self.assertEqual(change, {"pseudo_stats": [0] * 42})
        self.assertNotIn("pseudo_stats", live.live_row(row({1: 10}, pseudo_stats=pseudo), change))

    def test_a_new_weapon_skill_is_added(self):
        # Taran Icebreaker: no skill in the reference row, +1 Maces live.
        stated = live.read_tooltip(tooltip("Item Level 39", "Equip: Increased Maces +1."))
        change = live.changes(row({}), stated)
        self.assertEqual(change["pseudo_stats"][30], 1)
        self.assertEqual(sum(change["pseudo_stats"]), 1)

    def test_live_row_keeps_the_reference_row_but_the_change(self):
        reference = row({1: 15, 2: 4}, unique=True)
        out = live.live_row(reference, {"stats": {1: 10, 35: 10, 2: None}, "ilvl": 61})
        self.assertEqual(out["scaling_options"]["0"], {"stats": {"1": 10, "35": 10}, "ilvl": 61})
        self.assertTrue(out["unique"])
        self.assertEqual(reference["scaling_options"]["0"]["stats"], {"1": 15, "2": 4})


if __name__ == "__main__":
    unittest.main()
