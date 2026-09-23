package warrior

import (
	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (warrior *Warrior) registerThunderClapSpell() {
	// Forever beta client 1.60.1.69893: cost, cooldown, school, defense type and damage come from the
	// client table; the id stays ours (see registerHeroicStrikeSpell).
	spellID := int32(11581)
	row := spellData.ThunderClap.BySpellID(spellID)
	baseDamage := shared.SpellDataMin(row.Direct)
	has5pcConq := warrior.HasSetBonus(ItemSetConquerorsBattleGear, 5)
	// Forever doubles the slow to 20% and moves the cooldown from 4 to 6 sec. Conqueror's 5 piece
	// (26110) raises all of Thunder Clap's effects by 50%: 30%.
	attackSpeedReduction := core.TernaryInt32(has5pcConq, 30, 20)
	// Forever lets Thunder Clap be used in Defensive Stance as well.
	stanceMask := BattleStance | DefensiveStance

	warrior.ThunderClapAuras = warrior.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		return core.ThunderClapAura(target, spellID, attackSpeedReduction)
	})

	results := make([]*core.SpellResult, min(4, warrior.Env.GetNumTargets()))

	warrior.ThunderClap = warrior.RegisterSpell(stanceMask, core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskSpellDamage,
		Flags:       core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost: float64(row.Cost) - []float64{0, 2, 4, 6}[warrior.Talents.ImprovedThunderClap],
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    warrior.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: core.TernaryFloat64(has5pcConq, 1.5, 1),
		ThreatMultiplier: 2.5,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for idx := range results {
				results[idx] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
				target = sim.Environment.NextTargetUnit(target)
			}

			for _, result := range results {
				spell.DealDamage(sim, result)
				if result.Landed() {
					warrior.ThunderClapAuras.Get(result.Target).Activate(sim)
				}
			}
		},

		RelatedAuras: []core.AuraArray{warrior.ThunderClapAuras},
	})
}
