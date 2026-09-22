package priest

import (
	"github.com/wowsims/classic/sim/core"
)

func (priest *Priest) registerVampiricEmbraceSpell() {
	if !priest.Talents.VampiricEmbrace {
		return
	}

	// Spell ID, cost, duration (1 min in Classic), cooldown (10 sec in Classic) and school come from the
	// client table (see shadow_word_pain.go).
	row := spellData.VampiricEmbrace.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}
	manaCost := float64(row.Cost)
	duration := row.Duration
	cooldown := row.Cooldown

	partyPlayers := priest.Env.Raid.GetPlayerParty(&priest.Unit).Players
	healthMetrics := priest.NewHealthMetrics(actionID)
	healthReturnedMultuplier := 0.20

	priest.VampiricEmbraceAuras = priest.NewEnemyAuraArray(func(target *core.Unit) *core.Aura {
		return target.GetOrRegisterAura(core.Aura{
			ActionID: actionID,
			Label:    "Vampiric Embrace (Health) - " + target.Label,
			Duration: duration,
			OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				// Only the priest who cast the debuff heals from it, not every shadow caster on the target.
				if result.Landed() && spell.Unit == &priest.Unit && spell.SpellSchool.Matches(core.SpellSchoolShadow) {
					healthGained := result.Damage * healthReturnedMultuplier
					for _, player := range partyPlayers {
						player.GetCharacter().GainHealth(sim, healthGained, healthMetrics)
					}
				}
			},
		})
	})

	priest.VampiricEmbrace = priest.RegisterSpell(core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: row.SpellSchool,
		DefenseType: row.DefenseType,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       SpellFlagPriest | core.SpellFlagAPL,

		ManaCost: core.ManaCostOptions{
			FlatCost: manaCost,
		},

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    priest.NewTimer(),
				Duration: cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHit)
			if result.Landed() {
				priest.VampiricEmbraceAuras.Get(target).Activate(sim)
			}
			spell.DealOutcome(sim, result)
		},
	})
}
