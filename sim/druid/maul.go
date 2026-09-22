package druid

import (
	"github.com/wowsims/classic/sim/core"
)

const MaulRanks = 7

// The id, cost and flat bonus come from the client table (see wrath.go). The client does not carry threat, so the
// 1.75x stays ours.
var MaulLevel = [MaulRanks + 1]int{0, 10, 18, 26, 34, 42, 50, 58}

// Maul replaces the next melee swing, so it is queued through a separate APL spell
// and fired from the swing itself.
func (druid *Druid) registerMaulSpell() {
	rank := map[int32]int{
		25: 2,
		40: 4,
		50: 6,
		60: 7,
	}[druid.Level]

	level := MaulLevel[rank]
	row := spellData.Maul.ByRank(int32(rank))
	baseDamage, _ := row.Direct.Range()

	rageCost := float64(row.Cost) - float64(druid.Talents.Ferocity)

	switch druid.Ranged().ID {
	case IdolOfBrutality:
		rageCost -= 3
	}

	druid.Maul = druid.RegisterSpell(Bear, core.SpellConfig{
		SpellCode:      SpellCode_DruidMaul,
		ClassSpellMask: SpellMaskMaul,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeMHAuto,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagNoOnCastComplete,

		Rank:          rank,
		RequiredLevel: level,

		RageCost: core.RageCostOptions{
			Cost:   rageCost,
			Refund: 0.8,
		},

		DamageMultiplierAdditive: 1 + 0.05*float64(druid.Talents.SavageFury),
		DamageMultiplier:         1,
		ThreatMultiplier:         1.75,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := baseDamage + spell.Unit.MHWeaponDamage(sim, spell.MeleeAttackPower(target))
			result := spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMeleeSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}

			druid.MaulQueueAura.Deactivate(sim)
		},
	})

	druid.MaulQueueAura = druid.RegisterAura(core.Aura{
		Label:    "Maul Queue Aura",
		ActionID: druid.Maul.ActionID.WithTag(1),
		Duration: core.NeverExpires,
	})

	druid.MaulQueueSpell = druid.RegisterSpell(Bear, core.SpellConfig{
		ActionID: druid.Maul.ActionID.WithTag(1),
		Flags:    core.SpellFlagMeleeMetrics | core.SpellFlagAPL | core.SpellFlagCastTimeNoGCD,

		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !druid.MaulQueueAura.IsActive() &&
				druid.CurrentRage() >= druid.Maul.Cost.GetCurrentCost() &&
				!druid.IsCasting(sim)
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			druid.MaulQueueAura.Activate(sim)
		},
	})
}

// Swaps the queued Maul in for the regular swing while there is rage to pay for it.
func (druid *Druid) MaulReplaceMH(sim *core.Simulation, mhSwingSpell *core.Spell) *core.Spell {
	if !druid.MaulQueueAura.IsActive() {
		return mhSwingSpell
	}

	if !druid.Maul.CanCast(sim, druid.CurrentTarget) {
		druid.MaulQueueAura.Deactivate(sim)
		return mhSwingSpell
	}

	return druid.Maul.Spell
}
