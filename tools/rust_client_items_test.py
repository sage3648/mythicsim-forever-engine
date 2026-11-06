import json
import unittest

import rust_client_items as tool

WORN_CLOAK = {
    "name": "Worn Cloak",
    "quality": 0,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q0">Worn Cloak</b><!--nend--><span class="q">'
        '<br>Item Level <!--ilvl-->15</span><!--bo--><!--ue--><table width="100%"><tr><td>Back'
        "</td></tr></table><!--rf--><span><!--amr-->13 Armor</span><!--ebstats--><!--e-->"
        "</td></tr></table><table><tr><td>Requires Level <!--rlvl-->10<br><!--rr-->"
        '<!--itemEffects:0--><div class="whtt-sellprice">Sell Price: <span class="moneycopper">'
        "88</span></div></td></tr></table>"
    ),
}
BASTARD_SWORD = {
    "name": "Crude Bastard Sword",
    "quality": 0,
    "tooltip": (
        '<table><tr><td><b class="q0">Crude Bastard Sword</b><span class="q"><br>Item Level '
        '<!--ilvl-->7</span><!--bo--><table width="100%"><tr><td>Two-Hand</td><th>'
        '<span class="q1">Sword</span></th></tr></table><!--rf--><table width="100%"><tr>\n'
        "<td><span><!--dmg-->7 - 12 Damage</span></td>\n<th>Speed <!--spd-->3.20</th>\n"
        "</tr></table><!--dps-->(2.97 damage per second)<br>Durability 35 / 35</td></tr></table>"
        "<table><tr><td>Requires Level <!--rlvl-->2<br><!--itemEffects:0--></td></tr></table>"
    ),
}
BUCKLER = {
    "name": "Worn Wooden Buckler",
    "quality": 1,
    "tooltip": (
        '<table><tr><td><b class="q1">Worn Wooden Buckler</b><span class="q"><br>Item Level '
        '<!--ilvl-->1</span><table width="100%"><tr><td>Off Hand</td><th><span class="q1">'
        "Shield</span></th></tr></table><span><!--amr-->10 Armor</span><br>1 Block"
        "<br>Durability 20 / 20</td></tr></table><table><tr><td><!--itemEffects:0-->"
        "</td></tr></table>"
    ),
}


def sparse(inventory, level, delay=0):
    return {
        "Display_lang": "x",
        "InventoryType": str(inventory),
        "ItemLevel": str(level),
        "ItemDelay": str(delay),
    }


def item(class_id, subclass):
    return {"ClassID": str(class_id), "SubclassID": str(subclass)}


class ParseTests(unittest.TestCase):
    def test_cloak_has_cloth_armor_in_the_back_slot(self):
        row = tool.row_for(1733, {**sparse(16, 15), "Display_lang": "Worn Cloak"}, item(4, 1), WORN_CLOAK)
        self.assertEqual(
            tool.dump_row(row),
            '{"id":1733,"name":"Worn Cloak","type":"ItemTypeBack","armor_type":"ArmorTypeCloth",'
            '"scaling_options":{"0":{"stats":{"30":13},"ilvl":15}}}',
        )

    def test_two_hand_sword_has_damage_and_speed(self):
        row = tool.row_for(1412, sparse(17, 7, 3200), item(2, 8), BASTARD_SWORD)
        self.assertEqual(row["hand_type"], "HandTypeTwoHand")
        self.assertEqual(row["weapon_type"], "WeaponTypeSword")
        self.assertEqual(row["weapon_speed"], 3.2)
        self.assertEqual(row["scaling_options"]["0"]["weapon_damage_min"], 7)
        self.assertEqual(row["scaling_options"]["0"]["weapon_damage_max"], 12)

    def test_shield_has_armor_and_block_value(self):
        row = tool.row_for(876, sparse(14, 1), item(4, 6), BUCKLER)
        self.assertEqual(row["weapon_type"], "WeaponTypeShield")
        self.assertEqual(row["scaling_options"]["0"]["stats"], {"27": 1, "30": 10})

    def test_whole_numbers_are_written_without_a_fraction(self):
        self.assertEqual(tool.dump_row({"speed": 2.0, "other": 1.5}), '{"speed":2,"other":1.5}')

    def test_speed_must_agree_with_item_delay(self):
        with self.assertRaisesRegex(tool.Unreadable, "ItemDelay"):
            tool.row_for(1412, sparse(17, 7, 2800), item(2, 8), BASTARD_SWORD)

    def test_item_level_must_agree_with_the_table(self):
        with self.assertRaisesRegex(tool.Unreadable, "item level"):
            tool.row_for(1733, sparse(16, 14), item(4, 1), WORN_CLOAK)

    def test_an_effect_leaves_the_item_out(self):
        proc = json.loads(json.dumps(WORN_CLOAK))
        proc["tooltip"] = proc["tooltip"].replace("itemEffects:0", "itemEffects:1")
        with self.assertRaisesRegex(tool.Unreadable, "effects"):
            tool.row_for(1733, sparse(16, 15), item(4, 1), proc)

    def test_a_line_the_tool_does_not_read_leaves_the_item_out(self):
        odd = json.loads(json.dumps(WORN_CLOAK))
        odd["tooltip"] = odd["tooltip"].replace("<!--ebstats-->", "<br>Sturdy as a rock<!--ebstats-->")
        with self.assertRaisesRegex(tool.Unreadable, "unread line"):
            tool.row_for(1733, sparse(16, 15), item(4, 1), odd)

    def test_a_missing_tooltip_leaves_the_item_out(self):
        with self.assertRaisesRegex(tool.Unreadable, "no tooltip"):
            tool.row_for(1733, sparse(16, 15), item(4, 1), {"missing": True})

    def test_development_rows_are_not_items(self):
        for name in ["Deprecated Old Belt", "OLDRecruit's Belt", "Test Fire Sword", "Hidden Cloak (DNT)"]:
            self.assertTrue(tool.JUNK_NAME.search(name), name)
        for name in ["Worn Cloak", "Contested Sword", "Latest Boots"]:
            self.assertFalse(tool.JUNK_NAME.search(name), name)


class CommittedFileTests(unittest.TestCase):
    def test_file_passes_the_offline_audit(self):
        self.assertGreater(tool.check(), 300)

    def test_existing_rows_are_written_as_the_database_writes_them(self):
        with open(tool.ITEMS) as rows:
            for _, line in zip(range(3000), rows):
                self.assertEqual(tool.dump_row(json.loads(line)), line.rstrip("\n"))


if __name__ == "__main__":
    unittest.main()
