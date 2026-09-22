package paladin

import (
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

// Beta client 1.60.1.69893: 110 / 153 / 221 damage (Classic 65 / 95 / 130) at 0.08 (Classic 0.05), 4
// charges and 20% block (Classic 30%), trained at 40 / 50 / 60 as in Classic. Cost, cooldown,
// duration, charges, block, damage and coefficient come from the client table; the ids stay ours, as
// every rank is registered. The proc ids are Classic's learn-spell dummies, kept only to give the
// damage its own metrics line; in the client the block damage comes from the aura itself.
func (paladin *Paladin) registerHolyShield() {
	if !paladin.Talents.HolyShield {
		return
	}

	for i, level := range []int32{40, 50, 60} {
		rank := i + 1
		spellID := []int32{20925, 20927, 20928}[i]
		procID := []int32{20955, 20956, 20957}[i]

		if paladin.Level < level {
			break
		}

		row := spellData.HolyShield.BySpellID(spellID)
		numCharges := row.ProcCharges
		blockBonus := row.Effects[0].Value * core.BlockRatingPerBlockChance
		damage := shared.SpellDataMin(row.Direct)

		paladin.holyShieldProc[i] = paladin.RegisterSpell(core.SpellConfig{
			ActionID:       core.ActionID{SpellID: procID},
			SpellCode:      SpellCode_PaladinHolyShieldProc,
			ClassSpellMask: SpellMaskHolyShieldProc,
			SpellSchool:    row.SpellSchool,
			DefenseType:    row.DefenseType,
			ProcMask:       core.ProcMaskSpellDamage,

			RequiredLevel: int(level),
			Rank:          rank,

			DamageMultiplier: 1,
			ThreatMultiplier: 1.2,
			BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				// Spell damage from Holy Shield can crit, but does not miss.
				spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMagicCrit)
			},
		})

		paladin.holyShieldAura[i] = paladin.RegisterAura(core.Aura{
			Label:     "Holy Shield" + paladin.Label + strconv.Itoa(rank),
			ActionID:  core.ActionID{SpellID: spellID},
			Duration:  row.Duration,
			MaxStacks: numCharges,
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				aura.SetStacks(sim, numCharges)
				paladin.AddStatDynamic(sim, stats.Block, blockBonus)
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				paladin.AddStatDynamic(sim, stats.Block, -blockBonus)
			},
			OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				if result.DidBlock() {
					paladin.holyShieldProc[i].Cast(sim, spell.Unit)
					aura.RemoveStack(sim)
				}
			},
		})

		paladin.RegisterSpell(core.SpellConfig{
			ActionID:       core.ActionID{SpellID: spellID},
			SpellCode:      SpellCode_PaladinHolyShield,
			ClassSpellMask: SpellMaskHolyShield,
			Flags:          core.SpellFlagAPL,
			RequiredLevel:  int(level),
			Rank:           rank,
			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.benediction(),
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
				CD: core.Cooldown{
					Timer:    paladin.NewTimer(),
					Duration: row.Cooldown,
				},
			},
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				paladin.holyShieldAura[i].Activate(sim)
			},
		})
	}
}
