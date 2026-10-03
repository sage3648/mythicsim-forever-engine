// Paladin export: class spell names, client damage rows and the effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/paladin"
)

func init() {
	classExports[proto.Class_ClassPaladin] = classExport{spells: paladinClassSpells, damageRows: paladinDamageRows, effects: paladinEffects}
}

// sim/paladin/spell_masks.go.
var paladinClassSpells = []classSpellName{
	{paladin.SpellMaskJudgement, "judgement"},
	{paladin.SpellMaskHolyStrike, "holy_strike"},
	{paladin.SpellMaskConsecration, "consecration"},
	{paladin.SpellMaskExorcism, "exorcism"},
	{paladin.SpellMaskHammerOfWrath, "hammer_of_wrath"},
	{paladin.SpellMaskHolyWrath, "holy_wrath"},
	{paladin.SpellMaskHolyLight, "holy_light"},
	{paladin.SpellMaskFlashOfLight, "flash_of_light"},
	{paladin.SpellMaskLayOnHands, "lay_on_hands"},
	{paladin.SpellMaskRighteousFury, "righteous_fury"},
	{paladin.SpellMaskHammerOfTheRighteous, "hammer_of_the_righteous"},
	{paladin.SpellMaskDivineFavor, "divine_favor"},
	{paladin.SpellMaskHolyShock, "holy_shock"},
	{paladin.SpellMaskHolyShockHeal, "holy_shock_heal"},
	{paladin.SpellMaskHolyShield, "holy_shield"},
	{paladin.SpellMaskHolyShieldProc, "holy_shield_proc"},
	{paladin.SpellMaskSwiftJudgement, "swift_judgement"},
	{paladin.SpellMaskTemplarsBulwark, "templars_bulwark"},
	{paladin.SpellMaskLightsVigil, "lights_vigil"},
	{paladin.SpellMaskLightsVigilStrike, "lights_vigil_strike"},
	{paladin.SpellMaskSealOfRighteousness, "seal_of_righteousness"},
	{paladin.SpellMaskSealOfCommand, "seal_of_command"},
	{paladin.SpellMaskSealOfLight, "seal_of_light"},
	{paladin.SpellMaskSealOfWisdom, "seal_of_wisdom"},
	{paladin.SpellMaskSealOfJustice, "seal_of_justice"},
	{paladin.SpellMaskSealOfTheCrusader, "seal_of_the_crusader"},
	{paladin.SpellMaskSealOfFury, "seal_of_fury"},
	{paladin.SpellMaskSealOfRighteousnessProc, "seal_of_righteousness_proc"},
	{paladin.SpellMaskSealOfCommandProc, "seal_of_command_proc"},
	{paladin.SpellMaskSealOfLightProc, "seal_of_light_proc"},
	{paladin.SpellMaskSealOfWisdomProc, "seal_of_wisdom_proc"},
	{paladin.SpellMaskSealOfFuryProc, "seal_of_fury_proc"},
	{paladin.SpellMaskJudgementOfRighteousness, "judgement_of_righteousness"},
	{paladin.SpellMaskJudgementOfCommand, "judgement_of_command"},
	{paladin.SpellMaskJudgementOfLight, "judgement_of_light"},
	{paladin.SpellMaskJudgementOfWisdom, "judgement_of_wisdom"},
	{paladin.SpellMaskJudgementOfJustice, "judgement_of_justice"},
	{paladin.SpellMaskJudgementOfTheCrusader, "judgement_of_the_crusader"},
	{paladin.SpellMaskJudgementOfFury, "judgement_of_fury"},
	{paladin.SpellMaskDevotionAura, "devotion_aura"},
	{paladin.SpellMaskRetributionAura, "retribution_aura"},
	{paladin.SpellMaskConcentrationAura, "concentration_aura"},
	{paladin.SpellMaskFireResistanceAura, "fire_resistance_aura"},
	{paladin.SpellMaskFrostResistanceAura, "frost_resistance_aura"},
	{paladin.SpellMaskShadowResistanceAura, "shadow_resistance_aura"},
}

func paladinDamageRows(rows map[int32]*spelldata.Spell) {}

func paladinEffects(agent core.Agent, character *core.Character) []map[string]any {
	p := agent.(paladin.PaladinAgent).GetPaladin()
	effects := []map[string]any{}
	target := character.Env.Encounter.ActiveTargetUnits[0]
	// judgement.go: every landed melee strike refreshes the active judgement debuffs.
	judgements := []string{}
	for _, auras := range p.JudgementAuras {
		if aura := auras.Get(target); aura != nil {
			judgements = append(judgements, aura.Label)
		}
	}
	effects = append(effects, map[string]any{
		"kind": "judgement_refresh", "trigger_aura": "Judgement Refresh" + p.Label,
		"proc_mask": procMaskNames(core.ProcMaskMelee), "judgement_auras": judgements,
	})
	return effects
}
