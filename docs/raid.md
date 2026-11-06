# Raid simulations

`forever-engine raid --request FILE` simulates several players together: up to eight parties of
five, each player with their own gear, talents and rotation, against one encounter. `tools/route.py
--raid` runs it for the application. There is no Go reference for it: Go's raid sim exists in the
pinned fork (`sim/core/raid.go`, `RaidSimRequest`), but no MythicSim surface ever used it.

## Input

```json
{
  "schema": 1,
  "request": { "raid": { "parties": [ { "players": [], "buffs": {} } ], "buffs": {}, "debuffs": {},
                         "tanks": [] },
               "encounter": {}, "simOptions": { "iterations": 1000, "randomSeed": "42" } },
  "assignments": { "innervate": [ { "from": 3, "to": 7 } ], "powerInfusion": [] }
}
```

`request` is a `RaidSimRequest` with several players. A raid index is Go's: `5 * party + slot`.
The application composes the buffs: each party's buffs are what its members bring, the raid's
buffs and debuffs what any member brings. `raid.tanks` and the targets' `tankIndex` work as in
Go. An assignment is, for now, one more of the target's assumed Innervates or Power Infusions,
from a member who can cast it. `simOptions.randomSeed` is required.

## How it runs

1. **Split** (`engine::raid::split`). Each member becomes a single-player request: the raid's
   buffs and debuffs, its own party's buffs (with Leader of the Pack and Moonkin Aura from any
   party, Go's `spreadRaidWidePartyBuffs`), the tank list with the member as player 0 and every
   other member a raid index with no player behind it, and its own seed: the raid's seed plus
   `2^32` per member before it.
2. **Prepare and gate** every member as a Quick Sim. A member Rust does not support refuses the
   raid, naming it (exit status 3, the refusals on standard output).
3. **Lockstep** (`engine::raid::run`). Every iteration, the first member rolls the fight length
   and every other member fights to it; each still draws its own roll, so its random numbers
   stay its own. The runner always steps the fight whose next event is earliest, ties in raid
   order.
4. **Shared target debuffs** (`SHARED_DEBUFFS`). A debuff that a member can cast is one state
   on the target for the whole raid. When a member's fight changes it (gain, refresh, stacks,
   fade), the runner hands the new state to every other fight at that moment, which takes it on
   through its own copy (`Fight::apply_shared`): the member's own ranks when it casts the debuff
   too, otherwise the raid's copy the request's toggle registered. The raid's copy then no
   longer counts from the pull in any fight. A fight never sends back a state it was given.
   Today these are Judgement of Wisdom and Judgement of Light, whose effects the fight applies
   whenever the aura goes up. The other raid debuffs stay as the request's toggles say: their
   effects are part of the target's prepared armor, resistances and damage taken, so sharing
   them needs the target to carry them as live state first (next: Sunder Armor and Expose
   Armor through the major armor category, then Faerie Fire, the curses and Hunter's Mark).
5. **Report.** Go's `RaidSimResult`: each member's player metrics in its party, and the raid's
   and each party's DPS and HPS over each iteration's totals. The `mythicsim` block lists each
   member's raid index, party, slot, name, class, DPS, HPS and fight length, and each shared
   debuff with the members who can cast it and whether it was live.

## Properties the tests hold

- A raid of one is its Quick Sim, bit for bit.
- The first member of any raid fights exactly as alone, as long as nothing it shares changes.
- The raid's DPS is the sum of its members', and a party's the sum of its members'.
- Every member fights to the same length each iteration.
- Judgement of Wisdom on a druid's target follows a paladin in the raid: nothing when the
  paladin keeps Seal of Wisdom up without judging, and mana returns only from its judgements.

## Cost

About one Quick Sim per member, on one thread: 25 members at 1000 iterations take about 11 s
and 40 members about 17 s on an M-series core.
