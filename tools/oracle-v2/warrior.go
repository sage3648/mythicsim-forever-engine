// Warrior export: class spell names, client damage rows and the effects Go keeps in
// closures. Each formula mirrors the cited Go file at the pinned revision.
package main

import (
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
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

// talents_arms.go registerSweepingStrikes reads the highest rank.
var warriorSweepingStrikes = spelldata.Ranked(12292)

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
	warriorAngerManagement            = spelldata.Ranked(12296)
	warriorBerserkerRage              = spelldata.Ranked(18499)
	warriorBloodrage                  = spelldata.Ranked(2687)
	warriorBloodrageTriggered         = spelldata.Ranked(29131)
	warriorBloodthirst                = spelldata.Ranked(23881, 23892, 23893, 23894)
	warriorCleave                     = spelldata.Ranked(845, 7369, 11608, 11609, 20569)
	warriorHeroicStrike               = spelldata.Ranked(78, 284, 285, 1608, 11564, 11565, 11566, 11567, 25286)
	warriorDeathWish                  = spelldata.Ranked(12328)
	warriorDeepWounds                 = spelldata.Talent(12834, 3)
	warriorDeepWoundsTriggered        = spelldata.Ranked(12162, 412609)
	warriorExecute                    = spelldata.Ranked(5308, 20658, 20660, 20661, 20662)
	warriorFlurry                     = spelldata.Talent(12319, 5)
	warriorFlurryTriggered            = spelldata.Ranked(12966)
	warriorHamstring                  = spelldata.Ranked(1715, 7372, 7373)
	warriorImprovedHamstring          = spelldata.Talent(12289, 3)
	warriorImprovedHamstringTriggered = spelldata.Ranked(23694)
	warriorImprovedBerserkerRage      = spelldata.Talent(20500, 2)
	warriorImprovedBloodrage          = spelldata.Talent(12301, 2)
	warriorImprovedTactical           = spelldata.Talent(12295, 5)
	warriorRecklessness               = spelldata.Ranked(1719)
	warriorSunderArmor                = spelldata.Ranked(7386, 7405, 8380, 11596, 11597)
	warriorTacticalMastery            = spelldata.Ranked(1310185)
	warriorUnbridledWrath             = spelldata.Talent(12322, 5)
	warriorUnbridledWrathTrigger      = spelldata.Ranked(12964)
	warriorWhirlwind                  = spelldata.Ranked(1680)
	warriorBattleShout                = spelldata.Ranked(6673, 5242, 6192, 11549, 11550, 11551, 25289)
	warriorBattleStancePassive        = spelldata.Ranked(21156)
	warriorBerserkerStancePass        = spelldata.Ranked(7381)
	warriorDefensiveStancePass        = spelldata.Ranked(7376)
	warriorBloodthrill                = spelldata.Talent(1289682, 5)
	warriorBloodthrillTriggered       = spelldata.Ranked(1282733, 1289681)
	warriorMortalStrike               = spelldata.Ranked(12294, 21551, 21552, 21553)
	warriorOverpower                  = spelldata.Ranked(7384, 7887, 11584, 11585)
	warriorRend                       = spelldata.Ranked(772, 6546, 6547, 6548, 11572, 11573, 11574)
	warriorSlam                       = spelldata.Ranked(1240193, 1464, 8820, 11604, 11605)
	warriorShieldWall                 = spelldata.Ranked(871)
	warriorLastStand                  = spelldata.Ranked(12975)
	warriorLastStandTriggered         = spelldata.Ranked(12976)
	warriorSpearingStrike             = spelldata.Ranked(1310222)
	warriorWeaponmaster               = spelldata.Talent(1290261, 5)
	warriorRevenge                    = spelldata.Ranked(6572, 6574, 7379, 11600, 11601, 25288)
	warriorShieldSlam                 = spelldata.Ranked(23922, 23923, 23924, 23925)
	warriorThunderClap                = spelldata.Ranked(6343, 8198, 8204, 8205, 11580, 11581)
	warriorRetaliation                = spelldata.Ranked(20230)
	warriorRetaliationTriggered       = spelldata.Ranked(20240)
	warriorShieldSpecialization       = spelldata.Talent(12298, 5)
	warriorShieldSpecTriggered        = spelldata.Ranked(1310318)
	warriorMasterOfDefense            = spelldata.Talent(1310316, 2)
	warriorMasterOfDefenseTrig        = spelldata.Ranked(23602)
	warriorEnrage                     = spelldata.Talent(12317, 5)
	warriorBloodCraze                 = spelldata.Talent(16487, 3)
	warriorDefiance                   = spelldata.Talent(12792, 3)
)

// A client damage row's roll: spelldata Effect.Roll draws between these bounds unless the
// row has no variance.
func damageRoll(row *spelldata.Spell) map[string]any {
	effect := row.DamageEffect()
	average := effect.Average(core.CharacterLevel)
	return map[string]any{"average": average, "min": average * (1 - effect.Variance/2),
		"max": average * (1 + effect.Variance/2), "rolls": effect.Variance != 0, "variance": effect.Variance}
}

// exclusive_effect.go: a single aura category of the unit, with each member's aura, bid and
// spell in registration order. Nil when the category does not exist.
func exclusiveCategoryEffect(unit *core.Unit, side string, name string) map[string]any {
	categories := privateField(unit.ExclusiveEffectManager, "categories")
	for i := 0; i < categories.Len(); i++ {
		category := (*core.ExclusiveCategory)(categories.Index(i).UnsafePointer())
		if category.Name != name {
			continue
		}
		if !category.SingleAura {
			if classNotes != nil {
				*classNotes = append(*classNotes, "exclusive category "+name+" holds several auras")
			}
			return nil
		}
		effects := privateField(category, "effects")
		members := []map[string]any{}
		for j := 0; j < effects.Len(); j++ {
			effect := (*core.ExclusiveEffect)(effects.Index(j).UnsafePointer())
			members = append(members, map[string]any{"aura": effect.Aura.Label, "priority": effect.Priority,
				"spell_id": effect.Aura.ActionID.SpellID})
		}
		return map[string]any{"kind": "exclusive_category", "unit": side, "category": name, "members": members}
	}
	return nil
}

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
	// whirlwind.go: a warrior with an off hand weapon strikes with it too (hotfix 112347).
	effects = append(effects, map[string]any{
		"kind": "whirlwind", "spell_id": warriorWhirlwind.Highest().ID, "off_hand": war.HasOHWeapon(),
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
	// items.go Battlegear of Might 5 piece: landed hits taken that dealt damage, and periodic
	// damage taken, roll 20% under the trigger's name; a batch window later, 1 rage (29478). Go
	// literals.
	if aura := character.GetAura("Battlegear of Might 5P"); aura != nil {
		effects = append(effects, map[string]any{"kind": "battlegear_of_might_rage", "trigger_aura": aura.Label,
			"rng_label": "Battlegear of Might - 5PC", "proc_chance": 0.2, "rage": 1.0,
			"metrics_action_id": actionID(core.ActionID{SpellID: 29478})})
	}
	// talents_arms.go registerSweepingStrikes: in Battle Stance, an aura with the row's charges.
	// Its handler acts only with a second target, so with the one target in scope it never
	// copies a hit or spends a charge.
	if aura := character.GetAura("Sweeping Strikes"); aura != nil {
		row := warriorSweepingStrikes.Highest()
		effects = append(effects, map[string]any{"kind": "sweeping_strikes", "spell_id": int32(12723),
			"aura": aura.Label, "charges": int32(row.ProcCharges)})
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
		blocked := blockedForGood(sunder)
		effects = append(effects, map[string]any{
			"kind": "sunder_armor", "spell_id": warriorSunderArmor.Highest().ID, "aura": sunder.Label,
			"blocked": blocked,
		})
		// The armor category the stacks bid in, with the target's armor at each stack count,
		// read from separate reset simulations, as the raid's ramp reads its own.
		if !blocked && exportRequest != nil && len(sunder.ExclusiveEffects) == 1 {
			name := sunder.ExclusiveEffects[0].Category.Name
			if category := exclusiveCategoryEffect(target, "target", name); category != nil {
				armor := []float64{}
				for stacks := int32(0); stacks <= sunder.MaxStacks; stacks++ {
					armor = append(armor, targetArmorWithStacks(exportRequest, sunder.Label, stacks))
				}
				for _, member := range category["members"].([]map[string]any) {
					label := member["aura"].(string)
					if aura := target.GetAura(label); aura != nil && aura.MaxStacks > 0 {
						member["per_stack"] = stackBid(exportRequest, label, name)
					}
				}
				category["armor_by_stacks"] = armor
				effects = append(effects, category)
			}
		}
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
	// stances.go and battle_shout.go: the stances and the shouts are single aura categories.
	for _, name := range []string{"Stance", buffs.BattleShoutCategory} {
		if effect := exclusiveCategoryEffect(&character.Unit, "player", name); effect != nil {
			effects = append(effects, effect)
		}
	}
	// stances.go: each stance's passive multiplies threat, and Defensive and Berserker Stance
	// damage taken, Defensive damage dealt too, from client data. Defiance multiplies threat
	// in Defensive Stance with a shield, after the passive, and nothing swaps the shield.
	defiance := []map[string]any{}
	// shield_wall.go: its aura multiplies damage taken while up.
	if character.GetAura("Shield Wall") != nil {
		defiance = append(defiance, map[string]any{"aura": "Shield Wall", "stat": "damage_taken",
			"multiplier": 1 + warriorShieldWall.Highest().Effect(dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN, 127).Percent()})
	}
	if talents.Defiance > 0 && war.PseudoStats.CanBlock {
		defiance = append(defiance, map[string]any{"aura": "Defensive Stance", "stat": "threat",
			"multiplier": warriorDefiance.Effect(dbcenums.A_MOD_THREAT, 127).MultiplierAt(talents.Defiance)})
	}
	effects = append(effects, map[string]any{"kind": "pseudo_stat_auras", "auras": append([]map[string]any{
		{"aura": "Battle Stance", "stat": "threat",
			"multiplier": warriorBattleStancePassive.Effect(dbcenums.A_MOD_THREAT, 127).MultiplierAt(1)},
		{"aura": "Defensive Stance", "stat": "threat",
			"multiplier": warriorDefensiveStancePass.Effect(dbcenums.A_MOD_THREAT, 127).MultiplierAt(1)},
		{"aura": "Defensive Stance", "stat": "damage_taken",
			"multiplier": warriorDefensiveStancePass.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN, 127).MultiplierAt(1)},
		{"aura": "Defensive Stance", "stat": "damage_dealt",
			"multiplier": warriorDefensiveStancePass.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_DONE, 127).MultiplierAt(1)},
	}, append(defiance,
		map[string]any{"aura": "Berserker Stance", "stat": "threat",
			"multiplier": warriorBerserkerStancePass.Effect(dbcenums.A_MOD_THREAT, 127).MultiplierAt(1)},
		map[string]any{"aura": "Berserker Stance", "stat": "damage_taken",
			"multiplier": warriorBerserkerStancePass.Effect(dbcenums.A_MOD_DAMAGE_PERCENT_TAKEN, 127).MultiplierAt(1)},
	)...)})
	// battle_shout.go: the warrior's own shout, which outbids the party's at an equal value.
	if own := war.BattleShout; own != nil {
		value := buffs.BattleShoutValue(0)
		if war.UseBattleShout && war.HasBsT2 {
			value += buffs.BattleShoutT2Bonus
		}
		for _, aura := range character.GetAurasWithTag(buffs.BattleShoutCategory) {
			if aura.ActionID.Tag == 0 {
				effects = append(effects, map[string]any{"kind": "battle_shout", "spell_id": own.ActionID.SpellID,
					"aura": aura.Label, "value": value, "refresh_threshold_ns": nanos(warrior.ShoutExpirationThreshold)})
			}
		}
	}
	// rend.go: the client tick base and a share of attack power a tick, a Go literal.
	rendRow := warriorRend.Highest()
	effects = append(effects, map[string]any{
		"kind": "rend", "spell_id": rendRow.ID, "tick_base": rendRow.PeriodicEffect().Average(core.CharacterLevel),
		"attack_power_per_tick": 0.02, "tick_can_crit": rendRow.PeriodicCanCrit(),
		"tick_magic": rendRow.DefenseTypeCore() == core.DefenseTypeMagic,
	})
	// overpower.go: the client base on normalized main hand damage.
	effects = append(effects, map[string]any{
		"kind": "overpower", "spell_id": warriorOverpower.ByID(11585).ID,
		"base_damage": warriorOverpower.ByID(11585).DamageEffect().Average(core.CharacterLevel),
	})
	if talents.MortalStrike { // talents_arms.go registerMortalStrike
		row := warriorMortalStrike.Highest()
		effects = append(effects, map[string]any{
			"kind": "mortal_strike", "spell_id": row.ID, "base_damage": row.DamageEffect().Average(core.CharacterLevel),
		})
	}
	if talents.SpearingStrike { // talents_arms.go registerSpearingStrike
		row := warriorSpearingStrike.Highest()
		mobMultiplier := 1.0
		if target.MobType == proto.MobType_MobTypeGiant || target.MobType == proto.MobType_MobTypeDragonkin {
			mobMultiplier = 1 + row.Effects[2].BasePoints
		}
		effects = append(effects, map[string]any{
			"kind": "spearing_strike", "spell_id": row.ID, "weapon_share": row.Effects[1].Percent(),
			"mob_multiplier": mobMultiplier,
		})
	}
	// slam.go: the client base on main hand weapon damage. Without Improved Slam its cast stops
	// the swings, which Rust lacks.
	slamRow := warriorSlam.Highest()
	effects = append(effects, map[string]any{
		"kind": "slam", "spell_id": slamRow.ID, "base_damage": slamRow.DamageEffect().Average(core.CharacterLevel),
		"stops_swings": talents.ImprovedSlam == 0,
	})
	if talents.Bloodthrill > 0 { // talents_arms.go registerBloodthrill
		effects = append(effects, map[string]any{
			"kind": "bloodthrill", "trigger_aura": "Bloodthrill - Trigger",
			"proc_chance": warriorBloodthrill.FractionAt(talents.Bloodthrill),
			"window_ns":   nanos(warriorBloodthrillTriggered.ByID(1289681).Duration()),
			"delay_ns":    nanos(core.SpellBatchWindow),
		})
	}
	if talents.Weaponmaster > 0 && character.GetAura("Weaponmaster (Sword)") != nil { // talents_arms.go
		hands := []string{}
		swords := war.GetProcMaskForTypes(proto.WeaponType_WeaponTypeSword)
		if swords.Matches(core.ProcMaskMeleeMH) {
			hands = append(hands, "main")
		}
		if swords.Matches(core.ProcMaskMeleeOH) {
			hands = append(hands, "off")
		}
		effects = append(effects, map[string]any{
			"kind": "weaponmaster_sword", "trigger_aura": "Weaponmaster (Sword)",
			"proc_chance":      warriorWeaponmaster.EffectAt(3).FractionAt(talents.Weaponmaster),
			"extra_attack_tag": int32(1290261), "sword_hands": hands,
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
	effects = append(effects, warriorTankEffects(war, character, target)...)
	return effects
}

// The effects that act on hits the warrior takes and the tank's own spells. A listener of hits
// taken acts once anything hits the player, the target's swings or the Goblin Sapper Charge's
// self hit; Revenge's needs a blocked, dodged or parried attack, which only the swings give.
func warriorTankEffects(war *warrior.Warrior, character *core.Character, target *core.Unit) []map[string]any {
	talents := war.Talents
	tanking := target.CurrentTarget == &character.Unit
	takesDamage := playerTakesDamage(character, target)
	effects := []map[string]any{}
	inert := func(label string) {
		if character.GetAura(label) != nil {
			effects = append(effects, map[string]any{"kind": "inert_listener", "unit": "player", "aura": label, "reason": "acts only on hits the player takes"})
		}
	}
	// revenge.go: a block, dodge or parry opens Revenge at once; the strike rolls the client
	// row plus a quarter of attack power, a Go literal.
	if !tanking {
		inert("Revenge - Trigger")
	} else if character.GetAura("Revenge - Trigger") != nil {
		row := warriorRevenge.Highest()
		effects = append(effects, map[string]any{"kind": "revenge", "spell_id": row.ID, "trigger_aura": "Revenge - Trigger",
			"aura": "Revenge", "damage": damageRoll(row), "attack_power_share": 0.25})
	}
	// talents_protection.go registerShieldSlam: the client roll plus the block value.
	if talents.ShieldSlam {
		row := warriorShieldSlam.Highest()
		effects = append(effects, map[string]any{"kind": "shield_slam", "spell_id": row.ID, "damage": damageRoll(row),
			"can_block": war.PseudoStats.CanBlock})
	}
	// thunder_clap.go: the row's average plus a share of attack power, a Go literal; a landed
	// clap slows the target by its bid in the attack speed category, which holds only the clap.
	clapRow := warriorThunderClap.Highest()
	if clap := character.GetSpell(core.ActionID{SpellID: clapRow.ID}); clap != nil {
		slow := clapRow.Effects[1].BaseValue()
		bonus := privateField(war, "thunderClapEffectBonus").Float()
		var aura *core.Aura
		for _, array := range clap.RelatedAuraArrays {
			aura = array.Get(target)
		}
		members := 0
		if aura != nil && len(aura.ExclusiveEffects) == 1 {
			members = privateField(aura.ExclusiveEffects[0].Category, "effects").Len()
		}
		if aura != nil && members == 1 {
			effects = append(effects, map[string]any{"kind": "thunder_clap", "spell_id": clapRow.ID,
				"base_damage": clapRow.DamageEffect().Average(core.CharacterLevel), "attack_power_share": 0.0255,
				"max_targets": int32(clapRow.MaxTargets), "aura": aura.Label,
				"bid": 1 - 1/core.SlowedTimeMultiplier(slow*(1+bonus))})
		} else if classNotes != nil {
			*classNotes = append(*classNotes, "Thunder Clap's slow shares its category")
		}
	}
	// talents_protection.go registerLastStand: a survival cooldown whose aura raises maximum
	// health by its share, gaining that much health, and takes it back on expiry, leaving at
	// least 1.
	if aura := character.GetAura("Last Stand"); aura != nil {
		buff := warriorLastStandTriggered.Highest()
		effects = append(effects, map[string]any{"kind": "last_stand", "spell_id": warriorLastStand.Highest().ID,
			"aura": aura.Label, "health_share": buff.Effect(dbcenums.A_MOD_MAX_HEALTH, 0).Percent(),
			"metrics_action_id": actionID(aura.ActionID)})
	}
	// shield_wall.go: a survival cooldown a tank autocasts below 40% health, a Go literal, in
	// Defensive Stance with a shield.
	if aura := character.GetAura("Shield Wall"); aura != nil {
		effects = append(effects, map[string]any{"kind": "shield_wall", "spell_id": warriorShieldWall.Highest().ID,
			"aura": aura.Label, "autocast": war.Spec != proto.Spec_SpecDpsWarrior, "health_percent": 0.4,
			"can_block": war.PseudoStats.CanBlock})
	}
	// retaliation.go: charges that strike back at each landed melee hit taken that dealt damage.
	if aura := character.GetAura("Retaliation"); aura != nil {
		row, hit := warriorRetaliation.Highest(), warriorRetaliationTriggered.Highest()
		effects = append(effects, map[string]any{"kind": "retaliation", "spell_id": row.ID, "aura": aura.Label,
			"hit_spell_id": hit.ID, "charges": int32(row.ProcCharges),
			"hit_base_damage": hit.DamageEffect().Average(core.CharacterLevel)})
	}
	// talents_protection.go registerRageOnAvoid: rage at a chance on a block, or on a dodge or
	// parry while the warrior can block.
	avoid := []map[string]any{}
	if talents.ShieldSpecialization > 0 {
		energize := warriorShieldSpecTriggered.Highest()
		avoid = append(avoid, map[string]any{"aura": "Shield Specialization", "spell_id": energize.ID,
			"rage": energize.EnergizeEffect().Tenths(), "outcomes": []string{"block"}, "needs_block": false,
			"chance": warriorShieldSpecialization.EffectAt(2).FractionAt(talents.ShieldSpecialization)})
	}
	if talents.MasterOfDefense > 0 {
		energize := warriorMasterOfDefenseTrig.Highest()
		avoid = append(avoid, map[string]any{"aura": "Master of Defense", "spell_id": energize.ID,
			"rage": energize.EnergizeEffect().Tenths(), "outcomes": []string{"dodge", "parry"},
			"needs_block": true, "chance": warriorMasterOfDefense.FractionAt(talents.MasterOfDefense)})
	}
	if len(avoid) > 0 {
		effects = append(effects, map[string]any{"kind": "rage_on_avoid", "can_block": war.PseudoStats.CanBlock, "triggers": avoid})
	}
	// talents_fury.go registerEnrage: a landed hit taken that dealt damage enrages at a chance, a
	// spell batch window later; the aura raises physical damage done in its own category.
	if talents.Enrage > 0 && war.EnrageAura != nil {
		if takesDamage {
			effects = append(effects, map[string]any{"kind": "warrior_enrage", "trigger_aura": "Enrage - Trigger",
				"aura": war.EnrageAura.Label, "proc_chance": float64(warriorEnrage.Rank(talents.Enrage).ProcChance) / 100,
				"physical_damage_done": warriorEnrage.FractionAt(talents.Enrage)})
			if effect := exclusiveCategoryEffect(&character.Unit, "player", "Enrage"); effect != nil {
				effects = append(effects, effect)
			}
		} else {
			inert("Enrage - Trigger")
		}
	}
	// talents_fury.go registerBloodCraze: a heal of maximum health over the hot's ticks, after a
	// crit or a large hit taken, or a landed Bloodthirst; both triggers wait a spell batch window.
	// The Bloodthirst trigger acts whether or not anything hits the player.
	if talents.BloodCraze > 0 {
		if hot := character.GetSpell(core.ActionID{SpellID: 16488}); hot != nil {
			effects = append(effects, map[string]any{"kind": "blood_craze", "spell_id": hot.ActionID.SpellID,
				"heal":              healModifiers(character, &character.Unit),
				"damage_taken_aura": "Blood Craze - Damage Taken", "bloodthirst_aura": "Blood Craze - Bloodthirst",
				"health_fraction": warriorBloodCraze.EffectAt(1).FractionAt(talents.BloodCraze),
				"hit_threshold":   warriorBloodCraze.EffectAt(2).FractionAt(talents.BloodCraze)})
		}
	}
	// talents_protection.go registerImprovedShieldBash: landed Shield Bash hits silence the
	// target, which nothing reads; Rust has no Shield Bash, so the gate rejects a rotation that
	// reaches it and the trigger hears nothing.
	if aura := character.GetAura("Improved Shield Bash"); aura != nil {
		effects = append(effects, map[string]any{"kind": "inert_listener", "unit": "player", "aura": aura.Label,
			"reason": "acts only on Shield Bash hits, and the gate rejects a rotation that reaches Shield Bash"})
	}
	// talents_arms.go registerImprovedHamstring: a landed Hamstring roots the target at a
	// chance, a spell batch window later; the root has no effect in scope.
	if aura := character.GetAura("Improved Hamstring - Trigger"); aura != nil {
		row := warriorHamstring.Highest()
		var root *core.Aura
		if hamstring := character.GetSpell(core.ActionID{SpellID: row.ID}); hamstring != nil {
			for _, other := range target.GetAuras() {
				if other.ActionID.SpellID == warriorImprovedHamstringTriggered.Highest().ID {
					root = other
				}
			}
		}
		if root != nil {
			effects = append(effects, map[string]any{"kind": "improved_hamstring", "trigger_aura": aura.Label,
				"aura": root.Label, "proc_chance": warriorImprovedHamstring.FractionAt(talents.ImprovedHamstring),
				"delay_ns": nanos(core.SpellBatchWindow)})
		} else if classNotes != nil {
			*classNotes = append(*classNotes, "Improved Hamstring has no root aura")
		}
	}
	return effects
}

// What one stack of a target aura bids in its exclusive category: spelldata's stacking bid,
// read from a separate reset simulation. Zero when the aura cannot activate alone.
func stackBid(request *proto.RaidSimRequest, label string, category string) float64 {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	aura := simulation.Encounter.ActiveTargetUnits[0].GetAura(label)
	aura.Activate(simulation)
	if !aura.IsActive() {
		return 0
	}
	aura.SetStacks(simulation, 1)
	for _, effect := range aura.ExclusiveEffects {
		if effect.Category.Name == category {
			return effect.Priority
		}
	}
	return 0
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
	// talents_protection.go Last Stand: the maximum health it adds, a share of the maximum
	// health in each combination when it activates.
	return []string{"Recklessness", "Berserker Stance", "Battle Shout (Player)", "Last Stand"}
}

// spell_result.go calcHealingInternal's multipliers for a heal of the character on a unit, at
// reset: the caster's healing dealt and periodic healing dealt, the unit's healing taken and
// the attack table's healing dealt, and the healing power its debug line shows.
func healModifiers(character *core.Character, target *core.Unit) map[string]any {
	pseudo := &character.PseudoStats
	if classNotes != nil && (len(target.DynamicHealingTakenModifiers) != 0) {
		*classNotes = append(*classNotes, "dynamic healing taken modifiers are unsupported")
	}
	return map[string]any{"healing_dealt_multiplier": pseudo.HealingDealtMultiplier,
		"periodic_healing_dealt_multiplier": pseudo.PeriodicHealingDealtMultiplier,
		"healing_taken_multiplier":          target.PseudoStats.HealingTakenMultiplier,
		"table_healing_dealt_multiplier":    character.AttackTables[target.UnitIndex].HealingDealtMultiplier,
		"healing_power":                     character.GetStat(stats.HealingPower) + target.PseudoStats.BonusHealingTaken}
}
