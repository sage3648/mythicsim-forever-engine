package classic

import (
	"github.com/wowsims/forever/sim/common/shared"
	"github.com/wowsims/forever/sim/core"
)

// Item procs the old generator wrote and the spell-store pipeline (upstream #42) refuses, kept as the
// old generator had them until the store can resolve them:
//   - Freezing Band and The Lion Horn of Stormwind: their triggered spells (18799, 20847) have no name
//     row in this client, so the store cannot carry them.
//   - Iceblade Hacker and Warblade of Caer Darrow: Forever's proc masks set word-1 bit 37, which the
//     decoder does not model yet; the rest of the mask is plain melee hits. The hand holding the
//     weapon is the DPM's to pick, as for every generated weapon proc: with the mask alone, an
//     Iceblade Hacker in the main hand also procced off every off-hand swing.
//
// Remove an entry once the generated files register it (core.NewItemEffect panics on a second one).
func init() {
	// When struck in combat has a 1% chance of inflicting 50 Frost damage to the attacker and freezing them
	// for 5s.
	// https://www.wowhead.com/forever/spell=18798
	shared.NewProcDamageEffect(shared.ProcDamageEffect{
		ItemID:      942,
		SpellID:     18798,
		School:      core.SpellSchoolFrost,
		DefenseType: core.DefenseTypeMagic,
		MinDmg:      50,
		MaxDmg:      50,
		Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell | core.SpellFlagNoOnDamageDealt | core.SpellFlagProc,
		Trigger: core.ProcTrigger{
			Name:               "Freezing Band",
			ActionID:           core.ActionID{ItemID: 942},
			Callback:           core.CallbackOnSpellHitTaken,
			ProcMask:           core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto | core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeOHSpecial,
			Outcome:            core.OutcomeLanded,
			RequireDamageDealt: true,
			ProcChance:         0.01,
		},
	})

	// Melee attacks with this weapon deal 41 Frost damage.
	// https://www.wowhead.com/forever/spell=1298414
	shared.NewProcDamageEffect(shared.ProcDamageEffect{
		ItemID:      13952,
		SpellID:     1298414,
		School:      core.SpellSchoolFrost,
		DefenseType: core.DefenseTypeMelee,
		MinDmg:      40.70000076293945,
		MaxDmg:      40.70000076293945,
		Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell | core.SpellFlagNoOnDamageDealt | core.SpellFlagProc | core.SpellFlagSuppressWeaponProcs,
		Trigger: core.ProcTrigger{
			Name:               "Iceblade Hacker",
			ActionID:           core.ActionID{ItemID: 13952},
			Callback:           core.CallbackOnSpellHitDealt,
			ProcMask:           core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto | core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeOHSpecial,
			Outcome:            core.OutcomeLanded,
			RequireDamageDealt: true,
			ProcChance:         1,
		},
		// "Melee attacks with this weapon": only the hand holding it, like every generated weapon proc.
		TriggerDPM: func(character *core.Character) *core.DynamicProcManager {
			return character.NewDynamicLegacyProcForWeapon(13952, 0, 1)
		},
	})

	// Melee attacks with this weapon deal 28 Frost damage.
	// https://www.wowhead.com/forever/spell=1298499
	shared.NewProcDamageEffect(shared.ProcDamageEffect{
		ItemID:      13982,
		SpellID:     1298499,
		School:      core.SpellSchoolFrost,
		DefenseType: core.DefenseTypeMelee,
		MinDmg:      27.719999313354492,
		MaxDmg:      27.719999313354492,
		Flags:       core.SpellFlagNoOnCastComplete | core.SpellFlagPassiveSpell | core.SpellFlagNoOnDamageDealt | core.SpellFlagProc | core.SpellFlagSuppressWeaponProcs,
		Trigger: core.ProcTrigger{
			Name:               "Warblade of Caer Darrow",
			ActionID:           core.ActionID{ItemID: 13982},
			Callback:           core.CallbackOnSpellHitDealt,
			ProcMask:           core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto | core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeOHSpecial,
			Outcome:            core.OutcomeLanded,
			RequireDamageDealt: true,
			ProcChance:         1,
		},
		// "Melee attacks with this weapon": only the hand holding it, like every generated weapon proc.
		TriggerDPM: func(character *core.Character) *core.DynamicProcManager {
			return character.NewDynamicLegacyProcForWeapon(13982, 0, 1)
		},
	})

	// When struck in combat has a 5% chance of increasing all party member's armor by 725 for 30s. This chance
	// is doubled in Strongholds and Cities.
	// https://www.wowhead.com/forever/spell=18946
	shared.NewProcStatBonusEffectWithVariants(shared.ProcStatBonusEffect{
		Callback:           core.CallbackOnSpellHitTaken,
		ProcMask:           core.ProcMaskMeleeMHAuto | core.ProcMaskMeleeOHAuto | core.ProcMaskMeleeMHSpecial | core.ProcMaskMeleeOHSpecial,
		Outcome:            core.OutcomeLanded,
		RequireDamageDealt: true,
	}, []shared.ItemVariant{
		{ItemID: 14557, ItemName: "The Lion Horn of Stormwind"},
	})
}
