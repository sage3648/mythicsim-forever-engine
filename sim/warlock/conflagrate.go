package warlock

import (
	"github.com/wowsims/classic/sim/core"
)

const ConflagrateRanks = 6

// Beta client 1.60.1: Forever adds two ranks below Classic's four (1293817 at 25, 1293818 at 32),
// which makes Classic's 17962 rank 3, and every rank does about half Classic's damage. Everything but
// the damage comes from the client table (see shadowbolt.go).
var ConflagrateBaseDamage = [ConflagrateRanks + 1][]float64{{0}, {88, 111}, {113, 142}, {134, 170}, {179, 222}, {220, 273}, {251, 313}}

func (warlock *Warlock) getConflagrateConfig(rank int) core.SpellConfig {
	row := spellData.Conflagrate.ByRank(int32(rank))
	baseDamageMin := ConflagrateBaseDamage[rank][0]
	baseDamageMax := ConflagrateBaseDamage[rank][1]
	level := [ConflagrateRanks + 1]int{0, 25, 32, 40, 48, 54, 60}[rank]

	// 20% per point, so at 5/5 Conflagrate stops consuming Immolate altogether. The demo
	// only showed rank 1 and the tree repeated its 20% at every rank, which is why this was
	// held flat; the beta has since confirmed 80% at rank 4 and 100% at rank 5.
	keepImmolateChance := 0.2 * float64(warlock.Talents.ShadowAndFlame)

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockConflagrate,
		ClassSpellMask: SpellMaskConflagrate,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | WarlockFlagDestruction,
		Rank:           rank,
		RequiredLevel:  level,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warlock.getActiveImmolateSpell(target) != nil
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(baseDamageMin, baseDamageMax)

			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)

			immoSpell := warlock.getActiveImmolateSpell(target)
			if immoSpell != nil && !sim.Proc(keepImmolateChance, "Shadow and Flame") {
				immoSpell.Dot(target).Deactivate(sim)
			}
		},
	}
}

func (warlock *Warlock) registerConflagrateSpell() {
	if !warlock.Talents.Conflagrate {
		return
	}

	warlock.Conflagrate = make([]*core.Spell, 0)
	for rank := 1; rank <= ConflagrateRanks; rank++ {
		config := warlock.getConflagrateConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.Conflagrate = append(warlock.Conflagrate, warlock.GetOrRegisterSpell(config))
		}
	}
}
