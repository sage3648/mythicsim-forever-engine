// Paladin export: class spell names, client damage rows and the effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"fmt"
	"reflect"
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/paladin"
)

func init() {
	classExports[proto.Class_ClassPaladin] = classExport{
		spells: paladinClassSpells, damageRows: paladinDamageRows, effects: paladinEffects,
		unrepresented: paladinUnrepresented, statAuras: paladinStatAuras,
	}
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

// The client rows sim/paladin reads. spellData is private to the package, so the ladders are
// restated with the ids of sim/paladin/spell_data_auto_gen.go at the pinned revision.
var (
	paladinJudgementOfCommand       = spelldata.Ranked(20425, 20962, 20961, 20967, 20968)
	paladinJudgementOfRighteousness = spelldata.Ranked(20187, 20280, 20281, 20282, 20283, 20284, 20285, 20286)
	paladinSealOfCommandTriggered   = spelldata.Ranked(20424, 20467, 20963, 20964, 20965, 20966)
	paladinConsecrationTriggered    = spelldata.Ranked(1280345, 1280346, 1280347, 1280348, 1280349)
	paladinImprovedSeals            = spelldata.Talent(20224, 3)
	paladinSanctifiedJudgement      = spelldata.Talent(1311074, 3)
	paladinVengeance                = spelldata.Talent(20049, 3)
	paladinDivineFavor              = spelldata.Ranked(20216)
	paladinConsecratedGround        = spelldata.Talent(1310905, 2)
)

// seal_of_righteousness.go sealOfRighteousnessProcIDs: the damage spell each rank fires.
var paladinSealOfRighteousnessProcIDs = map[int32]int32{1: 25742, 2: 25740, 3: 25739, 4: 25738, 5: 25737, 6: 25736, 7: 25735, 8: 25713}

// Paladin spell rows whose ApplyEffects roll a client damage effect: every Holy Strike rank's flat
// part (holy_strike.go), every Hammer of Wrath rank (hammer_of_wrath.go), every Judgement of
// Righteousness rank (seal_of_righteousness.go) and, for every Judgement of Command rank, the
// triggered row its first effect names (seal_of_command.go).
func paladinDamageRows(rows map[int32]*spelldata.Spell) {
	paladin.HolyStrikeRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	paladin.HammerOfWrathRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	paladinJudgementOfRighteousness.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	paladinJudgementOfCommand.Each(func(_ int32, row *spelldata.Spell) {
		rows[row.ID] = paladinSealOfCommandTriggered.ByID(int32(row.EffectN(1).BaseValue()))
	})
}

// seals.go sealLabel: Seal of Justice has one rank and no rank subtext.
func paladinSealLabel(name string, p *paladin.Paladin, rank *spelldata.Spell) string {
	if rank.RankNumber() == 0 {
		return name + p.Label
	}
	return fmt.Sprintf("%s%s Rank %d", name, p.Label, rank.RankNumber())
}

func paladinEffects(agent core.Agent, character *core.Character) []map[string]any {
	p := agent.(paladin.PaladinAgent).GetPaladin()
	talents := p.Talents
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
	// judgement.go: the active seal's judgement, then a rotation wake a batch window after the
	// cooldown ends.
	effects = append(effects, map[string]any{
		"kind": "judgement", "spell_id": p.Judgement.ActionID.SpellID, "wake_delay_ns": nanos(core.SpellBatchWindow),
	})
	// seals.go dealAfterBatch: seal procs deal their damage a batch window after the hit.
	dealDelay := nanos(core.SpellBatchWindow)
	if talents.SealOfCommand { // seal_of_command.go
		procRank := paladinSealOfCommandTriggered.Rank(1)
		procEffect := procRank.EffectN(1)
		ranks := []map[string]any{}
		paladin.SealOfCommandRankMap.Each(func(_ int32, rank *spelldata.Spell) {
			ranks = append(ranks, map[string]any{
				"seal_spell_id": rank.ID, "aura": paladinSealLabel("Seal of Command", p, rank),
				"judgement_spell_id": paladinJudgementOfCommand.Rank(rank.RankNumber()).ID,
			})
		})
		// NewLegacyPPMManager(7, ProcMaskMeleeWhiteHit), rolled for the main hand auto: 7 procs a
		// minute is a Go literal; the chance comes from Go's own proc manager.
		dpm := character.NewStaticLegacyPPMManager(7, core.ProcMaskMeleeWhiteHit)
		effects = append(effects, map[string]any{
			"kind": "seal_of_command", "ranks": ranks, "proc_spell_id": procRank.ID,
			"weapon_percent": procEffect.Percent() * paladinImprovedSeals.MultiplierAt(talents.ImprovedSeals),
			"coefficient":    procEffect.Coeff(), "proc_chance": dpm.Chance(core.ProcMaskMeleeMHAuto, nil),
			"rng_label": "Seal of Command", "icd_ns": nanos(time.Second), "deal_delay_ns": dealDelay,
		})
	}
	{ // seal_of_righteousness.go
		mh := character.MainHand()
		handMultiplier := core.TernaryFloat64(mh.HandType == proto.HandType_HandTypeTwoHand, 1.2, 0.85)
		ranks := []map[string]any{}
		paladin.SealOfRighteousnessRankMap.Each(func(_ int32, rank *spelldata.Spell) {
			judgeRank := paladinJudgementOfRighteousness.ByID(int32(rank.EffectN(2).BaseValue()))
			ranks = append(ranks, map[string]any{
				"seal_spell_id": rank.ID, "aura": paladinSealLabel("Seal of Righteousness", p, rank),
				"judgement_spell_id": judgeRank.ID, "proc_spell_id": paladinSealOfRighteousnessProcIDs[rank.RankNumber()],
				"per_hit_value": judgeRank.EffectN(2).Average(core.CharacterLevel),
			})
		})
		effects = append(effects, map[string]any{
			"kind": "seal_of_righteousness", "ranks": ranks, "hand_multiplier": handMultiplier,
			"swing_speed": mh.SwingSpeed, "deal_delay_ns": dealDelay,
		})
	}
	// holy_strike.go: the weapon percent of the normalized swing plus the flat roll.
	holyStrikes := []map[string]any{}
	paladin.HolyStrikeRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		holyStrikes = append(holyStrikes, map[string]any{"spell_id": rank.ID, "weapon_percent": rank.EffectN(2).Percent()})
	})
	effects = append(effects, map[string]any{"kind": "holy_strike", "ranks": holyStrikes})
	// hammer_of_wrath.go: the damage rolls are on the spells.
	effects = append(effects, map[string]any{"kind": "hammer_of_wrath"})
	// consecration.go: the tick everyone takes, and the bonus the first targets take with its own
	// coefficient.
	consecrations := []map[string]any{}
	paladin.ConsecrationRankMap.Each(func(n int32, rank *spelldata.Spell) {
		tickSpell := paladinConsecrationTriggered.Rank(n)
		tick := tickSpell.EffectN(1)
		bonus := tickSpell.EffectN(2)
		dummy := rank.Effect(dbcenums.A_PERIODIC_DUMMY, 0)
		consecrations = append(consecrations, map[string]any{
			"spell_id": rank.ID, "tick": tick.Average(core.CharacterLevel), "bonus": bonus.Average(core.CharacterLevel),
			"bonus_coefficient": bonus.Coeff(), "bonus_targets": int32(dummy.Average(core.CharacterLevel)),
		})
	})
	consecration := map[string]any{"kind": "consecration", "ranks": consecrations}
	if talents.ConsecratedGround > 0 { // talents_holy.go applyConsecratedGround
		consecration["consecrated_ground"] = map[string]any{
			"aura":       "Consecrated Ground" + p.Label,
			"multiplier": paladinConsecratedGround.MultiplierAt(talents.ConsecratedGround),
		}
	}
	effects = append(effects, consecration)
	if talents.HolyShock { // holy_shock.go: the hand-written rank table, read through reflection
		ranks := []map[string]any{}
		table := reflect.ValueOf(paladin.HolyShockRanks)
		for i := 0; i < table.Len(); i++ {
			rank := table.Index(i)
			ranks = append(ranks, map[string]any{
				"spell_id": int32(rank.FieldByName("spellID").Int()),
				"min":      rank.FieldByName("damage").Index(0).Float(),
				"max":      rank.FieldByName("damage").Index(1).Float(),
			})
		}
		effects = append(effects, map[string]any{"kind": "holy_shock", "ranks": ranks})
	}
	if talents.DivineFavor { // divine_favor.go
		rank := paladinDivineFavor.Highest()
		effects = append(effects, map[string]any{
			"kind": "divine_favor", "spell_id": rank.ID, "aura": "Divine Favor" + p.Label,
			"crit":   rank.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_CRITICAL_CHANCE)).Average(core.CharacterLevel),
			"spells": []string{"holy_light", "flash_of_light", "holy_shock_heal", "holy_shock"},
		})
	}
	if talents.Vengeance > 0 { // talents_retribution.go applyVengeance
		// The damage mod's School and ProcMask, as core shouldApply matches them.
		spells := []int{}
		for i, spell := range character.Spellbook {
			if spell.Flags.Matches(core.SpellFlagNoSpellMods) || !(core.SpellSchoolHoly|core.SpellSchoolPhysical).Matches(spell.SpellSchool) ||
				!(^core.ProcMaskSpellHealing).Matches(spell.ProcMask) {
				continue
			}
			spells = append(spells, i)
		}
		effects = append(effects, map[string]any{
			"kind": "vengeance", "trigger_aura": "Vengeance - Trigger" + p.Label, "aura": "Vengeance" + p.Label,
			"per_stack": paladinVengeance.FractionAt(talents.Vengeance), "spells": spells,
		})
	}
	if talents.Vindication > 0 { // talents_retribution.go applyVindication: the chance is a Go literal
		effects = append(effects, map[string]any{
			"kind": "vindication", "trigger_aura": "Vindication - Trigger" + p.Label, "proc_chance": 1.0,
			"aura": "Vindication" + p.Label, "target_aura": "Vindication" + p.Label,
		})
	}
	if talents.SanctifiedJudgement > 0 { // talents_retribution.go applySanctifiedJudgement
		effects = append(effects, map[string]any{
			"kind": "sanctified_judgement", "trigger_aura": "Sanctified Judgement" + p.Label,
			"proc_chance":       paladinSanctifiedJudgement.EffectAt(1).FractionAt(talents.SanctifiedJudgement),
			"refund":            paladinSanctifiedJudgement.EffectAt(2).FractionAt(talents.SanctifiedJudgement),
			"metrics_action_id": actionID(core.ActionID{SpellID: paladinSanctifiedJudgement.Highest().ID}),
		})
	}
	if talents.SacredArbiter { // talents_retribution.go applySacredArbiter
		tagged := []string{}
		for _, aura := range target.GetAurasWithTag(buffs.JudgementAuraTag) {
			tagged = append(tagged, aura.Label)
		}
		effects = append(effects, map[string]any{
			"kind": "sacred_arbiter", "trigger_aura": "Sacred Arbiter" + p.Label, "judgement_auras": tagged,
		})
	}
	effects = append(effects, paladinTankEffects(p, character)...)
	if talents.TwistOfLight { // talents_retribution.go applyTwistOfLight, in its fixed order
		echoes := []map[string]any{}
		for _, echo := range []struct {
			id   int32
			seal string
		}{{1311703, "seal_of_command"}, {1311701, "seal_of_fury"}, {1311704, "seal_of_righteousness"}, {1311705, "seal_of_justice"}} {
			echoes = append(echoes, map[string]any{"aura": fmt.Sprintf("Echo (%d)%s", echo.id, p.Label), "seal": echo.seal})
		}
		effects = append(effects, map[string]any{"kind": "twist_of_light", "trigger_aura": "Twist of Light" + p.Label, "echoes": echoes})
	}
	return effects
}

// The paladin auras whose gain or loss changes the stats the runtime reads or the target's
// swings at a tanking paladin, so their rolls are read for every combination:
// talents_retribution.go Vindication's attack power, talents_protection.go Redoubt's block
// chance, Improved Righteous Fury's and Iron Creed's damage taken, and holy_shield.go's block
// chance, for the highest rank only, the one the runtime casts.
func paladinStatAuras(agent core.Agent, character *core.Character) []string {
	p := agent.(paladin.PaladinAgent).GetPaladin()
	talents := p.Talents
	labels := []string{}
	if talents.Vindication > 0 {
		labels = append(labels, "Vindication"+p.Label)
	}
	if character.Unit.HardcastAvoidanceAura == nil {
		return labels
	}
	if talents.Redoubt > 0 {
		labels = append(labels, "Redoubt"+p.Label)
	}
	if talents.HolyShield {
		labels = append(labels, fmt.Sprintf("Holy Shield%s Rank %d", p.Label, paladin.HolyShieldRankMap.Highest().RankNumber()))
	}
	if talents.IronCreed > 0 {
		labels = append(labels, "Iron Creed"+p.Label)
	}
	if talents.ImprovedRighteousFury > 0 {
		labels = append(labels, "Righteous Fury")
	}
	return labels
}

// The talents and spells of a paladin tanking the target.
func paladinTankEffects(p *paladin.Paladin, character *core.Character) []map[string]any {
	talents := p.Talents
	effects := []map[string]any{}
	// righteous_fury.go: a Holy threat mod while the aura holds; Instrument of Law's threat
	// reduction holds only while it is down.
	fury := spelldata.Ranked(25780).Highest()
	rf := map[string]any{
		"kind": "righteous_fury", "spell_id": fury.ID, "aura": "Righteous Fury",
		"threat_percent": fury.Effect(dbcenums.A_MOD_THREAT, 2).Percent(),
	}
	if talents.InstrumentOfLaw > 0 {
		law := spelldata.Talent(1311085, 2)
		rf["instrument_of_law"] = map[string]any{
			"aura":       "Instrument of Law" + p.Label,
			"multiplier": 1 - law.Effect(dbcenums.A_MOD_THREAT, 127).FractionAt(talents.InstrumentOfLaw),
		}
	}
	effects = append(effects, rf)
	if talents.SwiftJudgement { // swift_judgement.go
		rank := spelldata.Ranked(1310994).Highest()
		effects = append(effects, map[string]any{
			"kind": "swift_judgement", "spell_id": rank.ID, "aura": "Swift Judgement" + p.Label,
			"cost_percent_add": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Percent(),
		})
	}
	if talents.TemplarsBulwark { // templars_bulwark.go: a survival cooldown Go never fires at 0 health
		effects = append(effects, map[string]any{"kind": "templars_bulwark", "spell_id": spelldata.Ranked(1311015).Highest().ID})
	}
	if talents.Redoubt > 0 { // talents_protection.go applyRedoubt: the chance is a Go literal
		effects = append(effects, map[string]any{
			"kind": "redoubt", "trigger_aura": "Redoubt - Trigger" + p.Label, "aura": "Redoubt" + p.Label,
			"proc_chance": 0.02 * float64(talents.Redoubt),
		})
	}
	if talents.ShieldSpecialization > 0 { // talents_protection.go applyShieldSpecialization
		share := spelldata.Ranked(1310925).Highest()
		effects = append(effects, map[string]any{
			"kind": "shield_specialization", "trigger_aura": "Shield Specialization" + p.Label,
			"proc_chance":       spelldata.Talent(20150, 3).EffectAt(2).FractionAt(talents.ShieldSpecialization),
			"mana_share":        share.EffectN(1).Percent(),
			"metrics_action_id": actionID(core.ActionID{SpellID: share.ID}),
		})
	}
	if talents.Reckoning > 0 { // talents_protection.go applyReckoning
		block := spelldata.Talent(20177, 5).FractionAt(talents.Reckoning)
		effects = append(effects, map[string]any{
			"kind": "reckoning", "block_aura": "Reckoning - Block" + p.Label, "crit_aura": "Reckoning - Crit" + p.Label,
			"block_chance": block, "crit_chance": block * 2.5,
		})
	}
	if talents.IronCreed > 0 { // talents_protection.go applyIronCreed
		effects = append(effects, map[string]any{
			"kind": "iron_creed", "trigger_aura": "Iron Creed - Trigger" + p.Label, "aura": "Iron Creed" + p.Label,
		})
	}
	if talents.HolyShield { // holy_shield.go, the highest rank
		rank := paladin.HolyShieldRankMap.Highest()
		proc := -1
		for i, spell := range character.Spellbook {
			if spell.ActionID == (core.ActionID{SpellID: rank.ID, Tag: 2}) {
				proc = i
			}
		}
		effects = append(effects, map[string]any{
			"kind": "holy_shield", "spell_id": rank.ID, "proc_spell": proc,
			"aura":    fmt.Sprintf("Holy Shield%s Rank %d", p.Label, rank.RankNumber()),
			"charges": int32(rank.ProcCharges), "damage": rank.Effect(dbcenums.A_PROC_TRIGGER_DAMAGE, 0).Average(core.CharacterLevel),
		})
	}
	return effects
}

// Paladin behavior the effects cannot describe.
func paladinUnrepresented(agent core.Agent, character *core.Character) []string {
	p := agent.(paladin.PaladinAgent).GetPaladin()
	notes := []string{}
	target := character.Env.Encounter.ActiveTargetUnits[0]
	// seal_of_command.go doubles its judgement on a stunned target.
	if target.PseudoStats.Stunned {
		notes = append(notes, "Judgement of Command against a stunned target is unsupported")
	}
	// holy_shield.go casts only with a shield to block with.
	if p.Talents.HolyShield && !character.PseudoStats.CanBlock {
		notes = append(notes, "Holy Shield without a shield is unsupported")
	}
	// holy_shock.go: Light's Vigil on the enemy turns Holy Shock into the vigil's strike.
	if p.Talents.LightsVigil {
		notes = append(notes, "Light's Vigil is unsupported")
	}
	return notes
}
