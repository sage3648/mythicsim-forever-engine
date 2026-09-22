package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// Forever beta client 1.60.1.69893: cost, school, defense type, flat damage and coefficient come from
// the client table. The id stays ours: one rank is registered, and reading it from the table would
// make spell_sources_test.go file the other ranks as registered. The same holds for every ranked
// warrior ability. The threat is not in the table, so it stays ours.
func (warrior *Warrior) registerHeroicStrikeSpell(realismICD *core.Cooldown) {
	spellID := core.TernaryInt32(core.IncludeAQ, 25286, 11567)
	row := spellData.HeroicStrike.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)
	// No known equation
	threat := core.TernaryFloat64(core.IncludeAQ, 173, 145)

	warrior.HeroicStrike = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeMHAuto,
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost) - float64(warrior.Talents.ImprovedHeroicStrike),
			Refund: 0.8,
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		FlatThreatBonus:  threat,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
			result := warrior.calcQueuedSwing(sim, spell, target, baseDamage)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}

			spell.DealDamage(sim, result)
			if warrior.curQueueAura != nil {
				warrior.curQueueAura.Deactivate(sim)
			}
		},
	})
	warrior.HeroicStrikeQueue = warrior.makeQueueSpellsAndAura(warrior.HeroicStrike, realismICD)
}

// Cost, school, defense type, flat damage and coefficient come from the client table; the id and
// threat stay ours (see registerHeroicStrikeSpell).
func (warrior *Warrior) registerCleaveSpell(realismICD *core.Cooldown) {
	spellID := int32(20569)
	row := spellData.Cleave.BySpellID(spellID)
	flatDamageBonus := shared.SpellDataMin(row.Direct)
	threat := 100.0

	// Improved Cleave discounts Rage in Forever instead of adding damage, and Raging Blows
	// takes another 2 off the top.
	rageCost := float64(row.Cost) - float64(warrior.Talents.ImprovedCleave) - core.TernaryFloat64(warrior.Talents.RagingBlows, 2, 0)

	results := make([]*core.SpellResult, min(int32(2), warrior.Env.GetNumTargets()))

	warrior.Cleave = warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeMHAuto,
		Flags:       core.SpellFlagMeleeMetrics | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost: rageCost,
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		FlatThreatBonus:  threat,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for idx := range results {
				baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
				results[idx] = warrior.calcQueuedSwing(sim, spell, target, baseDamage)
				target = sim.Environment.NextTargetUnit(target)
			}

			for _, result := range results {
				spell.DealDamage(sim, result)
			}

			if warrior.curQueueAura != nil {
				warrior.curQueueAura.Deactivate(sim)
			}
		},
	})
	warrior.CleaveQueue = warrior.makeQueueSpellsAndAura(warrior.Cleave, realismICD)
}

func (warrior *Warrior) makeQueueSpellsAndAura(srcSpell *WarriorSpell, realismICD *core.Cooldown) *WarriorSpell {
	isQueueQueued := false

	queueAura := warrior.RegisterAura(core.Aura{
		Label:    "HS/Cleave Queue Aura-" + srcSpell.ActionID.String(),
		ActionID: srcSpell.ActionID.WithTag(1),
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			isQueueQueued = false
		},
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			if warrior.curQueueAura != nil {
				warrior.curQueueAura.Deactivate(sim)
			}
			warrior.curQueueAura = aura
			warrior.curQueuedAutoSpell = srcSpell
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warrior.curQueueAura = nil
			warrior.curQueuedAutoSpell = nil
		},
	})

	queueSpell := warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID: srcSpell.ActionID.WithTag(1),
		Flags:    core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagCastTimeNoGCD,

		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.curQueueAura == nil &&
				!isQueueQueued &&
				warrior.CurrentRage() >= srcSpell.DefaultCast.Cost &&
				!warrior.IsCasting(sim) &&
				realismICD.IsReady(sim)
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			if realismICD.IsReady(sim) {
				isQueueQueued = true
				realismICD.Use(sim)
				sim.AddPendingAction(&core.PendingAction{
					NextActionAt: sim.CurrentTime + realismICD.Duration,
					OnAction: func(sim *core.Simulation) {
						queueAura.Activate(sim)
						isQueueQueued = false
					},
				})
			}
		},
	})

	return queueSpell
}

// Heroic Strike and Cleave replace the main hand swing but roll on the special attack table,
// so they skip the dual wield miss penalty. The penalty flag is character wide, so it is only
// lifted for the swing itself: an off-hand auto that lands while the queue is up still pays it.
func (warrior *Warrior) calcQueuedSwing(sim *core.Simulation, spell *core.Spell, target *core.Unit, baseDamage float64) *core.SpellResult {
	warrior.PseudoStats.DisableDWMissPenalty = true
	result := spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
	warrior.PseudoStats.DisableDWMissPenalty = false
	return result
}

func (warrior *Warrior) TryHSOrCleave(sim *core.Simulation, mhSwingSpell *core.Spell) *core.Spell {
	if !warrior.curQueueAura.IsActive() {
		return mhSwingSpell
	}

	if !warrior.curQueuedAutoSpell.CanCast(sim, warrior.CurrentTarget) {
		warrior.curQueueAura.Deactivate(sim)
		return mhSwingSpell
	}

	return warrior.curQueuedAutoSpell.Spell
}
