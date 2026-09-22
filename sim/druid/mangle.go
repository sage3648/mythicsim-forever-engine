package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Berserk widens Mangle to a cleave.
const MangleBerserkTargets = 3

func (druid *Druid) registerMangleCatSpell() {
	if !druid.Talents.Mangle {
		return
	}

	// TODO: the beta client 1.60.1.69893 has no Mangle (Cat): the talent teaches only the Bear Mangle (407995 and
	// its ranks), and Season of Discovery's cat version 407993 is gone from the spellbook. The cat keeps this
	// Classic shaped Mangle because the feral rotation is built around it.
	flatDamageBonus := 26.0
	results := make([]*core.SpellResult, min(MangleBerserkTargets, druid.Env.GetNumTargets()))

	druid.MangleCat = druid.RegisterSpell(Cat, core.SpellConfig{
		SpellCode:      SpellCode_DruidMangle,
		ClassSpellMask: SpellMaskMangle,
		ActionID:       core.ActionID{SpellID: 33876},
		SpellSchool:    core.SpellSchoolPhysical,
		DefenseType:    core.DefenseTypeMelee,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagBuilder,

		EnergyCost: core.EnergyCostOptions{
			Cost:   45 - float64(druid.Talents.Ferocity),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: time.Second,
			},
			IgnoreHaste: true,
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			numHits := 1
			if druid.BerserkAura.IsActive() {
				numHits = len(results)
			}

			for idx := 0; idx < numHits; idx++ {
				baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
				results[idx] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
				target = sim.Environment.NextTargetUnit(target)
			}

			for idx := 0; idx < numHits; idx++ {
				spell.DealDamage(sim, results[idx])
			}

			if results[0].Landed() {
				druid.AddComboPoints(sim, 1, results[0].Target, spell.ComboPointMetrics())
			} else {
				spell.IssueRefund(sim)
			}
		},
	})
}

// Beta client 1.60.1.69893: Mangle is 20 Rage, a 6 sec cooldown, and 100% weapon damage plus a bonus that grows by
// rank (407995, 1238069, 1238070, 1238073 at levels 25, 36, 48, 60). The client does not carry threat, so the 1.5x is
// still Season of Discovery's. The cost, cooldown and flat bonus come from the client table (see wrath.go); the id
// stays the one the APLs name.
func (druid *Druid) registerMangleBearSpell() {
	if !druid.Talents.Mangle {
		return
	}

	row := spellData.Mangle.ByRank(map[int32]int32{25: 1, 40: 2, 50: 3, 60: 4}[druid.Level])
	flatDamageBonus, _ := row.Direct.Range()
	results := make([]*core.SpellResult, min(MangleBerserkTargets, druid.Env.GetNumTargets()))

	druid.MangleBear = druid.RegisterSpell(Bear, core.SpellConfig{
		SpellCode:      SpellCode_DruidMangle,
		ClassSpellMask: SpellMaskMangle,
		ActionID:       core.ActionID{SpellID: 33878},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost) - float64(druid.Talents.Ferocity),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    druid.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1.5,
		BonusCoefficient: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			numHits := 1
			if druid.BerserkAura.IsActive() {
				numHits = len(results)
			}

			for idx := 0; idx < numHits; idx++ {
				baseDamage := flatDamageBonus + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
				results[idx] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
				target = sim.Environment.NextTargetUnit(target)
			}

			for idx := 0; idx < numHits; idx++ {
				spell.DealDamage(sim, results[idx])
			}

			if !results[0].Landed() {
				spell.IssueRefund(sim)
			}

			if druid.BerserkAura.IsActive() {
				spell.CD.Reset()
			}
		},
	})
}
