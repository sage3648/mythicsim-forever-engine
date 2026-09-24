package core

import (
	"fmt"
	"time"

	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/stats"
)

// Scrolls share StatBuffCategory with the raid buffs granting the same stat, so a
// scroll doesn't stack with e.g. Arcane Brilliance or Divine Spirit; only the
// strongest source of that stat applies.
func registerScrollAura(character *Character, label string, itemID int32, stat stats.Stat, amount float64) *Aura {
	aura := character.GetOrRegisterAura(Aura{
		Label:      label,
		ActionID:   ActionID{ItemID: itemID},
		Duration:   NeverExpires,
		BuildPhase: CharacterBuildPhaseConsumes,
	})
	makeExclusiveFlatStatBuff(aura, stat, amount, StatBuffCategory)
	return MakePermanent(aura)
}

// Registers all consume-related effects to the Agent.
func applyConsumeEffects(agent Agent, partyBuffs *proto.PartyBuffs) {
	character := agent.GetCharacter()
	consumables := character.Consumables
	if consumables == nil {
		return
	}

	if consumables.FlaskId != 0 {
		flask := GetConsumableByID(consumables.FlaskId)
		character.AddStats(flask.Stats)
	}

	if consumables.BattleElixirId != 0 {
		// Elixir of Demonslaying
		if consumables.BattleElixirId == 9224 {
			character.Env.RegisterPostFinalizeEffect(func() {
				for _, at := range character.AttackTables {
					at.MobTypeBonusStats[proto.MobType_MobTypeDemon] = at.MobTypeBonusStats[proto.MobType_MobTypeDemon].Add(stats.Stats{
						stats.AttackPower:       265,
						stats.RangedAttackPower: 265,
					})
				}
			})
		} else {
			elixir := GetConsumableByID(consumables.BattleElixirId)
			character.AddStats(elixir.Stats)
		}
	}

	if consumables.GuardianElixirId != 0 {
		// Gift of Arthas
		if consumables.GuardianElixirId == 9088 {
			character.AddStat(stats.ShadowResistance, 10)
			auras := character.NewEnemyAuraArray(func(target *Unit) *Aura {
				return GiftOfArthasAura(target)
			})
			procSpell := character.RegisterSpell(SpellConfig{
				ActionID:    ActionID{SpellID: 11374},
				SpellSchool: SpellSchoolNature,
				ProcMask:    ProcMaskEmpty,

				FlatThreatBonus: 90,

				ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
					spell.CalcAndDealOutcome(sim, target, spell.OutcomeAlwaysHit)
					auras.Get(target).Activate(sim)
				},
			})

			character.MakeProcTriggerAura(ProcTrigger{
				Name:       "Gift of Arthas - Trigger",
				ICD:        time.Second * 3,
				ProcChance: 0.3,
				Outcome:    OutcomeLanded,
				Callback:   CallbackOnSpellHitTaken,
				Handler: func(sim *Simulation, spell *Spell, _ *SpellResult) {
					procSpell.Cast(sim, spell.Unit)
				},
			})
		} else {
			elixir := GetConsumableByID(consumables.GuardianElixirId)
			character.AddStats(elixir.Stats)
		}
	}
	if consumables.FoodId != 0 {
		food := GetConsumableByID(consumables.FoodId)
		character.AddStats(food.Stats)
	}

	// Classic buffs that stack beside the elixirs: jujus, Blasted Lands/Zanza, alcohol, the
	// school power and armor elixirs. Their stats come from the client like every other consumable.
	for _, id := range []int32{consumables.StrengthBuffId, consumables.AttackPowerBuffId, consumables.ZanzaId,
		consumables.AlcoholId, consumables.SpellPowerElixirId, consumables.SchoolElixirId, consumables.DefenseElixirId} {
		if id != 0 {
			character.AddStats(GetConsumableByID(id).Stats)
		}
	}
	if consumables.DragonbreathChili {
		registerDragonbreathChili(character)
	}

	// Static Imbues
	if consumables.MhImbueId != 0 && partyBuffs.WindfuryTotem == proto.TristateEffect_TristateEffectMissing {
		registerStaticImbue(agent, consumables.MhImbueId)
	}
	if consumables.OhImbueId != 0 {
		registerStaticImbue(agent, consumables.OhImbueId)
	}

	// Scrolls
	if consumables.ScrollAgi {
		registerScrollAura(character, "Scroll of Agility", 27498, stats.Agility, 20)
	}
	if consumables.ScrollStr {
		registerScrollAura(character, "Scroll of Strength", 27503, stats.Strength, 20)
	}
	if consumables.ScrollInt {
		registerScrollAura(character, "Scroll of Intellect", 27499, stats.Intellect, 20)
	}
	if consumables.ScrollSpi {
		registerScrollAura(character, "Scroll of Spirit", 27501, stats.Spirit, 30)
	}
	if consumables.ScrollArm {
		registerScrollAura(character, "Scroll of Protection", 27500, stats.Armor, 300)
	}

	// Bloodthistle (Blood Elf only): +10 spell damage and healing for 10 min.
	if consumables.Bloodthistle && character.Race == proto.Race_RaceBloodElf && !character.RacialsDisabled() {
		character.AddStat(stats.SpellDamage, 10)
		character.AddStat(stats.HealingPower, 10)
	}

	// Bogling Root: +1 physical damage for 10 min (item 5206, spell 5665).
	if consumables.BoglingRoot {
		character.AddStat(stats.PhysicalDamage, 1)
	}

	// Pet Consumes
	for _, pet := range character.Pets {
		if pet.isGuardian {
			continue
		}

		if consumables.PetScrollAgi {
			pet.AddStat(stats.Agility, 20)
		}
		if consumables.PetScrollStr {
			pet.AddStat(stats.Strength, 20)
		}
		if consumables.PetFoodId != 0 {
			petFood := GetConsumableByID(consumables.PetFoodId)
			pet.AddStats(petFood.Stats)
		}
	}

	drumsBombsSharedTimer := character.NewTimer()

	registerPotionCD(agent, consumables)
	registerConjuredCD(agent, consumables)
	registerExplosivesCD(agent, consumables, drumsBombsSharedTimer)
	registerDrumsCD(agent, consumables, drumsBombsSharedTimer)
}

// Dragonbreath Chili (12217): its aura (15852) has a 5% chance, 10 s cooldown, on landed melee
// hits to cast 15851, 65 Fire damage (+-12.3%, SP coefficient 1) on every enemy - all client values.
func registerDragonbreathChili(character *Character) {
	procSpell := character.RegisterSpell(SpellConfig{
		ActionID:         ActionID{SpellID: 15851},
		SpellSchool:      SpellSchoolFire,
		DefenseType:      DefenseTypeMagic,
		ProcMask:         ProcMaskSpellDamageProc,
		Flags:            SpellFlagNoOnCastComplete | SpellFlagPassiveSpell,
		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: 1,
		ApplyEffects: func(sim *Simulation, _ *Unit, spell *Spell) {
			for _, aoeTarget := range sim.Encounter.ActiveTargetUnits {
				spell.CalcAndDealDamage(sim, aoeTarget, sim.Roll(57, 73), spell.OutcomeMagicHitAndCrit)
			}
		},
	})
	character.MakeProcTriggerAura(ProcTrigger{
		Name:       "Dragonbreath Chili",
		ActionID:   ActionID{SpellID: 15852},
		Callback:   CallbackOnSpellHitDealt,
		ProcMask:   ProcMaskMelee,
		Outcome:    OutcomeLanded,
		ProcChance: 0.05,
		ICD:        time.Second * 10,
		Handler: func(sim *Simulation, _ *Spell, result *SpellResult) {
			procSpell.Cast(sim, result.Target)
		},
	})
}

var PotionAuraTag = "Potion"

func registerPotionCD(agent Agent, consumes *proto.ConsumesSpec) {
	character := agent.GetCharacter()
	defaultPotion := consumes.PotId

	for _, potionId := range consumes.Potions {
		potion := GetConsumableByID(potionId)
		if potion.Type == proto.ConsumableType_ConsumableTypePotion {
			potMCD := makePotionActivationSpell(potion.Id, character)
			if defaultPotion == potion.Id {
				potMCD.Spell.Flags |= SpellFlagCombatPotion
				character.AddMajorCooldown(potMCD)
			}
		}
	}
}

// Empty: the eight ids this carried are MoP-era alchemist stones and none of them is in
// this client's item database, so HasAlchStone was always false. The lookup is kept for
// whatever Forever's equivalent turns out to be.
var AlchStoneItemIDs = []int32{}

func (character *Character) HasAlchStone() bool {
	alchStoneEquipped := false
	for _, itemID := range AlchStoneItemIDs {
		alchStoneEquipped = alchStoneEquipped || character.HasTrinketEquipped(itemID)
	}
	return character.HasProfession(proto.Profession_Alchemy) && alchStoneEquipped
}

func makePotionActivationSpell(potionId int32, character *Character) MajorCooldown {
	potion := GetConsumableByID(potionId)
	categoryCooldownDuration := TernaryDuration(potion.CategoryCooldownDuration > 0, potion.CategoryCooldownDuration, time.Minute*2)
	mcd := makePotionActivationSpellInternal(potion, character)

	if mcd.Spell != nil {
		// Mark as 'Encounter Only' so that users are forced to select the generic Potion
		// placeholder action instead of specific potion spells, in APL prepull. This
		// prevents a mismatch between Consumes and Rotation settings.
		mcd.Spell.Flags |= SpellFlagEncounterOnly | SpellFlagPotion | SpellFlagAPL
		oldApplyEffects := mcd.Spell.ApplyEffects
		mcd.Spell.ApplyEffects = func(sim *Simulation, target *Unit, spell *Spell) {
			oldApplyEffects(sim, target, spell)
			if sim.CurrentTime < 0 {
				spell.SharedCD.Set(sim.CurrentTime + categoryCooldownDuration)
				character.UpdateMajorCooldowns()
			}
		}
	}

	return mcd
}

type resourceGainConfig struct {
	resType  proto.ResourceType
	min      float64
	spread   float64
	period   time.Duration // Duration between ticks; 0 for one-shot gains.
	duration time.Duration // Total duration of periodic gains.
}

func makePotionActivationSpellInternal(potion Consumable, character *Character) MajorCooldown {
	stoneMul := TernaryFloat64(character.HasAlchStone(), 1.4, 1.0)
	cooldownDuration := TernaryDuration(potion.CooldownDuration > 0, potion.CooldownDuration, time.Minute*2)

	potionCast := CastConfig{
		CD: Cooldown{
			Timer:    character.NewTimer(),
			Duration: cooldownDuration,
		},
		SharedCD: Cooldown{
			Timer:    character.GetPotionCD(),
			Duration: cooldownDuration,
		},
	}

	actionID := ActionID{ItemID: potion.Id}
	var aura *StatBuffAura
	mcd := MajorCooldown{
		Spell: character.GetOrRegisterSpell(SpellConfig{
			ActionID: actionID,
			Flags:    SpellFlagNoOnCastComplete,
			Cast:     potionCast,
		}),
	}
	if potion.BuffDuration > 0 {
		// Add stat buff aura if applicable
		aura = character.NewTemporaryStatsAura(potion.Name, actionID, potion.Stats, potion.BuffDuration)
		mcd.Spell.RelatedSelfBuff = aura.Aura
		mcd.Type = aura.InferCDType()
		mcd.BuffAura = aura
	}
	var gains []resourceGainConfig
	resourceMetrics := make(map[proto.ResourceType]*ResourceMetrics)

	// Stats applied by triggered auras (e.g. Fel Mana Potion's spell damage reduction).
	// These may be positive or negative.
	var auraStats stats.Stats
	var auraDuration time.Duration
	var auraSpellId int32

	for _, effectID := range potion.EffectIds {
		e := GetSpellEffectByID(effectID)
		resourceType := e.GetResourceType()
		isPeriodic := e.AuraPeriodMs > 0
		if resourceType != 0 && (isPeriodic || e.Type == proto.EffectType_EffectTypeResourceGain) {
			if resourceType == proto.ResourceType_ResourceTypeMana && mcd.Type != CooldownTypeSurvival {
				mcd.Type = CooldownTypeMana
			} else if resourceType == proto.ResourceType_ResourceTypeHealth {
				mcd.Type = CooldownTypeSurvival
			} else {
				mcd.Type = CooldownTypeDPS
			}
			gains = append(gains, resourceGainConfig{
				resType:  resourceType,
				min:      e.MinEffectSize,
				spread:   e.EffectSpread,
				period:   time.Duration(e.AuraPeriodMs) * time.Millisecond,
				duration: time.Duration(e.DurationMs) * time.Millisecond,
			})
			if _, exists := resourceMetrics[resourceType]; !exists {
				resourceMetrics[resourceType] = character.Metrics.NewResourceMetrics(actionID, resourceType)
			}
			// Preload resource types that are found on this item
			if resourceMetrics[resourceType] == nil {
				resourceMetrics[resourceType] = character.Metrics.NewResourceMetrics(actionID, resourceType)
			}
			continue
		}
		if effectStats := stats.FromProtoArray(e.Stats); effectStats != (stats.Stats{}) {
			auraStats = auraStats.Add(effectStats)
			auraDuration = max(auraDuration, time.Duration(e.DurationMs)*time.Millisecond)
			if auraSpellId == 0 {
				auraSpellId = e.SpellId
			}
		}
	}

	var debuffAura *StatBuffAura
	if auraDuration > 0 && auraStats != (stats.Stats{}) {
		debuffAura = character.NewTemporaryStatsAura(fmt.Sprintf("%s Debuff (%d)", potion.Name, auraSpellId), ActionID{SpellID: auraSpellId}, auraStats, auraDuration)
	}

	mcd.Spell.ApplyEffects = func(sim *Simulation, _ *Unit, _ *Spell) {
		if aura != nil {
			aura.Activate(sim)
		}
		if debuffAura != nil {
			debuffAura.Activate(sim)
		}
		for _, config := range gains {
			gain := config.min + sim.RandomFloat(potion.Name)*config.spread
			gain *= stoneMul
			if config.period > 0 {
				// Periodic gains (e.g. Fel Mana Potion) tick over the effect's duration.
				startPeriodicResourceGain(sim, character, config, gain, resourceMetrics[config.resType])
			} else {
				if config.resType == proto.ResourceType_ResourceTypeHealth {
					gain *= character.PseudoStats.HealingTakenMultiplier
				}
				character.ExecuteResourceGain(sim, config.resType, gain, resourceMetrics[config.resType])
			}
		}
	}

	mcd.ShouldActivate = func(sim *Simulation, character *Character) bool {
		shouldActivate := true
		for _, config := range gains {
			switch config.resType {
			case proto.ResourceType_ResourceTypeMana:
				totalRegen := character.ManaRegenPerSecondWhileCasting() * 5
				manaGain := config.min + config.spread
				manaGain *= stoneMul
				if config.period > 0 {
					manaGain *= float64(startPeriodicResourceGainTicks(config))
				}
				shouldActivate = character.MaxMana()-(character.CurrentMana()+totalRegen) >= manaGain
			}
		}
		return shouldActivate
	}

	return mcd

}

// startPeriodicResourceGain ticks a periodic resource gain (e.g. Fel Mana Potion's mana over
// time) across the effect's duration.
func startPeriodicResourceGain(sim *Simulation, character *Character, config resourceGainConfig, gainPerTick float64, metrics *ResourceMetrics) {
	if config.resType != proto.ResourceType_ResourceTypeMana && config.resType != proto.ResourceType_ResourceTypeHealth {
		return
	}
	if config.resType == proto.ResourceType_ResourceTypeHealth {
		gainPerTick *= character.PseudoStats.HealingTakenMultiplier
	}
	StartPeriodicAction(sim, PeriodicActionOptions{
		Period:   config.period,
		NumTicks: int(startPeriodicResourceGainTicks(config)),
		Priority: ActionPriorityDOT,
		OnAction: func(sim *Simulation) {
			character.ExecuteResourceGain(sim, config.resType, gainPerTick, metrics)
		},
	})
}

func startPeriodicResourceGainTicks(config resourceGainConfig) int32 {
	return int32(config.duration / config.period)
}

var ConjuredAuraTag = "Conjured"

func registerConjuredCD(agent Agent, consumes *proto.ConsumesSpec) {
	character := agent.GetCharacter()

	for _, conjuredId := range consumes.ConjuredItems {
		var conjuredMCD MajorCooldown
		switch conjuredId {
		case 22788:
			conjuredMCD = makeConjuredActivationSpell(conjuredId, character)

			flameCapProc := character.RegisterSpell(SpellConfig{
				ActionID:    conjuredMCD.Spell.ActionID,
				SpellSchool: SpellSchoolFire,
				DefenseType: DefenseTypeMagic, // Flamecap Fire (28715)
				ProcMask:    ProcMaskSpellDamage,
				Flags:       SpellFlagProc,

				DamageMultiplier: 1,
				ThreatMultiplier: 1,

				ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
					spell.CalcAndDealDamage(sim, target, 40, spell.OutcomeMagicHitAndCrit)
				},
			})

			procTrigger := character.MakeProcTriggerAura(ProcTrigger{
				Name:       "Flame Cap - Proc",
				ActionID:   conjuredMCD.Spell.ActionID,
				Duration:   time.Minute * 1,
				ProcChance: 0.185,
				ProcMask:   ProcMaskMeleeWhiteHit | ProcMaskRangedAuto,
				Outcome:    OutcomeLanded,
				Callback:   CallbackOnSpellHitDealt,
				Handler: func(sim *Simulation, spell *Spell, result *SpellResult) {
					flameCapProc.Cast(sim, result.Target)
				},
			})

			flameCapAura := character.NewTemporaryStatsAura("Flame Cap", conjuredMCD.Spell.ActionID, stats.Stats{stats.FireDamage: 80}, time.Minute)
			flameCapAura.AttachDependentAura(procTrigger)

			oldApplyEffects := conjuredMCD.Spell.ApplyEffects
			conjuredMCD.Spell.ApplyEffects = func(sim *Simulation, target *Unit, spell *Spell) {
				oldApplyEffects(sim, target, spell)
				flameCapAura.Activate(sim)
			}
			conjuredMCD.Spell.RelatedSelfBuff = flameCapAura.Aura
		default:
			conjuredMCD = makeConjuredActivationSpell(conjuredId, character)
		}

		if conjuredMCD.Spell != nil {
			oldShouldActivate := conjuredMCD.ShouldActivate
			conjuredMCD.ShouldActivate = func(sim *Simulation, character *Character) bool {
				return oldShouldActivate(sim, character) && consumes.ConjuredId == conjuredId
			}
			character.AddMajorCooldown(conjuredMCD)
		}
	}

}

func makeConjuredActivationSpell(conjuredId int32, character *Character) MajorCooldown {
	conjured := GetConsumableByID(conjuredId)
	categoryCooldownDuration := TernaryDuration(conjured.CategoryCooldownDuration > 0, conjured.CategoryCooldownDuration, time.Minute*2)
	mcd := makeConjuredActivationSpellInternal(conjured, character)

	if mcd.Spell != nil {
		mcd.Spell.Flags |= SpellFlagConjured | SpellFlagAPL
		oldApplyEffects := mcd.Spell.ApplyEffects
		mcd.Spell.ApplyEffects = func(sim *Simulation, target *Unit, spell *Spell) {
			oldApplyEffects(sim, target, spell)
			if sim.CurrentTime < 0 {
				spell.SharedCD.Set(sim.CurrentTime + categoryCooldownDuration)
				character.UpdateMajorCooldowns()
			}
		}
	}

	return mcd
}

func makeConjuredActivationSpellInternal(conjured Consumable, character *Character) MajorCooldown {
	cooldownDuration := TernaryDuration(conjured.CooldownDuration > 0, conjured.CooldownDuration, time.Minute*2)

	conjuredCast := CastConfig{
		CD: Cooldown{
			Timer:    character.NewTimer(),
			Duration: cooldownDuration,
		},
		SharedCD: Cooldown{
			Timer:    character.GetOrInitSpellCategoryTimer(conjured.CategoryId),
			Duration: time.Minute * 2,
		},
	}

	actionID := ActionID{ItemID: conjured.Id}
	var aura *StatBuffAura
	mcd := MajorCooldown{
		Spell: character.GetOrRegisterSpell(SpellConfig{
			ActionID: actionID,
			Flags:    SpellFlagNoOnCastComplete,
			Cast:     conjuredCast,
		}),
	}
	if conjured.BuffDuration > 0 {
		// Add stat buff aura if applicable
		aura = character.NewTemporaryStatsAura(conjured.Name, actionID, conjured.Stats, conjured.BuffDuration)
		mcd.Spell.RelatedSelfBuff = aura.Aura
		mcd.Type = aura.InferCDType()
	}
	var gains []resourceGainConfig
	resourceMetrics := make(map[proto.ResourceType]*ResourceMetrics)

	for _, effectID := range conjured.EffectIds {
		e := GetSpellEffectByID(effectID)
		resourceType := e.GetResourceType()
		if e.Type == proto.EffectType_EffectTypeResourceGain && resourceType != 0 {
			if resourceType == proto.ResourceType_ResourceTypeMana && mcd.Type != CooldownTypeSurvival {
				mcd.Type = CooldownTypeMana
			} else if resourceType == proto.ResourceType_ResourceTypeHealth {
				mcd.Type = CooldownTypeSurvival
			} else {
				mcd.Type = CooldownTypeDPS
			}
			gains = append(gains, resourceGainConfig{
				resType: resourceType,
				min:     e.MinEffectSize,
				spread:  e.EffectSpread,
			})

			if _, exists := resourceMetrics[resourceType]; !exists {
				resourceMetrics[resourceType] = character.Metrics.NewResourceMetrics(actionID, resourceType)
			}
			// Preload resource types that are found on this item
			if resourceMetrics[resourceType] == nil {
				resourceMetrics[resourceType] = character.Metrics.NewResourceMetrics(actionID, resourceType)
			}
		}
	}

	mcd.Spell.ApplyEffects = func(sim *Simulation, _ *Unit, _ *Spell) {
		if aura != nil {
			aura.Activate(sim)
		}

		for _, config := range gains {
			gain := config.min + TernaryFloat64(config.spread > 1, sim.RandomFloat(conjured.Name)*config.spread, config.spread)
			switch config.resType {
			case proto.ResourceType_ResourceTypeHealth:
				gain *= character.PseudoStats.HealingTakenMultiplier
			case proto.ResourceType_ResourceTypeEnergy:
				// Thistle Tea 100 - 2 * max(0, CharacterLevel - 40) energy gain
				if conjured.Id == 7676 {
					gain -= 2 * max(0, CharacterLevel-40)
				}
			}
			character.ExecuteResourceGain(sim, config.resType, gain, resourceMetrics[config.resType])
		}
	}

	mcd.ShouldActivate = func(sim *Simulation, character *Character) bool {
		shouldActivate := true
		for _, config := range gains {
			switch config.resType {
			case proto.ResourceType_ResourceTypeMana:
				totalRegen := character.ManaRegenPerSecondWhileCasting() * 5
				manaGain := config.min + config.spread
				shouldActivate = character.MaxMana()-(character.CurrentMana()+totalRegen) >= manaGain
			case proto.ResourceType_ResourceTypeEnergy:
				if conjured.Id == 7676 {
					gain := (config.min + config.spread) - 2*max(0, CharacterLevel-40)
					shouldActivate = character.MaximumEnergy()-(character.CurrentEnergy()) >= gain
				}
			}
		}
		return shouldActivate
	}

	return mcd

}

var SuperSapperActionID = ActionID{ItemID: 23827}
var GoblinSapperActionID = ActionID{ItemID: 10646}
var EzThroDynamiteTwoActionID = ActionID{ItemID: 18588}
var CrystalChargeActionID = ActionID{ItemID: 11566}
var DenseDynamiteActionID = ActionID{ItemID: 18641}
var FelIronBombActionID = ActionID{ItemID: 23736}
var AdamantiteGrenadeActionID = ActionID{ItemID: 23737}
var GnomishFlameTurretActionID = ActionID{ItemID: 23841}

func registerExplosivesCD(agent Agent, consumes *proto.ConsumesSpec, sharedTimer *Timer) {
	character := agent.GetCharacter()
	if !character.HasProfession(proto.Profession_Engineering) {
		return
	}
	if !consumes.GoblinSapper && !consumes.SuperSapper && consumes.ExplosiveId == 0 {
		return
	}

	if consumes.SuperSapper {
		character.AddMajorCooldown(MajorCooldown{
			Spell:    character.newSuperSapperSpell(sharedTimer),
			Type:     CooldownTypeDPS | CooldownTypeExplosive,
			Priority: CooldownPriorityLow + 30,
		})
	}
	if consumes.GoblinSapper {
		character.AddMajorCooldown(MajorCooldown{
			Spell:    character.newGoblinSapperSpell(sharedTimer),
			Type:     CooldownTypeDPS | CooldownTypeExplosive,
			Priority: CooldownPriorityLow + 20,
		})
	}
	if consumes.ExplosiveId > 0 {
		var filler *Spell
		switch consumes.ExplosiveId {
		case 18588:
			filler = character.newEzThroDynamiteTwoSpell(sharedTimer)
		case 15239:
			filler = character.newCrystalChargeSpell(sharedTimer)
		case 18641:
			filler = character.newDenseDynamiteSpell(sharedTimer)
		case 30217:
			filler = character.newAdamantiteGrenadeSpell(sharedTimer)
		case 30216:
			filler = character.newFelIronBombSpell(sharedTimer)
		case 30526:
			// Summon Gnomish Turret? Just treat it like a DoT? TBD
		}

		character.AddMajorCooldown(MajorCooldown{
			Spell:    filler,
			Type:     CooldownTypeDPS | CooldownTypeExplosive,
			Priority: CooldownPriorityLow + 10,
		})
	}
}

// Creates a spell object for the common explosive case.
func (character *Character) newBasicExplosiveSpellConfig(sharedTimer *Timer, actionID ActionID, school SpellSchool, minDamage float64, maxDamage float64, speed float64, castTime time.Duration, cooldown Cooldown) SpellConfig {
	var selfDamage *Spell
	if actionID.SameAction(SuperSapperActionID) || actionID.SameAction(GoblinSapperActionID) {
		selfDamage = character.newSapperSelfDamageSpell(actionID, school)
	}

	return SpellConfig{
		ActionID:     actionID,
		SpellSchool:  school,
		DefenseType:  DefenseTypeMagic, // Every explosive's damage spell is Magic in SpellCategories, so they crit for 150%
		ProcMask:     ProcMaskEmpty,
		Flags:        SpellFlagExplosive,
		MissileSpeed: speed,

		Cast: CastConfig{
			DefaultCast: Cast{
				CastTime: castTime,
			},
			CD: cooldown,
			SharedCD: Cooldown{
				Timer:    sharedTimer,
				Duration: time.Minute,
			},
		},

		// Explosives always have 1% resist chance, so just give them hit cap.
		BonusHitPercent:  100,
		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
			baseDamage := sim.Roll(minDamage, maxDamage) * sim.Encounter.AOECapMultiplier()
			spell.CalcAoeDamage(sim, baseDamage, spell.OutcomeMagicHitAndCrit)
			if speed > 0 {
				spell.WaitTravelTime(sim, func(sim *Simulation) {
					spell.DealBatchedAoeDamage(sim)
				})
			} else {
				spell.DealBatchedAoeDamage(sim)
			}

			if selfDamage != nil {
				baseDamage := sim.Roll(minDamage, maxDamage)
				selfDamage.CalcAndDealDamage(sim, &character.Unit, baseDamage, selfDamage.OutcomeMagicHitAndCrit)
			}
		},
	}
}

// The half of a sapper charge that goes off in the thrower's face. Its own spell so that the hit
// carries the kind it is - a harmful spell landing on the character - which is what a listener on
// spell damage taken hears. The tag keeps it apart from the charge's outgoing damage, whose own
// hits state no kind.
func (character *Character) newSapperSelfDamageSpell(actionID ActionID, school SpellSchool) *Spell {
	return character.GetOrRegisterSpell(SpellConfig{
		ActionID:    actionID.WithTag(1),
		SpellSchool: school,
		DefenseType: DefenseTypeMagic,
		ProcMask:    ProcMaskSpellDamage,
		Flags:       SpellFlagExplosive,

		BonusHitPercent:  100,
		DamageMultiplier: 1,
		ThreatMultiplier: 1,
	})
}
func (character *Character) newSuperSapperSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, SuperSapperActionID, SpellSchoolFire, 900, 1500, 0, 0, Cooldown{Timer: character.NewTimer(), Duration: time.Minute * 5}))
}
func (character *Character) newGoblinSapperSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, GoblinSapperActionID, SpellSchoolFire, 450, 750, 0, 0, Cooldown{Timer: character.NewTimer(), Duration: time.Minute * 5}))
}
func (character *Character) newAdamantiteGrenadeSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, AdamantiteGrenadeActionID, SpellSchoolFire, 450, 750, 14, time.Second, Cooldown{}))
}
func (character *Character) newFelIronBombSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, FelIronBombActionID, SpellSchoolFire, 330, 770, 14, time.Second, Cooldown{}))
}
func (character *Character) newCrystalChargeSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, CrystalChargeActionID, SpellSchoolFire, 383, 517, 0, 0, Cooldown{}))
}

// Dense Dynamite (item 18641, spell 23063): 400 Fire, variance 0.3 (340-460), 1s cast, missile speed 14.
func (character *Character) newDenseDynamiteSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, DenseDynamiteActionID, SpellSchoolFire, 340, 460, 14, time.Second, Cooldown{}))
}
func (character *Character) newEzThroDynamiteTwoSpell(sharedTimer *Timer) *Spell {
	return character.GetOrRegisterSpell(character.newBasicExplosiveSpellConfig(sharedTimer, EzThroDynamiteTwoActionID, SpellSchoolFire, 213, 287, 14, time.Second, Cooldown{}))
}

func registerDrumsCD(agent Agent, consumables *proto.ConsumesSpec, sharedTimer *Timer) {
	if consumables.DrumsId > 0 && int(consumables.DrumsId) < len(proto.Drums_value) {
		character := agent.GetCharacter()
		config := drumsSpellConfig(character, consumables.DrumsId, false)
		config.Cast = CastConfig{
			DefaultCast: Cast{
				CastTime: TernaryDuration(consumables.DrumsId <= proto.Drums_GreaterDrumsOfWar, 0, time.Second),
				GCD:      GCDDefault,
			},
			CD: Cooldown{
				Timer:    character.NewTimer(),
				Duration: time.Minute * 2,
			},
			SharedCD: Cooldown{
				Timer:    sharedTimer,
				Duration: time.Minute * 2,
			},
		}
		spell := character.RegisterSpell(config)

		character.AddMajorCooldown(MajorCooldown{
			Spell:    spell,
			Type:     CooldownTypeDPS,
			Priority: CooldownPriorityDrums,
		})
	}
}

func registerStaticImbue(agent Agent, imbueId int32) {
	character := agent.GetCharacter()
	switch imbueId {
	case 25123: // Mana Oil
		character.AddStat(stats.HealingPower, 25)
		character.AddStat(stats.MP5, 12)
	case 20749, 25122: // Brilliant Wizard Oil (20749 is the Classic item Forever ships)
		character.AddStat(stats.SpellDamage, 36)
		character.AddStat(stats.SpellCritRating, 14)
	case 18262: // Elemental Sharpening Stone: 2% melee crit (master: ranged crit unchanged)
		character.AddStat(stats.PhysicalCritPercent, 2)
		character.AddStat(stats.RangedCritPercent, -2)
	case 28017: // Superior Wizard Oil
		character.AddStat(stats.SpellDamage, 42)
	case 29453, 34340: // Adamantite Sharpening Stone / Adamantite Weightstone
		character.AddStat(stats.MeleeCritRating, 14)
		for _, weapon := range []*Weapon{character.AutoAttacks.MH(), character.AutoAttacks.OH(), character.AutoAttacks.Ranged()} {
			if weapon != nil {
				weapon.BaseDamageMin += 12
				weapon.BaseDamageMax += 12
			}
		}
		// Keep Ranged Crit the same
		character.AddStat(stats.RangedCritPercent, -(14 / PhysicalCritRatingPerCritPercent))
	case 28891: // Consecrated Sharpening Stone
		character.Env.RegisterPostFinalizeEffect(func() {
			for _, at := range character.AttackTables {
				at.MobTypeBonusStats[proto.MobType_MobTypeUndead] = at.MobTypeBonusStats[proto.MobType_MobTypeUndead].Add(stats.Stats{
					stats.AttackPower:       100,
					stats.RangedAttackPower: 100,
				})
			}
		})
	}
}
