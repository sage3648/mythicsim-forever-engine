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
# Serpentbane Crown and Bigger Rock on Bigger Stick, Forever items whose use and chance on hit the
# engine models by hand (Wowhead, 10 November 2026).
SERPENTBANE = {
    "name": "Serpentbane Crown",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Serpentbane Crown</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->42</span><!--bo--><br>Binds when '
        'picked up<!--ue--><!--ue--><table width="100%"><tr><td>Head</td><th><!--scstart4:3-->'
        '<span class="q1">Mail</span><!--scend--></th></tr></table><!--rf--><span><!--amr-->213 '
        "Armor</span><br><span><!--stat7-->+16 Stamina</span><!--ebstats--><!--egstats-->"
        "<!--eistats--><!--nameDescStats--><!--rs--><br>+8 Nature Resistance<!--e--><!--ps--><br>"
        "Durability 70 / 70</td></tr></table><table><tr><td>Requires Level <!--rlvl-->37<br>"
        '<!--rr--><!--useText:0:1--><span id="useText1" class="q2">Use: <!--useEffect:0:1--><a '
        'href="/forever/spell=1295039/serpentbane" class="q2">Inflict 89 to 112 Fire damage to '
        "your current target and an additional 225 damage over 10 sec. Inflicts 2 times as much "
        "damage to Beast and Serpentine targets.</a> (5 Min Cooldown)<!--useEffect:1--></span>"
        "<!--useText:1--><!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--></td></tr></table>"
    ),
}
BIGGER_ROCK = {
    "name": "Bigger Rock on Bigger Stick",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Bigger Rock on Bigger Stick</b><!--nend-->'
        '<!--ndstart--><!--ndend--><span class="q"><br>Item Level <!--ilvl-->47</span><!--bo-->'
        '<br>Binds when picked up<!--ue--><!--ue--><table width="100%"><tr><td>Two-Hand</td><th>'
        '<!--scstart2:5--><span class="q1">Mace</span><!--scend--></th></tr></table><!--rf-->'
        '<table width="100%"><tr>\n    <td><span><!--dmg-->112 - 168 Damage</span></td>\n    <th>'
        "Speed <!--spd-->3.40</th>\n</tr></table><!--dps-->(41.18 damage per second)<br><span>"
        "<!--stat4-->+14 Strength</span><!--ebstats--><!--egstats--><!--eistats-->"
        "<!--nameDescStats--><!--rs--><!--e--><!--ps--></td></tr></table><table><tr><td><!--rr-->"
        '<!--useText:0:1--><span id="useText1" class="q2">Chance on hit: <!--useEffect:0:1--><a '
        'href="/forever/spell=1306816/bigger-rock-to-the-face" class="q2">Bash the target in the '
        "face, causing 14 Physical damage every 2 sec for 20 sec.</a><!--useEffect:1--></span>"
        "<!--useText:1--><!--itemEffects:1--><br><span class=\"q\">&quot;That's more like it!&quot;"
        "</span><br><!--pvpEquip--><!--pvpEquip--></td></tr></table>"
    ),
}
# Gahz'ridian Detector, a head piece of the client's quest class with no armor and no stats
# (Wowhead, 11 November 2026).
DETECTOR = {
    "name": "Gahz'ridian Detector",
    "quality": 1,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q1">Gahz\'ridian Detector</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->1</span><!--bo--><br>Quest Item<br>'
        'Unique<!--ue--><!--ue--><table width="100%"><tr><td>Head</td></tr></table><!--rf-->'
        "<!--ebstats--><!--egstats--><!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps-->"
        '</td></tr></table><table><tr><td><!--rr--><!--useText:0:1--><span id="useText1" '
        'class="q2">Equip: <!--useEffect:0:1--><a href="/forever/spell=11992/detect-gahzridian" '
        'class="q2">Nearby Gahz\'ridian appears on the minimap.</a><!--useEffect:1--></span>'
        "<!--useText:1--><!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--></td></tr></table>"
    ),
}
# Toofbreaker, a Forever mace whose chance on hit the engine models by hand (Wowhead, 11 November
# 2026).
TOOFBREAKER = {
    "name": "Toofbreaker",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Toofbreaker</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->45</span><!--bo--><br>Binds '
        'when picked up<!--ue--><!--ue--><table width="100%"><tr><td>One-Hand</td><th><!--scstart2:4-->'
        '<span class="q1">Mace</span><!--scend--></th></tr></table><!--rf--><table width="100%">'
        '<tr>\n    <td><span><!--dmg-->40 - 75 Damage</span></td>\n    <th>Speed <!--spd-->'
        '1.90</th>\n</tr></table><!--dps-->(30.26 damage per second)<br><span><!--stat4-->'
        '+7 Strength</span><br><span><!--stat7-->+5 Stamina</span><!--ebstats--><!--egstats-->'
        '<!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--><br>Durability 90 / '
        '90</td></tr></table><table><tr><td>Requires Level <!--rlvl-->40<br><!--rr--><!--useText:0:1-->'
        '<span id="useText1" class="q2">Chance on hit: <!--useEffect:0:1--><a href="/forever/spell=1320269/toofbreak" '
        'class="q2">Bash the target, causing 42 Physical damage. Deals 2 times as much damage '
        'if the target is facing you.</a><!--useEffect:1--></span><!--useText:1--><!--itemEffects:1-->'
        '<br><!--pvpEquip--><!--pvpEquip--><div class="whtt-sellprice">Sell Price: <span '
        'class="moneygold">2</span> <span class="moneysilver">3</span> <span class="moneycopper">'
        '18</span></div><div class="whtt-extra whtt-droppedby">Dropped by: Bossman Toof</div>'
        '<div class="whtt-extra whtt-dropchance">Drop Chance: 20.00%</div></td></tr></table>'
    ),
}
# Dreadfrost Saber, a Forever sword whose equip line the engine models by hand (Wowhead, 10 November
# 2026).
DREADFROST = {
    "name": "Dreadfrost Saber",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Dreadfrost Saber</b><!--nend--><!--ndstart-->'
        '<!--ndend--><span class="q"><br>Item Level <!--ilvl-->40</span><!--bo--><br>Binds when '
        'picked up<!--ue--><!--ue--><table width="100%"><tr><td>Main Hand</td><th>'
        '<!--scstart2:7--><span class="q1">Sword</span><!--scend--></th></tr></table><!--rf-->'
        '<table width="100%"><tr>\n    <td><span><!--dmg-->46 - 86 Damage</span></td>\n    <th>'
        "Speed <!--spd-->2.40</th>\n</tr></table><!--dps-->(27.50 damage per second)<br><span>"
        "<!--stat7-->+8 Stamina</span><!--ebstats--><!--egstats--><!--eistats-->"
        "<!--nameDescStats--><!--rs--><!--e--><!--ps--></td></tr></table><table><tr><td>Requires "
        'Level <!--rlvl-->35<br><!--rr--><!--useText:0:1--><span id="useText1" class="q2">Equip: '
        '<!--useEffect:0:1--><a href="/forever/spell=1294939/dreadfrost-saber" class="q2">Adds 1 '
        "Frost damage to your melee attacks.</a><!--useEffect:1--></span><!--useText:1-->"
        "<!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--></td></tr></table>"
    ),
}
# Totem of Rage, a relic whose equip line the engine models by hand (Wowhead Forever, 11 November
# 2026).
TOTEM_OF_RAGE = {
    "name": "Totem of Rage",
    "quality": 3,
    "tooltip": (
        "<table><tr><td><!--nstart--><b class=\"q3\">Totem of Rage</b><!--nend--><!--ndstart-->"
        "<!--ndend--><span class=\"q\"><br>Item Level <!--ilvl-->57</span><!--bo--><br>Binds wh"
        "en picked up<!--ue--><!--ue--><table width=\"100%\"><tr><td>Relic</td><th><!--scstart4"
        ":9--><span class=\"q1\">Totem</span><!--scend--></th></tr></table><!--rf--><!--ebstats"
        "--><!--egstats--><!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--></td></"
        "tr></table><table><tr><td>Requires Level <!--rlvl-->52<br><!--rr--><!--useText:0:1--"
        "><span id=\"useText1\" class=\"q2\">Equip: <!--useEffect:0:1--><a href=\"/forever/spell=2"
        "7859/increased-shock-damage\" class=\"q2\">Increases the damage of your Shock spells by"
        " 2%.</a><!--useEffect:1--></span><!--useText:1--><!--itemEffects:1--><br><!--pvpEqui"
        "p--><!--pvpEquip--><div class=\"whtt-sellprice\">Sell Price: <span class=\"moneygold\">1"
        "</span> <span class=\"moneysilver\">27</span> <span class=\"moneycopper\">69</span></div"
        "><div class=\"whtt-extra whtt-droppedby\">Dropped by: Magmus</div><div class=\"whtt-ext"
        "ra whtt-dropchance\">Drop Chance: 7.95%</div></td></tr></table>"
    ),
}
# Maw of Min'loth, a Forever shield whose equip line the engine models by hand (Wowhead Forever, 11
# November 2026).
MAW_OF_MINLOTH = {
    "name": "Maw of Min'loth",
    "quality": 3,
    "tooltip": (
        "<table><tr><td><!--nstart--><b class=\"q3\">Maw of Min'loth</b><!--nend--><!--ndstart-"
        "-><!--ndend--><span class=\"q\"><br>Item Level <!--ilvl-->42</span><!--bo--><br>Binds "
        "when picked up<!--ue--><!--ue--><table width=\"100%\"><tr><td>Off Hand</td><th><!--scs"
        "tart4:6--><span class=\"q1\">Shield</span><!--scend--></th></tr></table><!--rf--><span"
        "><!--amr-->1287 Armor</span><br>22 Block<br><span><!--stat7-->+8 Stamina</span><!--e"
        "bstats--><!--egstats--><!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--><"
        "br>Durability 100 / 100</td></tr></table><table><tr><td>Requires Level <!--rlvl-->37"
        "<br><!--rr--><!--useText:0:1--><span id=\"useText1\" class=\"q2\">Equip: <!--useEffect:0"
        ":1--><a href=\"/forever/spell=1295064/maw-of-minloth\" class=\"q2\">When struck in melee"
        " combat, has a 10% chance of stealing 40 life from the target enemy. (Proc chance: 1"
        "0%, 3s cooldown)</a><!--useEffect:1--></span><!--useText:1--><!--itemEffects:1--><br"
        "><!--pvpEquip--><!--pvpEquip--><div class=\"whtt-sellprice\">Sell Price: <span class=\""
        "moneysilver\">99</span> <span class=\"moneycopper\">7</span></div><div class=\"whtt-extr"
        "a whtt-droppedby\">Dropped by: Min'loth the Serpent</div><div class=\"whtt-extra whtt-"
        "dropchance\">Drop Chance: 35.00%</div></td></tr></table>"
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


# Wowhead Forever tooltips of 12 November 2026.
HEART_OF_THE_PRIMEVAL = {
    "name": "Heart of the Primeval",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Heart of the Primeval</b><!--nend--><!--nd'
        'start--><!--ndend--><span class="q"><br>Item Level <!--ilvl-->40</span><!--bo--><br>'
        'Binds when picked up<br>Unique-Equipped<!--ue--><!--ue--><table width="100%"><tr><td'
        '>Trinket</td><th><!--scstart4:-4--><span class="q1"></span><!--scend--></th></tr></t'
        'able><!--rf--><!--ebstats--><!--egstats--><!--eistats--><!--nameDescStats--><!--rs--'
        '>+18 Frost Resistance<!--e--><!--ps--></td></tr></table><table><tr><td>Requires Leve'
        'l <!--rlvl-->35<br><!--rr--><!--useText:0:2--><span id="useText2" class="q2">Use: <!'
        '--useEffect:0:2--><a href="/forever/spell=1295233/heart-of-the-primeval" class="q2">'
        'Increases damage and healing done by magical spells and effects by up to 19 for 30 s'
        'ec.</a> (6 Min Cooldown)<!--useEffect:2--></span><!--useText:2--><!--itemEffects:1--'
        '><br><!--pvpEquip--><!--pvpEquip--><div class="whtt-sellprice">Sell Price: <span cla'
        'ss="moneysilver">54</span> <span class="moneycopper">44</span></div><div class="whtt'
        '-extra whtt-droppedby">Dropped by: Primeval Elemental</div><div class="whtt-extra wh'
        'tt-dropchance">Drop Chance: 41.83%</div></td></tr></table>'
    ),
}

ZEALOT_PLATE = {
    "name": "Zealot Plate",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Zealot Plate</b><!--nend--><!--ndstart--><'
        '!--ndend--><span class="q"><br>Item Level <!--ilvl-->48</span><!--bo--><br>Binds whe'
        'n picked up<!--ue--><!--ue--><table width="100%"><tr><td>Chest</td><th><!--scstart4:'
        '4--><span class="q1">Plate</span><!--scend--></th></tr></table><!--rf--><span><!--am'
        'r-->509 Armor</span><br><span><!--stat4-->+13 Strength</span><!--ebstats--><!--egsta'
        'ts--><!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--><br>Durability 135 '
        '/ 135</td></tr></table><table><tr><td>Requires Level <!--rlvl-->43<br><!--rr--><!--u'
        'seText:0:1--><span id="useText1" class="q2">Equip: <!--useEffect:0:1--><a href="/for'
        'ever/spell=1294668/proc-self-energize-88" class="q2">When struck in combat, has a ch'
        'ance to energize you for 66 to 110 mana. This effect cannot occur more than once eve'
        'ry 15 sec. (15s cooldown)</a><!--useEffect:1--></span><!--useText:1--><!--itemEffect'
        's:1--><br><!--pvpEquip--><!--pvpEquip--><div class="whtt-sellprice">Sell Price: <spa'
        'n class="moneygold">1</span> <span class="moneysilver">78</span> <span class="moneyc'
        'opper">34</span></div><div class="whtt-extra whtt-droppedby">Dropped by: Hydromancer'
        ' Velratha</div><div class="whtt-extra whtt-dropchance">Drop Chance: 31.79%</div></td'
        '></tr></table>'
    ),
}

GIRDLE_OF_EXTRACTED_ANSWERS = {
    "name": "Girdle of Extracted Answers",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Girdle of Extracted Answers</b><!--nend-->'
        '<!--ndstart--><!--ndend--><span class="q"><br>Item Level <!--ilvl-->53</span><!--bo-'
        '-><br>Binds when picked up<!--ue--><!--ue--><table width="100%"><tr><td>Waist</td><t'
        'h><!--scstart4:4--><span class="q1">Plate</span><!--scend--></th></tr></table><!--rf'
        '--><span><!--amr-->313 Armor</span><br><span><!--stat4-->+12 Strength</span><br><spa'
        'n><!--stat7-->+15 Stamina</span><!--ebstats--><!--egstats--><!--eistats--><!--nameDe'
        'scStats--><!--rs--><!--e--><!--ps--></td></tr></table><table><tr><td>Requires Level '
        '<!--rlvl-->48<br><!--rr--><!--useText:0:1--><span id="useText1" class="q2">Equip: <!'
        '--useEffect:0:1--><a href="/forever/spell=1308777/shield-spike" class="q2">Deals 20 '
        'to 30 Physical damage every time you block.</a><!--useEffect:1--></span><!--useText:'
        '1--><!--itemEffects:1--><br><!--pvpEquip--><!--pvpEquip--><div class="whtt-sellprice'
        '">Sell Price: <span class="moneygold">1</span> <span class="moneysilver">13</span> <'
        'span class="moneycopper">34</span></div><div class="whtt-extra whtt-droppedby">Dropp'
        'ed by: The Questioner</div><div class="whtt-extra whtt-dropchance">Drop Chance: 27.2'
        '7%</div></td></tr></table>'
    ),
}

DEFIAS_ENFORCERS_MASK = {
    "name": "Defias Enforcer's Mask",
    "quality": 3,
    "tooltip": (
        '<table><tr><td><!--nstart--><b class="q3">Defias Enforcer\'s Mask</b><!--nend--><!--n'
        'dstart--><!--ndend--><span class="q"><br>Item Level <!--ilvl-->53</span><!--bo--><br'
        '>Binds when equipped<!--ue--><!--ue--><table width="100%"><tr><td>Head</td><th><!--s'
        'cstart4:2--><span class="q1">Leather</span><!--scend--></th></tr></table><!--rf--><s'
        'pan><!--amr-->124 Armor</span><br><span><!--stat7-->+20 Stamina</span><!--ebstats-->'
        '<!--egstats--><!--eistats--><!--nameDescStats--><!--rs--><!--e--><!--ps--></td></tr>'
        '</table><table><tr><td>Requires Level <!--rlvl-->48<br><!--rr--><span class="q2">Equ'
        'ip: Reduces chance to be Dodged or Parried by 1.0%.</span><br><span class="q2">Equip'
        ': +<!--rtg38-->30 Attack Power.</span><!--itemEffects:1--><br /><br /><span class="q'
        '"><a href="/forever/item-set=2136/defias-enforcers-garb" class="q">Defias Enforcer\'s'
        ' Garb</a> (0/5)</span><div class="q0 indent"><span><!--si287019--><a href="/forever/'
        'item=287019/defias-enforcers-mask">Defias Enforcer\'s Mask</a></span><br /><span><!--'
        'si276741--><a href="/forever/item=276741/defias-enforcers-shoulderpads">Defias Enfor'
        'cer\'s Shoulderpads</a></span><br /><span><!--si287020--><a href="/forever/item=28702'
        '0/defias-enforcers-chest">Defias Enforcer\'s Chest</a></span><br /><span><!--si276746'
        '--><a href="/forever/item=276746/defias-enforcers-legguards">Defias Enforcer\'s Leggu'
        'ards</a></span><br /><span><!--si287021--><a href="/forever/item=287021/defias-enfor'
        'cers-prowlers">Defias Enforcer\'s Prowlers</a></span></div><br /><span class="q0"><sp'
        'an>(2) Set : <a href="/forever/spell=1324673/increased-fire-resist-10">+10 Fire Resi'
        'stance.</a></span><br /><span>(3) Set : <a href="/forever/spell=1324674/increased-st'
        'amina-10">+15 Stamina.</a></span><br /><span>(4) Set : <a href="/forever/spell=13246'
        '75/increased-1h-sword">Increased Swords +1.</a></span><br /><span>(5) Set : <a href='
        '"/forever/spell=1324684/defias-juggernaut">Melee attacks have a chance to increase y'
        'our Critical Strike chance by 2% for 20 sec. This effect is doubled in swamps.</a></'
        'span></span><br><!--pvpEquip--><!--pvpEquip--><div class="whtt-sellprice">Sell Price'
        ': <span class="moneygold">1</span> <span class="moneysilver">24</span> <span class="'
        'moneycopper">99</span></div></td></tr></table><!--i?287019:1:60:48-->'
    ),
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

    def test_a_worn_quest_item_is_an_armor_row_of_no_armor_type(self):
        # Item row 9978: class 12 (quest), subclass 0, InventoryType 1.
        quest = {**item(tool.QUEST_CLASS, 0), "InventoryType": "1"}
        sparse = {"Display_lang": "Gahz'ridian Detector", "ItemLevel": "1", "InventoryType": "1"}
        self.assertEqual(
            tool.dump_row(tool.row_for(9978, sparse, quest, DETECTOR)),
            '{"id":9978,"name":"Gahz\'ridian Detector","type":"ItemTypeHead",'
            '"scaling_options":{"0":{"ilvl":1}},"unique":true}',
        )
        # A quest item that is not listed as worn is no row.
        with self.assertRaisesRegex(tool.Unreadable, "not listed as worn"):
            tool.row_for(9979, sparse, quest, DETECTOR)

    def test_a_use_or_chance_on_hit_the_engine_models_is_left_out_of_the_row(self):
        crown = tool.row_for(274609, None, {**item(4, 3), "InventoryType": "1"}, SERPENTBANE)
        self.assertEqual(
            tool.dump_row(crown),
            '{"id":274609,"name":"Serpentbane Crown","type":"ItemTypeHead","armor_type":'
            '"ArmorTypeMail","scaling_options":{"0":{"stats":{"2":16,"30":213,"38":8},"ilvl":42}}}',
        )
        rock = tool.row_for(279480, None, {**item(2, 5), "InventoryType": "17"}, BIGGER_ROCK)
        self.assertEqual(
            tool.dump_row(rock),
            '{"id":279480,"name":"Bigger Rock on Bigger Stick","type":"ItemTypeWeapon",'
            '"weapon_type":"WeaponTypeMace","hand_type":"HandTypeTwoHand","weapon_speed":3.4,'
            '"scaling_options":{"0":{"weapon_damage_min":112,"weapon_damage_max":168,'
            '"stats":{"0":14},"ilvl":47}}}',
        )
        self.assertIsNone(tool.bonus_row(279480, rock["name"], tool.parse_tooltip(BIGGER_ROCK)))

    def test_a_direct_chance_on_hit_the_engine_models_is_left_out_of_the_row(self):
        mace = tool.row_for(286430, None, {**item(2, 4), "InventoryType": "13"}, TOOFBREAKER)
        self.assertEqual(
            tool.dump_row(mace),
            '{"id":286430,"name":"Toofbreaker","type":"ItemTypeWeapon","weapon_type":'
            '"WeaponTypeMace","hand_type":"HandTypeOneHand","weapon_speed":1.9,'
            '"scaling_options":{"0":{"weapon_damage_min":40,"weapon_damage_max":75,'
            '"stats":{"0":7,"2":5},"ilvl":45}}}',
        )
        # Other numbers are another effect: the item stays out until the engine follows.
        odd = json.loads(json.dumps(TOOFBREAKER).replace("causing 42 Physical", "causing 50 Physical"))
        with self.assertRaisesRegex(tool.Unreadable, "has an effect"):
            tool.row_for(286430, None, {**item(2, 4), "InventoryType": "13"}, odd)

    def test_an_equip_line_the_engine_models_is_left_out_of_the_row(self):
        saber = tool.row_for(274603, None, {**item(2, 7), "InventoryType": "21"}, DREADFROST)
        self.assertEqual(
            tool.dump_row(saber),
            '{"id":274603,"name":"Dreadfrost Saber","type":"ItemTypeWeapon","weapon_type":'
            '"WeaponTypeSword","hand_type":"HandTypeMainHand","weapon_speed":2.4,'
            '"scaling_options":{"0":{"weapon_damage_min":46,"weapon_damage_max":86,'
            '"stats":{"2":8},"ilvl":40}}}',
        )
        self.assertIsNone(tool.bonus_row(274603, saber["name"], tool.parse_tooltip(DREADFROST)))

    def test_a_relic_is_a_ranged_slot_row_of_its_kind(self):
        # As the reference rows hold a relic (Totem of the Storm 23199). Totem of Rage has an
        # ItemSparse row, so the tooltip's "Relic" is only read past.
        totem = tool.row_for(
            22395, {"ItemLevel": "57", "Display_lang": "Totem of Rage", "InventoryType": "28"},
            item(4, 9), TOTEM_OF_RAGE,
        )  # fmt: skip
        self.assertEqual(
            tool.dump_row(totem),
            '{"id":22395,"name":"Totem of Rage","type":"ItemTypeRanged","ranged_weapon_type":'
            '"RangedWeaponTypeTotem","scaling_options":{"0":{"ilvl":57}}}',
        )
        with self.assertRaisesRegex(tool.Unreadable, "relic subclass 0"):
            tool.row_for(
                22395, {"ItemLevel": "57", "Display_lang": "Totem of Rage", "InventoryType": "28"},
                item(4, 0), TOTEM_OF_RAGE,
            )  # fmt: skip

    def test_a_struck_proc_the_engine_models_is_left_out_of_the_row(self):
        maw = tool.row_for(274608, None, {**item(4, 6), "InventoryType": "14"}, MAW_OF_MINLOTH)
        self.assertEqual(
            tool.dump_row(maw),
            '{"id":274608,"name":"Maw of Min\'loth","type":"ItemTypeWeapon","weapon_type":'
            '"WeaponTypeShield","hand_type":"HandTypeOffHand","scaling_options":{"0":{"stats":'
            '{"2":8,"27":22,"30":1287},"ilvl":42}}}',
        )

    def test_a_struck_effect_the_engine_refuses_is_left_out_of_the_row(self):
        # Zealot Plate's mana when struck and Girdle of Extracted Answers' damage on block: the
        # rows are the plain stats, and the engine refuses the character the effect can reach.
        plate = tool.row_for(274651, None, {**item(4, 4), "InventoryType": "5"}, ZEALOT_PLATE)
        self.assertEqual(
            tool.dump_row(plate),
            '{"id":274651,"name":"Zealot Plate","type":"ItemTypeChest","armor_type":'
            '"ArmorTypePlate","scaling_options":{"0":{"stats":{"0":13,"30":509},"ilvl":48}}}',
        )
        girdle = tool.row_for(
            276742, None, {**item(4, 4), "InventoryType": "6"}, GIRDLE_OF_EXTRACTED_ANSWERS
        )
        self.assertEqual(
            tool.dump_row(girdle),
            '{"id":276742,"name":"Girdle of Extracted Answers","type":"ItemTypeWaist",'
            '"armor_type":"ArmorTypePlate","scaling_options":{"0":{"stats":{"0":12,"2":15,'
            '"30":313},"ilvl":53}}}',
        )

    def test_a_stat_use_is_the_rows_item_effect(self):
        # As the reference rows hold a stat trinket's use (Talisman of Ephemeral Power 18820).
        heart = tool.row_for(274615, None, {**item(4, 0), "InventoryType": "12"}, HEART_OF_THE_PRIMEVAL)
        self.assertEqual(
            tool.dump_row(heart),
            '{"id":274615,"name":"Heart of the Primeval","type":"ItemTypeTrinket",'
            '"scaling_options":{"0":{"stats":{"37":18},"ilvl":40}},"item_effects":[{"buff_id":'
            '1295233,"buff_name":"Heart of the Primeval (1295233)","effect_duration_ms":30000,'
            '"scaling_options":{"0":{"stats":{"4":19,"5":19}}},"on_use":{"cooldown_ms":360000,'
            '"category_id":1141,"category_cooldown_ms":30000}}],"unique":true}',
        )
        # The same line on another item is not that item's listed use.
        with self.assertRaisesRegex(tool.Unreadable, "listed for another item"):
            tool.row_for(274616, None, {**item(4, 0), "InventoryType": "12"}, HEART_OF_THE_PRIMEVAL)

    def test_a_piece_of_the_second_client_set_reads_its_stats_and_set(self):
        mask = tool.row_for(287019, None, {**item(4, 2), "InventoryType": "1"}, DEFIAS_ENFORCERS_MASK)
        self.assertEqual(
            tool.dump_row(mask),
            '{"id":287019,"name":"Defias Enforcer\'s Mask","type":"ItemTypeHead","armor_type":'
            '"ArmorTypeLeather","set_name":"Defias Enforcer\'s Garb","set_id":2136,'
            '"scaling_options":{"0":{"stats":{"2":20,"17":30,"18":30,"24":10,"30":124},"ilvl":53}}}',
        )

    def test_a_modelled_line_with_other_numbers_leaves_the_item_out(self):
        # The engine states the use's numbers, so a tooltip that changes them waits for it.
        for response, old, new in [
            (SERPENTBANE, "89 to 112", "90 to 113"),
            (SERPENTBANE, "(5 Min Cooldown)", "(3 Min Cooldown)"),
            (BIGGER_ROCK, "14 Physical", "15 Physical"),
            (DREADFROST, "Adds 1 Frost", "Adds 2 Frost"),
            (TOTEM_OF_RAGE, "by 2%", "by 3%"),
            (MAW_OF_MINLOTH, "stealing 40 life", "stealing 50 life"),
            (MAW_OF_MINLOTH, "3s cooldown", "5s cooldown"),
            (ZEALOT_PLATE, "66 to 110 mana", "70 to 120 mana"),
            (GIRDLE_OF_EXTRACTED_ANSWERS, "20 to 30 Physical", "25 to 35 Physical"),
            (HEART_OF_THE_PRIMEVAL, "by up to 19 for", "by up to 25 for"),
            (HEART_OF_THE_PRIMEVAL, "(6 Min Cooldown)", "(2 Min Cooldown)"),
        ]:
            changed = {**response, "tooltip": response["tooltip"].replace(old, new)}
            self.assertNotEqual(changed["tooltip"], response["tooltip"])
            with self.assertRaisesRegex(tool.Unreadable, "has an effect"):
                tool.parse_tooltip(changed)

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
    def test_a_listed_set_piece_carries_its_set_and_an_unlisted_one_is_left_out(self):
        lines = [
            "Krol'dok Warplate Chest", "Item Level 46", "Chest", "Plate", "490 Armor",
            "Krol'dok Battlegear (0/5)", "Helmet", "Pauldrons", "Chest", "Legplates", "Gauntlets",
            "(2) Set : +100 Armor.", "(3) Set : Increased Defense +5.",
        ]  # fmt: skip
        self.assertEqual(tool.without_client_set(lines), (lines[:5], "Krol'dok Battlegear"))
        other = [line.replace("Krol'dok Battlegear", "Garb of No Listed Set") for line in lines]
        self.assertEqual(tool.without_client_set(other), (other, None))
        # A block that does not close the tooltip with its bonus lines is not read.
        self.assertEqual(tool.without_client_set(lines + ["Use: Something."])[1], None)
        self.assertEqual(tool.without_client_set(lines[:8])[1], None)
        rows = {row["id"]: row for row in map(json.loads, tool.OUT.read_text().splitlines())}
        for item_id in (275005, 275006, 287012, 287013, 287014):
            self.assertEqual(rows[item_id]["set_name"], "Krol'dok Battlegear")
            self.assertEqual(rows[item_id]["set_id"], tool.CLIENT_SETS["Krol'dok Battlegear"])
        for item_id in (276741, 276746, 287019, 287020, 287021):
            self.assertEqual(rows[item_id]["set_name"], "Defias Enforcer's Garb")
            self.assertEqual(rows[item_id]["set_id"], tool.CLIENT_SETS["Defias Enforcer's Garb"])
        self.assertEqual(sum("set_name" in row for row in rows.values()), 10)

    def test_a_real_item_with_a_development_name_is_taken(self):
        self.assertTrue(tool.JUNK_NAME.search("Old Leather Jacket"))
        self.assertIn(280505, tool.REAL_ITEMS)
        rows = {row["id"]: row for row in map(json.loads, tool.OUT.read_text().splitlines())}
        self.assertEqual(rows[280505]["name"], "Old Leather Jacket")
        self.assertEqual(
            rows[280505]["scaling_options"]["0"],
            {"stats": {"0": 15, "1": 2, "2": 2, "30": 109}, "ilvl": 39},
        )

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

    def test_the_modelled_effect_items_are_committed(self):
        with open(tool.OUT) as rows:
            ids = {json.loads(line)["id"] for line in rows}
        self.assertLessEqual({22395, 274603, 274608, 274609, 279480, 279852, 286430}, ids)
        # The stat use and the two struck effects of 12 November 2026.
        self.assertLessEqual({274615, 274651, 276742}, ids)
        # The two chance on hit weapons and the spell cast effect of 13 November 2026.
        self.assertLessEqual({274647, 286650, 287005}, ids)
        # The chance on hit weapon, the Feral Combat skill and the bleed condition of 13
        # November 2026.
        self.assertLessEqual({276419, 276743, 280016}, ids)

    def test_the_worn_quest_item_is_committed(self):
        with open(tool.OUT) as rows:
            detector = next(json.loads(line) for line in rows if '"id":9978,' in line)
        self.assertEqual(detector["type"], "ItemTypeHead")
        self.assertNotIn("armor_type", detector)
        self.assertEqual(detector["scaling_options"], {"0": {"ilvl": 1}})

    def test_existing_rows_are_written_as_the_database_writes_them(self):
        with open(tool.ITEMS) as rows:
            for _, line in zip(range(3000), rows):
                self.assertEqual(tool.dump_row(json.loads(line)), line.rstrip("\n"))


if __name__ == "__main__":
    unittest.main()
