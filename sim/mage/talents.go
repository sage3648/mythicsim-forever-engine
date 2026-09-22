package mage

import (
	"slices"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

func (mage *Mage) ApplyTalents() {
	mage.applyArcaneTalents()
	mage.applyFireTalents()
	mage.applyFrostTalents()
}

// Mage spell modifiers from talents, as SpellMods (see core/spell_mod.go). The mod kinds are
// additive or multiplicative exactly like the hand-written code they replace, and each mod is
// added at the point the old OnSpellRegistered handler was, so every spell sees the same
// operations in the same order.
func (mage *Mage) addMageMod(kind core.SpellModType, school core.SpellSchool, value float64) {
	mage.AddStaticMod(core.SpellModConfig{
		Kind:       kind,
		School:     school,
		SpellFlag:  SpellFlagMage,
		FloatValue: value,
	})
}

func (mage *Mage) applyArcaneTalents() {
	mage.applyArcaneConcentration()
	mage.applyMissileBarrage()
	mage.registerPresenceOfMindCD()
	mage.registerArcanePowerCD()

	// Arcane Subtlety, 8 and 15 spell penetration and 15% threat reduction per point in the beta client.
	if mage.Talents.ArcaneSubtlety > 0 {
		mage.AddStat(stats.SpellPenetration, []float64{0, 8, 15}[mage.Talents.ArcaneSubtlety])
		mage.addMageMod(core.SpellMod_ThreatMultiplier_Pct, core.SpellSchoolArcane, -.15*float64(mage.Talents.ArcaneSubtlety))
	}

	// Arcane Focus
	if mage.Talents.ArcaneFocus > 0 {
		mage.addMageMod(core.SpellMod_BonusHit_Percent, core.SpellSchoolArcane, 1*float64(mage.Talents.ArcaneFocus))
	}

	// Magic Absorption
	if mage.Talents.MagicAbsorption > 0 {
		magicAbsorptionBonus := 5 * float64(mage.Talents.MagicAbsorption)
		mage.AddResistances(magicAbsorptionBonus)
	}

	// Arcane Resilience
	if mage.Talents.ArcaneResilience > 0 {
		mage.AddStatDependency(stats.Intellect, stats.Armor, .25*float64(mage.Talents.ArcaneResilience))
	}

	// Arcane Impact
	if mage.Talents.ArcaneImpact > 0 {
		mage.addMageMod(core.SpellMod_BonusCrit_Percent, core.SpellSchoolArcane, 2*float64(mage.Talents.ArcaneImpact))
	}

	// Arcane Meditation
	mage.PseudoStats.SpiritRegenRateCasting += []float64{0, .17, .33, .50}[mage.Talents.ArcaneMeditation]

	// Arcane Mind
	if mage.Talents.ArcaneMind > 0 {
		mage.MultiplyStat(stats.Intellect, 1.0+0.02*float64(mage.Talents.ArcaneMind))
		mage.addMageMod(core.SpellMod_CritMultiplier_Flat, core.SpellSchoolArcane, .20*float64(mage.Talents.ArcaneMind))
	}

	// Arcane Instability
	if mage.Talents.ArcaneInstability > 0 {
		mage.addMageMod(core.SpellMod_DamageDone_Flat, core.SpellSchoolNone, .01*float64(mage.Talents.ArcaneInstability))
		mage.addMageMod(core.SpellMod_BonusCrit_Percent, core.SpellSchoolNone, 1*float64(mage.Talents.ArcaneInstability))
	}
}

func (mage *Mage) applyFireTalents() {
	mage.applyIgnite()
	mage.applyImprovedScorch()
	mage.applyMasterOfElements()
	mage.applyHotStreak()

	mage.registerCombustionCD()

	// Incineration
	if mage.Talents.Incineration > 0 {
		mage.AddStaticMod(core.SpellModConfig{
			Kind:       core.SpellMod_BonusCrit_Percent,
			ClassMask:  SpellMaskArcaneBlast | SpellMaskFireBlast | SpellMaskIceLance | SpellMaskScorch,
			FloatValue: 2 * float64(mage.Talents.Incineration),
		})
	}

	// Burning Soul
	if mage.Talents.BurningSoul > 0 {
		mage.addMageMod(core.SpellMod_ThreatMultiplier_Pct, core.SpellSchoolFire, -.10*float64(mage.Talents.BurningSoul))
	}

	// Critical Mass
	if mage.Talents.CriticalMass > 0 {
		mage.addMageMod(core.SpellMod_BonusCrit_Percent, core.SpellSchoolFire, 2*float64(mage.Talents.CriticalMass))
	}

	// Fire Power buffs pretty much all mage fire spells EXCEPT ignite
	if mage.Talents.FirePower > 0 {
		mage.AddStaticMod(core.SpellModConfig{
			Kind:       core.SpellMod_DamageDone_Flat,
			ClassMask:  SpellMaskAll &^ SpellMaskIgnite,
			School:     core.SpellSchoolFire,
			SpellFlag:  SpellFlagMage,
			FloatValue: 0.02 * float64(mage.Talents.FirePower),
		})
	}
}

func (mage *Mage) applyFrostTalents() {
	mage.registerColdSnapCD()
	mage.registerIceBarrierSpell()
	mage.applyFingersOfFrost()
	mage.applyWintersChill()

	// Elemental Precision
	if mage.Talents.ElementalPrecision > 0 {
		mage.addMageMod(core.SpellMod_BonusHit_Percent, core.SpellSchoolFire|core.SpellSchoolFrost, 1*float64(mage.Talents.ElementalPrecision))
	}

	// Ice Shards
	if mage.Talents.IceShards > 0 {
		mage.addMageMod(core.SpellMod_CritMultiplier_Flat, core.SpellSchoolFrost, .20*float64(mage.Talents.IceShards))
	}

	// Piercing Ice
	if mage.Talents.PiercingIce > 0 {
		mage.addMageMod(core.SpellMod_DamageDone_Flat, core.SpellSchoolFrost, 0.02*float64(mage.Talents.PiercingIce))
	}

	// Frost Channeling
	if mage.Talents.FrostChanneling > 0 {
		mage.addMageMod(core.SpellMod_PowerCost_Pct_Add, core.SpellSchoolFrost, -.05*float64(mage.Talents.FrostChanneling))
		mage.addMageMod(core.SpellMod_ThreatMultiplier_Pct, core.SpellSchoolFrost, -.10*float64(mage.Talents.FrostChanneling))
	}
}

func (mage *Mage) applyArcaneConcentration() {
	if mage.Talents.ArcaneConcentration == 0 {
		return
	}

	procChance := 0.02 * float64(mage.Talents.ArcaneConcentration)

	mage.ClearcastingAura = mage.RegisterAura(core.Aura{
		Label:    "Clearcasting",
		ActionID: core.ActionID{SpellID: 12577},
		Duration: time.Second * 15,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolCostMultiplier.AddToMagicSchools(-100)
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			aura.Unit.PseudoStats.SchoolCostMultiplier.AddToMagicSchools(100)
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			// OnCastComplete is called after OnSpellHitDealt / etc, so don't deactivate if it was just activated.
			if aura.RemainingDuration(sim) == aura.Duration {
				return
			}
			if !spell.Flags.Matches(SpellFlagMage) {
				return
			}
			// Only spells that cost mana use up the proc. Check the base cost: Clearcasting itself
			// zeroes the current cost, so testing that meant the proc was never consumed.
			if spell.Cost == nil || spell.Cost.BaseCost == 0 {
				return
			}
			aura.Deactivate(sim)
		},
	})

	mage.RegisterAura(core.Aura{
		Label:    "Arcane Concentration",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() || !spell.Flags.Matches(SpellFlagMage) || spell.SpellCode == SpellCode_MageArcaneMissiles {
				return
			}

			// TODO: Classic verify arcane missile proc chance
			// Arcane Missile ticks can proc CC, just at a low rate of about 1.5% with 5/5 Arcane Concentration
			// if spell == mage.ArcaneMissilesTickSpell {
			// 	procChance *= 0.15
			// }

			if sim.Proc(procChance, "Arcane Concentration") {
				mage.ClearcastingAura.Activate(sim)
			}
		},
	})
}

// Arcane Blast feeds Missile Barrage at twice the rate of the other nukes, so the two of them
// are the backbone of the Forever arcane rotation.
func (mage *Mage) applyMissileBarrage() {
	if !mage.Talents.MissileBarrage {
		return
	}

	freeMissiles := mage.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		ClassMask:  SpellMaskArcaneMissiles,
		FloatValue: -1,
	})

	mage.MissileBarrageAura = mage.RegisterAura(core.Aura{
		Label:    "Missile Barrage",
		ActionID: core.ActionID{SpellID: 44404},
		Duration: time.Second * 15,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			freeMissiles.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			freeMissiles.Deactivate()
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if spell.SpellCode == SpellCode_MageArcaneMissiles {
				aura.Deactivate(sim)
			}
		},
	})

	mage.RegisterAura(core.Aura{
		Label:    "Missile Barrage Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			procChance := 0.0
			switch spell.SpellCode {
			case SpellCode_MageArcaneBlast:
				procChance = .40
			case SpellCode_MageFireball, SpellCode_MageFrostbolt:
				procChance = .20
			default:
				return
			}

			if sim.Proc(procChance, "Missile Barrage") {
				mage.MissileBarrageAura.Activate(sim)
			}
		},
	})
}

func (mage *Mage) registerPresenceOfMindCD() {
	if !mage.Talents.PresenceOfMind {
		return
	}

	actionID := core.ActionID{SpellID: 12043}
	cooldown := time.Second * 180

	affectedSpells := []*core.Spell{}
	pomAura := mage.RegisterAura(core.Aura{
		Label:    "Presence of Mind",
		ActionID: actionID,
		Duration: time.Second * 15,
		OnInit: func(aura *core.Aura, sim *core.Simulation) {
			for spellIdx := range mage.Spellbook {
				if spell := mage.Spellbook[spellIdx]; spell.DefaultCast.CastTime > 0 {
					affectedSpells = append(affectedSpells, spell)
				}
			}
		},
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			core.Each(affectedSpells, func(spell *core.Spell) {
				spell.CastTimeMultiplier -= 1
			})
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			core.Each(affectedSpells, func(spell *core.Spell) {
				spell.CastTimeMultiplier += 1
			})
			mage.PresenceOfMind.CD.Use(sim)
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if !slices.Contains(affectedSpells, spell) {
				return
			}

			aura.Deactivate(sim)
		},
	})

	mage.PresenceOfMind = mage.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagNoOnCastComplete,
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    mage.NewTimer(),
				Duration: cooldown,
			},
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return mage.GCD.IsReady(sim)
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			pomAura.Activate(sim)
		},
	})

	mage.AddMajorCooldown(core.MajorCooldown{
		Spell: mage.PresenceOfMind,
		Type:  core.CooldownTypeDPS,
	})
}

func (mage *Mage) registerArcanePowerCD() {
	if !mage.Talents.ArcanePower {
		return
	}

	actionID := core.ActionID{SpellID: 12042}

	damageMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_DamageDone_Flat,
		SpellFlag:  SpellFlagMage,
		FloatValue: 0.3,
	})
	costMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind:       core.SpellMod_PowerCost_Pct_Add,
		SpellFlag:  SpellFlagMage,
		FloatValue: 0.3,
	})

	mage.ArcanePowerAura = mage.RegisterAura(core.Aura{
		Label:    "Arcane Power",
		ActionID: actionID,
		Duration: time.Second * 15,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Activate()
			costMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Deactivate()
			costMod.Deactivate()
		},
	})
	core.RegisterPercentDamageModifierEffect(mage.ArcanePowerAura, 1.3)

	spell := mage.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagNoOnCastComplete,
		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    mage.NewTimer(),
				Duration: time.Second * 180,
			},
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			mage.ArcanePowerAura.Activate(sim)
		},
	})

	mage.AddMajorCooldown(core.MajorCooldown{
		Spell: spell,
		Type:  core.CooldownTypeDPS,
	})
}

// The raid debuff version of Improved Scorch is a Classic mechanic, in Forever the fire
// vulnerability only raises the damage the mage who stacked it deals.
func (mage *Mage) applyImprovedScorch() {
	if mage.Talents.ImprovedScorch == 0 {
		return
	}

	mage.ImprovedScorchAura = mage.RegisterAura(core.Aura{
		Label:     "Improved Scorch",
		ActionID:  core.ActionID{SpellID: 12873},
		Duration:  time.Second * 30,
		MaxStacks: 5,
		OnStacksChange: func(aura *core.Aura, sim *core.Simulation, oldStacks int32, newStacks int32) {
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] /= 1 + .03*float64(oldStacks)
			aura.Unit.PseudoStats.SchoolDamageDealtMultiplier[stats.SchoolIndexFire] *= 1 + .03*float64(newStacks)
		},
	})
}

func (mage *Mage) applyMasterOfElements() {
	if mage.Talents.MasterOfElements == 0 {
		return
	}

	refundCoeff := 0.1 * float64(mage.Talents.MasterOfElements)
	manaMetrics := mage.NewManaMetrics(core.ActionID{SpellID: 29076})

	mage.RegisterAura(core.Aura{
		Label:    "Master of Elements",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !spell.SpellSchool.Matches(core.SpellSchoolFire | core.SpellSchoolFrost) {
				return
			}
			if spell.CurCast.Cost == 0 {
				return
			}
			if result.DidCrit() {
				mage.AddMana(sim, spell.Cost.BaseCost*refundCoeff, manaMetrics)
			}
		},
	})
}

// Hot Streak shaves cast time off Pyroblast rather than making it instant, so the stacks are
// worth holding. Frostfire Bolt is named in the tooltip but has no Classic spell to attach to.
func (mage *Mage) applyHotStreak() {
	if !mage.Talents.HotStreak {
		return
	}

	triggerSpellCodes := []int32{SpellCode_MageFireball, SpellCode_MageFireBlast, SpellCode_MageScorch}

	castTimeMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_CastTime_Pct,
		ClassMask: SpellMaskPyroblast,
	})

	mage.HotStreakAura = mage.RegisterAura(core.Aura{
		Label:     "Hot Streak",
		ActionID:  core.ActionID{SpellID: 44445},
		Duration:  time.Second * 15,
		MaxStacks: 3,
		OnStacksChange: func(aura *core.Aura, sim *core.Simulation, oldStacks int32, newStacks int32) {
			castTimeMod.UpdateFloatValue(-.25 * float64(newStacks))
			castTimeMod.Activate()
		},
	})

	mage.RegisterAura(core.Aura{
		Label:    "Hot Streak Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.DidCrit() || !slices.Contains(triggerSpellCodes, spell.SpellCode) {
				return
			}

			mage.HotStreakAura.Activate(sim)
			mage.HotStreakAura.AddStack(sim)
		},
	})
}

// Number of non-periodic fire crits Combustion lasts for, up from 3 in Classic.
const CombustionCrits = 4

func (mage *Mage) registerCombustionCD() {
	if !mage.Talents.Combustion {
		return
	}

	actionID := core.ActionID{SpellID: 11129}
	cd := core.Cooldown{
		Timer:    mage.NewTimer(),
		Duration: time.Minute * 3,
	}

	critMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_BonusCrit_Percent,
		School:    core.SpellSchoolFire,
		SpellFlag: SpellFlagMage,
	})

	numCrits := 0

	mage.CombustionAura = mage.RegisterAura(core.Aura{
		Label:     "Combustion",
		ActionID:  actionID,
		Duration:  core.NeverExpires,
		MaxStacks: 20,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			numCrits = 0
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			cd.Use(sim)
			mage.UpdateMajorCooldowns()
		},
		OnStacksChange: func(aura *core.Aura, sim *core.Simulation, oldStacks int32, newStacks int32) {
			critMod.UpdateFloatValue(10 * float64(newStacks))
			critMod.Activate()
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() || numCrits >= CombustionCrits || !spell.SpellSchool.Matches(core.SpellSchoolFire) || !spell.Flags.Matches(SpellFlagMage) {
				return
			}

			// Ignite never consumes a crit stack, its damage isn't a cast of its own.
			if spell.SpellCode == SpellCode_MageIgnite {
				return
			}

			// TODO: This wont work properly with flamestrike
			aura.AddStack(sim)

			if result.DidCrit() {
				numCrits++
				if numCrits == CombustionCrits {
					aura.Deactivate(sim)
				}
			}
		},
	})

	spell := mage.RegisterSpell(core.SpellConfig{
		ActionID: actionID,
		Flags:    core.SpellFlagNoOnCastComplete,
		Cast: core.CastConfig{
			CD: cd,
		},
		ExtraCastCondition: func(sim *core.Simulation, target *core.Unit) bool {
			return !mage.CombustionAura.IsActive()
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			mage.CombustionAura.Activate(sim)
			mage.CombustionAura.AddStack(sim)
		},
	})

	mage.AddMajorCooldown(core.MajorCooldown{
		Spell: spell,
		Type:  core.CooldownTypeDPS,
	})
}

func (mage *Mage) registerColdSnapCD() {
	if !mage.Talents.ColdSnap {
		return
	}

	// Grab all frost spells with a CD > 0
	var affectedSpells = []*core.Spell{}
	mage.OnSpellRegistered(func(spell *core.Spell) {
		if spell.SpellSchool.Matches(core.SpellSchoolFrost) && spell.CD.Duration > 0 {
			affectedSpells = append(affectedSpells, spell)
		}
	})

	spell := mage.RegisterSpell(core.SpellConfig{
		ActionID: core.ActionID{SpellID: 12472},
		Flags:    core.SpellFlagNoOnCastComplete,

		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    mage.NewTimer(),
				Duration: time.Duration(time.Minute * 10),
			},
		},
		ApplyEffects: func(sim *core.Simulation, _ *core.Unit, _ *core.Spell) {
			for _, spell := range affectedSpells {
				spell.CD.Reset()
			}
		},
	})

	mage.AddMajorCooldown(core.MajorCooldown{
		Spell: spell,
		Type:  core.CooldownTypeDPS,
	})
}

// Raid bosses can't be chilled or frozen, so Fingers of Frost is the only thing that gets
// Shatter and the Ice Lance bonus going on one. Shatter is folded in here because the two
// talents only ever fire together.
func (mage *Mage) applyFingersOfFrost() {
	if mage.Talents.FingersOfFrost == 0 {
		return
	}

	// The beta tooltip for rank 2 settles what the demo could not: the proc chance does not
	// scale. Both ranks give Chill effects a 15% chance; the second point buys a second
	// charge, "treats your next 2 spells cast as if the target were Frozen".
	procChance := core.TernaryFloat64(mage.Talents.FingersOfFrost > 0, .15, 0)
	// Beta client 1.60.1: three ranks, 17/33/50%.
	shatterCrit := []float64{0, 17, 33, 50}[mage.Talents.Shatter] * core.SpellCritRatingPerCritChance

	var affectedSpells []*core.Spell
	mage.OnSpellRegistered(func(spell *core.Spell) {
		if spell.Flags.Matches(SpellFlagMage) {
			affectedSpells = append(affectedSpells, spell)
		}
	})

	// Chill effects land while the mage is already part way through the next cast. That cast is
	// not the "next spell cast" the talent grants, so it is held out of the Shatter bonus and
	// doesn't spend the charge either; the cast after it gets both.
	// TODO: the demo tooltip only says "your next 1 spell cast", beta will confirm whether a cast
	// already in progress when the chill lands counts as that one.
	var inFlight *core.Spell

	mage.FingersOfFrostAura = mage.RegisterAura(core.Aura{
		Label:    "Fingers of Frost",
		ActionID: core.ActionID{SpellID: 44543},
		Duration: time.Second * 15,
		// One charge per point: rank 2 treats the next two spells as if the target were
		// frozen rather than raising the proc chance.
		MaxStacks: int32(mage.Talents.FingersOfFrost),
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			for _, spell := range affectedSpells {
				spell.BonusCritRating += shatterCrit
			}

			inFlight = nil
			if mage.IsCasting(sim) {
				for _, spell := range affectedSpells {
					if spell.ActionID.SameAction(mage.Hardcast.ActionID) {
						spell.BonusCritRating -= shatterCrit
						inFlight = spell
						break
					}
				}
			}
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			if inFlight != nil {
				inFlight.BonusCritRating += shatterCrit
				inFlight = nil
			}

			for _, spell := range affectedSpells {
				spell.BonusCritRating -= shatterCrit
			}
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if !spell.Flags.Matches(SpellFlagMage) || !spell.ProcMask.Matches(core.ProcMaskSpellDamage) {
				return
			}

			if spell == inFlight {
				spell.BonusCritRating += shatterCrit
				inFlight = nil
				return
			}

			// OnCastComplete runs after the damage is rolled, so the consuming cast keeps the
			// bonus. Each cast spends one charge; the aura falls when the last one goes.
			aura.RemoveStack(sim)
		},
	})

	mage.RegisterAura(core.Aura{
		Label:    "Fingers of Frost Trigger",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() || !spell.Flags.Matches(SpellFlagChillSpell) {
				return
			}

			if sim.Proc(procChance, "Fingers of Frost") {
				mage.FingersOfFrostAura.Activate(sim)
				mage.FingersOfFrostAura.SetStacks(sim, mage.FingersOfFrostAura.MaxStacks)
			}
		},
	})
}

// IsTargetFrozen reports whether the mage's next spell is treated as hitting a frozen target.
func (mage *Mage) IsTargetFrozen() bool {
	return mage.FingersOfFrostAura != nil && mage.FingersOfFrostAura.IsActive()
}

// The raid debuff version of Winter's Chill is a Classic mechanic, in Forever it only helps the
// mage's own Frostbolt and Ice Lance. The beta client gives 2% crit a stack, stacking once per
// talent point: "Stacks up to 5 times" at 5/5.
func (mage *Mage) applyWintersChill() {
	if mage.Talents.WintersChill == 0 {
		return
	}

	procChance := .20 * float64(mage.Talents.WintersChill)
	critMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind:      core.SpellMod_BonusCrit_Percent,
		ClassMask: SpellMaskFrostbolt | SpellMaskIceLance,
	})

	mage.WintersChillAura = mage.RegisterAura(core.Aura{
		Label:     "Winter's Chill",
		ActionID:  core.ActionID{SpellID: 28593},
		Duration:  time.Second * 15,
		MaxStacks: int32(mage.Talents.WintersChill),
		OnStacksChange: func(aura *core.Aura, sim *core.Simulation, oldStacks int32, newStacks int32) {
			critMod.UpdateFloatValue(2 * float64(newStacks))
			critMod.Activate()
		},
	})

	mage.RegisterAura(core.Aura{
		Label:    "Winters Chill Talent",
		Duration: core.NeverExpires,
		OnReset: func(aura *core.Aura, sim *core.Simulation) {
			aura.Activate(sim)
		},
		OnSpellHitDealt: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
			if !result.Landed() || !spell.SpellSchool.Matches(core.SpellSchoolFrost) {
				return
			}

			if sim.Proc(procChance, "Winters Chill") {
				mage.WintersChillAura.Activate(sim)
				mage.WintersChillAura.AddStack(sim)
			}
		},
	})
}
