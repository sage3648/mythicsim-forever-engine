package warlock

import (
	"slices"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
)

func (warlock *Warlock) ApplyTalents() {
	warlock.applyWeaponImbue()

	// Affliction
	warlock.applySuppression()
	warlock.applyMalediction()
	warlock.applyImprovedBaneOfAgony()
	warlock.applyPandemic()
	warlock.applyMalevolence()
	warlock.applyNightfall()
	warlock.applyShadowMastery()

	// Demonology
	warlock.applyDemonicEmbrace()
	warlock.applyUnholyPower()
	warlock.applyFelVitality()
	warlock.registerFelDominationCD()
	warlock.applyMasterSummoner()
	warlock.applyDecimation()
	warlock.applyDemonicBrand()
	warlock.applyDemonicKnowledge()
	warlock.applyMasterDemonologist()
	warlock.applyDemonicSacrifice()
	warlock.applySoulLink()

	// Destruction
	warlock.applyImprovedShadowBolt()
	warlock.applyMoltenSkin()
	warlock.applyCataclysm()
	warlock.applyBane()
	warlock.applyRuin()
	warlock.applyAgonizingFlames()
	warlock.applyFireAndBrimstone()
	warlock.applyShadowAndFlame()
}

func (warlock *Warlock) applyWeaponImbue() {
	if warlock.GetCharacter().Equipment.OffHand().Type != proto.ItemType_ItemTypeUnknown {
		return
	}

	level := warlock.Level
	if warlock.Options.WeaponImbue == proto.WarlockOptions_Firestone {
		warlock.applyFirestone()
	}
	if warlock.Options.WeaponImbue == proto.WarlockOptions_Spellstone {
		if level >= 55 {
			warlock.AddStat(stats.SpellCrit, 1*core.SpellCritRatingPerCritChance)
		}
	}
}

// Forever turned the Firestone from a weapon proc into a passive. Era's tooltip reads
// "Enchants the main hand weapon with fire, granting each attack a chance to deal additional
// fire damage"; the beta client's reads "Imbues your weapon with Fire, increasing your spell
// critical strike chance by X% and the damage done by your Fire spells by up to Y", with no
// proc in it at all.
//
// The numbers come from the spell each rank's tooltip points at - 758 -> 23480, 17945 ->
// 23481, 17947 -> 23482, 17949 -> 23483 - which carry the fire damage on aura 13 and the
// crit on aura 57:
//
//	rank 1, level 28:  10 fire damage,  1% spell crit
//	rank 2, level 36:  14 fire damage,  1% spell crit
//	rank 3, level 46:  17 fire damage,  2% spell crit
//	rank 4, level 56:  21 fire damage,  2% spell crit
//
// The fire damage was already right. The crit was missing and the proc should not be there.
func (warlock *Warlock) applyFirestone() {
	if warlock.Consumes.MainHandImbue != proto.WeaponImbue_WeaponImbueUnknown {
		return
	}

	level := warlock.Level
	firePower := 0.0
	spellCrit := 0.0

	switch {
	case level >= 56:
		firePower, spellCrit = 21, 2
	case level >= 46:
		firePower, spellCrit = 17, 2
	case level >= 36:
		firePower, spellCrit = 14, 1
	case level >= 28:
		firePower, spellCrit = 10, 1
	default:
		return
	}

	warlock.AddStat(stats.FirePower, firePower)
	warlock.AddStat(stats.SpellCrit, spellCrit*core.SpellCritRatingPerCritChance)
}

///////////////////////////////////////////////////////////////////////////
//                            Affliction
///////////////////////////////////////////////////////////////////////////

func (warlock *Warlock) applySuppression() {
	if warlock.Talents.Suppression == 0 {
		return
	}

	points := float64(warlock.Talents.Suppression)
	warlock.AddStat(stats.SpellHit, points*core.SpellHitRatingPerHitChance)
	warlock.AddStat(stats.MeleeHit, points*core.MeleeHitRatingPerHitChance)
	warlock.PseudoStats.ThreatMultiplier *= 1 - 0.04*points
}

// Improved Bane of Agony: the beta client's 18827 is an op 22 (periodic damage) % modifier, so it
// scales the whole tick, spell power included, not only the base damage.
func (warlock *Warlock) applyImprovedBaneOfAgony() {
	if warlock.Talents.ImprovedBaneOfAgony == 0 {
		return
	}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_PeriodicDamageDone_Flat,
		ClassMask:  SpellMaskBaneOfAgony,
		FloatValue: spellData.ImprovedBaneOfAgony.FractionAt(warlock.Talents.ImprovedBaneOfAgony),
	})
}

func (warlock *Warlock) applyMalediction() {
	if warlock.Talents.Malediction == 0 {
		return
	}

	// Every warlock spell with a dot or an AoE dot.
	warlock.AddStaticMod(core.SpellModConfig{
		Kind: core.SpellMod_PeriodicDamageDone_Flat,
		ClassMask: SpellMaskCorruption | SpellMaskBaneOfAgony | SpellMaskBaneOfDoom | SpellMaskDrainLife | SpellMaskDrainSoul |
			SpellMaskImmolate | SpellMaskSiphonLife | SpellMaskWrack | SpellMaskRainOfFire,
		SpellFlag:  SpellFlagWarlock,
		FloatValue: 0.01 * float64(warlock.Talents.Malediction),
	})
}

// Drain Life, Drain Soul and Wrack damage as the beta client's talents pay it. Improved Drains is a flat
// 7/13/20%. Soul Siphon adds 4% per point for each of the warlock's other Affliction effects on the
// target, counting up to three. The per-effect bonus and its cap used to be read into Improved Drains,
// with a tripling below 20% health that neither talent has, and Soul Siphon was a faster drain with a
// healing penalty; the client carries none of that.
func (warlock *Warlock) improvedDrainsMultiplier(sim *core.Simulation, target *core.Unit) float64 {
	multiplier := 1 + []float64{0, 0.07, 0.13, 0.20}[warlock.Talents.ImprovedDrains]
	if warlock.Talents.SoulSiphon == 0 {
		return multiplier
	}

	effects := 0
	for _, spell := range warlock.DoTSpells {
		if spell.Flags.Matches(WarlockFlagAffliction) && spell.Dot(target).IsActive() {
			effects++
		}
	}
	for _, spell := range warlock.DebuffSpells {
		if spell.Flags.Matches(WarlockFlagAffliction) && spell.RelatedAuras[0].Get(target).IsActive() {
			effects++
		}
	}

	return multiplier * (1 + 0.04*float64(warlock.Talents.SoulSiphon)*float64(min(effects, 3)))
}

func (warlock *Warlock) applyPandemic() {
	if warlock.Talents.Pandemic == 0 {
		return
	}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind: core.SpellMod_CritMultiplier_Flat,
		ClassMask: SpellMaskCorruption | SpellMaskBaneOfAgony | SpellMaskBaneOfDoom | SpellMaskDrainSoul | SpellMaskDrainLife |
			SpellMaskSiphonLife | SpellMaskWrack,
		FloatValue: []float64{0, 0.33, 0.67, 1.00}[warlock.Talents.Pandemic],
	})
}

func (warlock *Warlock) applyMalevolence() {
	if warlock.Talents.Malevolence == 0 {
		return
	}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BonusCrit_Percent,
		School:     core.SpellSchoolShadow,
		SpellFlag:  SpellFlagWarlock,
		FloatValue: float64(warlock.Talents.Malevolence),
	})
}

func (warlock *Warlock) applyNightfall() {
	if warlock.Talents.Nightfall <= 0 {
		return
	}

	instantShadowBolt := warlock.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_CastTime_Pct,
		ClassMask:  SpellMaskShadowBolt,
		FloatValue: -1,
	})

	shadowTranceAura := warlock.RegisterAura(core.Aura{
		Label:    "Nightfall Shadow Trance",
		ActionID: core.ActionID{SpellID: 17941},
		Duration: time.Second * 10,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			instantShadowBolt.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			instantShadowBolt.Deactivate()
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			// Check if the shadowbolt was instant cast and not a normal one
			if spell.SpellCode == SpellCode_WarlockShadowBolt && spell.CurCast.CastTime == 0 {
				aura.Deactivate(sim)
			}
		},
	})

	// The beta client adds Wrack to Corruption, Drain Life and Drain Soul
	affectedSpellCodes := []int32{SpellCode_WarlockCorruption, SpellCode_WarlockDrainLife, SpellCode_WarlockDrainSoul, SpellCode_WarlockWrack}
	procChance := 0.02 * float64(warlock.Talents.Nightfall)

	core.MakePermanent(warlock.RegisterAura(core.Aura{
		Label: "Nightfall Hidden Aura",
		OnPeriodicDamageDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if slices.Contains(affectedSpellCodes, spell.SpellCode) && sim.Proc(procChance, "Nightfall") {
				shadowTranceAura.Activate(sim)
			}
		},
	}))
}

func (warlock *Warlock) applyShadowMastery() {
	if warlock.Talents.ShadowMastery == 0 {
		return
	}

	// Classic's 18271 carries three modifiers: damage (op 0), periodic damage (op 22) and
	// spell effectiveness (op 8), the last of which scales base points only. Four spells were
	// singled out for that treatment, multiplying their own base damage and opting out of the
	// spell multiplier. Forever dropped the op 8 effect: the beta client leaves 18271 with op 0
	// and op 22, both Apply Aura: Add % Modifier, so every shadow spell takes it the same way.
	//
	// The split had also drifted. Siphon Life multiplied its base damage and was never in the
	// exclusion list, so it took Shadow Mastery twice; Drain Soul was in the list but never
	// multiplied anything, so it took none at all.
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_DamageDone_Flat,
		School:     core.SpellSchoolShadow,
		SpellFlag:  SpellFlagWarlock,
		FloatValue: .01 * float64(warlock.Talents.ShadowMastery),
	})
}

///////////////////////////////////////////////////////////////////////////
//                            Demonology Talents
///////////////////////////////////////////////////////////////////////////

func (warlock *Warlock) applyDemonicEmbrace() {
	if warlock.Talents.DemonicEmbrace == 0 {
		return
	}

	warlock.MultiplyStat(stats.Stamina, 1+.03*float64(warlock.Talents.DemonicEmbrace))
}

func (warlock *Warlock) applyUnholyPower() {
	if warlock.Talents.UnholyPower == 0 {
		return
	}

	multiplier := 1 + 0.02*float64(warlock.Talents.UnholyPower)
	for _, pet := range warlock.BasePets {
		pet.PseudoStats.DamageDealtMultiplier *= multiplier
	}
}

func (warlock *Warlock) applyFelVitality() {
	if warlock.Talents.FelVitality == 0 {
		return
	}

	multiplier := 1 + 0.05*float64(warlock.Talents.FelVitality)
	warlock.MultiplyStat(stats.Mana, multiplier)
	for _, pet := range warlock.BasePets {
		pet.MultiplyStat(stats.Health, multiplier)
		pet.MultiplyStat(stats.Mana, multiplier)
	}
}

func (warlock *Warlock) applyMasterSummoner() {
	if warlock.Talents.MasterSummoner == 0 {
		return
	}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Flat,
		ClassMask: SpellMaskSummonDemon,
		TimeValue: -time.Second * 2 * time.Duration(warlock.Talents.MasterSummoner),
	})
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		ClassMask:  SpellMaskSummonDemon,
		FloatValue: -.20 * float64(warlock.Talents.MasterSummoner),
	})
}

func (warlock *Warlock) applyDecimation() {
	if warlock.Talents.Decimation == 0 {
		return
	}

	// Beta client 1.60.1 (talent 440870, buff 440873): the damage bonus (3/6%), the cast time reduction
	// (20/40%) and the Soul Fire cooldown reduction in soul_fire.go (45/90%) all grow with the second
	// point, while the 35% health threshold and the ten second buff are single values.
	points := float64(warlock.Talents.Decimation)

	// The damage half of the tooltip is about the two spells that trigger it, not about
	// everything the warlock casts; only the Soul Fire half carries the ten second window.
	damageMod := warlock.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_DamageDone_Flat,
		ClassMask:  SpellMaskShadowBolt | SpellMaskSearingPain,
		FloatValue: 0.03 * points,
	})
	castTimeMod := warlock.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_CastTime_Pct,
		ClassMask:  SpellMaskSoulFire,
		FloatValue: -0.2 * points,
	})

	warlock.DecimationAura = warlock.RegisterAura(core.Aura{
		Label:    "Decimation",
		ActionID: core.ActionID{SpellID: 440873},
		Duration: time.Second * 10,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Activate()
			castTimeMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Deactivate()
			castTimeMod.Deactivate()
		},
	})

	affectedSpellCodes := []int32{SpellCode_WarlockShadowBolt, SpellCode_WarlockSearingPain}
	core.MakePermanent(warlock.RegisterAura(core.Aura{
		Label: "Decimation Trigger",
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.Landed() && sim.IsExecutePhase35() && slices.Contains(affectedSpellCodes, spell.SpellCode) {
				warlock.DecimationAura.Activate(sim)
			}
		},
	}))
}

func (warlock *Warlock) applyDemonicBrand() {
	if warlock.Talents.DemonicBrand == 0 {
		return
	}

	// Beta client 1.60.1 (talent 1293695): Searing Pain threat falls 17/33/50% and the brand arms 2/4/6 of
	// the pet's attacks; the brand itself (1293696) lasts 10 sec at every rank.
	// TODO: the client writes the pet hit as a $<minDam> to $<maxDam> formula that the exported tables
	// do not carry, so the 39 to 42 from the BlizzCon tooltip is kept.
	actionID := core.ActionID{SpellID: 18821}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_ThreatMultiplier_Pct,
		ClassMask:  SpellMaskSearingPain,
		FloatValue: -[]float64{0, 0.17, 0.33, 0.50}[warlock.Talents.DemonicBrand],
	})

	for _, pet := range warlock.BasePets {
		brandSpell := pet.RegisterSpell(core.SpellConfig{
			ActionID:    actionID,
			SpellSchool: core.SpellSchoolShadow,
			DefenseType: core.DefenseTypeMagic,
			ProcMask:    core.ProcMaskEmpty,
			Flags:       core.SpellFlagPassiveSpell | core.SpellFlagNoOnCastComplete,

			DamageMultiplier: 1,
			ThreatMultiplier: 3,

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				spell.CalcAndDealDamage(sim, target, sim.Roll(39, 42), spell.OutcomeMagicHit)
			},
		})

		pet.DemonicBrandAura = pet.RegisterAura(core.Aura{
			Label:     "Demonic Brand",
			ActionID:  actionID,
			Duration:  time.Second * 10,
			MaxStacks: 2 * warlock.Talents.DemonicBrand,
			OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				if result.Landed() && spell.ProcMask.Matches(core.ProcMaskMelee) {
					brandSpell.Cast(sim, result.Target)
					aura.RemoveStack(sim)
				}
			},
		})
	}

	core.MakePermanent(warlock.RegisterAura(core.Aura{
		Label: "Demonic Brand Trigger",
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.Landed() && spell.SpellCode == SpellCode_WarlockSearingPain && warlock.ActivePet != nil {
				brandAura := warlock.ActivePet.DemonicBrandAura
				brandAura.Activate(sim)
				brandAura.SetStacks(sim, brandAura.MaxStacks)
			}
		},
	}))
}

func (warlock *Warlock) applyDemonicKnowledge() {
	if warlock.Talents.DemonicKnowledge == 0 {
		return
	}

	// The tooltip only pays the bonus out while a demon is active, so it rides on the pet
	// rather than sitting on the character sheet.
	// 33/67/100% of level, the beta client's curve for the talent (412732).
	bonus := []float64{0, 0.33, 0.67, 1.00}[warlock.Talents.DemonicKnowledge] * float64(warlock.Level)

	demonicKnowledgeAura := warlock.RegisterAura(core.Aura{
		Label:    "Demonic Knowledge",
		ActionID: core.ActionID{SpellID: 35696},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warlock.AddStatDynamic(sim, stats.SpellPower, bonus)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warlock.AddStatDynamic(sim, stats.SpellPower, -bonus)
		},
	})

	for _, pet := range warlock.BasePets {
		pet.ApplyOnPetEnable(func(sim *core.Simulation) {
			demonicKnowledgeAura.Activate(sim)
		})
		pet.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
			demonicKnowledgeAura.Deactivate(sim)
		})
	}
}

func (warlock *Warlock) applyMasterDemonologist() {
	if warlock.Talents.MasterDemonologist == 0 {
		return
	}

	points := float64(warlock.Talents.MasterDemonologist)
	damageDealtMultiplier := 1 + 0.02*points
	damageTakenMultiplier := 1 - 0.02*points

	impConfig := core.Aura{
		Label:    "Master Demonologist (Imp)",
		ActionID: core.ActionID{SpellID: 23825, Tag: 1},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] *= damageDealtMultiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] /= damageDealtMultiplier
		},
	}

	// Physical damage only, per the beta client's talent text (23785)
	voidwalkerConfig := core.Aura{
		Label:    "Master Demonologist (Voidwalker)",
		ActionID: core.ActionID{SpellID: 23825, Tag: 2},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageTakenMultiplier[stats.SchoolIndexPhysical] *= damageTakenMultiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageTakenMultiplier[stats.SchoolIndexPhysical] /= damageTakenMultiplier
		},
	}

	succubusConfig := core.Aura{
		Label:    "Master Demonologist (Succubus)",
		ActionID: core.ActionID{SpellID: 23825, Tag: 3},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] *= damageDealtMultiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] /= damageDealtMultiplier
		},
	}

	felhunterConfig := core.Aura{
		Label:    "Master Demonologist (Felhunter)",
		ActionID: core.ActionID{SpellID: 23825, Tag: 4},
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			for school := stats.SchoolIndexArcane; school <= stats.SchoolIndexShadow; school++ {
				aura.Unit.PseudoStats.SchoolDamageTakenMultiplier[school] *= damageTakenMultiplier
			}
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			for school := stats.SchoolIndexArcane; school <= stats.SchoolIndexShadow; school++ {
				aura.Unit.PseudoStats.SchoolDamageTakenMultiplier[school] /= damageTakenMultiplier
			}
		},
	}

	for _, pet := range warlock.BasePets {
		pet.ApplyOnPetEnable(func(sim *core.Simulation) {
			if warlock.MasterDemonologistAura != nil {
				warlock.MasterDemonologistAura.Deactivate(sim)
			}
		})
	}

	warlockImpAura := warlock.RegisterAura(impConfig)
	impAura := warlock.Imp.RegisterAura(impConfig)
	warlock.Imp.ApplyOnPetEnable(func(sim *core.Simulation) {
		impAura.Activate(sim)
		warlock.MasterDemonologistAura = warlockImpAura
	})
	warlock.Imp.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
		impAura.Deactivate(sim)
	})

	warlockVoidwalkerAura := warlock.RegisterAura(voidwalkerConfig)
	voidwalkerAura := warlock.Voidwalker.RegisterAura(voidwalkerConfig)
	warlock.Voidwalker.ApplyOnPetEnable(func(sim *core.Simulation) {
		voidwalkerAura.Activate(sim)
		warlock.MasterDemonologistAura = warlockVoidwalkerAura
	})
	warlock.Voidwalker.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
		voidwalkerAura.Deactivate(sim)
	})

	warlockSuccubusAura := warlock.RegisterAura(succubusConfig)
	succubusAura := warlock.Succubus.RegisterAura(succubusConfig)
	warlock.Succubus.ApplyOnPetEnable(func(sim *core.Simulation) {
		succubusAura.Activate(sim)
		warlock.MasterDemonologistAura = warlockSuccubusAura
	})
	warlock.Succubus.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
		succubusAura.Deactivate(sim)
	})

	warlockFelhunterAura := warlock.RegisterAura(felhunterConfig)
	felhunterAura := warlock.Felhunter.RegisterAura(felhunterConfig)
	warlock.Felhunter.ApplyOnPetEnable(func(sim *core.Simulation) {
		felhunterAura.Activate(sim)
		warlock.MasterDemonologistAura = warlockFelhunterAura
	})
	warlock.Felhunter.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
		felhunterAura.Deactivate(sim)
	})

	for _, pet := range warlock.BasePets {
		pet.ApplyOnPetEnable(func(sim *core.Simulation) {
			warlock.MasterDemonologistAura.Activate(sim)
		})

		pet.ApplyOnPetDisable(func(sim *core.Simulation, isSacrifice bool) {
			if warlock.MasterDemonologistAura != nil {
				warlock.MasterDemonologistAura.Deactivate(sim)
				warlock.MasterDemonologistAura = nil
			}
		})
	}
}

func (warlock *Warlock) applySoulLink() {
	if !warlock.Talents.SoulLink {
		return
	}

	actionID := core.ActionID{SpellID: 19028}
	soulLinkConfig := core.Aura{
		Label:    "Soul Link Aura",
		ActionID: actionID,
		Duration: core.NeverExpires,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.DamageTakenMultiplier /= 1.3
			aura.Unit.PseudoStats.DamageDealtMultiplier *= 1.03
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.DamageDealtMultiplier /= 1.03
			aura.Unit.PseudoStats.DamageTakenMultiplier *= 1.3
		},
	}

	warlock.SoulLinkAura = warlock.RegisterAura(soulLinkConfig)
	for _, pet := range warlock.BasePets {
		pet.SoulLinkAura = pet.RegisterAura(soulLinkConfig)

		oldOnPetDisable := pet.OnPetDisable
		pet.OnPetDisable = func(sim *core.Simulation, isSacrifice bool) {
			oldOnPetDisable(sim, isSacrifice)
			warlock.SoulLinkAura.Deactivate(sim)
			pet.SoulLinkAura.Deactivate(sim)
		}
	}

	warlock.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagAPL,

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warlock.ActivePet != nil
		},

		ManaCost: core.ManaCostOptions{
			BaseCost: 0.2,
		},

		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			warlock.SoulLinkAura.Activate(sim)
			warlock.ActivePet.SoulLinkAura.Activate(sim)
		},
	})
}

func (warlock *Warlock) applyDemonicSacrifice() {
	if !warlock.Talents.DemonicSacrifice {
		return
	}

	duration := time.Hour * 2

	// Each demon leaves behind the opposing aspect, and the pairing is the reverse of Classic's. The
	// beta client's talent text and its buffs agree: the Imp leaves Shadow damage (18789, now Burning
	// Shadow, school mask 32), the Succubus Fire damage (18791, now Touch of Fire, mask 4), the
	// Voidwalker mana (18792) and the Felhunter health (18790).
	impAura := warlock.GetOrRegisterAura(core.Aura{
		Label:    "Burning Shadow",
		ActionID: core.ActionID{SpellID: 18789},
		Duration: duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] *= 1.15
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] /= 1.15
		},
	})

	succubusAura := warlock.GetOrRegisterAura(core.Aura{
		Label:    "Touch of Fire",
		ActionID: core.ActionID{SpellID: 18791},
		Duration: duration,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] *= 1.15
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] /= 1.15
		},
	})

	var fhPa *core.PendingAction
	healthMetric := warlock.NewHealthMetrics(core.ActionID{SpellID: 18790})
	felhunterAura := warlock.GetOrRegisterAura(core.Aura{
		Label:    "Fel Stamina",
		ActionID: core.ActionID{SpellID: 18790},
		Duration: duration,

		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			fhPa = core.NewPeriodicAction(sim, core.PeriodicActionOptions{
				Period: time.Second * 4,
				OnAction: func(s *core.Simulation) {
					warlock.GainHealth(sim, warlock.MaxHealth()*0.03, healthMetric)
				},
			})
			sim.AddPendingAction(fhPa)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			fhPa.Cancel(sim)
		},
	})

	var vwPa *core.PendingAction
	manaMetric := warlock.NewManaMetrics(core.ActionID{SpellID: 18792})
	voidwalkerAura := warlock.GetOrRegisterAura(core.Aura{
		Label:    "Fel Energy",
		ActionID: core.ActionID{SpellID: 18792},
		Duration: duration,

		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			vwPa = core.NewPeriodicAction(sim, core.PeriodicActionOptions{
				Period: time.Second * 4,
				OnAction: func(s *core.Simulation) {
					warlock.AddMana(sim, warlock.MaxMana()*0.02, manaMetric)
				},
			})
			sim.AddPendingAction(vwPa)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			vwPa.Cancel(sim)
		},
	})

	warlock.DemonicSacrificeAuras = map[*WarlockPet]*core.Aura{
		warlock.Felhunter:  felhunterAura,
		warlock.Imp:        impAura,
		warlock.Succubus:   succubusAura,
		warlock.Voidwalker: voidwalkerAura,
	}
	for _, pet := range warlock.BasePets {
		pet.ApplyOnPetEnable(func(sim *core.Simulation) {
			// With Demonic Pact only resummoning the sacrificed demon ends the effect
			if warlock.Talents.DemonicPact && pet != warlock.SacrificedPet {
				return
			}

			for _, dsAura := range warlock.DemonicSacrificeAuras {
				dsAura.Deactivate(sim)
			}
			warlock.SacrificedPet = nil
		})
	}

	warlock.GetOrRegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_WarlockDemonicSacrifice,
		ClassSpellMask: SpellMaskDemonicSacrifice,
		ActionID:       core.ActionID{SpellID: 18788},
		SpellSchool:    core.SpellSchoolShadow,
		Flags:          core.SpellFlagAPL,

		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return warlock.ActivePet != nil
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			for _, dsAura := range warlock.DemonicSacrificeAuras {
				dsAura.Deactivate(sim)
			}
			warlock.DemonicSacrificeAuras[warlock.ActivePet].Activate(sim)

			warlock.SacrificedPet = warlock.ActivePet
			warlock.changeActivePet(sim, nil, true)
		},
	})
}

///////////////////////////////////////////////////////////////////////////
//                            Destruction Talents
///////////////////////////////////////////////////////////////////////////

func (warlock *Warlock) applyImprovedShadowBolt() {
	if warlock.Talents.ImprovedShadowBolt == 0 {
		return
	}

	// 4% per point; the debuff (17794 in the beta client) lasts 12 sec at every rank.
	damageMultiplier := 1 + 0.04*float64(warlock.Talents.ImprovedShadowBolt)

	warlock.ImprovedShadowBoltAuras = warlock.NewEnemyAuraArray(func(unit *core.Unit) *core.Aura {
		return unit.RegisterAura(core.Aura{
			Label:    "Improved Shadow Bolt-" + warlock.Label,
			ActionID: core.ActionID{SpellID: 17800},
			Duration: time.Second * 12,
		})
	})

	// Only the warlock's own shadow damage benefits, so this can't go through the target's school multiplier
	for _, target := range warlock.Env.Encounter.TargetUnits {
		target.AddDynamicDamageTakenModifier(func(sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if spell.Unit == &warlock.Unit && spell.SpellSchool.Matches(core.SpellSchoolShadow) && warlock.ImprovedShadowBoltAuras.Get(result.Target).IsActive() {
				result.Damage *= damageMultiplier
			}
		})
	}

	core.MakePermanent(warlock.RegisterAura(core.Aura{
		Label: "ISB Trigger",
		OnInit: func(aura *core.Aura, sim *core.Simulation) {
			for _, spell := range warlock.ShadowBolt {
				spell.RelatedAuras = []core.AuraArray{warlock.ImprovedShadowBoltAuras}
			}
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if result.DidCrit() && spell.SpellCode == SpellCode_WarlockShadowBolt {
				warlock.ImprovedShadowBoltAuras.Get(result.Target).Activate(sim)
			}
		},
	}))
}

func (warlock *Warlock) applyMoltenSkin() {
	if warlock.Talents.MoltenSkin == 0 {
		return
	}

	// 2% less damage taken per point (beta client talent 1225220)
	warlock.PseudoStats.DamageTakenMultiplier *= 1 - 0.02*float64(warlock.Talents.MoltenSkin)
}

func (warlock *Warlock) applyCataclysm() {
	if warlock.Talents.Cataclysm == 0 {
		return
	}

	// 3/6/10% in the beta client, not 9% at 3/3
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		SpellFlag:  WarlockFlagDestruction,
		FloatValue: -[]float64{0, .03, .06, .10}[warlock.Talents.Cataclysm],
	})
}

func (warlock *Warlock) applyBane() {
	if warlock.Talents.Bane == 0 {
		return
	}

	points := time.Duration(warlock.Talents.Bane)
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Flat,
		ClassMask: SpellMaskShadowBolt | SpellMaskImmolate | SpellMaskIncinerate,
		TimeValue: -time.Millisecond * 100 * points,
	})
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Flat,
		ClassMask: SpellMaskSoulFire,
		TimeValue: -time.Millisecond * 400 * points,
	})
}

func (warlock *Warlock) applyRuin() {
	if warlock.Talents.Ruin == 0 {
		return
	}

	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_CritMultiplier_Flat,
		SpellFlag:  WarlockFlagDestruction,
		FloatValue: 0.2 * float64(warlock.Talents.Ruin),
	})
}

func (warlock *Warlock) applyAgonizingFlames() {
	if warlock.Talents.AgonizingFlames == 0 {
		return
	}

	// 3/7/10% in the beta client, not 9% at 3/3
	bonus := []float64{0, 3, 7, 10}[warlock.Talents.AgonizingFlames]
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_DamageDone_Flat,
		SpellFlag:  WarlockFlagDestruction,
		FloatValue: bonus / 100,
	})
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BonusCrit_Percent,
		ClassMask:  SpellMaskSearingPain,
		FloatValue: bonus,
	})
}

func (warlock *Warlock) applyFireAndBrimstone() {
	if warlock.Talents.FireAndBrimstone == 0 {
		return
	}

	// 8/17/25, not the 8/16/24 that multiplying rank 1 gives. Rank 3's 25% is confirmed on
	// the beta; rank 2's 17 is the tree's own reading.
	warlock.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BonusCrit_Percent,
		ClassMask:  SpellMaskConflagrate,
		FloatValue: []float64{0, 8, 17, 25}[warlock.Talents.FireAndBrimstone],
	})
}

func (warlock *Warlock) applyShadowAndFlame() {
	if warlock.Talents.ShadowAndFlame == 0 {
		return
	}

	// 2% per point. The demo only ever showed rank 1, and the tree repeated its 2% at every
	// rank, which is why this was held flat; the beta has since confirmed rank 4 at 8% and
	// rank 5 at 10%, so the repeated text was the extractor's, not the game's.
	multiplier := 1 + 0.02*float64(warlock.Talents.ShadowAndFlame)

	shadowAura := warlock.RegisterAura(core.Aura{
		Label:    "Shadow and Flame (Shadow)",
		ActionID: core.ActionID{SpellID: 30288, Tag: 1},
		Duration: time.Second * 20,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] *= multiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexShadow] /= multiplier
		},
	})

	fireAura := warlock.RegisterAura(core.Aura{
		Label:    "Shadow and Flame (Fire)",
		ActionID: core.ActionID{SpellID: 30288, Tag: 2},
		Duration: time.Second * 20,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] *= multiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			warlock.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] /= multiplier
		},
	})

	core.MakePermanent(warlock.RegisterAura(core.Aura{
		Label: "Shadow and Flame Trigger",
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() {
				return
			}

			switch spell.SpellCode {
			case SpellCode_WarlockConflagrate:
				shadowAura.Activate(sim)
			case SpellCode_WarlockShadowburn:
				fireAura.Activate(sim)
			}
		},
	}))
}
