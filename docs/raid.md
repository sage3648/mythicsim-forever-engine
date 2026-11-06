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
4. **Debuffs members cast.** What a member's rotation casts decides what the raid has on the
   target; a debuff nobody casts stays as the request's toggle says (an assumption from outside
   the raid).
   - **Live** (`SHARED_DEBUFFS`): Sunder Armor, Expose Armor, Faerie Fire, Curse of the
     Elements, Curse of Recklessness, Judgement of Wisdom and Judgement of Light, when a member's
     rotation casts the spell or keeps the seal up. The raid's copy is registered but no longer
     up from the pull, and its changes are not in the target's prepared values (`RaidLive`).
     Sunder Armor has no ramp, and both major armor debuffs bid in a target major armor category
     (`raid_armor_category`) with the target's armor at each stack. Faerie Fire and the curses
     export each target category their raid copy bids in (`raid_debuff_category`) with every
     member's change while it holds the category: armor for the per-stat Minor Armor Reduction
     category Faerie Fire and Curse of Recklessness share, resistances and school damage taken
     for Curse of the Elements. Each member's change is read off a separate reset simulation, the
     class's own copies included, and the fight applies the holder's change as the holder changes
     (Go's effects' `OnGain` and `OnExpire`), so a Faerie Fire beside a Curse of Recklessness
     lowers the armor once. When a member's fight changes the debuff (gain, refresh, stacks,
     fade), the runner hands the new state to every other fight at that moment, which takes it on
     through its own copy (`Fight::apply_shared`): the member's own when its class has the
     debuff, otherwise the raid's copy. A caster of Faerie Fire or a curse has no raid copy in its
     fight, so a raid of one is its Quick Sim with the toggle off. A fight never sends back a state
     it was given. A stronger armor debuff the request assumes from the pull, such as the raid's
     Expose Armor beside members' Sunder Armor, keeps the members' stacks out, as in a Quick Sim.
     Forever has no Faerie Fire (Feral): a feral druid casts the Balance spell, and two druids
     keep one Faerie Fire up between them, as two warlocks keep one Curse of the Elements.
   - **Assumed** (`CAST_DEBUFFS`): the tank debuffs, Insect Swarm and Scorpid Sting, when a
     member's rotation casts its own copy ("<name> (Player)"). Every other member's fight has the
     raid's copy from the pull; the caster's fight has only its own casts. Their effects are part
     of the target's prepared values, so a member's casts cannot reach another fight yet.
     Preparing each member once before the run shows what its rotation casts. Without this, a
     member beside the raid's copy would recast its own every global cooldown, which never lands.
     No Forever class casts Hunter's Mark (the reference leaves `hunters_mark.go` out), so its
     toggle always holds.
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
- Sunder Armor on a hunter's target follows an Arms Warrior's stacks; a warrior who never
  sunders leaves the hunter exactly as alone with the raid's toggle.
- Faerie Fire, Curse of the Elements and Curse of Recklessness on a receiving member's target
  follow the caster's uptime, and the receiver's damage gain over a raid without them is about
  that share of what the debuff assumed from the pull gives. A caster whose rotation never casts
  it leaves the receiver as alone without it, whatever the raid's toggle.
- Two warlocks keep one Curse of the Elements up, and two druids one Faerie Fire: each casts it
  less than alone. Faerie Fire beside Curse of Recklessness lowers armor less than both assumed.
- In the 20 reference build raid (`fixtures/raid`), the four warriors' Sunder Armor, the feral
  druid's Faerie Fire and the three warlocks' Curse of the Elements are live, and no warlock
  recasts its curse every global cooldown.

## Cost

About one Quick Sim per member, on one thread: 25 members at 1000 iterations take about 11 s
and 40 members about 17 s on an M-series core.
