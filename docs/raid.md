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
  "assignments": { "innervate": [ { "from": 3, "to": 7 } ], "powerInfusion": [] },
  "tankSwapSeconds": 30
}
```

`request` is a `RaidSimRequest` with several players. A raid index is Go's: `5 * party + slot`.
The application composes the buffs: each party's buffs are what its members bring, the raid's
buffs and debuffs what any member brings. `raid.tanks` and the targets' `tankIndex` work as in
Go, and a raid can have several tanks, one per target (see "Tanks" below). An assignment names a druid's Innervate or a priest's Power Infusion and the member it goes
to: each caster casts on one other member, and the target loses its own request's assumed
Innervates or Power Infusions. `simOptions.randomSeed` is required.

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
5. **Assignments.** An assigned Innervate or Power Infusion is the caster's own cast: its
   rotation and cooldown decide when, in its own fight, and the cast's aura goes to the target
   instead of the caster. The target's fight has the aura (`Innervates (External)` or `Power
   Infusions (External)`, `RaidLive`) but no assumed cooldown to bring it up; the runner hands
   it the cast at that moment (`Fight::give`, `Fight::take_gift`), for the caster's duration.
   As Go's cast condition reads the target, the cast does not land while another aura of its
   tag is up on the target (the target's own Innervate, say), and an assigned druid does not
   innervate again while its last one is up; it knows only its own casts. A caster whose
   rotation never casts the spell gives nothing: a druid's Innervate is cast only by its
   rotation, a priest's Power Infusion also by `autocastOtherCooldowns`.
6. **Report.** Go's `RaidSimResult`: each member's player metrics in its party, and the raid's
   and each party's DPS and HPS over each iteration's totals. Its `logs` are every member's
   first iteration, in raid order. The `mythicsim` block lists each member's raid index, party,
   slot, name, class, DPS, HPS and fight length, each shared debuff with the members who can
   cast it and whether it was live, and each assignment with its caster's casts over every
   iteration, how many landed, and `cast: false` when the caster never cast it.

## Tanks

Each member fights as a single-player Quick Sim, whose targets are identical copies of the boss
that all swing at the one player. A raid with several tanks gives each target to one of them:
`raid.tanks` lists the tanks (`{ "type": "Player", "index": raidIndex }`) and each target's
`tankIndex` names its tank's position in that list. A raid with two bosses, two tanks and a
`tankIndex` of 0 and 1 on them runs like this:

- A tank's fight has every target copy, but only the copies its tank list entry names swing at
  it. The other copies keep their swing timers on the raid clock and skip the swing that comes
  due: no roll, no damage, no metric. A tank's damage taken per second is therefore what it takes
  tanking those targets alone.
- A member that no target names is not hit and `mythicsim.members[].tanking` is false for it. So is
  a hunter in the same party. (A Protection Warrior no target names is refused, as alone: its
  rotation reads the block value of a tanked target.)
- The tank fights the first target it holds: that copy goes first in its own fight, so its attacks
  and the parry haste of the boss's swing are on the boss that swings at it. The runner renames
  the targets of a shared debuff between members whose fights list them in another order. In
  `result.encounterMetrics` (the first member's view) the targets are in that member's order.
- A raid whose targets all name the same tank, or the same first tank, is left exactly as its
  request says, as before.

### Tank swaps

`tankSwapSeconds` (5 to 600, beside `assignments`) swaps a target between two tanks on a timer. A
target with both a `tankIndex` and a `secondTankIndex` naming another tank (and, if the same
raid member is behind both entries, nothing to swap) swings at the first tank for the first
`tankSwapSeconds` from the pull, then at the second for the next, and so on. A target with no
second tank ignores the swap time, and a raid with one tank and a swap time is identical to the
same raid without it. A target with a second tank and no `tankSwapSeconds` is refused, naming
the tanks, with a message that says to give a swap time. There is no threat: the swap is a taunt
rotation on a timer, and nothing makes a tank lose the target.

Each tank's fight keeps the target's swing timer running on the raid clock, as for any copy it
does not hold, and the swing that comes due at a moment lands on whoever holds the target then.
A swing is whole, never split across the edge, and a tank that takes the target at the edge gets
the next swing its own timer has due, so a tank may take one swing more or fewer than half and
the average is half. The swap does not carry swing timers over between the two fights: each has
its own opening offset, slows and parry haste. Rage from damage taken, block, avoidance procs and
what the tank's rotation reads follow from the swings that land, as for any hit.

Limits: a second tank of a different raid member needs both tanks in the raid; a tank list entry
for a raid index with no player holds the target for its windows and nobody takes the swings.

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
- An assigned Innervate and Power Infusion land on the target exactly when the caster casts
  them, never on the caster, and replace the target's assumed ones; a caster whose rotation
  never casts the spell leaves the target as with no assignment.
- With two bosses and two tanks, one each, each tank takes about what it takes alone tanking one
  boss (within 5% at 300 iterations; the run to run noise is about 2%), not double. With both bosses
  on one tank, it takes about twice that and the other takes nothing and is not a tank.
- One target and two tanks swapping every 30 seconds in a 120 second fight: each tank takes about
  half of what it takes tanking the boss alone (within 4 points of half at 300 iterations), the
  swings in the first iteration's log fall in the tank's own 30 second windows, and a raid with
  one tank and a swap time, or with targets that name no second tank, is identical to the same raid
  without it.
- In the 20 reference build raid (`fixtures/raid`), the four warriors' Sunder Armor, the feral
  druid's Faerie Fire and the three warlocks' Curse of the Elements are live, and no warlock
  recasts its curse every global cooldown.

## Healers and the tank's damage (design, not implemented)

A healer in a raid heals under MythicSim's healer model, as alone: every heal lands on a player
who needs it, less the share real healers overheal (`prepare::overheal`, measured on live
reports), and Prayer of Mending and Chain Heal follow the assumed party hit rates
(`prepare::party_damage`). The tank's fight already takes the targets' real swings
(`core::fight::enemy`) and tracks its health, so a raid could heal that damage instead:

1. **Damage out.** The tank's fight sends each landed swing (its time and the damage after
   avoidance, armor and blocks) through the cross-fight channel, as `Fight::give` sends a
   cast. The runner keeps each tank's missing health, capped at its maximum.
2. **Healing in.** A healer assigned to a tank (an assignment list beside `innervate`, or the
   raid's `tanks`) mirrors that tank's missing health. A heal on the tank counts up to it and
   overheals the rest, in place of the overheal share (`Fight::landed_healing`); the runner
   takes the heal off the tank's missing health and hands the new value to every other healer
   of that tank at once, ties in raid order, as shared debuffs are.
3. **Hits drive the heals that follow damage.** Prayer of Mending on the tank moves when the
   tank is hit, from the tank's swings instead of a drawn wait.
4. **What stays assumed.** Nothing else in a raid takes damage: the encounter has no raid
   damage. Heals on the other members, party heals and Chain Heal's jumps keep the assumed
   rates and overheal shares, and a healer with no tank keeps today's model.

It is not implemented here. It changes every raid healer's HPS through the core healing path
(`core::fight::healing`, `core::fight::heals`) and each class's Prayer of Mending, and a tank's
swing damage alone would make a raid's healers overheal far more than the live reports the
overheal table measures, where raids take more than the tank's damage. It needs a measurement
of healing done against a tank's damage taken in Forever logs before it can replace the model.
No Go golden is affected either way: Go has no raid sim surface.

## Cost

About one Quick Sim per member, on one thread: 25 members at 1000 iterations take about 11 s
and 40 members about 17 s on an M-series core.
