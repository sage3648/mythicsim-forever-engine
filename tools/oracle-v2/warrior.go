// Warrior export: class spell names, client damage rows and the effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/warrior"
)

func init() {
	classExports[proto.Class_ClassWarrior] = classExport{
		spells: warriorClassSpells, damageRows: func(map[int32]*spelldata.Spell) {}, effects: warriorEffects,
		unrepresented: warriorUnrepresented, statAuras: warriorStatAuras,
	}
}

// sim/warrior/warrior.go: every class mask bit by a stable name.
var warriorClassSpells = []classSpellName{
	{warrior.SpellMaskBattleShout, "battle_shout"}, {warrior.SpellMaskBerserkerRage, "berserker_rage"},
	{warrior.SpellMaskRecklessness, "recklessness"}, {warrior.SpellMaskDeathWish, "death_wish"},
	{warrior.SpellMaskRetaliation, "retaliation"}, {warrior.SpellMaskRetaliationHit, "retaliation_hit"},
	{warrior.SpellMaskShieldWall, "shield_wall"}, {warrior.SpellMaskLastStand, "last_stand"},
	{warrior.SpellMaskCharge, "charge"}, {warrior.SpellMaskIntercept, "intercept"},
	{warrior.SpellMaskDemoralizingShout, "demoralizing_shout"}, {warrior.SpellMaskBattleStance, "battle_stance"},
	{warrior.SpellMaskBerserkerStance, "berserker_stance"}, {warrior.SpellMaskDefensiveStance, "defensive_stance"},
	{warrior.SpellMaskRend, "rend"}, {warrior.SpellMaskDeepWounds, "deep_wounds"},
	{warrior.SpellMaskSweepingStrikes, "sweeping_strikes"}, {warrior.SpellMaskSweepingStrikesHit, "sweeping_strikes_hit"},
	{warrior.SpellMaskSweepingStrikesNormalizedHit, "sweeping_strikes_normalized_hit"},
	{warrior.SpellMaskHeroicStrike, "heroic_strike"}, {warrior.SpellMaskCleave, "cleave"},
	{warrior.SpellMaskExecute, "execute"}, {warrior.SpellMaskOverpower, "overpower"},
	{warrior.SpellMaskRevenge, "revenge"}, {warrior.SpellMaskSlam, "slam"},
	{warrior.SpellMaskSunderArmor, "sunder_armor"}, {warrior.SpellMaskThunderClap, "thunder_clap"},
	{warrior.SpellMaskWhirlwind, "whirlwind"}, {warrior.SpellMaskWhirlwindOh, "whirlwind_off_hand"},
	{warrior.SpellMaskShieldSlam, "shield_slam"}, {warrior.SpellMaskConcussionBlow, "concussion_blow"},
	{warrior.SpellMaskShieldBash, "shield_bash"}, {warrior.SpellMaskBloodthirst, "bloodthirst"},
	{warrior.SpellMaskMortalStrike, "mortal_strike"}, {warrior.SpellMaskShieldBlock, "shield_block"},
	{warrior.SpellMaskHamstring, "hamstring"}, {warrior.SpellMaskPummel, "pummel"},
	{warrior.SpellMaskMockingBlow, "mocking_blow"}, {warrior.SpellMaskChallengingShout, "challenging_shout"},
	{warrior.SpellMaskIntimidatingShout, "intimidating_shout"}, {warrior.SpellMaskDisarm, "disarm"},
	{warrior.SpellMaskTaunt, "taunt"}, {warrior.SpellMaskVictoryRush, "victory_rush"},
	{warrior.SpellMaskSpearingStrike, "spearing_strike"},
}

// The client rows sim/warrior reads. spellData is private to the package, so the ladders are
// restated with the ids of sim/warrior/spell_data_auto_gen.go at the pinned revision.
var (
	warriorAngerManagement       = spelldata.Ranked(12296)
	warriorBerserkerRage         = spelldata.Ranked(18499)
	warriorBloodrage             = spelldata.Ranked(2687)
	warriorBloodrageTriggered    = spelldata.Ranked(29131)
	warriorBloodthirst           = spelldata.Ranked(23881, 23892, 23893, 23894)
	warriorCleave                = spelldata.Ranked(845, 7369, 11608, 11609, 20569)
	warriorHeroicStrike          = spelldata.Ranked(78, 284, 285, 1608, 11564, 11565, 11566, 11567, 25286)
	warriorDeathWish             = spelldata.Ranked(12328)
	warriorDeepWounds            = spelldata.Talent(12834, 3)
	warriorDeepWoundsTriggered   = spelldata.Ranked(12162, 412609)
	warriorExecute               = spelldata.Ranked(5308, 20658, 20660, 20661, 20662)
	warriorFlurry                = spelldata.Talent(12319, 5)
	warriorFlurryTriggered       = spelldata.Ranked(12966)
	warriorHamstring             = spelldata.Ranked(1715, 7372, 7373)
	warriorImprovedBerserkerRage = spelldata.Talent(20500, 2)
	warriorImprovedBloodrage     = spelldata.Talent(12301, 2)
	warriorImprovedTactical      = spelldata.Talent(12295, 5)
	warriorRecklessness          = spelldata.Ranked(1719)
	warriorSunderArmor           = spelldata.Ranked(7386, 7405, 8380, 11596, 11597)
	warriorTacticalMastery       = spelldata.Ranked(1310185)
	warriorUnbridledWrath        = spelldata.Talent(12322, 5)
	warriorUnbridledWrathTrigger = spelldata.Ranked(12964)
	warriorWhirlwind             = spelldata.Ranked(1680)
)

// The stance names Rust reads, in sim/warrior/stances.go's order.
func warriorStanceName(stance proto.WarriorStance) string {
	switch stance {
	case proto.WarriorStance_WarriorStanceBattle:
		return "battle"
	case proto.WarriorStance_WarriorStanceDefensive:
		return "defensive"
	case proto.WarriorStance_WarriorStanceBerserker:
		return "berserker"
	}
	return "none"
}

// core/rage.go EnableRageBar's OnSpellHitDealt: the rage each landed white hit gives, with Go's
// operation order: the hit factor, halved for the off hand, raised for a two-handed weapon,
// times the swing speed, the bar's base multiplier and the hand's multiplier.
func rageBarEffect(character *core.Character, baseRageMultiplier float64) map[string]any {
	bar := privateField(character, "rageBar")
	hitRage := func(weapon *core.Weapon, offHand bool) float64 {
		hitFactor := core.BaseRageHitFactor
		handMultiplier := 1.0
		if offHand {
			hitFactor /= 2
			handMultiplier = bar.FieldByName("offHandRageMultiplier").Float()
		}
		if weapon.NormalizedSwingSpeed == core.TwoHandNormalizedSwingSpeed {
			hitFactor *= core.TwoHandRageMultiplier
		}
		return hitFactor * weapon.SwingSpeed * baseRageMultiplier * handMultiplier
	}
	return map[string]any{
		"kind": "rage_bar", "aura": "RageBar", "max_rage": character.MaximumRage(),
		"starting_rage":   bar.FieldByName("startingRage").Float(),
		"main_hand_rage":  hitRage(character.AutoAttacks.MH(), false),
		"off_hand_rage":   hitRage(character.AutoAttacks.OH(), true),
		"crit_multiplier": core.CritRageMultiplier, "threat_per_rage": float64(core.ThreatPerRageGained),
	}
}

func warriorEffects(agent core.Agent, character *core.Character) []map[string]any {
	war := agent.(warrior.WarriorAgent).GetWarrior()
	talents := war.Talents
	effects := []map[string]any{rageBarEffect(character, 1)} // warrior.go NewWarrior: BaseRageMultiplier 1.

	// stances.go: the stance the warrior starts in and each stance's cast and aura.
	maxRetainedRage := warriorTacticalMastery.ValueAt(1) + warriorImprovedTactical.ValueAt(talents.ImprovedTacticalMastery)
	stances := []map[string]any{}
	for _, entry := range []struct {
		spell  *core.Spell
		stance string
	}{{war.BattleStance, "battle"}, {war.DefensiveStance, "defensive"}, {war.BerserkerStance, "berserker"}} {
		stances = append(stances, map[string]any{"spell_id": entry.spell.ActionID.SpellID, "stance": entry.stance,
			"aura": entry.spell.RelatedSelfBuff.Label})
	}
	effects = append(effects, map[string]any{
		"kind": "warrior_stances", "default_stance": warriorStanceName(war.DefaultStance), "stances": stances,
		"max_retained_rage": maxRetainedRage,
	})

	if talents.Bloodthirst { // talents_fury.go registerBloodthirst
		row := warriorBloodthirst.Highest()
		effects = append(effects, map[string]any{
			"kind": "bloodthirst", "spell_id": row.ID, "attack_power_share": row.Effects[1].Percent(),
			"base_damage": row.DamageEffect().Average(core.CharacterLevel),
		})
	}
	// whirlwind.go: Raging Blows adds the off hand's strike.
	effects = append(effects, map[string]any{
		"kind": "whirlwind", "spell_id": warriorWhirlwind.Highest().ID, "off_hand": talents.RagingBlows,
	})
	// execute.go: the dummy effect's base and ten times its chain amplitude per extra rage.
	executeRow := warriorExecute.Highest()
	effects = append(effects, map[string]any{
		"kind": "execute", "spell_id": executeRow.ID, "base_damage": executeRow.EffectN(1).Average(core.CharacterLevel),
		"damage_per_rage": float64(executeRow.EffectN(1).ChainAmp) * 10,
	})
	// hamstring.go
	effects = append(effects, map[string]any{
		"kind": "hamstring", "spell_id": warriorHamstring.Highest().ID,
		"base_damage": warriorHamstring.Highest().DamageEffect().Average(core.CharacterLevel),
	})
	// bloodrage.go: instant and periodic rage scaled by Improved Bloodrage, and a share of base health.
	bloodrageRow := warriorBloodrage.Highest()
	overTime := warriorBloodrageTriggered.Highest()
	improvedBloodrage := warriorImprovedBloodrage.MultiplierAt(talents.ImprovedBloodrage)
	effects = append(effects, map[string]any{
		"kind": "bloodrage", "spell_id": bloodrageRow.ID,
		"instant_rage":   warriorBloodrage.EffectAt(1).TenthsAt(1) * improvedBloodrage,
		"rage_per_tick":  overTime.PeriodicEffect().Tenths() * improvedBloodrage,
		"ticks":          int32(overTime.Duration() / overTime.PeriodicEffect().Period()),
		"period_ns":      nanos(overTime.PeriodicEffect().Period()),
		"health_cost":    war.GetBaseStats()[stats.Health] * float64(bloodrageRow.Power(dbcenums.POWER_HEALTH).CostPct) / 100,
		"rage_threshold": 70.0,
	})
	// berserker_rage.go: Improved Berserker Rage's rage, and the damage taken rage multiplier, a Go
	// literal that has no effect while nothing attacks the player.
	berserkerRage := warriorBerserkerRage.Highest()
	effects = append(effects, map[string]any{
		"kind": "berserker_rage", "spell_id": berserkerRage.ID, "aura": "Berserker Rage",
		"rage_gain": warriorImprovedBerserkerRage.EffectAt(1).TenthsAt(talents.ImprovedBerserkerRage),
	})
	if talents.DeathWish { // talents_fury.go registerDeathWish
		row := warriorDeathWish.Highest()
		effects = append(effects, map[string]any{
			"kind": "death_wish", "spell_id": row.ID, "aura": "Death Wish",
			"physical_multiplier": 1 + row.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 1).Percent(),
			"wait_ns":             nanos(core.GCDDefault),
		})
	}
	// Auras that multiply the warrior's damage taken while up: talents_fury.go Death Wish and
	// recklessness.go, each 1 plus its client effect.
	damageTaken := []map[string]any{}
	if aura := character.GetAura("Death Wish"); aura != nil && !aura.IsActive() {
		damageTaken = append(damageTaken, map[string]any{"aura": aura.Label,
			"multiplier": 1 + warriorDeathWish.Highest().Effect(dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN, 127).Percent()})
	}
	if aura := character.GetAura("Recklessness"); aura != nil && !aura.IsActive() {
		damageTaken = append(damageTaken, map[string]any{"aura": aura.Label,
			"multiplier": 1 + warriorRecklessness.Highest().Effect(dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN, 127).Percent()})
	}
	effects = append(effects, map[string]any{"kind": "player_damage_taken", "auras": damageTaken})
	// recklessness.go: its crit is a temporary stat change.
	effects = append(effects, map[string]any{
		"kind": "recklessness", "spell_id": warriorRecklessness.Highest().ID, "aura": "Recklessness",
	})
	// sunder_armor.go: the warrior's own stacks, refused while another aura holds the armor category.
	target := character.Env.Encounter.ActiveTargetUnits[0]
	if sunder := war.SunderArmorAuras.Get(target); sunder != nil {
		effects = append(effects, map[string]any{
			"kind": "sunder_armor", "spell_id": warriorSunderArmor.Highest().ID, "aura": sunder.Label,
			"blocked": blockedForGood(sunder),
		})
	}
	if talents.DeepWounds > 0 { // talents_arms.go registerDeepWounds
		bleed := warriorDeepWoundsTriggered.ByID(412609)
		effects = append(effects, map[string]any{
			"kind": "deep_wounds", "spell_id": bleed.ID, "trigger_aura": "Deep Wounds - Trigger",
			"share":         warriorDeepWounds.FractionAt(talents.DeepWounds),
			"tick_can_crit": bleed.PeriodicCanCrit(), "tick_magic": bleed.DefenseTypeCore() == core.DefenseTypeMagic,
		})
	}
	if talents.UnbridledWrath > 0 { // talents_fury.go registerUnbridledWrath
		effects = append(effects, map[string]any{
			"kind": "unbridled_wrath", "trigger_aura": "Unbridled Wrath", "spell_id": warriorUnbridledWrathTrigger.Highest().ID,
			"proc_chance": warriorUnbridledWrath.FractionAt(talents.UnbridledWrath),
			"rage":        warriorUnbridledWrathTrigger.Highest().EnergizeEffect().Tenths(),
			"two_handed":  war.GetMainHandType() == proto.HandType_HandTypeTwoHand,
			"delay_ns":    nanos(core.SpellBatchWindow),
		})
	}
	if talents.Flurry > 0 { // talents_fury.go registerFlurry
		buff := warriorFlurryTriggered.Highest()
		effects = append(effects, map[string]any{
			"kind": "warrior_flurry", "trigger_aura": "Flurry - Trigger", "aura": "Flurry",
			"melee_speed_multiplier": warriorFlurry.MultiplierAt(talents.Flurry), "charges": int32(buff.ProcCharges),
		})
	}
	if talents.AngerManagement { // talents_arms.go registerAngerManagement
		row := warriorAngerManagement.Highest()
		effects = append(effects, map[string]any{
			"kind": "anger_management", "spell_id": row.ID, "rage": row.Effects[1].BasePoints,
			"period_ns": nanos(time.Duration(row.Effects[2].BasePoints) * time.Second),
		})
	}
	// heroic_strike_cleave.go: each strike's queue spell and aura, the queue delay and the base
	// damage the strike adds to a main hand weapon swing.
	strikes := []map[string]any{}
	for _, entry := range []struct {
		ladder spelldata.Ladder
		cleave bool
	}{{warriorHeroicStrike, false}, {warriorCleave, true}} {
		entry := struct {
			spell  *core.Spell
			ladder spelldata.Ladder
			cleave bool
		}{character.GetSpell(core.ActionID{SpellID: entry.ladder.Highest().ID}), entry.ladder, entry.cleave}
		if entry.spell == nil {
			continue
		}
		strikes = append(strikes, map[string]any{
			"spell_id": entry.spell.ActionID.SpellID, "queue_aura": "HS/Cleave Queue Aura-" + entry.spell.ActionID.String(),
			"base_damage": entry.ladder.Highest().DamageEffect().Average(core.CharacterLevel), "cleave": entry.cleave,
		})
	}
	effects = append(effects, map[string]any{
		"kind": "heroic_strike_queue", "queue_delay_ns": nanos(time.Millisecond * time.Duration(war.QueueDelay)),
		"strikes": strikes,
	})
	// overpower.go: a dodge opens the Overpower window.
	effects = append(effects, map[string]any{"kind": "overpower_window", "trigger_aura": "Overpower - Trigger", "aura": war.OverpowerAura.Label})
	// Listeners that hear only hits the player takes, and nothing attacks the player.
	for _, label := range []string{"Revenge - Trigger", "Enrage - Trigger", "Blood Craze - Damage Taken"} {
		if character.GetAura(label) != nil {
			effects = append(effects, map[string]any{"kind": "inert_listener", "unit": "player", "aura": label, "reason": "acts only on hits the player takes"})
		}
	}
	return effects
}

// Whether an aura can never activate: each of its exclusive categories is a single aura
// category held by another aura that never expires, as the raid's Expose Armor holds the armor
// category against the warrior's own Sunder Armor.
func blockedForGood(aura *core.Aura) bool {
	for _, effect := range aura.ExclusiveEffects {
		active := effect.Category.GetActiveEffect()
		if active == nil || active == effect || active.Aura.Duration != core.NeverExpires || !effect.Category.SingleAura {
			return false
		}
	}
	return len(aura.ExclusiveEffects) != 0
}

// Warrior behavior the exporter cannot describe.
func warriorUnrepresented(agent core.Agent, _ *core.Character) []string {
	war := agent.(warrior.WarriorAgent).GetWarrior()
	unrepresented := []string{}
	if war.StanceSnapshot {
		unrepresented = append(unrepresented, "warrior stance snapshots are unsupported")
	}
	return unrepresented
}

// Warrior auras whose gain and expiry change stats through AddStatsDynamic: recklessness.go's
// crit.
func warriorStatAuras(_ core.Agent, _ *core.Character) []string {
	return []string{"Recklessness"}
}
