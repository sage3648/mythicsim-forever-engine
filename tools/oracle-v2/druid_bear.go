// Druid Feral (bear) export: Bear Form, the rage bar, Enrage, Demoralizing Roar, Maul's queue,
// Lacerate, Primal Bite and Natural Reaction. Each formula mirrors the cited Go file at the
// pinned revision.
package main

import (
	"math"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/druid"
)

var (
	bearDireBearForm      = spelldata.Ranked(9634)
	bearEnrage            = spelldata.Ranked(5229)
	bearMaul              = spelldata.Ranked(6807, 6808, 6809, 8972, 9745, 9880, 9881)
	bearLacerate          = spelldata.Ranked(414644, 1235826, 1235827)
	bearPrimalBite        = spelldata.Ranked(407995, 1238069, 1238070, 1238073)
	bearSwipe             = spelldata.Ranked(779, 780, 769, 9754, 9908)
	bearNaturalReaction   = spelldata.Talent(417051, 5)
	bearNaturalReactionOn = spelldata.Ranked(417053)
	bearBarkskin          = spelldata.Ranked(22812)
	bearFrenziedRegen     = spelldata.Ranked(22842)
)

// forms.go Bear Form's health on a shift, read from separate reset simulations: the health its
// stat bonus, a flat 1240 health and formShiftStats without any, adds to the maximum. OnGain takes
// the health fraction once the bonus is in and OnExpire once it is out, both before Heart of the
// Wild's Stamina moves, then restoreHealthFraction applies it to the new maximum. Both shifts are
// checked against Go.
func bearFormHealthBonus(unrepresented func(string)) float64 {
	fresh := func() (*core.Simulation, *druid.Druid) {
		simulation := core.NewSim(exportRequest, simsignals.CreateSignals())
		simulation.Reset()
		return simulation, simulation.Raid.Parties[0].Players[0].(druid.DruidAgent).GetDruid()
	}
	simulation, d := fresh()
	if !d.BearFormAura.IsActive() {
		unrepresented("Bear Form is not up after the reset")
		return 0
	}
	// AddStatsDynamic recomputes the stats from those before dependencies, so a health
	// multiplier such as Tauren Endurance also scales the bonus.
	inForm := d.MaxHealth()
	without := d.GetStatsWithoutDeps()
	without[stats.Health] -= 1240
	bonus := inForm - d.ApplyStatDependencies(without).FloorGameStats()[stats.Health]
	d.RemoveHealth(simulation, d.CurrentHealth()/3)
	health := d.CurrentHealth()
	simulation.CurrentTime = time.Second
	d.BearFormAura.Deactivate(simulation)
	outOfForm := d.MaxHealth()
	if outOfForm > inForm-bonus {
		unrepresented("leaving Bear Form raises maximum health")
	}
	if math.Abs(health/(inForm-bonus)*outOfForm-d.CurrentHealth()) > 1e-9*health {
		unrepresented("leaving Bear Form does not keep the health fraction Rust expects")
	}
	// Entering it from caster form.
	simulation, d = fresh()
	d.BearFormAura.Deactivate(simulation)
	d.RemoveHealth(simulation, d.CurrentHealth()/3)
	health = d.CurrentHealth()
	outOfForm = d.MaxHealth()
	simulation.CurrentTime = time.Second
	d.BearFormAura.Activate(simulation)
	if math.Abs(health/(outOfForm+bonus)*d.MaxHealth()-d.CurrentHealth()) > 1e-9*health {
		unrepresented("entering Bear Form does not keep the health fraction Rust expects")
	}
	return bonus
}

// maul.go, lacerate.go and primal_bite.go roll their damage rows.
func druidBearDamageRows(rows map[int32]*spelldata.Spell) {
	for _, ladder := range []spelldata.Ladder{bearMaul, bearPrimalBite} {
		if row := ladder.Highest(); row != nil {
			rows[row.ID] = row
		}
	}
}

func druidBearEffects(d *druid.Druid, character *core.Character) []map[string]any {
	effects := []map[string]any{}
	talents := d.Talents
	if d.BearForm != nil && d.BearFormAura != nil {
		unrepresented := func(reason string) { *classNotes = append(*classNotes, reason) }

		// rage.go EnableRageBar: feralbear.go passes BaseRageMultiplier 1.
		effects = append(effects, rageBarEffect(character, 1))
		// forms.go RegisterBearFormAura and registerBearFormSpell: the aura the agent's reset enters,
		// a stat aura whose bit carries its stats; its threat, spirit regeneration, rage bar, paw and
		// Faerie Fire's free cast; the health fraction kept on a shift; and the cast, which spends
		// all Rage and rolls Furor's 10.
		initial := initialPseudoStats(&character.Unit)
		breaking := []int{}
		for i, spell := range character.Spellbook {
			if spell.Flags.Matches(core.SpellFlagPotion | core.SpellFlagConjured | core.SpellFlagExplosive) {
				breaking = append(breaking, i)
			}
		}
		mainHand := d.WeaponFromMainHand()
		paw := d.GetBearWeapon()
		effects = append(effects, map[string]any{
			"kind": "bear_form", "spell_id": bearDireBearForm.Highest().ID, "aura": d.BearFormAura.Label,
			"spell": spellPosition(character, d.BearForm.Spell), "health_bonus": bearFormHealthBonus(unrepresented),
			"initial_threat_multiplier":       formStartThreat(d.BearFormAura.Label, druid.BearFormThreatMultiplier),
			"threat_multiplier":               druid.BearFormThreatMultiplier,
			"initial_spirit_regen_multiplier": initial.SpiritRegenMultiplier,
			"spirit_regen_multiplier":         druid.AnimalSpiritRegenSuppression,
			"furor_proc_chance":               d.FurorProcChance,
			"cost_spells":                     spellsMatching(character, druid.DruidSpellFaerieFire),
			"form_breaking_spells":            breaking, "main_hand": exportWeapon(&mainHand), "bear_weapon": exportWeapon(&paw),
		})

		if d.Enrage != nil && d.EnrageAura != nil { // enrage.go
			rank := bearEnrage.Highest()
			effects = append(effects, map[string]any{
				"kind": "enrage", "spell_id": rank.ID, "aura": d.EnrageAura.Label,
				"instant_rage":  rank.Effect(dbcenums.A_NONE, 1).Tenths() + d.IntensityEnrageRageBonus + d.WolfsheadEnrageRage,
				"rage_per_tick": rank.Effect(dbcenums.A_PERIODIC_ENERGIZE, 1).BaseValue() / 10,
				"ticks":         int(rank.Duration() / time.Second), "period_ns": nanos(time.Second),
			})
		}
		if d.DemoralizingRoar != nil && d.DemoralizingRoarAuras != nil { // demoralizing_roar.go
			target := character.Env.Encounter.ActiveTargetUnits[0]
			effects = append(effects, map[string]any{
				"kind": "demoralizing_roar", "spell": spellPosition(character, d.DemoralizingRoar.Spell),
				"aura": d.DemoralizingRoarAuras.Get(target).Label,
			})
		}
		if d.Maul != nil { // maul.go: the queue spell, its aura and realism cooldown, and the strike
			strike := -1
			rank := bearMaul.Highest()
			for i, spell := range character.Spellbook {
				if spell.ActionID == (core.ActionID{SpellID: rank.ID}) {
					strike = i
				}
			}
			queueAura := character.GetAura("Maul Queue Aura")
			realism := readPrivate(privateField(d, "maulRealismICD")).Interface().(*core.Cooldown)
			if strike < 0 || queueAura == nil || realism == nil {
				unrepresented("Maul is incomplete")
			} else {
				effects = append(effects, map[string]any{
					"kind": "maul", "spell": strike, "queue_spell": spellPosition(character, d.Maul.Spell),
					"queue_aura": queueAura.Label, "realism_ns": nanos(realism.Duration),
					"flat_damage": rank.DamageEffect().Average(core.CharacterLevel),
				})
			}
		}
		if d.Lacerate != nil { // lacerate.go: a flat tick a stack and a weapon share a stack
			rank := bearLacerate.Highest()
			effects = append(effects, map[string]any{
				"kind": "lacerate", "spell": spellPosition(character, d.Lacerate.Spell),
				"tick_base":              rank.PeriodicEffect().Average(core.CharacterLevel),
				"weapon_share_per_stack": bearLacerate.EffectAt(2).FractionAt(rank.RankNumber()),
				"max_stacks":             druid.LacerateMaxStacks,
				"tick_can_crit":          rank.PeriodicCanCrit(),
				"tick_magic":             rank.DefenseTypeCore() == core.DefenseTypeMagic,
			})
		}
		if d.PrimalBite != nil { // primal_bite.go: Berserk lifts its cooldown, a Go literal
			effects = append(effects, map[string]any{
				"kind": "primal_bite", "spell": spellPosition(character, d.PrimalBite.Spell),
				"flat_damage": bearPrimalBite.Highest().DamageEffect().Average(core.CharacterLevel),
			})
		}
		if d.Swipe != nil { // swipe.go: up to three targets, each a flat hit and a share of the attack power
			effects = append(effects, map[string]any{
				"kind": "swipe", "spell": spellPosition(character, d.Swipe.Spell),
				"flat_damage": bearSwipe.Highest().DamageEffect().Average(core.CharacterLevel),
				// swipeAttackPowerCoefficient, a Go literal the client rows do not carry.
				"attack_power_coefficient": 0.03,
			})
		}
		if d.Barkskin != nil { // barkskin.go: the aura's physical damage taken cut is a stat aura
			effects = append(effects, map[string]any{
				"kind": "barkskin", "spell_id": bearBarkskin.Highest().ID, "aura": "Barkskin",
			})
		}
		if d.FrenziedRegeneration != nil && d.FrenziedRegenerationAura != nil { // frenzied_regeneration.go
			rank := bearFrenziedRegen.Highest()
			effects = append(effects, map[string]any{
				"kind": "frenzied_regeneration", "spell": spellPosition(character, d.FrenziedRegeneration.Spell),
				"aura": d.FrenziedRegenerationAura.Label, "ticks": int(rank.Duration() / time.Second),
				"period_ns": nanos(time.Second), "max_rage_per_tick": 10.0, "health_share_per_rage": 0.01,
				"healing_taken_multiplier": character.PseudoStats.HealingTakenMultiplier,
			})
		}
	}
	// talents_feral_combat.go applyNaturalReaction: its listener needs Bear Form, so a cat's is
	// inert in effect, which the runtime's form check gives.
	if talents.NaturalReaction > 0 {
		triggered := bearNaturalReactionOn.Highest()
		trigger := core.ProcTrigger{Name: "Natural Reaction", Callback: core.CallbackOnSpellHitTaken,
			Outcome: core.OutcomeDodge, ProcChance: bearNaturalReaction.EffectAt(2).FractionAt(talents.NaturalReaction)}
		effects = append(effects, map[string]any{
			"kind": "natural_reaction", "trigger_aura": trigger.Name, "proc_chance": trigger.ProcChance,
			"outcome": outcomeNames(trigger.Outcome), "trigger_immediately": trigger.TriggerImmediately,
			"rage": triggered.EffectN(1).BaseValue() / 10, "metrics_action_id": actionID(core.ActionID{SpellID: triggered.ID}),
		})
	}
	return effects
}
