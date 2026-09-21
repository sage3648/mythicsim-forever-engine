# MythicSim Seal of Fury delta

Base: ElliotWood/Forever commit `0eb921c12cd3ce6501a915cd602726100bc46a78`.
This is a focused downstream implementation intended to converge with upstream.

## Client evidence

Values were rechecked against beta build `1.60.1.69913`, using
`tools/data_watch/spell_client.py` and the [Wago SpellEffect table](https://wago.tools/db2/SpellEffect?build=1.60.1.69913).
`SpellLevels`, `SpellPower` and `SpellDuration` supply level scaling, costs and durations.

| Rank | Learn level | Seal | Proc | Holy damage | Mana | Judgement | Base range | Per level | Max scaling level |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | 10 | 1311649 | 1311647 | 6 | 40 | 1311650 | 22 to 24 | 1.71 | 16 |
| 2 | 18 | 1311656 | 1311654 | 9 | 60 | 1311655 | 35 to 39 | 2.16 | 24 |
| 3 | 25 | 20163 | 20231 | 14 | 90 | 20183 | 51 to 57 | 2.52 | 31 |
| 4 | 34 | 20419 | 20415 | 19 | 120 | 20411 | 70 to 78 | 2.79 | 40 |
| 5 | 42 | 20421 | 20416 | 25 | 140 | 20412 | 91 to 101 | 3.42 | 48 |
| 6 | 50 | 20422 | 20417 | 32 | 170 | 20413 | 118 to 128 | 3.69 | 56 |
| 7 | 58 | 20423 | 20418 | 35 | 200 | 20414 | 146 to 160 | 3.69 | 64 |

The damage proc has a 0.1 spell-power coefficient, the Judgement has 0.45.
The 30-second seal grants an absorb for 50% of Holy proc damage while a shield is equipped.
Light's Fury (1310927) supplies the 10-second absorb aura.
Improved Seal of Fury (1314103) refunds one mana per character level on full depletion,
plus 15% per attacker level above the paladin, capped at three levels.
The resource effect uses 1314104. At level 60 against level 63 this is 87 mana per break.

## Implementation and experimental assumptions

- A landed white melee hit fires the proc, following the existing Righteousness trigger pattern.
  The proc cannot independently miss and uses the same melee crit table and weapon
  specialization modifier as Righteousness. Both procs have identical client
  `SpellMisc` attribute flags and `SpellCategories.DefenseType = 2`. Exact special-attack
  and secondary proc-chain eligibility still need combat-log verification.
- Absorb strength uses damage after mitigation and Improved Seals. A new proc replaces
  remaining absorb and refreshes the 10-second duration. Replacement does not award mana.
  Stacking/overwrite behaviour needs an in-client check.
- Incoming damage consumes the remaining shield through the existing dynamic damage-taken
  hook. Partial consumption, expiry, replacement and iteration reset do not refund mana.
  Core shielding metrics count shield amounts applied, consistent with other core shields.
- The implementation does not redesign the core absorb model or Templar's Bulwark approximation.
  Encounter threat swaps and Judgement's four-second taunt are not simulated. Tanks remain
  assigned through the encounter's tank list. Do not present this as validated survivability.
- Twist of Light can bank Fury through the existing seal-proc registry.
- Spell names are added to the engine database with the existing Fury seal icon, reused for
  its shield and mana effect. Item data is unchanged.

## Convergence

Keep this delta separate from unrelated paladin changes. On each upstream update, compare
`proto/paladin.proto`, `sim/paladin/sof.go`, seal registration, the Protection UI preset and
spell metadata. If upstream implements equivalent Fury behaviour, use their implementation
and retain these tests as acceptance criteria, adapting interfaces as needed. Remove the
MythicSim pin guard only after damage, shield depletion, talent mana and both seal choices
pass against the replacement engine. Never force-sync over the fork's commits.

Validation: `go test -tags=with_db ./sim/paladin/...`. Focused tests cover partial/break/expiry/
replacement/reset, talent and attacker-level mana, real Fury/Judgement damage, no shield,
no incoming attacks, and Righteousness isolation.
