package paladin

import (
	"slices"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

func (paladin *Paladin) registerLayOnHands() {

	minLevels := []int32{50, 30, 10}
	idx := slices.IndexFunc(minLevels, func(level int32) bool {
		return paladin.Level >= level
	})

	if idx == -1 {
		return
	}

	// Mana returned, school and the Forever cooldown come from the client table; the id stays ours, as
	// the rank is picked by level.
	spellID := []int32{10310, 2800, 633}[idx]
	row := spellData.LayOnHands.BySpellID(spellID)
	manaReturn := shared.SpellDataMin(row.Energize)

	// Only register the highest available rank of LoH (no benefit to using lower ranks)
	actionID := core.ActionID{SpellID: spellID}
	layOnHandsManaMetrics := paladin.NewManaMetrics(actionID)
	layOnHandsHealthMetrics := paladin.NewHealthMetrics(actionID)
	layOnHands := paladin.RegisterSpell(core.SpellConfig{
		ActionID:       actionID,
		ProcMask:       core.ProcMaskSpellHealing,
		Flags:          core.SpellFlagAPL | core.SpellFlagMCD,
		SpellSchool:    row.SpellSchool,
		SpellCode:      SpellCode_PaladinLayOnHands,
		ClassSpellMask: SpellMaskLayOnHands,
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer: paladin.NewTimer(),
				// Forever cuts the cooldown from an hour to 20 min at every rank, which the
				// beta client (1.60.1.69893) confirms.
				Duration: core.TernaryDuration(paladin.Env.IsForever(), row.Cooldown, time.Minute*60),
			},
		},
		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			paladin.SpendMana(sim, paladin.CurrentMana(), layOnHandsManaMetrics)
			paladin.GainHealth(sim, paladin.MaxHealth(), layOnHandsHealthMetrics)
			paladin.AddMana(sim, manaReturn, layOnHandsManaMetrics)
		},
	})

	paladin.AddMajorCooldown(core.MajorCooldown{
		Spell:    layOnHands,
		Priority: core.CooldownPriorityBloodlust,
		Type:     core.CooldownTypeSurvival,
		ShouldActivate: func(sim *core.Simulation, character *core.Character) bool {
			return character.CurrentHealthPercent() < 0.1 // TODO: better default condition
		},
	})
}
