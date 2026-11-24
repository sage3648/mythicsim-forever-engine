# Damage before the pull

A cast pressed before the pull can finish before 0:00: haste shortens it, or the rotation presses
it early. The Go reference, like wowsims, then drops its damage. On 4 November 2026 this made a
High Order Skyborne Balance Druid look worse than a Night Elf: Skyborne's 2% haste finished the
-3.0 s Starfire precast at -0.06 s, and its 1,533 damage was not counted. App PR #794 worked
around it by pressing the Balance Starfire at -2.9 s and the Elemental Enhancement Lightning Bolt
at -1.9 s. The Hunter turret board's Aimed Shot at -2.5 s still finished at -0.76 s, its arrow
landing at -0.46 s, for every race.

## What established sims do

### wowsims (classic, sod, wotlk, cata)

The fight starts at the earliest prepull action's time, which may be negative, and runs prepull
actions in real time. Damage dealt before 0:00 is left out of the metrics, and DPS divides by the
fight from 0:00 only.

- `dealDamageInternal` records every spell metric, total damage included, only when
  `sim.CurrentTime >= 0`:
  [classic spell_result.go L410-L446](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/spell_result.go#L410-L446).
  The same gate is in sod, wotlk and cata. It came from wotlk PR #4016, "Prepull damage
  shouldn't count", after issue #4007 (a prepull Death and Decay added damage to the results).
- The encounter's damage taken, which health-based fights read, is not gated, and on-hit procs
  still fire:
  [L448-L451](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/spell_result.go#L448-L451).
- DPS is the gated total over `sim.Duration`, the fight from 0:00:
  [metrics_aggregator.go L34-L35](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/metrics_aggregator.go#L34-L35),
  [L427-L430](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/metrics_aggregator.go#L427-L430),
  [sim.go L452-L456](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/sim.go#L452-L456).
- `PrePull` sets the clock to the first prepull action's time, queues every action and queues
  the pull at 0:00:
  [sim.go L425-L447](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/sim.go#L425-L447).
  The only check on a prepull time is that it is not after 0:00:
  [apl.go L85-L90](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/apl.go#L85-L90).
  Nothing checks when a cast finishes.
- A hardcast finishes at its start plus its cast time:
  [cast.go L226-L258](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/sim/core/cast.go#L226-L258).
  A hasted precast that finishes before 0:00 loses its damage with no warning.
- The presets press a precast at minus its cast time so it lands on the pull: Classic Elemental's
  2 s Lightning Bolt at -2 s
  ([default.apl.json](https://github.com/wowsims/classic/blob/7779ebbf79dc7f1341e6ab939b28a3402c9a730a/ui/elemental_shaman/apls/default.apl.json)),
  Cata Balance's Starfire at -2 s
  ([t11.apl.json](https://github.com/wowsims/cata/blob/79c0988027a3db09590a1dad8ed87da7293b7599/ui/druid/balance/apls/t11.apl.json)).

### SimulationCraft

Time starts at 0:00 and never runs before it. The one harmful precombat action is executed at
0:00 with its cast time skipped, so its damage lands at 0:00, or 0:00 plus its travel time, and
counts in full.

- Only a harmful action with a cast time or a travel time may be in the precombat list:
  [action.cpp L2407-L2416](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/action/action.cpp#L2407-L2416),
  [L2847-L2856](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/action/action.cpp#L2847-L2856).
- `player_t::combat_begin` calls `execute()` on it directly rather than scheduling the cast, and
  only the first harmful one runs:
  [player.cpp L6139-L6190](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/player/player.cpp#L6139-L6190).
  Its cast time is not charged:
  [action.cpp L1927-L1928](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/action/action.cpp#L1927-L1928).
- The clock is 0 when combat begins:
  [event_manager.cpp L393-L401](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/sim/event_manager.cpp#L393-L401),
  [sim.cpp L1790-L1813](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/sim/sim.cpp#L1790-L1813).
  DPS divides by the time from then:
  [player_collected_data.cpp L317-L352](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/player/player_collected_data.cpp#L317-L352).
- Things a player does well before the pull (a Paladin's Consecration, Army of the Dead) are
  modelled as state at 0:00 through `precombat_time` options:
  [sc_paladin.cpp L506-L507](https://github.com/simulationcraft/simc/blob/36e21fecf8e1eac46a121d2bd0d9d2879ba574e1/engine/class_modules/paladin/sc_paladin.cpp#L506-L507).

Neither sim counts damage before 0:00 toward the fight, and neither starts the clock at the first
damage. wowsims drops it and leaves timing to the preset; SimulationCraft lands the precast on the
pull.

## The rule since 25 November 2026

Forever follows how its game plays, not SimulationCraft's convention: a caster times a precast so
its damage lands as the fight starts, and a Warlock sends the Imp in so its first Firebolt does
too. Under Forever's rules (`prepull_casts_finish_on_pull`) a precast that haste or an early press
would finish too soon finishes its travel time before the pull, so a missile arrives on 0:00; a
spell without travel finishes on 0:00 as before. A pet's first cast with a cast time begun on the
pull is its precast: it lands on 0:00 with its cast and GCD behind it (the Imp's Firebolt; every
other pet ability is instant). Damage before 0:00 still never counts, the clock still starts at
0:00, and `Rules::GoReference` keeps the Go engine's timing. The paragraphs below describe the
rule as it stood from 4 to 25 November.

## The rule until 25 November 2026

Under Forever's rules (`prepull_casts_finish_on_pull`, written by Forever preparation only) a
cast begun before the pull that would finish before 0:00 finishes on 0:00, and it completes
before the pull's first swings and rotation, as SimulationCraft executes its precast first at
combat start. This is a player timing the precast to the pull: pressed early or hasted, it lands
as the fight starts. A missile then travels as usual, so the Hunter's Aimed Shot lands 0.3 s
into the fight. A precast that finishes after 0:00 is unchanged, and damage before 0:00 is still
never counted, so an instant pressed early (or a second precast, which the first one's cast now
blocks) adds nothing. `Rules::GoReference` keeps the Go engine's timing, which drops the hasted
precast.

The rule follows SimulationCraft rather than counting damage before 0:00 (which would count
several precasts over a fight measured from 0:00) or starting the clock at the first damage
(which would move every timer and buff window against the encounter).

## What it moves

Because the precast now completes ahead of everything else at 0:00, a precast that already
finished exactly on 0:00 (the Mages' Pyroblast at -6.0 s) spends its mana before the 0:00 mana
tick, which under the reference landed first on a full bar and was wasted (103 mana on the Arcane
board). On the live board requests of bundle b1f646e (10,000 iterations each), with healer
requests unchanged:

| Request | Precast | b1f646e | With the rule |
|---|---|---|---|
| Hunter turret | Aimed Shot finished -0.76 s, landed -0.46 s | 785.97 | 792.33 |
| Arcane Mage | Pyroblast finished 0:00, mana tick | 924.19 | 925.53 |
| Frostfire Mage | Pyroblast finished 0:00, mana tick | 799.90 | 801.05 |
| Fire Mage | Pyroblast finished 0:00, mana tick | 907.59 | 907.83 |
| Affliction Warlock | Shadow Bolt finished -0.05 s | 836.85 | 837.04 |
| Destruction Warlock | Shadow Bolt finished -0.55 s | 889.74 | 889.89 |
| Frost Mage | Frostbolt finished -0.55 s | 715.41 | 715.10 |
| Marksmanship Hunter | finished -0.02 s | 995.86 | 995.95 |
| Survival weave Hunter | Aimed Shot finished -0.26 s | 1111.63 | 1111.69 |

The Warlock and Frost bolts already landed after 0:00 by their travel time; they now leave on
0:00, as SimulationCraft's precast does. The other Hunter boards move by under 0.1 DPS and the
Elemental Shaman board only by an aura's uptime.

## The live presets

Under the rule, app PR #794's shifts are no longer needed and cost a little: the Balance
Starfire at -2.9 s finishes 0.04 s (Skyborne) to 0.1 s (Night Elf, Tauren) after the pull, where
-3.0 s finishes every race on 0:00, and the same holds for the Elemental Enhancement Lightning
Bolt at -1.9 s against -2.0 s. On the race board requests -3.0 s is 0.26 to 0.71 DPS better for
every Balance race and -2.0 s 0.20 to 1.13 DPS better for every Elemental Enhancement race,
with no change of order. Both can go back once the rule is live.
