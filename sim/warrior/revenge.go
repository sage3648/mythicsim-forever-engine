package warrior

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const RevengeRanks = 6

var RevengeSpellId = [RevengeRanks + 1]int32{0, 6572, 6574, 7379, 11600, 11601, 25288}

// Forever raises every rank by about 70% over Classic's. The client table holds only the centre of each
// range (spell_damage_test.go checks it), so the ranges stay ours.
var RevengeBaseDamage = [RevengeRanks + 1][]float64{{0, 0}, {20, 24}, {31, 37}, {43, 53}, {73, 91}, {109, 133}, {138, 168}}
var RevengeLevel = [RevengeRanks + 1]int{0, 14, 24, 34, 44, 54, 60}

func (warrior *Warrior) registerRevengeSpell(cdTimer *core.Timer) {
	// Cost, cooldown, school, defense type and coefficient come from the client table; the id stays ours
	// (see registerHeroicStrikeSpell), and so does the damage (see RevengeBaseDamage).
	actionID := core.ActionID{SpellID: core.TernaryInt32(core.IncludeAQ, 25288, 11601)}
	rank := core.TernaryInt32(core.IncludeAQ, 6, 5)
	row := spellData.Revenge.BySpellID(actionID.SpellID)
	basedamageLow, basedamageHigh := RevengeBaseDamage[rank][0], RevengeBaseDamage[rank][1]
	revengeLevel := float64(RevengeLevel[rank])

	warrior.revengeProcAura = warrior.RegisterAura(core.Aura{
		Label:    "Revenge",
		Duration: 5 * time.Second,
		ActionID: actionID,
	})

	warrior.RegisterAura(core.Aura{
		Label:    "Revenge Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.Outcome.Matches(core.OutcomeBlock | core.OutcomeDodge | core.OutcomeParry) {
				warrior.revengeProcAura.Activate(sim)
			}
		},
	})

	warrior.Revenge = warrior.RegisterSpell(DefensiveStance, core.SpellConfig{
		SpellCode:      SpellCode_WarriorRevenge,
		ClassSpellMask: SpellMaskRevenge,
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL | SpellFlagOffensive,

		RageCost: core.RageCostOptions{
			Cost:   float64(row.Cost),
			Refund: 0.8,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    cdTimer,
				Duration: row.Cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warrior.revengeProcAura.IsActive()
		},

		CritDamageBonus: warrior.impale(),

		DamageMultiplier: 1 + 0.2*float64(warrior.Talents.ImprovedRevenge),
		ThreatMultiplier: 2.25,
		FlatThreatBonus:  2.25 * 2 * revengeLevel,
		BonusCoefficient: row.Direct.BonusCoefficient(),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := sim.Roll(basedamageLow, basedamageHigh)
			result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)

			if !result.Landed() {
				spell.IssueRefund(sim)
			}

			warrior.revengeProcAura.Deactivate(sim)
		},
	})
}
