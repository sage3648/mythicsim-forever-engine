import json
import re
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

# Berserker Shackles, a Forever item with no readable ItemSparse row (Wowhead, 8 November 2026).
SHACKLES = {
    "name": "Berserker Shackles",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Berserker Shackles</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->39</span><!--bo--><br>Binds when '
        'picked up<!--ue--><!--ue--><table width="100%"><tr><td>Wrist</td><th><!--scstart4:3-->'
        '<span class="q1">Mail</span><!--scend--></th></tr></table><!--rf--><span><!--amr-->109 '
        "Armor</span><br><span><!--stat4-->+7 Strength</span><!--ebstats--><!--egstats-->"
        "<!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--></td></tr></table><table><tr>"
        '<td>Requires Level <!--rlvl-->34<br><!--rr--><span class="q2">Equip: Increased Defense '
        "+<!--rtg12-->6.</span><!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--><div "
        'class="whtt-sellprice">Sell Price: <span class="moneysilver">38</span> <span '
        'class="moneycopper">48</span></div></td></tr></table><!--i?274600:1:60:34-->'
    ),
}
# Ceremonial Mallet, a Forever item with no readable ItemSparse row and a creature type attack
# power line (Wowhead, 9 November 2026).
MALLET = {
    "name": "Ceremonial Mallet",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Ceremonial Mallet</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->42</span><!--bo--><br>Binds when '
        'picked up<!--ue--><!--ue--><table width="100%"><tr><td>Two-Hand</td><th><!--scstart2:5-->'
        '<span class="q1">Mace</span><!--scend--></th></tr></table><!--rf--><table width="100%">'
        "<tr>\n    <td><span><!--dmg-->107 - 161 Damage</span></td>\n    <th>Speed "
        "<!--spd-->3.60</th>\n</tr></table><!--dps-->(37.22 damage per second)<br><span>"
        "<!--stat7-->+15 Stamina</span><!--ebstats--><!--egstats--><!--eistats-->"
        "<!--nameDescStats--><!--rs--><!--e--><!--ps--><br>Durability 100 / 100</td></tr></table>"
        '<table><tr><td><!--rr--><span class="q2">Equip: +<!--rtg38-->22 Attack Power.</span><br>'
        '<span class="q2">Equip: +<!--rtg128-->33 Attack Power against Undead.</span>'
        "<!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--></td></tr></table>"
    ),
}
# Staff of Jordan's equip lines (Wowhead Forever), the reference row: {"20":15,"4":60,"5":60}.
JORDAN_EFFECTS = (
    '<span class="q2">Equip: Improves your chance to hit by 1.5%.</span><br><span class="q2">'
    "Equip: Increases damage and healing done by magical spells and effects by up to <span>"
    "<!--rtg41-->60</span>.</span><!--itemEffects:1-->"
)


def with_effects(response, effects):
    """A copy of a tooltip response with its effect lines replaced."""
    out = json.loads(json.dumps(response))
    out["tooltip"] = re.sub(
        r'<span class="q2">.*?<!--itemEffects:\d+-->', lambda _: effects, out["tooltip"]
    )
    return out


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

    def test_an_item_without_an_itemsparse_row_reads_from_its_tooltip(self):
        row = tool.row_for(274600, None, {**item(4, 3), "InventoryType": "9"}, SHACKLES)
        self.assertEqual(
            tool.dump_row(row),
            '{"id":274600,"name":"Berserker Shackles","type":"ItemTypeWrist",'
            '"armor_type":"ArmorTypeMail","scaling_options":{"0":{"stats":{"0":7,"25":6,"30":109},'
            '"ilvl":39}}}',
        )

    def test_where_the_item_drops_is_not_an_item_line(self):
        # The same tooltip on 9 November 2026, with Wowhead's drop notes after the sell price.
        notes = (
            '</div><div class="whtt-extra whtt-droppedby">Dropped by: Zul\'Alai</div>'
            '<div class="whtt-extra whtt-dropchance">Drop Chance: 66.67%</div></td>'
        )
        dropped = {**SHACKLES, "tooltip": SHACKLES["tooltip"].replace("</div></td>", notes, 1)}
        self.assertIn("Dropped by", dropped["tooltip"])
        shackles = {**item(4, 3), "InventoryType": "9"}
        self.assertEqual(
            tool.row_for(274600, None, shackles, dropped),
            tool.row_for(274600, None, shackles, SHACKLES),
        )

    def test_the_tooltip_must_name_the_item_rows_slot(self):
        with self.assertRaisesRegex(tool.Unreadable, "slot"):
            tool.row_for(274600, None, {**item(4, 3), "InventoryType": "10"}, SHACKLES)

    def test_an_item_without_an_itemsparse_row_passes_the_name_filter(self):
        test = {**SHACKLES, "name": "Test Shackles"}
        test["tooltip"] = test["tooltip"].replace("Berserker Shackles", "Test Shackles")
        with self.assertRaisesRegex(tool.Unreadable, "development name"):
            tool.row_for(274600, None, {**item(4, 3), "InventoryType": "9"}, test)

    def test_equip_lines_read_as_the_reference_rows_hold_them(self):
        staff = tool.parse_tooltip(with_effects(SHACKLES, JORDAN_EFFECTS))
        self.assertEqual(staff["stats"], {0: 7, 4: 60, 5: 60, 20: 15})
        for line, stats in [
            ("Equip: +20 Attack Power.", {17: 20, 18: 20}),
            ("Equip: Improves your chance to get a critical strike by 1%.", {21: 14}),
            ("Equip: Improves your chance to hit with spells by 1%.", {12: 10}),
            ("Equip: Improves your chance to get a critical strike with spells by 2%.", {13: 28}),
            ("Equip: Restores 3 mana per 5 sec.", {34: 3}),
            ("Equip: Increases damage done by Holy spells and effects by up to 3.", {9: 3}),
            (
                "Equip: Increases healing done by up to 15 and damage done by up to 5 for all "
                "magical spells and effects.",
                {4: 15, 5: 5},
            ),
            ("Equip: Increases your chance to Dodge an attack by 1.0%.", {28: 12}),
            ("Equip: Increases your chance to Parry an attack by 1.0%.", {29: 15}),
        ]:
            self.assertEqual(tool.equip_stats(line), stats, line)

    def test_attack_power_against_a_creature_type_is_a_bonus_beside_the_row(self):
        mallet_item = {**item(2, 5), "InventoryType": "17"}
        row = tool.row_for(279885, None, mallet_item, MALLET)
        # The row holds the plain stats (Stamina 15, Attack Power 22 melee and ranged) and the
        # weapon.
        self.assertEqual(
            tool.dump_row(row),
            '{"id":279885,"name":"Ceremonial Mallet","type":"ItemTypeWeapon",'
            '"weapon_type":"WeaponTypeMace","hand_type":"HandTypeTwoHand","weapon_speed":3.6,'
            '"scaling_options":{"0":{"weapon_damage_min":107,"weapon_damage_max":161,'
            '"stats":{"2":15,"17":22,"18":22},"ilvl":42}}}',
        )
        parsed = tool.parse_tooltip(MALLET)
        self.assertEqual(parsed["versus"], {"MobTypeUndead": 33})
        self.assertEqual(
            tool.dump_row(tool.bonus_row(279885, row["name"], parsed)),
            '{"id":279885,"name":"Ceremonial Mallet","melee_attack_power_versus":{"MobTypeUndead":33}}',
        )

    def test_an_item_without_a_creature_type_bonus_has_no_bonus_row(self):
        self.assertIsNone(tool.bonus_row(274600, "Berserker Shackles", tool.parse_tooltip(SHACKLES)))

    def test_each_creature_type_reads_as_the_request_names_it(self):
        for word, mob_type in [
            ("Beasts", "MobTypeBeast"),
            ("Demons", "MobTypeDemon"),
            ("Dragonkin", "MobTypeDragonkin"),
            ("Elementals", "MobTypeElemental"),
            ("Giants", "MobTypeGiant"),
            ("Humanoids", "MobTypeHumanoid"),
            ("Mechanicals", "MobTypeMechanical"),
            ("Undead", "MobTypeUndead"),
        ]:
            line = f'<span class="q2">Equip: +12 Attack Power against {word}.</span><!--itemEffects:1-->'
            parsed = tool.parse_tooltip(with_effects(SHACKLES, line))
            self.assertEqual(parsed["versus"], {mob_type: 12}, word)

    def test_a_creature_type_the_tool_does_not_know_leaves_the_item_out(self):
        line = '<span class="q2">Equip: +12 Attack Power against Aberrations.</span><!--itemEffects:1-->'
        with self.assertRaisesRegex(tool.Unreadable, "has an effect"):
            tool.parse_tooltip(with_effects(SHACKLES, line))

    def test_other_effect_lines_leave_the_item_out(self):
        for line in [
            "Equip: Immune to Disarm.",
            "Use: Restores 100 health.",
            "Chance on hit: Wounds the target for 20 damage.",
        ]:
            odd = with_effects(SHACKLES, f'<span class="q2">{line}</span><!--itemEffects:1-->')
            with self.assertRaisesRegex(tool.Unreadable, "has an effect"):
                tool.row_for(274600, None, {**item(4, 3), "InventoryType": "9"}, odd)

    def test_lines_with_no_fight_effect_are_left_out_of_the_row(self):
        shackles = {**item(4, 3), "InventoryType": "9"}
        # The Shackles without their Defense line, which with_effects replaces.
        plain = tool.row_for(274600, None, shackles, with_effects(SHACKLES, "<!--itemEffects:0-->"))
        for line in [
            "Equip: Increases mount speed by 7% in Desert areas.",
            "Equip: Increases movement speed while mounted by 5%. This effect does not stack with "
            "similar effects.",
            "Equip: Movement speed increased by 2% in Forest and Grassland areas.",
            "Equip: Restores 4 Mana per 5 sec in Forest and Grassland areas.",
            "Equip: Underwater Breath lasts 50% longer than normal.",
            "Equip: Increased Herbalism +4.",
            "Equip: Reduces the cast time of Disenchant by 25%.",
            "Equip: Reduces damage from falling.",
            "Equip: Increases your effective stealth detection level by 2.",
            "Equip: Reduces the hit chance of Fear effects against you by 2%.",
            "Chance on hit: Knocks down all nearby enemies for 3 sec.",
            "Use: Stuns target and yourself for 3 sec. Ogres are Stunned for 5 sec and refund 50% "
            "of the cooldown. (15 Min Cooldown)",
        ]:
            odd = with_effects(SHACKLES, f'<span class="q2">{line}</span><!--itemEffects:1-->')
            self.assertEqual(tool.row_for(274600, None, shackles, odd), plain, line)

    def test_a_line_doubled_in_cities_reads_as_its_base_amount(self):
        line = (
            "Equip: Increases healing done by up to 11 and damage done by up to 4 for all magical "
            "spells and effects. This effect is doubled in Strongholds and Cities."
        )
        parsed = tool.parse_tooltip(
            with_effects(SHACKLES, f'<span class="q2">{line}</span><!--itemEffects:1-->')
        )
        self.assertEqual(parsed["stats"], {0: 7, 4: 11, 5: 4})

    def test_expertise_and_weapon_damage_read_as_the_reference_rows_hold_them(self):
        # Servomechanic Sledgehammer (1.0%: 10), Dwarven Tree Chopper (0.6%: 6), Might of
        # Cenarius (+4 Weapon Damage: 4 PhysicalDamage).
        for line, stats in [
            ("Equip: Reduces chance to be Dodged or Parried by 1.0%.", {24: 10}),
            ("Equip: Reduces chance to be Dodged or Parried by 0.6%.", {24: 6}),
            ("Equip: +4 Weapon Damage.", {40: 4}),
        ]:
            self.assertEqual(tool.equip_stats(line), stats, line)

    def test_a_weapon_skill_is_a_pseudo_stat(self):
        # Servomechanic Sledgehammer's "Increased Two-Handed Maces +1" is 1 at 35.
        lines = (
            '<span class="q2">Equip: Increased Two-Handed Maces +1.</span><br>'
            '<span class="q2">Equip: Increased Fist Weapons +2.</span><!--itemEffects:2-->'
        )
        row = tool.row_for(274600, None, {**item(4, 3), "InventoryType": "9"}, with_effects(SHACKLES, lines))
        self.assertEqual(len(row["pseudo_stats"]), 42)
        self.assertEqual(
            {index: value for index, value in enumerate(row["pseudo_stats"]) if value},
            {32: 2, 35: 1},
        )
        self.assertEqual(list(row)[-1], "pseudo_stats")

    def test_all_resistances_is_each_resistance(self):
        line = '+7 to All Resistances<br><span class="q2">Equip: Increased Defense +6.</span><!--itemEffects:1-->'
        parsed = tool.parse_tooltip(with_effects(SHACKLES, line))
        self.assertEqual(parsed["stats"], {0: 7, 25: 6, 35: 7, 36: 7, 37: 7, 38: 7, 39: 7})

    def test_spell_damage_against_a_creature_type_is_a_bonus_beside_the_row(self):
        line = (
            '<span class="q2">Equip: Increases damage done to Humanoids by magical spells and '
            'effects by up to 10.</span><!--itemEffects:1-->'
        )
        parsed = tool.parse_tooltip(with_effects(SHACKLES, line))
        self.assertEqual(parsed["versus_spell"], {"MobTypeHumanoid": 10})
        self.assertEqual(
            tool.dump_row(tool.bonus_row(279833, "Courier's Clasp", parsed)),
            '{"id":279833,"name":"Courier\'s Clasp","spell_damage_versus":{"MobTypeHumanoid":10}}',
        )

    def test_development_rows_are_not_items(self):
        for name in ["Deprecated Old Belt", "OLDRecruit's Belt", "Test Fire Sword", "Hidden Cloak (DNT)"]:
            self.assertTrue(tool.JUNK_NAME.search(name), name)
        for name in ["Worn Cloak", "Contested Sword", "Latest Boots"]:
            self.assertFalse(tool.JUNK_NAME.search(name), name)


class CommittedFileTests(unittest.TestCase):
    def test_file_passes_the_offline_audit(self):
        self.assertGreater(tool.check(), 300)

    def test_the_mallets_bonus_is_committed_with_its_row(self):
        with open(tool.OUT) as rows:
            mallet = next(json.loads(line) for line in rows if f'"id":{279885},' in line)
        self.assertEqual(mallet["scaling_options"]["0"]["stats"], {"2": 15, "17": 22, "18": 22})
        with open(tool.BONUSES) as rows:
            bonuses = {row["id"]: row for row in map(json.loads, rows)}
        self.assertEqual(
            bonuses[279885],
            {"id": 279885, "name": "Ceremonial Mallet", "melee_attack_power_versus": {"MobTypeUndead": 33}},
        )
        # Every bonus belongs to a committed row.
        with open(tool.OUT) as rows:
            ids = {json.loads(line)["id"] for line in rows}
        self.assertLessEqual(set(bonuses), ids)

    def test_existing_rows_are_written_as_the_database_writes_them(self):
        with open(tool.ITEMS) as rows:
            for _, line in zip(range(3000), rows):
                self.assertEqual(tool.dump_row(json.loads(line)), line.rstrip("\n"))


if __name__ == "__main__":
    unittest.main()
