package hunter

import (
	"github.com/wowsims/classic/sim/core"
)

func (hunter *Hunter) getWingClipConfig(rank int) core.SpellConfig {
	// Spell ID, cost, flat damage, school and defense type from the client table (see aimed_shot.go).
	// Its coefficient of 1 is not used: ours has never added weapon damage or attack power.
	row := spellData.WingClip.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()
	level := [4]int{0, 12, 38, 60}[rank]

	return core.SpellConfig{
		SpellCode:      SpellCode_HunterWingClip,
		ClassSpellMask: SpellMaskWingClip,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagBinary,
		Rank:           rank,
		RequiredLevel:  level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return hunter.DistanceFromTarget <= core.MaxMeleeAttackDistance
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeWeaponSpecialHitAndCrit)
		},
	}
}

func (hunter *Hunter) registerWingClipSpell() {
	rank := map[int32]int{
		25: 1,
		40: 2,
		50: 3,
		60: 3,
	}[hunter.Level]

	config := hunter.getWingClipConfig(rank)
	hunter.WingClip = hunter.GetOrRegisterSpell(config)
}
