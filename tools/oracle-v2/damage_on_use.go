// On-use items whose spell deals damage: common/shared/shared_utils.go NewSpellDataDamageOnUse.
// Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/spelldata"
)

// common/forever/stat_bonus_cds_auto_gen.go: the on-use items shared.NewSpellDataDamageOnUse
// registers.
var damageOnUseItems = map[int32]bool{8348: true, 11905: true, 13171: true, 21891: true, 219345: true, 274759: true}

// The outcome shared_utils.go damageOutcome picks for the defense type, by the runtime's name.
func onUseOutcome(defense core.DefenseType, cannotCrit bool) string {
	switch {
	case defense == core.DefenseTypeMelee && cannotCrit:
		return "melee_special_hit"
	case defense == core.DefenseTypeMelee:
		return "melee_special_hit_and_crit"
	case defense == core.DefenseTypeMagic && cannotCrit:
		return "magic_hit"
	case defense == core.DefenseTypeMagic:
		return "magic_hit_and_crit"
	}
	return ""
}

// shared_utils.go spellDataOnUseDamageSpell on the one target: the row's direct hit, rolled once
// and scaled as calcMultiTargetDamage scales it for one target (an uncapped area that does not
// split takes the AoE cap; a split share over one target is the roll), then the damage over time
// it carries, ticking the row's amount on current stats with TickOutcomeHitRolled. A row with only
// damage over time rolls its application on the hit table without a crit. Either lands after the
// row's travel. Nil for any other spell; a row it cannot describe is unrepresented and returns an
// empty effect.
func damageOnUseEffect(character *core.Character, spell *core.Spell, unrepresented *[]string) map[string]any {
	item := spell.ActionID.ItemID
	if !damageOnUseItems[item] || spell.ActionID.SpellID != 0 {
		return nil
	}
	position := -1
	for i, registered := range character.Spellbook {
		if registered == spell {
			position = i
		}
	}
	onUse := onlyOnUseEffect(item)
	if onUse == nil || position < 0 {
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d has no single on-use spell", item))
		return map[string]any{}
	}
	row := spelldata.MustFind(onUse.BuffId)
	direct, periodic := row.DamageEffect(), row.PeriodicDamageEffect()
	defense := spell.DefenseType
	physical := spell.SpellSchool.Matches(core.SpellSchoolPhysical)
	outcome := onUseOutcome(defense, row.CannotCrit())
	dot := spell.Dot(&character.Env.Encounter.AllTargets[0].Unit)
	switch {
	case outcome == "" || len(row.DebuffEffects()) != 0 || (direct == spelldata.NilEffect && periodic == spelldata.NilEffect):
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d's row is not a hit or a damage over time", item))
		return map[string]any{}
	case physical && (spell.BonusCoefficient != 0 || (dot != nil && dot.BonusCoefficient != 0)):
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d scales a physical hit with spell power", item))
		return map[string]any{}
	case periodic != spelldata.NilEffect && row.PeriodicCanCrit() && defense != core.DefenseTypeMagic:
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d's ticks roll a physical crit", item))
		return map[string]any{}
	case periodic != spelldata.NilEffect && dot == nil:
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d has no dot", item))
		return map[string]any{}
	}
	several := direct != spelldata.NilEffect && (direct.HitsAnArea() || direct.ChainTargets > 1) && character.Env.ActiveTargetCount() > 1
	if several && periodic != spelldata.NilEffect {
		*unrepresented = append(*unrepresented, fmt.Sprintf("damage on-use item %d hits several targets and leaves a damage over time", item))
		return map[string]any{}
	}
	effect := map[string]any{"kind": "damage_on_use", "item_id": item, "spell": position}
	if direct != spelldata.NilEffect {
		scale := 1.0
		if direct.HitsAnArea() && row.MaxTargets == 0 && !row.SplitsDamage {
			scale = character.Env.Encounter.AOECapMultiplier()
		}
		shape := map[string]any{"average": direct.Average(character.Level), "variance": direct.Variance,
			"scale": scale, "outcome": outcome}
		// Past one target the spell is the proc spell of procDamageShape: an area calculated on
		// every target or the row's cap of them, or a chain, every hit before any is dealt.
		switch {
		case several && direct.HitsAnArea():
			shape["area"] = map[string]any{"max_targets": int32(row.MaxTargets), "splits": row.SplitsDamage,
				"aoe_cap_multiplier": character.Env.Encounter.AOECapMultiplier()}
		case several:
			shape["chain"] = map[string]any{"targets": int32(direct.ChainTargets), "amp": float64(direct.ChainAmp)}
		}
		effect["direct"] = shape
	}
	if periodic != spelldata.NilEffect {
		ticks := map[string]any{"tick_base": periodic.Average(character.Level), "tick_can_crit": row.PeriodicCanCrit()}
		if direct == spelldata.NilEffect {
			ticks["application_outcome"] = onUseOutcome(defense, true)
		}
		effect["periodic"] = ticks
	}
	return effect
}
