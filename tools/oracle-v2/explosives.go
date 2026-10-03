// The basic explosives, consumes.go newBasicExplosiveSpellConfig without the Goblin Sapper
// Charge's self hit. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"

	"github.com/wowsims/forever/sim/core"
)

type basicExplosive struct {
	min, max, speed float64
	school          core.SpellSchool
}

// consumes.go's damage literals, keyed on the item ID of the spell each registers: the named
// constructors and the saftBombs table, which the package keeps private.
var basicExplosives = map[int32]basicExplosive{
	18588:  {213, 287, 14, core.SpellSchoolFire}, // newEzThroDynamiteTwoSpell
	11566:  {383, 517, 0, core.SpellSchoolFire},  // newCrystalChargeSpell
	15993:  {300, 500, 25, core.SpellSchoolFire}, // newThoriumGrenadeSpell
	18641:  {340, 460, 14, core.SpellSchoolFire}, // newDenseDynamiteSpell
	217495: {183, 247, 0, core.SpellSchoolFrost}, // newCryoblastSpell
	260793: {22, 28, 14, core.SpellSchoolFire},   // SAF-T Copper Bomb
	260792: {26, 34, 14, core.SpellSchoolFire},   // SAF-T Dynamite
	260795: {43, 57, 14, core.SpellSchoolFire},   // EZ-Thro Copper Bomb XL
	260797: {73, 97, 14, core.SpellSchoolFire},   // SAF-T Bronze Bomb
	260798: {128, 172, 14, core.SpellSchoolFire}, // SAF-T Jumbo Dynamite
	260805: {149, 201, 14, core.SpellSchoolFire}, // SAF-T Bomb
	260809: {149, 201, 14, core.SpellSchoolFire}, // Tru-Trigger Frag Bomb
	260803: {213, 287, 14, core.SpellSchoolFire}, // EZ-Thro Grenade
	260814: {340, 460, 14, core.SpellSchoolFire}, // SAF-T Clever Dynamite
	260816: {300, 500, 25, core.SpellSchoolFire}, // EZ-Thro Thorium Grenade
	260817: {225, 675, 14, core.SpellSchoolFire}, // EZ-Thro Dark Bomb
}

// A rolled hit on every target scaled by the AoE cap, dealt after travel when the explosive
// flies. The spell's registered school and missile speed must agree with the restated literals.
func basicExplosiveEffect(character *core.Character, spell *core.Spell, unrepresented *[]string) map[string]any {
	explosive := basicExplosives[spell.ActionID.ItemID]
	if spell.ActionID.Tag != 0 || spell.SpellSchool != explosive.school || spell.MissileSpeed != explosive.speed {
		*unrepresented = append(*unrepresented, fmt.Sprintf("explosive %s differs from its restated literals", spell.ActionID))
	}
	return map[string]any{
		"kind": "basic_explosive", "item_id": spell.ActionID.ItemID,
		"min_damage": explosive.min, "max_damage": explosive.max,
		"aoe_cap_multiplier": character.Env.Encounter.AOECapMultiplier(),
	}
}
