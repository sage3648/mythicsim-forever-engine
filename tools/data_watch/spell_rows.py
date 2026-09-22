#!/usr/bin/python

# Every client row that decides how a spell behaves, for implementing one from the client rather
# than from memory.
#
#   python tools/data_watch/spell_rows.py 1237313 401502
#
# Prints the name and tooltip, then SpellEffect, SpellLevels, SpellPower, SpellCooldowns,
# SpellCastTimes (resolved), SpellDuration (resolved), SpellAuraOptions and SpellMisc, with
# empty and zero columns dropped. Nothing is interpreted - that is the reader's job - because
# the two ways this goes wrong are guessing what a dummy effect means and misreading a column.

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from spell_client import FOREVER, Client, table

TABLES = ['SpellEffect', 'SpellLevels', 'SpellPower', 'SpellCooldowns', 'SpellAuraOptions', 'SpellMisc',
          'SpellCategories', 'SpellClassOptions']


def compact(row):
    return {k: v for k, v in row.items() if v not in ('0', '', None, '0.0') and k not in ('ID',)}


def main(ids):
    client = Client(FOREVER)
    wanted = {str(i) for i in ids}
    rows = {name: [r for r in table(FOREVER, name) if r.get('SpellID') in wanted] for name in TABLES}
    cast_times = {r['ID']: r for r in table(FOREVER, 'SpellCastTimes')}
    durations = {r['ID']: r for r in table(FOREVER, 'SpellDuration')}

    for sid in ids:
        spell = client.spell(sid) or {}
        print(f"\n=== {sid} {spell.get('name')!r} {spell.get('subtext') or ''}")
        print('   ', ' '.join((spell.get('description') or '').split()))
        for name in TABLES:
            for r in sorted((r for r in rows[name] if r['SpellID'] == str(sid)), key=lambda r: int(r.get('EffectIndex') or 0)):
                c = compact(r)
                c.pop('SpellID', None)
                if name == 'SpellMisc':
                    ct = cast_times.get(r.get('CastingTimeIndex'))
                    du = durations.get(r.get('DurationIndex'))
                    c['castMs'] = ct and ct.get('Base')
                    c['durationMs'] = du and du.get('Duration')
                print(f'    {name}: {c}')


if __name__ == '__main__':
    main([int(a) for a in sys.argv[1:]])
