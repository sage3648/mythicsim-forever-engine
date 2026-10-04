// Paladin export: class spell names, client damage rows and the effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"encoding/json"
	"fmt"
	"reflect"
	"time"

	"google.golang.org/protobuf/encoding/protojson"

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
	paladinJudgementOfCommand         = spelldata.Ranked(20425, 20962, 20961, 20967, 20968)
	paladinJudgementOfRighteousness   = spelldata.Ranked(20187, 20280, 20281, 20282, 20283, 20284, 20285, 20286)
	paladinSealOfCommandTriggered     = spelldata.Ranked(20424, 20467, 20963, 20964, 20965, 20966)
	paladinSealOfTheCrusaderTriggered = spelldata.Ranked(21183, 20188, 20300, 20301, 20302, 20303)
	paladinSealOfFuryTriggered        = spelldata.Ranked(20183, 20231, 20232, 20411, 20412, 20413, 20414, 20415, 20416, 20417, 20418, 1311647, 1311650, 1311654, 1311655)
	paladinImprovedSealOfFury         = spelldata.Ranked(1314103)
	paladinEyeForAnEye                = spelldata.Talent(9799, 2)
	paladinInfusionOfLight            = spelldata.Talent(426065, 2)
	paladinBlessingOfLight            = spelldata.Ranked(25890)
	paladinIllumination               = spelldata.Talent(20210, 5)
	paladinImprovedRighteousFury      = spelldata.Talent(20468, 3)
	paladinIronCreed                  = spelldata.Talent(1311034, 5)
	paladinPursuitOfJustice           = spelldata.Talent(26022, 2)
	paladinConsecrationTriggered      = spelldata.Ranked(1280345, 1280346, 1280347, 1280348, 1280349)
	paladinImprovedSeals              = spelldata.Talent(20224, 3)
	paladinSanctifiedJudgement        = spelldata.Talent(1311074, 3)
	paladinVengeance                  = spelldata.Talent(20049, 3)
	paladinDivineFavor                = spelldata.Ranked(20216)
	paladinLightsVigil                = spelldata.Ranked(1310911, 1311590, 1311595)
	// lights_vigil.go lightsVigilTriggered: each rank's vigil aura and enemy strike, by hand.
	paladinLightsVigilTriggered = map[int32][2]int32{1: {1310909, 1310914}, 2: {1311593, 1311592}, 3: {1311597, 1311598}}
	paladinConsecratedGround    = spelldata.Talent(1310905, 2)
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
	paladin.ExorcismRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	paladin.HolyWrathRankMap.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	paladinJudgementOfRighteousness.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	// seal_of_fury.go: each rank's judgement, the triggered row its third effect names.
	paladin.SealOfFuryRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		row := paladinSealOfFuryTriggered.ByID(int32(rank.EffectN(3).BaseValue()))
		rows[row.ID] = row
	})
	// lights_vigil.go: each rank's enemy strike.
	for _, triggered := range paladinLightsVigilTriggered {
		row := spelldata.MustFind(triggered[1])
		rows[row.ID] = row
	}
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
	paladinRecordRotation(exportRequest)
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
	{ // seal_of_fury.go: the per-hit Holy damage and its absorb shield, and each rank's judgement
		ranks := []map[string]any{}
		paladin.SealOfFuryRankMap.Each(func(_ int32, rank *spelldata.Spell) {
			judgeRank := paladinSealOfFuryTriggered.ByID(int32(rank.EffectN(3).BaseValue()))
			procRank := rank.Refs()[0]
			ranks = append(ranks, map[string]any{
				"seal_spell_id": rank.ID, "aura": paladinSealLabel("Seal of Fury", p, rank),
				"judgement_spell_id": judgeRank.ID, "proc_spell_id": procRank.ID,
				"proc_damage":  procRank.DamageEffect().Average(core.CharacterLevel),
				"shield_aura":  fmt.Sprintf("Seal of Fury Shield%s Rank %d", p.Label, rank.RankNumber()),
				"shield_share": rank.EffectN(2).Percent(),
			})
		})
		fury := map[string]any{"kind": "seal_of_fury", "ranks": ranks, "can_block": character.PseudoStats.CanBlock,
			"deal_delay_ns": dealDelay}
		// talents_protection.go applyImprovedSealOfFury: mana when a shield is spent, more per
		// level the target is above the paladin.
		if talents.ImprovedSealOfFury {
			rank := paladinImprovedSealOfFury.Highest()
			fury["improved"] = map[string]any{
				"mana": rank.EffectN(1).Average(core.CharacterLevel), "per_level": rank.EffectN(2).Percent(),
				"max_levels": rank.EffectN(3).Average(core.CharacterLevel),
				"levels":     float64(target.Level - p.Level), "metrics_action_id": actionID(core.ActionID{SpellID: rank.ID}),
			}
		}
		effects = append(effects, fury)
	}
	{ // seal_of_the_crusader.go
		ranks := []map[string]any{}
		paladin.SealOfTheCrusaderRankMap.Each(func(_ int32, rank *spelldata.Spell) {
			judgeRank := paladinSealOfTheCrusaderTriggered.ByID(int32(rank.EffectN(3).BaseValue()))
			speed := 1 + rank.Effect(dbcenums.A_MOD_ATTACKSPEED, 0).Percent()
			ranks = append(ranks, map[string]any{
				"seal_spell_id": rank.ID, "aura": paladinSealLabel("Seal of the Crusader", p, rank),
				"judgement_spell_id": judgeRank.ID,
				"judgement_aura":     fmt.Sprintf("Judgement of the Crusader Rank %d", rank.RankNumber()),
				"melee_speed":        speed, "auto_damage_percent": 1/speed - 1,
			})
		})
		// The seal's spell mod: every spell with the main hand auto's proc mask that takes mods.
		autos := []int{}
		for i, spell := range character.Spellbook {
			if !spell.Flags.Matches(core.SpellFlagNoSpellMods) && spell.ProcMask.Matches(core.ProcMaskMeleeMHAuto) {
				autos = append(autos, i)
			}
		}
		effects = append(effects, map[string]any{"kind": "seal_of_the_crusader", "ranks": ranks, "auto_spells": autos})
		// buffs/paladin.go JudgementOfTheCrusaderAura: every rank and the raid's debuff share one
		// single aura category, each bidding the Holy damage it adds.
		if category := exclusiveCategoryEffect(target, "target", "Judgement of the Crusader"); category != nil {
			effects = append(effects, category)
		}
	}
	// holy_strike.go: the weapon percent of the normalized swing plus the flat roll.
	holyStrikes := []map[string]any{}
	paladin.HolyStrikeRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		holyStrikes = append(holyStrikes, map[string]any{"spell_id": rank.ID, "weapon_percent": rank.EffectN(2).Percent()})
	})
	effects = append(effects, map[string]any{"kind": "holy_strike", "ranks": holyStrikes})
	// hammer_of_wrath.go: the damage rolls are on the spells.
	effects = append(effects, map[string]any{"kind": "hammer_of_wrath"})
	// exorcism.go and holy_wrath.go: the rolls are on the spells; both hit only an Undead or
	// Demon target, and Holy Wrath's cast pauses the swing.
	effects = append(effects, map[string]any{"kind": "exorcism"}, map[string]any{"kind": "holy_wrath"})
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
	if talents.LightsVigil { // lights_vigil.go: the vigil, its strike and its refund per rank
		ranks := []map[string]any{}
		paladinLightsVigil.Each(func(_ int32, rank *spelldata.Spell) {
			triggered := paladinLightsVigilTriggered[rank.RankNumber()]
			ranks = append(ranks, map[string]any{
				"spell_id": rank.ID, "strike_spell_id": triggered[1],
				"aura":              fmt.Sprintf("Light's Vigil%s Rank %d", p.Label, rank.RankNumber()),
				"refund":            rank.EffectN(2).Percent(),
				"metrics_action_id": actionID(core.ActionID{SpellID: rank.ID, Tag: 1}),
			})
		})
		effects = append(effects, map[string]any{"kind": "lights_vigil", "ranks": ranks})
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
			if spell.Flags.Matches(core.SpellFlagNoSpellMods) || !(core.SpellSchoolHoly | core.SpellSchoolPhysical).Matches(spell.SpellSchool) ||
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
	// talents_holy.go applyInfusionOfLight and item_librams.go Libram of Holy Alacrity: an aura
	// that speeds only Holy Light, on a Holy Shock crit or any Holy Shock cast.
	holyLights := spellsMatching(character, paladin.SpellMaskHolyLight)
	if talents.InfusionOfLight > 0 {
		effects = append(effects, map[string]any{
			"kind": "holy_light_haste", "trigger_aura": "Infusion of Light - Trigger" + p.Label,
			"aura": "Infusion of Light" + p.Label, "on_crit": true, "holy_light_spells": holyLights,
			"cast_time_ns": nanos(time.Duration(paladinInfusionOfLight.ValueAt(talents.InfusionOfLight)) * time.Millisecond),
		})
	}
	if character.GetAura("Libram of Holy Alacrity"+p.Label) != nil {
		effects = append(effects, map[string]any{
			"kind": "holy_light_haste", "trigger_aura": "Libram of Holy Alacrity" + p.Label,
			"aura": "Holy Alacrity" + p.Label, "on_crit": false, "holy_light_spells": holyLights,
			"cast_time_ns": nanos(-200 * time.Millisecond),
		})
	}
	effects = append(effects, paladinHealEffect(p, character, target))
	// lay_on_hands.go: each rank the rotation names drains the paladin's mana, restores mana to
	// a healed unit with a mana bar and heals it for the paladin's maximum health.
	layOnHands := []map[string]any{}
	paladin.LayOnHandsRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		if paladinRotationSpells[rank.ID] {
			layOnHands = append(layOnHands, map[string]any{"spell_id": rank.ID,
				"mana": rank.EnergizeEffect().Average(core.CharacterLevel)})
		}
	})
	if len(layOnHands) != 0 {
		effects = append(effects, map[string]any{"kind": "lay_on_hands", "ranks": layOnHands})
		if target.HasManaBar() {
			*classNotes = append(*classNotes, "Lay on Hands restoring the target's mana is unsupported")
		}
	}
	if talents.EyeForAnEye > 0 { // talents_retribution.go applyEyeForAnEye
		effects = append(effects, map[string]any{
			"kind": "eye_for_an_eye", "trigger_aura": "Eye for an Eye" + p.Label,
			"spell_id": paladinEyeForAnEye.Highest().ID, "share": paladinEyeForAnEye.FractionAt(talents.EyeForAnEye),
		})
	}
	if talents.PursuitOfJustice > 0 { // talents_retribution.go applyPursuitOfJustice
		bonus := paladinPursuitOfJustice.Effect(dbcenums.A_MOD_INCREASE_SPEED, 0).FractionAt(talents.PursuitOfJustice)
		aura := character.GetAura("Pursuit of Justice")
		// movement.go NewPassiveMovementSpeedEffect: the bonus holds while no stronger passive
		// speed effect shares the category, and the reset already applied it.
		for _, ee := range aura.ExclusiveEffects {
			if privateField(ee.Category, "effects").Len() != 1 {
				*classNotes = append(*classNotes, "Pursuit of Justice's movement speed shares its category")
			}
		}
		if !aura.IsActive() {
			*classNotes = append(*classNotes, "Pursuit of Justice is not active after the reset")
		}
		effects = append(effects, map[string]any{
			"kind": "pursuit_of_justice", "aura": aura.Label, "bonus": bonus,
			"initial_multiplier": character.PseudoStats.MovementSpeedMultiplier / (1 + bonus),
		})
	}
	return effects
}

// The paladin auras whose gain or loss changes the stats the runtime reads or the target's
// swings at a tanking paladin, so their rolls are read for every combination:
// talents_retribution.go Vindication's attack power, seal_of_the_crusader.go's attack power,
// talents_protection.go Redoubt's block chance and holy_shield.go's block chance, for the ranks
// the rotation names. Improved Righteous Fury's and Iron Creed's damage taken are tracked live.
func paladinStatAuras(agent core.Agent, character *core.Character) []string {
	p := agent.(paladin.PaladinAgent).GetPaladin()
	talents := p.Talents
	labels := []string{}
	if talents.Vindication > 0 {
		labels = append(labels, "Vindication"+p.Label)
	}
	// seal_of_the_crusader.go: the attack power of each rank the rotation names.
	paladin.SealOfTheCrusaderRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		if paladinRotationSpells[rank.ID] {
			labels = append(labels, paladinSealLabel("Seal of the Crusader", p, rank))
		}
	})
	if character.Unit.HardcastAvoidanceAura == nil {
		return labels
	}
	if talents.Redoubt > 0 {
		labels = append(labels, "Redoubt"+p.Label)
	}
	if talents.HolyShield {
		for _, rank := range paladinHolyShieldRanks() {
			labels = append(labels, fmt.Sprintf("Holy Shield%s Rank %d", p.Label, rank.RankNumber()))
		}
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
	if talents.TemplarsBulwark { // templars_bulwark.go: the survival cooldown's absorb shield and Forbearance
		rank := spelldata.Ranked(1311015).Highest()
		effects = append(effects, map[string]any{
			"kind": "templars_bulwark", "spell_id": rank.ID, "aura": "Templar's Bulwark" + p.Label,
			"health_share": rank.Effect(dbcenums.A_SCHOOL_ABSORB, 127).Percent(),
		})
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
	// talents_protection.go: Improved Righteous Fury and Iron Creed multiply the paladin's
	// damage taken while their auras hold, which the runtime tracks live.
	damageTaken := []map[string]any{}
	if talents.ImprovedRighteousFury > 0 {
		damageTaken = append(damageTaken, map[string]any{"aura": "Righteous Fury", "stat": "damage_taken",
			"multiplier": paladinImprovedRighteousFury.Effect(dbcenums.A_ADD_FLAT_MODIFIER, int32(dbcenums.SPELLMOD_EFFECT2)).MultiplierAt(talents.ImprovedRighteousFury)})
	}
	if talents.IronCreed > 0 {
		damageTaken = append(damageTaken, map[string]any{"aura": "Iron Creed" + p.Label, "stat": "damage_taken",
			"multiplier": 1 - paladinIronCreed.Effect(dbcenums.A_PROC_TRIGGER_SPELL_WITH_VALUE, 0).FractionAt(talents.IronCreed)})
	}
	if len(damageTaken) > 0 {
		effects = append(effects, map[string]any{"kind": "pseudo_stat_auras", "auras": damageTaken})
	}
	if talents.Illumination > 0 { // talents_holy.go applyIllumination: heal crits, the chance and the share of the base cost
		effects = append(effects, map[string]any{
			"kind": "illumination", "trigger_aura": "Illumination" + p.Label,
			"proc_chance":       paladinIllumination.EffectAt(1).FractionAt(talents.Illumination),
			"refund":            paladinIllumination.EffectAt(3).FractionAt(talents.Illumination),
			"metrics_action_id": actionID(core.ActionID{SpellID: paladinIllumination.Highest().ID}),
		})
	}
	if talents.HolyShield { // holy_shield.go, the ranks the rotation names
		ranks := []map[string]any{}
		for _, rank := range paladinHolyShieldRanks() {
			proc := -1
			for i, spell := range character.Spellbook {
				if spell.ActionID == (core.ActionID{SpellID: rank.ID, Tag: 2}) {
					proc = i
				}
			}
			ranks = append(ranks, map[string]any{
				"spell_id": rank.ID, "proc_spell": proc,
				"aura":    fmt.Sprintf("Holy Shield%s Rank %d", p.Label, rank.RankNumber()),
				"charges": int32(rank.ProcCharges), "damage": rank.Effect(dbcenums.A_PROC_TRIGGER_DAMAGE, 0).Average(core.CharacterLevel),
			})
		}
		effects = append(effects, map[string]any{
			"kind": "holy_shield", "ranks": ranks,
			// The cast's ExtraCastCondition: a shield to block with, static in scope.
			"can_block": character.PseudoStats.CanBlock,
		})
	}
	return effects
}

// The spell ids the rotation of the request being exported names anywhere, which paladinEffects
// records so the stat aura hook can read the Holy Shield ranks it names.
var paladinRotationSpells = map[int32]bool{}

func paladinRecordRotation(request *proto.RaidSimRequest) {
	paladinRotationSpells = map[int32]bool{}
	data, err := protojson.Marshal(request.Raid.Parties[0].Players[0].GetRotation())
	fail(err)
	var tree any
	fail(json.Unmarshal(data, &tree))
	var walk func(node any)
	walk = func(node any) {
		switch value := node.(type) {
		case map[string]any:
			for key, child := range value {
				if id, ok := child.(float64); ok && key == "spellId" {
					paladinRotationSpells[int32(id)] = true
				}
				walk(child)
			}
		case []any:
			for _, child := range value {
				walk(child)
			}
		}
	}
	walk(tree)
}

// The Holy Shield ranks the rotation names, whose block chance auras the stat aura combinations
// carry; a rank it never names cannot become active.
func paladinHolyShieldRanks() []*spelldata.Spell {
	ranks := []*spelldata.Spell{}
	paladin.HolyShieldRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		if paladinRotationSpells[rank.ID] {
			ranks = append(ranks, rank)
		}
	})
	return ranks
}

// Paladin behavior the effects cannot describe.
func paladinUnrepresented(_ core.Agent, character *core.Character) []string {
	notes := []string{}
	target := character.Env.Encounter.ActiveTargetUnits[0]
	// seal_of_command.go doubles its judgement on a stunned target.
	if target.PseudoStats.Stunned {
		notes = append(notes, "Judgement of Command against a stunned target is unsupported")
	}
	return notes
}

// holy_light.go, flash_of_light.go and holy_shock.go: every heal rank's roll, the Libram of
// Light's Flash of Light bonus, Blessing of Light's bonuses on a unit that carries it, and the
// healing modifiers of the paladin on itself and on the target, which a plain cast heals.
func paladinHealEffect(p *paladin.Paladin, character *core.Character, target *core.Unit) map[string]any {
	ranks := []map[string]any{}
	roll := func(name string) func(int32, *spelldata.Spell) {
		return func(_ int32, rank *spelldata.Spell) {
			heal := rank.HealEffect()
			average := heal.Average(core.CharacterLevel)
			ranks = append(ranks, map[string]any{"spell_id": rank.ID, "heal": name,
				"min": average * (1 - heal.Variance/2), "max": average * (1 + heal.Variance/2),
				"average": average, "variance": heal.Variance})
		}
	}
	paladin.HolyLightRankMap.Each(roll("holy_light"))
	paladin.FlashOfLightRankMap.Each(roll("flash_of_light"))
	table := reflect.ValueOf(paladin.HolyShockRanks)
	for i := 0; i < table.Len(); i++ {
		rank := table.Index(i)
		heal := rank.FieldByName("heal").Float()
		ranks = append(ranks, map[string]any{"spell_id": int32(rank.FieldByName("healID").Int()),
			"heal": "holy_shock_heal", "min": heal, "max": heal})
	}
	blessing := paladinBlessingOfLight.Highest()
	unit := func(u *core.Unit) map[string]any {
		modifiers := healModifiers(character, u)
		modifiers["bonus_healing_taken"] = u.PseudoStats.BonusHealingTaken
		modifiers["blessing_of_light"] = u.HasActiveAuraWithTag(buffs.GreaterBlessingOfLightCategory)
		// A blessing the fight could gain or lose would change the bonus mid-fight.
		for _, aura := range u.GetAurasWithTag(buffs.GreaterBlessingOfLightCategory) {
			if aura.IsActive() != (aura.Duration == core.NeverExpires) && classNotes != nil {
				*classNotes = append(*classNotes, "a Blessing of Light that can change during the fight is unsupported")
			}
		}
		return modifiers
	}
	return map[string]any{
		"kind": "paladin_heals", "ranks": ranks,
		"flash_of_light_bonus":    privateField(p, "flashOfLightBonusHealing").Float(),
		"blessing_holy_light":     blessing.EffectN(1).Average(core.CharacterLevel),
		"blessing_flash_of_light": blessing.EffectN(2).Average(core.CharacterLevel),
		"player":                  unit(&character.Unit), "target": unit(target),
	}
}
