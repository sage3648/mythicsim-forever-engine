#!/usr/bin/env python3
"""Write the Warlock census: one change at a time on the production Warlock requests.

Reads affliction-warlock, demonology-warlock and destruction-warlock .request.json from --bases
and writes one RaidSimRequest per variant into --output:

- each spell the pinned Go Warlock casts, cast from the top of the priority list, and each
  spell a talent grants cast with that talent at its maximum;
- a multidot line for each dot spell, and for the four main dots together, with the talents
  that grant or act on them at their maximum;
- each talent raised to its maximum or removed, where that changes the build, with Rain of
  Fire and Hellfire in the rotation behind a `numberTargets` condition, so a fight against
  several targets casts them.

Talent trees are read from the pinned checkout (--ui). A base holding several copies of the
boss gives a census of a fight against several targets. Uses only Python's standard library;
compare the output with tools/prepared_v2.py compare.
"""

import argparse
import copy
import json
from pathlib import Path

BUILDS = ['affliction', 'demonology', 'destruction']
# The highest rank of each Warlock spell the application's rotations name.
SPELLS = {
    'rain-of-fire': 11678, 'hellfire': 11684, 'shadow-bolt': 25307, 'immolate': 25309, 'corruption': 25311,
    'bane-of-agony': 11713, 'curse-of-the-elements': 1311680, 'curse-of-recklessness': 11717,
    'searing-pain': 17923, 'soul-fire': 17924, 'shadowburn': 18871, 'conflagrate': 18932, 'life-tap': 11689,
    'drain-life': 11700, 'siphon-life': 18881, 'death-coil': 17926, 'incinerate': 1293813,
    'bane-of-doom': 603, 'wrack': 1316697, 'bane-of-havoc': 1225228, 'amplify-curse': 18288,
}
RAIN_OF_FIRE, HELLFIRE = 11678, 11684
# A spell that a talent grants acts only with that talent, so it is cast with it as well.
COMBOS = [
    ('wrack', 'wrack'), ('amplify-curse', 'amplifyCurse'), ('siphon-life', 'siphonLife'),
    ('bane-of-havoc', 'baneOfHavoc'), ('conflagrate', 'conflagrate'), ('shadowburn', 'shadowburn'),
    ('incinerate', 'incinerate'),
]
BANE_OF_HAVOC = 1225228
# The dot spells a multidot line can keep up on every target, with the talents that grant them.
MULTIDOTS = {
    'corruption': 25311, 'bane-of-agony': 11713, 'immolate': 25309, 'siphon-life': 18881, 'bane-of-doom': 603,
    'drain-life': 11700, 'wrack': 1316697,
}
MULTIDOT_TALENTS = ['siphonLife', 'wrack', 'soulSiphon', 'nightfall', 'amplifyCurse', 'improvedShadowBolt',
                    'shadowAndFlame', 'conflagrate', 'baneOfHavoc']


def player(request):
    return request['raid']['parties'][0]['players'][0]


def cast(spell_id, condition=None):
    action = {'castSpell': {'spellId': {'spellId': spell_id}}}
    if condition is not None:
        action['condition'] = condition
    return {'action': action}


def multidot(spell_id):
    return {'action': {'multidot': {'spellId': {'spellId': spell_id}, 'maxDots': 5,
                                    'maxOverlap': {'const': {'val': '0ms'}}}}}


def targets_at_least(count):
    return {'cmp': {'lhs': {'numberTargets': {}}, 'op': 'OpGe', 'rhs': {'const': {'val': str(count)}}}}


class Census:
    def __init__(self, ui, output):
        trees = json.loads((ui / 'sim/talents/trees/warlock.json').read_text())
        self.max_points = [[t['maxPoints'] for t in tree['talents']] for tree in trees]
        self.names = [[t['fieldName'] for t in tree['talents']] for tree in trees]
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
        for spell, spell_id in SPELLS.items():
            request = copy.deepcopy(base)
            player(request)['rotation']['priorityList'].insert(0, cast(spell_id))
            self.write(f'{name}--cast-{spell}', request)
        points = self.parse(player(base)['talentsString'])
        # A multidot line for each dot spell, and for the four main dots together, with every
        # talent that grants one or acts on them at its maximum.
        talented = copy.deepcopy(points)
        for talent in MULTIDOT_TALENTS:
            for t, tree in enumerate(self.names):
                if talent in tree:
                    talented[t][tree.index(talent)] = self.max_points[t][tree.index(talent)]
        for label, spells in [(spell, [spell_id]) for spell, spell_id in MULTIDOTS.items()] + [
                ('dots', [MULTIDOTS[dot] for dot in ('siphon-life', 'immolate', 'bane-of-agony', 'corruption')])]:
            request = copy.deepcopy(base)
            player(request)['talentsString'] = self.render(talented)
            priority = player(request)['rotation']['priorityList']
            for spell_id in spells:
                priority.insert(0, multidot(spell_id))
            self.write(f'{name}--multidot-{label}', request)
        for spell, talent in COMBOS:
            request = copy.deepcopy(base)
            changed = copy.deepcopy(points)
            for t, tree in enumerate(self.names):
                if talent in tree:
                    changed[t][tree.index(talent)] = self.max_points[t][tree.index(talent)]
            player(request)['talentsString'] = self.render(changed)
            if spell == 'bane-of-havoc':
                # Cast again only once it is gone, since the cast takes no time.
                held = {'not': {'val': {'auraIsActive': {'auraId': {'spellId': BANE_OF_HAVOC},
                                                         'sourceUnit': {'type': 'CurrentTarget'}}}}}
                action = cast(SPELLS[spell], held)
            else:
                action = cast(SPELLS[spell])
            priority = player(request)['rotation']['priorityList']
            priority.insert(0, action)
            if spell == 'bane-of-havoc':
                # With Rain of Fire and Hellfire hitting the targets the bane does not hold.
                priority[1:1] = [cast(RAIN_OF_FIRE, targets_at_least(2)), cast(HELLFIRE, targets_at_least(4))]
            self.write(f'{name}--cast-{spell}-with-{talent}', request)
        for t, tree in enumerate(points):
            for i, rank in enumerate(tree):
                for target, label in ((self.max_points[t][i], 'max'), (0, 'zero')):
                    if rank == target:
                        continue
                    request = copy.deepcopy(base)
                    changed = copy.deepcopy(points)
                    changed[t][i] = target
                    player(request)['talentsString'] = self.render(changed)
                    priority = player(request)['rotation']['priorityList']
                    priority[-1:-1] = [cast(RAIN_OF_FIRE, targets_at_least(2)), cast(HELLFIRE, targets_at_least(4))]
                    self.write(f'{name}--{self.names[t][i]}-{label}', request)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--bases', type=Path, required=True, help='directory of the production requests')
    parser.add_argument('--ui', type=Path, default=Path('oracle-cache/source/ui'), help='the pinned ui directory')
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    census = Census(args.ui, args.output)
    for build in BUILDS:
        census.build(build, json.loads((args.bases / f'{build}-warlock.request.json').read_text()))
    print(census.count, 'variants')


if __name__ == '__main__':
    main()
