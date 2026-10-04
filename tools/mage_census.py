#!/usr/bin/env python3
"""Write the Mage census: one change at a time on the four production Mage requests.

Reads arcane-mage, fire-mage, frost-mage and frostfire-mage .request.json from --bases and
writes one RaidSimRequest per variant into --output:

- each talent raised to its maximum or removed, where that changes the build;
- each other mage armor option;
- each spell the pinned Go Mage registers, cast from the top of the priority list;
- each talent spell or spell talent together with the spell it changes, as Blast Wave or
  Improved Blizzard, cast from the top;
- each other race a Mage can be;
- the pinned reference's Mage presets: every rotation preset with the talent presets of
  its school, on the P0 and P1 gear presets. The launch gear set is left out because its
  item 276765 panics in the pinned Go engine itself.

Talent trees and presets are read from the pinned checkout (--ui). Uses only Python's
standard library; compare the output with tools/prepared_v2.py compare.
"""

import argparse
import copy
import json
from pathlib import Path

BUILDS = ['arcane', 'fire', 'frost', 'frostfire']
SPELLS = {
    'arcane-explosion': 10202, 'blast-wave': 13021, 'blizzard': 10187, 'cone-of-cold': 10161,
    'fire-blast': 10199, 'fireball': 25306, 'flamestrike': 10216, 'frost-nova': 10230,
    'frostbolt': 25304, 'frostfire-bolt': 1237313, 'ice-lance': 1240047, 'pyroblast': 18809,
    'scorch': 10207, 'arcane-missiles': 25345, 'arcane-blast': 1239700, 'evocation': 12051,
    'presence-of-mind': 12043, 'arcane-power': 12042, 'combustion': 11129, 'cold-snap': 12472,
    'mage-armor': 22783, 'ice-armor': 10220, 'frost-armor': 7301,
}
# A talent spell or a talent that changes a spell acts only with both.
COMBOS = [
    ('blast-wave', ['blastWave']),
    ('flamestrike', ['improvedFlamestrike']),
    ('frost-nova', ['improvedFrostNova']),
    ('cone-of-cold', ['improvedConeOfCold']),
    ('blizzard', ['improvedBlizzard']),
    ('blizzard', ['improvedBlizzard', 'fingersOfFrost']),
    ('cone-of-cold', ['improvedConeOfCold', 'fingersOfFrost']),
    ('pyroblast', ['pyroblast']),
    ('presence-of-mind', ['presenceOfMind']),
    ('arcane-power', ['arcanePower']),
    ('combustion', ['combustion']),
    ('cold-snap', ['coldSnap']),
    ('ice-lance', ['iceLance']),
    ('arcane-blast', ['arcaneBlast']),
]
ARMORS = ['MageArmorNone', 'MageArmorFrostArmor', 'MageArmorMoltenArmor', 'MageArmorMageArmor']
RACES = ['RaceHuman', 'RaceGnome', 'RaceUndead', 'RaceTroll', 'RaceOrc', 'RaceHighOrderSkyborne',
         'RaceWindshaperSkyborne', 'RaceNightElf', 'RaceDwarf', 'RaceTauren']
# ui/specs/mage/dps/presets.ts: each rotation preset with the talent presets of its school.
PRESETS = {
    'frost': ('frost', ['0502050030003--055500033100030024', '050005013--0555003301001301251']),
    'arcane': ('arcane', ['050215003100311531-2305003202003-', '055005023100311531--005500033']),
    'fire': ('fire', ['0502252000003-23550000130133051-', '-03552020130133151-005500033']),
    'fire_lowrank': ('fire', ['0502252000003-23550000130133051-', '-03552020130133151-005500033']),
    'frostfire': ('frostfire', [None]),
}
GEARS = {'p0': 'p0.bis.gear.json', 'p1': 'p1.bis.gear.json'}


def player(request):
    return request['raid']['parties'][0]['players'][0]


class Census:
    def __init__(self, ui, output):
        trees = json.loads((ui / 'sim/talents/trees/mage.json').read_text())
        self.max_points = [[t['maxPoints'] for t in tree['talents']] for tree in trees]
        self.names = [[t['fieldName'] for t in tree['talents']] for tree in trees]
        self.ui = ui
        self.output = output
        self.count = 0

    def parse(self, talents):
        digits = talents.split('-')
        digits += [''] * (3 - len(digits))
        return [[int(c) for c in tree.ljust(len(self.max_points[i]), '0')] for i, tree in enumerate(digits)]

    @staticmethod
    def render(points):
        return '-'.join(''.join(map(str, tree)).rstrip('0') for tree in points).rstrip('-')

    def write(self, name, request):
        (self.output / f'{name}.json').write_text(json.dumps(request))
        self.count += 1

    def build(self, name, base):
        points = self.parse(player(base)['talentsString'])
        for t, tree in enumerate(points):
            for i, rank in enumerate(tree):
                for target, label in ((self.max_points[t][i], 'max'), (0, 'zero')):
                    if rank != target:
                        request = copy.deepcopy(base)
                        changed = copy.deepcopy(points)
                        changed[t][i] = target
                        player(request)['talentsString'] = self.render(changed)
                        self.write(f'{name}--{self.names[t][i]}-{label}', request)
        options = player(base)['mage']['options']['classOptions']
        for armor in ARMORS:
            if armor != options.get('defaultMageArmor'):
                request = copy.deepcopy(base)
                player(request)['mage']['options']['classOptions']['defaultMageArmor'] = armor
                self.write(f'{name}--{armor}', request)
        for spell, spell_id in SPELLS.items():
            request = copy.deepcopy(base)
            self.cast_first(request, spell_id)
            self.write(f'{name}--cast-{spell}', request)
        for spell, talents in COMBOS:
            request = copy.deepcopy(base)
            changed = copy.deepcopy(points)
            for talent in talents:
                t = next(i for i, tree in enumerate(self.names) if talent in tree)
                i = self.names[t].index(talent)
                changed[t][i] = self.max_points[t][i]
            player(request)['talentsString'] = self.render(changed)
            self.cast_first(request, SPELLS[spell])
            self.write(f'{name}--cast-{spell}-with-{"-".join(talents)}', request)
        for race in RACES:
            if race != player(base)['race']:
                request = copy.deepcopy(base)
                player(request)['race'] = race
                self.write(f'{name}--{race}', request)

    @staticmethod
    def cast_first(request, spell_id):
        player(request)['rotation']['priorityList'].insert(
            0, {'action': {'castSpell': {'spellId': {'spellId': spell_id}}}})

    def presets(self, bases):
        specs = self.ui / 'specs/mage/dps'
        for apl, (build, talent_sets) in PRESETS.items():
            rotation = json.loads((specs / f'apls/{apl}.apl.json').read_text())
            for t, talents in enumerate(talent_sets):
                for gear_name, gear_file in GEARS.items():
                    request = copy.deepcopy(bases[build])
                    player(request)['rotation'] = rotation
                    if talents:
                        player(request)['talentsString'] = talents
                    player(request)['equipment'] = json.loads((specs / f'gear_sets/{gear_file}').read_text())
                    self.write(f'preset--{apl}-talents{t}-{gear_name}', request)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--bases', type=Path, required=True, help='directory of the four production requests')
    parser.add_argument('--ui', type=Path, default=Path('oracle-cache/source/ui'), help='the pinned ui directory')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    census = Census(args.ui, args.output)
    bases = {build: json.loads((args.bases / f'{build}-mage.request.json').read_text()) for build in BUILDS}
    for build in BUILDS:
        census.build(build, bases[build])
    census.presets(bases)
    print(census.count, 'variants')


if __name__ == '__main__':
    main()
