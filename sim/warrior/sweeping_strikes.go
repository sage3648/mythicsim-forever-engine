package warrior

import (
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerSweepingStrikesCD() {
	if !warrior.Talents.SweepingStrikes {
		return
	}

	numTargets := min(2, warrior.Env.GetNumTargets())

	// Procs from auto attacks and most abilities https://www.wowhead.com/classic/spell=12723/sweeping-strikes
	var curDmg float64
	hitSchoolDamagWithValue := warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: 12723},
		SpellSchool: core.SpellSchoolPhysical,
		DefenseType: core.DefenseTypeMelee,
		ProcMask:    core.ProcMaskEmpty, // No proc mask, so it won't proc itself.
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			spell.CalcAndDealDamage(sim, target, curDmg, spell.OutcomeAlwaysHit)
		},
	})

	// Procs from WW, also Execute? https://www.wowhead.com/classic/spell=26654/sweeping-strikes
	hitSpecialNormalized := warrior.RegisterSpell(AnyStance, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: 26654},
		SpellSchool: core.SpellSchoolPhysical,
		DefenseType: core.DefenseTypeMelee,
		ProcMask:    core.ProcMaskEmpty, // No proc mask, so it won't proc itself.
		Flags:       core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target))
			spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeSpecialCritOnly)
		},
	})

	// Forever beta client 1.60.1.69893: id, cost, cooldown, school, charges and duration come from the
	// client table. The talent's trait grants 12292, whose SpellMisc duration is 20 s (index 18; the 10 s
	// row is 1228365, not granted), so Classic's 10 s is gone.
	row := spellData.SweepingStrikes.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	ssAura := warrior.RegisterAura(core.Aura{
		Label:     "Sweeping Strikes",
		ActionID:  actionID,
		Duration:  row.Duration,
		MaxStacks: row.ProcCharges,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.SetStacks(sim, row.ProcCharges)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if aura.GetStacks() == 0 || result.Damage <= 0 || !spell.ProcMask.Matches(core.ProcMaskMelee) {
				return
			}

			var spellToUse *WarriorSpell
			if spell.SpellCode == SpellCode_WarriorWhirlwind {
				spellToUse = hitSpecialNormalized
			} else {
				curDmg = result.Damage
				curDmg /= result.ResistanceMultiplier // Undo armor reduction to get the raw damage value.
				spellToUse = hitSchoolDamagWithValue
			}

			if numTargets > 1 {
				target := warrior.Env.NextTargetUnit(result.Target)
				spellToUse.Cast(sim, target)
				spellToUse.SpellMetrics[target.UnitIndex].Casts--
			}

			aura.RemoveStack(sim)
		},
	})

	SweepingStrikes := warrior.RegisterSpell(BattleStance, core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: row.SpellSchool,
		Flags:       core.SpellFlagHelpful,

		RageCost: core.RageCostOptions{
			Cost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			ssAura.Activate(sim)
		},
	})

	warrior.AddMajorCooldown(core.MajorCooldown{
		Spell: SweepingStrikes.Spell,
		Type:  core.CooldownTypeDPS,
		ShouldActivate: func(sim *core.Simulation, character *core.Character) bool {
			return sim.GetNumTargets() >= 2
		},
	})
}
