package druid

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (druid *Druid) ApplyTalents() {
	// Balance
	druid.applyGenesis()
	druid.applyMoonglow()
	druid.applyImprovedMoonfire()
	druid.applyNaturesMajesty()
	druid.applyNaturesReach()
	druid.applyNaturesSplendor()
	druid.applyVengeance()
	druid.applyNaturesGrace()
	druid.applyEclipse()
	druid.applyMoonfury()
	druid.registerMoonkinFormSpell()

	// Feral Combat
	druid.applyHeartOfTheWild()
	druid.applyFeralSwiftness()
	druid.applyThickHide()
	druid.applyPrimalFury()
	druid.applyPredatoryInstincts()
	druid.applyNaturalReaction()
	druid.applyRendAndTear()

	// Restoration
	druid.applyFuror()
	druid.applyNaturalist()
	druid.applySubtlety()
	druid.applyLivingSpirit()

	druid.PseudoStats.SpiritRegenRateCasting += []float64{0, .17, .33, .50}[druid.Talents.Reflection]
}

// The Balance spells that the Arcane and Nature talents apply to.
func (druid *Druid) balanceSpells() []*DruidSpell {
	return core.FilterSlice(
		core.Flatten(
			[][]*DruidSpell{
				druid.Wrath,
				druid.Starfire,
				druid.Moonfire,
				druid.InsectSwarm,
				druid.Hurricane,
			},
		),
		func(spell *DruidSpell) bool { return spell != nil },
	)
}

func (druid *Druid) applyGenesis() {
	if druid.Talents.Genesis == 0 {
		return
	}

	// Every druid spell with a dot or AoE dot. A new dot spell has to be added here.
	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_PeriodicDamageDone_Flat,
		ClassMask:  SpellMaskMoonfire | SpellMaskInsectSwarm | SpellMaskHurricane | SpellMaskRake | SpellMaskRip | SpellMaskLacerateBleed,
		FloatValue: 0.01 * float64(druid.Talents.Genesis),
	})
}

func (druid *Druid) applyMoonglow() {
	if druid.Talents.Moonglow == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		CostType:   core.CostTypeMana,
		FloatValue: -[]float64{0, 0.08, 0.17, 0.25}[druid.Talents.Moonglow],
	})
}

func (druid *Druid) applyImprovedMoonfire() {
	if druid.Talents.ImprovedMoonfire == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BaseDamageDone_Flat,
		ClassMask:  SpellMaskMoonfire,
		FloatValue: 0.05 * float64(druid.Talents.ImprovedMoonfire),
	})
	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BonusCrit_Percent,
		ClassMask:  SpellMaskMoonfire,
		FloatValue: 5 * float64(druid.Talents.ImprovedMoonfire),
	})
}

func (druid *Druid) applyNaturesMajesty() {
	if druid.Talents.NaturesMajesty == 0 {
		return
	}

	points := float64(druid.Talents.NaturesMajesty)
	druid.AddStat(stats.SpellCrit, 2*points*core.SpellCritRatingPerCritChance)
	druid.AddStat(stats.MeleeCrit, 2*points*core.CritRatingPerCritChance)
}

func (druid *Druid) applyNaturesReach() {
	if druid.Talents.NaturesReach == 0 {
		return
	}

	points := float64(druid.Talents.NaturesReach)
	druid.AddStat(stats.SpellHit, 2*points*core.SpellHitRatingPerHitChance)
	druid.AddStat(stats.MeleeHit, 2*points*core.MeleeHitRatingPerHitChance)
}

// Moonfire ticks every 3 sec and Insect Swarm every 2 sec, so the added duration
// is one extra tick on each.
func (druid *Druid) applyNaturesSplendor() {
	if !druid.Talents.NaturesSplendor {
		return
	}

	druid.RegisterAura(core.Aura{
		Label: "Nature's Splendor",
		OnInit: func(aura *core.Aura, sim *core.Simulation) {
			for _, spell := range druid.balanceSpells() {
				if spell.SpellCode != SpellCode_DruidMoonfire && spell.SpellCode != SpellCode_DruidInsectSwarm {
					continue
				}

				for _, dot := range spell.Dots() {
					if dot == nil {
						continue
					}

					dot.OriginalNumberOfTicks += 1
					dot.NumberOfTicks += 1
					dot.RecomputeAuraDuration()
				}
			}
		},
	})
}

func (druid *Druid) applyVengeance() {
	if druid.Talents.Vengeance == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_CritMultiplier_Flat,
		ClassMask:  SpellMaskBalance,
		FloatValue: 0.20 * float64(druid.Talents.Vengeance),
	})
}

// Forever replaces the cast time reduction on the next cast with a short haste buff,
// which also shortens the global cooldown.
func (druid *Druid) applyNaturesGrace() {
	if !druid.Talents.NaturesGrace {
		return
	}

	// The GCD isn't affected by haste in Classic, so it's shortened by hand to match
	hasteMultiplier := 1.1
	gcdReduction := core.GCDDefault - max(core.GCDMin, time.Duration(float64(core.GCDDefault)/hasteMultiplier))

	// Wrath and Starfire are the druid spells with a cast time and the default GCD.
	gcdMod := druid.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_GlobalCooldown_Flat,
		ClassMask: SpellMaskWrath | SpellMaskStarfire,
		TimeValue: -gcdReduction,
	})

	druid.NaturesGraceHasteAura = druid.RegisterAura(core.Aura{
		Label:    "Natures Grace",
		ActionID: core.ActionID{SpellID: 16886},
		Duration: time.Second * 3,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			druid.MultiplyCastSpeed(hasteMultiplier)
			gcdMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			druid.MultiplyCastSpeed(1 / hasteMultiplier)
			gcdMod.Deactivate()
		},
	})

	core.MakePermanent(druid.RegisterAura(core.Aura{
		Label: "Natures Grace Trigger",
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			// Spells with travel times have their own implementation because the proc occurs as the cast finishes
			if spell.MissileSpeed == 0 && spell.ProcMask.Matches(core.ProcMaskSpellDamage) && result.DidCrit() {
				druid.procNaturesGrace(sim)
			}
		},
	}))
}

func (druid *Druid) procNaturesGrace(sim *core.Simulation) {
	if druid.NaturesGraceHasteAura == nil {
		return
	}

	druid.NaturesGraceHasteAura.Activate(sim)
}

// Every Wrath banks charges that each shorten one Starfire cast.
func (druid *Druid) applyEclipse() {
	if druid.Talents.Eclipse == 0 {
		return
	}

	// The beta client's curve is 0.17 / 0.33 / 0.5 sec.
	castTimeReduction := time.Millisecond * []time.Duration{0, 170, 330, 500}[druid.Talents.Eclipse]
	const chargesPerWrath = 2

	castTimeMod := druid.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Flat,
		ClassMask: SpellMaskStarfire,
		TimeValue: -castTimeReduction,
	})

	druid.EclipseAura = druid.RegisterAura(core.Aura{
		Label:     "Eclipse",
		ActionID:  core.ActionID{SpellID: 48518},
		Duration:  time.Second * 15,
		MaxStacks: 4,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			castTimeMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			castTimeMod.Deactivate()
		},
	})

	core.MakePermanent(druid.RegisterAura(core.Aura{
		Label: "Eclipse Trigger",
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			switch spell.SpellCode {
			case SpellCode_DruidWrath:
				druid.EclipseAura.Activate(sim)
				druid.EclipseAura.AddStacks(sim, chargesPerWrath)
			case SpellCode_DruidStarfire:
				if druid.EclipseAura.IsActive() {
					druid.EclipseAura.RemoveStack(sim)
				}
			}
		},
	}))
}

func (druid *Druid) applyMoonfury() {
	if druid.Talents.Moonfury == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_BaseDamageDone_Flat,
		ClassMask:  SpellMaskBalance,
		FloatValue: 0.02 * float64(druid.Talents.Moonfury),
	})
}

///////////////////////////////////////////////////////////////////////////
//                            Feral Combat
///////////////////////////////////////////////////////////////////////////

func (druid *Druid) applyHeartOfTheWild() {
	if druid.Talents.HeartOfTheWild == 0 {
		return
	}

	druid.MultiplyStat(stats.Intellect, 1+0.02*float64(druid.Talents.HeartOfTheWild))
}

func (druid *Druid) applyFeralSwiftness() {
	if druid.Talents.FeralSwiftness == 0 {
		return
	}

	druid.AddStat(stats.Dodge, 2*float64(druid.Talents.FeralSwiftness)*core.DodgeRatingPerDodgeChance)
}

// Forever swaps the armor multiplier for flat base Armor from level and defense skill,
// and the form's own armor multiplier applies on top of it. The sim's druids never
// leave their starting form, so the form requirement is read from that form and the
// defense skill is the gear's when the talents are applied.
func (druid *Druid) applyThickHide() {
	if druid.Talents.ThickHide == 0 {
		return
	}

	points := float64(druid.Talents.ThickHide)
	armor := points*float64(druid.Level) + 0.67*points*druid.EquipStats()[stats.Defense]
	if druid.StartingForm.Matches(Bear) {
		armor *= BearFormArmorMultiplier
	}
	druid.AddStat(stats.Armor, armor)
}

// Forever folds the old Blood Frenzy combo point proc into Primal Fury.
func (druid *Druid) applyPrimalFury() {
	if druid.Talents.PrimalFury == 0 {
		return
	}

	// The beta client's curves are 50 / 100% for both halves, and the Rage is 16959's 5.
	procChance := []float64{0, 0.5, 1}[druid.Talents.PrimalFury]
	actionID := core.ActionID{SpellID: 37117}
	rageMetrics := druid.NewRageMetrics(actionID)
	cpMetrics := druid.NewComboPointMetrics(actionID)

	core.MakePermanent(druid.RegisterAura(core.Aura{
		Label: "Primal Fury",
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Outcome.Matches(core.OutcomeCrit) {
				return
			}

			if druid.InForm(Bear) {
				if sim.Proc(procChance, "Primal Fury") {
					druid.AddRage(sim, 5, rageMetrics)
				}
			} else if druid.InForm(Cat) &&
				result.Target == aura.Unit.CurrentTarget &&
				spell.Flags.Matches(SpellFlagBuilder) &&
				sim.Proc(procChance, "Primal Fury") {
				druid.AddComboPoints(sim, 1, result.Target, cpMetrics)
			}
		},
	}))
}

func (druid *Druid) applyPredatoryInstincts() {
	if druid.Talents.PredatoryInstincts == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:        core.SpellMod_CritMultiplier_Flat,
		DefenseType: core.DefenseTypeMelee,
		ProcMask:    core.ProcMaskMeleeSpecial,
		FloatValue:  0.1 * float64(druid.Talents.PredatoryInstincts),
	})
}

// Dodge chance, and a chance at Rage on every dodge.
func (druid *Druid) applyNaturalReaction() {
	if druid.Talents.NaturalReaction == 0 {
		return
	}

	// The beta client's curves are 1-5% dodge and a 20-100% chance at the Rage.
	druid.AddStat(stats.Dodge, float64(druid.Talents.NaturalReaction)*core.DodgeRatingPerDodgeChance)

	procChance := 0.2 * float64(druid.Talents.NaturalReaction)
	rageMetrics := druid.NewRageMetrics(core.ActionID{SpellID: 57878})

	core.MakePermanent(druid.RegisterAura(core.Aura{
		Label: "Natural Reaction",
		OnSpellHitTaken: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if druid.InForm(Bear) && result.Outcome.Matches(core.OutcomeDodge) && sim.Proc(procChance, "Natural Reaction") {
				druid.AddRage(sim, 5, rageMetrics)
			}
		},
	}))
}

func (druid *Druid) applyRendAndTear() {
	if druid.Talents.RendAndTear == 0 {
		return
	}

	multiplier := 1 + 0.02*float64(druid.Talents.RendAndTear)

	for _, target := range druid.Env.Encounter.TargetUnits {
		target.AddDynamicDamageTakenModifier(func(sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if spell.Unit == &druid.Unit && spell.ProcMask.Matches(core.ProcMaskMeleeSpecial) && druid.IsBleeding(result.Target) {
				result.Damage *= multiplier
			}
		})
	}
}

func (druid *Druid) IsBleeding(target *core.Unit) bool {
	return druid.AssumeBleedActive ||
		(druid.Rip != nil && druid.Rip.Dot(target).IsActive()) ||
		(druid.Rake != nil && druid.Rake.Dot(target).IsActive()) ||
		(druid.LacerateBleed != nil && druid.LacerateBleed.Dot(target).IsActive())
}

///////////////////////////////////////////////////////////////////////////
//                            Restoration
///////////////////////////////////////////////////////////////////////////

// We're using an aura so that the APL can know if the Druid has furor for powershifting logic
func (druid *Druid) applyFuror() {
	if druid.Talents.Furor == 0 {
		return
	}

	spellID := []int32{0, 17056, 17058, 17059, 17060, 17061}[druid.Talents.Furor]

	druid.FurorAura = druid.RegisterAura(core.Aura{
		Label:    "Furor",
		ActionID: core.ActionID{SpellID: spellID},
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
	})
}

func (druid *Druid) applyNaturalist() {
	if druid.Talents.Naturalist == 0 {
		return
	}

	// The beta client's curve is 1-5%.
	druid.PseudoStats.DamageDealtMultiplier *= 1 + 0.01*float64(druid.Talents.Naturalist)
}

func (druid *Druid) applySubtlety() {
	if druid.Talents.Subtlety == 0 {
		return
	}

	druid.AddStaticMod(core.SpellModConfig{
		Kind:       core.SpellMod_ThreatMultiplier_Pct,
		School:     core.SpellSchoolArcane | core.SpellSchoolNature,
		FloatValue: -0.1 * float64(druid.Talents.Subtlety),
	})
}

func (druid *Druid) applyLivingSpirit() {
	if druid.Talents.LivingSpirit == 0 {
		return
	}

	druid.MultiplyStat(stats.Spirit, 1+0.05*float64(druid.Talents.LivingSpirit))
}
