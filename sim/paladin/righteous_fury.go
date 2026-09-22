package paladin

import (
	"github.com/wowsims/classic/sim/core"
)

func (paladin *Paladin) registerRighteousFury() {
	if !paladin.Options.RighteousFury {
		return
	}
	// Id and threat come from the client table.
	row := spellData.RighteousFury.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	// Beta client 1.60.1.69893: Holy threat +90% (Classic +60%).
	// 90 / 100 is 0.9, and 1 + 0.9 is exactly 1.9 in float64, so the threat is bit-identical to the
	// old *= 1.9.
	paladin.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_ThreatMultiplier_Pct,
		School:     core.SpellSchoolHoly,
		FloatValue: row.Effects[0].Value / 100,
	})

	// Improved Righteous Fury no longer adds threat, it cuts damage taken while Righteous Fury is up.
	paladin.PseudoStats.DamageTakenMultiplier *= 1 - 0.02*float64(paladin.Talents.ImprovedRighteousFury)

	rfAura := core.MakePermanent(&core.Aura{Label: "Righteous Fury", ActionID: actionID})
	paladin.RegisterAura(*rfAura)
}
