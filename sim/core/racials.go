package core

import (
	"fmt"
	"slices"
	"time"

	"github.com/wowsims/classic/sim/core/proto"
	"github.com/wowsims/classic/sim/core/stats"
)

func applyRaceEffects(agent Agent) {
	character := agent.GetCharacter()

	// Forever drops every +10 resistance racial and turns the weapon skill racials into
	// critical strike while that weapon is held. Races also pick up a new passive each.
	forever := character.Env.IsForever()

	switch character.Race {
	case proto.Race_RaceDwarf:
		if forever {
			character.AddWeaponSpecializationCrit(1, proto.WeaponType_WeaponTypeMace)
			character.beastSlayingAura(1.05)
		} else {
			character.AddStat(stats.FrostResistance, 10)
			character.GunSpecializationAura()
		}

		actionID := ActionID{SpellID: 20594}

		statDep := character.NewDynamicMultiplyStat(stats.Armor, 1.1)
		stoneFormAura := character.NewTemporaryStatsAuraWrapped("Stoneform", actionID, stats.Stats{}, time.Second*8, func(aura *Aura) {
			aura.ApplyOnGain(func(aura *Aura, sim *Simulation) {
				aura.Unit.EnableDynamicStatDep(sim, statDep)
			})
			aura.ApplyOnExpire(func(aura *Aura, sim *Simulation) {
				aura.Unit.DisableDynamicStatDep(sim, statDep)
			})
		})

		spell := character.RegisterSpell(SpellConfig{
			ActionID: actionID,
			Flags:    SpellFlagNoOnCastComplete,
			Cast: CastConfig{
				CD: Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Minute * 3,
				},
			},
			ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
				stoneFormAura.Activate(sim)
			},
		})

		character.AddMajorCooldown(MajorCooldown{
			Spell: spell,
			Type:  CooldownTypeSurvival,
			ShouldActivate: func(s *Simulation, c *Character) bool {
				// Only castable with manual APL Action
				return false
			},
		})
	case proto.Race_RaceGnome:
		if forever {
			// Expansive Mind raises the resource pool itself now rather than Intellect.
			// Only the mana half is modelled; rage and energy have no max stat here.
			character.MultiplyStat(stats.Mana, 1.05)
			character.registerEureka()
		} else {
			character.AddStat(stats.ArcaneResistance, 10)
			character.MultiplyStat(stats.Intellect, 1.05)
		}
	case proto.Race_RaceHuman:
		character.MultiplyStat(stats.Spirit, 1.05)
		if forever {
			// Mace Specialization moved to the Dwarves.
			character.AddWeaponSpecializationCrit(2, proto.WeaponType_WeaponTypeSword)
		} else {
			character.SwordSpecializationAura()
			character.MaceSpecializationAura()
		}
	case proto.Race_RaceNightElf:
		if forever {
			character.registerElunesLight()
		} else {
			character.AddStat(stats.NatureResistance, 10)
		}
		character.AddStat(stats.Dodge, 1)
		// TODO: Shadowmeld?
	case proto.Race_RaceOrc:
		if forever {
			character.AddWeaponSpecializationCrit(1, proto.WeaponType_WeaponTypeAxe)
		} else {
			character.AxeSpecializationAura()
		}

		// Command is gone under Forever; Shatter Curse takes its place in the Orc's four,
		// and dispelling a curse is nothing the sim measures.
		if !forever && (character.Class == proto.Class_ClassHunter || character.Class == proto.Class_ClassWarlock) {
			// Command Damage dealt by Hunter and Warlock pets increased by 5%
			for _, pet := range character.Pets {
				if !pet.IsGuardian() {
					pet.PseudoStats.DamageDealtMultiplier *= 1.05
				}
			}
		}

		// Blood Fury
		actionID := ActionID{SpellID: 20572}
		var bloodFuryAura *Aura
		castConfig := CastConfig{
			CD: Cooldown{
				Timer:    character.NewTimer(),
				Duration: time.Minute * 2,
			},
		}
		if forever {
			bloodFuryAura = character.newForeverBloodFuryAura(actionID)
		} else {
			var bloodFuryAP float64
			bloodFuryAura = character.RegisterAura(Aura{
				Label:    "Blood Fury",
				ActionID: actionID,
				Duration: time.Second * 15,
				// Tooltip is misleading; ap bonus is base AP plus AP from current strength, does not include +attackpower on items/buffs
				OnGain: func(aura *Aura, sim *Simulation) {
					bloodFuryAP = (character.GetBaseStats()[stats.AttackPower] + (character.GetStat(stats.Strength) * APPerStrength[character.Class]) + (character.GetStat(stats.Agility) * APPerAgility[character.Class])) * 0.25
					character.AddStatDynamic(sim, stats.AttackPower, bloodFuryAP)
				},
				OnExpire: func(aura *Aura, sim *Simulation) {
					character.AddStatDynamic(sim, stats.AttackPower, -bloodFuryAP)
				},
			})
			castConfig.DefaultCast = Cast{GCD: GCDDefault}
		}

		spell := character.RegisterSpell(SpellConfig{
			ActionID: actionID,
			Flags:    SpellFlagNoOnCastComplete,
			Cast:     castConfig,
			ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
				bloodFuryAura.Activate(sim)
			},
		})

		character.AddMajorCooldown(MajorCooldown{
			Spell: spell,
			Type:  CooldownTypeDPS,
		})
	case proto.Race_RaceTauren:
		character.MultiplyStat(stats.Health, 1.05)
		if forever {
			// Endurance carries a point of hit alongside the health.
			character.AddStat(stats.MeleeHit, 1*MeleeHitRatingPerHitChance)
			character.AddStat(stats.SpellHit, 1*SpellHitRatingPerHitChance)
		} else {
			character.AddStat(stats.NatureResistance, 10)
		}
	case proto.Race_RaceTroll:
		// Forever's troll has four racials and neither ranged weapon specialization is
		// among them; Rapid Regeneration and Regeneration took their place.
		if !forever {
			character.BowSpecializationAura()
			character.ThrownSpecializationAura()
		}

		character.beastSlayingAura(1.05)

		// Berserking
		berserkingTimer := character.NewTimer()
		if forever {
			makeForeverBerserkingCooldown(character, berserkingTimer)
		} else {
			// Baseline cooldown
			makeBerserkingCooldown(character, 0, berserkingTimer)
			// Hard-coded percentage cooldown options
			makeBerserkingCooldown(character, .1, berserkingTimer)
			makeBerserkingCooldown(character, .15, berserkingTimer)
			makeBerserkingCooldown(character, .2, berserkingTimer)
			makeBerserkingCooldown(character, .25, berserkingTimer)
			makeBerserkingCooldown(character, .3, berserkingTimer)
		}
	case proto.Race_RaceUndead:
		if !forever {
			character.AddStat(stats.ShadowResistance, 10)
		} else {
			character.registerTouchOfTheGrave()
		}
	case proto.Race_RaceSkyborneHighOrder, proto.Race_RaceSkyborneWindshaper:
		// The Skyborne are a Forever race, so under Classic rules they have no racials to
		// grant. A saved setting can still name one - the race picker offers whatever the
		// spec allows - and without this guard that saved race would carry Forever's haste
		// and elemental damage into a Classic sim.
		if !forever {
			break
		}

		// Both halves share one racial skill line in the client (2980), so they sim alike.
		// Its only combat effects are the two passives below. Read Ley Line (regeneration),
		// Walk on Air and Skysight (movement) do nothing the sim measures.

		// Wind Blessed (1259710)
		character.PseudoStats.MeleeSpeedMultiplier *= 1.01
		character.PseudoStats.RangedSpeedMultiplier *= 1.01
		character.PseudoStats.CastSpeedMultiplier *= 1.01

		// Elemental Insight (1259707)
		character.mobTypeDamageAura(proto.MobType_MobTypeElemental, 1.05)
	}
}

// Eureka!, the gnome's Forever racial cooldown. The client has one per gnome class: warrior
// 1259813, rogue 1259812, priest 1259823, mage 1259817 and warlock 1259821. Each lasts 15 s
// on a two minute cooldown, off the global cooldown, with three charges. While it is up, the
// class's abilities deal 10% more damage, direct and periodic, and cost less: 40% less rage,
// 20% less energy, 50% less mana for mages and warlocks, 15% less for priests.
//
// Both halves are spell modifiers, so they reach abilities and spells but not white swings.
// A charge goes to any of those abilities; the proc mask (87376) leaves out auto attacks.
func (character *Character) registerEureka() {
	var actionID ActionID
	var costReduction float64
	switch character.Class {
	case proto.Class_ClassWarrior:
		actionID, costReduction = ActionID{SpellID: 1259813}, 0.40
	case proto.Class_ClassRogue:
		actionID, costReduction = ActionID{SpellID: 1259812}, 0.20
	case proto.Class_ClassMage:
		actionID, costReduction = ActionID{SpellID: 1259817}, 0.50
	case proto.Class_ClassWarlock:
		actionID, costReduction = ActionID{SpellID: 1259821}, 0.50
	case proto.Class_ClassPriest:
		actionID, costReduction = ActionID{SpellID: 1259823}, 0.15
	default:
		// No other class can be a gnome, and the client has no Eureka! for one.
		return
	}

	damageMod := character.AddDynamicMod(SpellModConfig{
		Kind:       SpellMod_DamageDone_Pct,
		ProcMask:   ProcMaskSpecial,
		FloatValue: 0.10,
	})
	costMod := character.AddDynamicMod(SpellModConfig{
		Kind:       SpellMod_PowerCost_Pct_Add,
		ProcMask:   ProcMaskSpecial,
		FloatValue: -costReduction,
	})

	aura := character.RegisterAura(Aura{
		Label:     "Eureka!",
		ActionID:  actionID,
		Duration:  time.Second * 15,
		MaxStacks: 3,
		OnGain: func(aura *Aura, sim *Simulation) {
			damageMod.Activate()
			costMod.Activate()
		},
		OnExpire: func(aura *Aura, sim *Simulation) {
			damageMod.Deactivate()
			costMod.Deactivate()
		},
		OnStacksChange: func(aura *Aura, sim *Simulation, _ int32, newStacks int32) {
			if newStacks == 0 {
				aura.Deactivate(sim)
			}
		},
		// OnCastComplete runs after the cast's effects, so the ability that spends the
		// last charge still gets the bonus.
		OnCastComplete: func(aura *Aura, sim *Simulation, spell *Spell) {
			if spell.ProcMask.Matches(ProcMaskSpecial) {
				aura.RemoveStack(sim)
			}
		},
	})

	spell := character.RegisterSpell(SpellConfig{
		ActionID: actionID,
		Flags:    SpellFlagNoOnCastComplete,
		Cast: CastConfig{
			CD: Cooldown{
				Timer:    character.NewTimer(),
				Duration: time.Minute * 2,
			},
		},
		ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
			aura.Activate(sim)
			aura.SetStacks(sim, aura.MaxStacks)
		},
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
}

// Touch of the Grave, the undead's Forever racial, which replaces Classic's Shadow
// Resistance: spells and attacks have a chance to drain health from the target.
// Client: 1260189 (warrior, paladin, rogue) procs 5% of the time, 1260201 (priest, mage,
// warlock) 10%, both with a 1 s proc cooldown (SpellAuraOptions). The drain, 1260198, is a
// health leech of 5% of the caster's maximum health. It is taken to be Shadow damage that
// can be resisted; whether it can crit is unknown.
func (character *Character) registerTouchOfTheGrave() {
	procChance := 0.05
	switch character.Class {
	case proto.Class_ClassPriest, proto.Class_ClassMage, proto.Class_ClassWarlock:
		procChance = 0.10
	}
	actionID := ActionID{SpellID: 1260198}
	healthMetrics := character.NewHealthMetrics(actionID)

	drain := character.RegisterSpell(SpellConfig{
		ActionID:    actionID,
		SpellSchool: SpellSchoolShadow,
		DefenseType: DefenseTypeMagic,
		ProcMask:    ProcMaskEmpty,
		// The drain is a proc off another hit, so it must not feed the procs that spawned
		// it or two undead attacks would chain into each other.
		Flags: SpellFlagNoOnCastComplete | SpellFlagPassiveSpell,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
			result := spell.CalcAndDealDamage(sim, target, character.MaxHealth()*0.05, spell.OutcomeMagicHit)

			// Only the specs that track a health bar can be healed; for everyone else the
			// drain is still damage, it just has nothing to return the health to.
			if result.Landed() && character.HasHealthBar() {
				character.GainHealth(sim, result.Damage, healthMetrics)
			}
		},
	})

	icd := Cooldown{Timer: character.NewTimer(), Duration: time.Second}
	MakePermanent(character.RegisterAura(Aura{
		Label:    "Touch of the Grave",
		ActionID: actionID,
		OnSpellHitDealt: func(_ *Aura, sim *Simulation, spell *Spell, result *SpellResult) {
			if !result.Landed() || spell == drain || !icd.IsReady(sim) {
				return
			}
			if sim.RandomFloat("Touch of the Grave") < procChance {
				icd.Use(sim)
				drain.Cast(sim, result.Target)
			}
		},
	}))
}

// Elune's Light, the night elf's Forever racial cooldown: 10% critical strike for 15
// seconds on a three minute cooldown.
func (character *Character) registerElunesLight() {
	actionID := ActionID{SpellID: 460520}

	aura := character.RegisterAura(Aura{
		Label:    "Elune's Light",
		ActionID: actionID,
		Duration: time.Second * 15,
		OnGain: func(aura *Aura, sim *Simulation) {
			character.AddStatsDynamic(sim, stats.Stats{
				stats.MeleeCrit: 10 * CritRatingPerCritChance,
				stats.SpellCrit: 10 * SpellCritRatingPerCritChance,
			})
		},
		OnExpire: func(aura *Aura, sim *Simulation) {
			character.AddStatsDynamic(sim, stats.Stats{
				stats.MeleeCrit: -10 * CritRatingPerCritChance,
				stats.SpellCrit: -10 * SpellCritRatingPerCritChance,
			})
		},
	})

	spell := character.RegisterSpell(SpellConfig{
		ActionID: actionID,
		Flags:    SpellFlagNoOnCastComplete,
		Cast: CastConfig{
			CD: Cooldown{
				Timer:    character.NewTimer(),
				Duration: time.Minute * 3,
			},
		},
		ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
			aura.Activate(sim)
		},
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
}

// Blood Fury under Forever (client 20572): 10% more melee attack power, ranged attack
// power and spell power for 15 s on a two minute cooldown, off the global cooldown. The
// three effects are percentage modifiers (auras 166, 167 and 317), so they scale whatever
// the orc gains while the buff is up rather than a snapshot taken when it is cast.
//
// Spell power here is split into generic and school stats. The school stats cannot carry a
// stat dependency, so 10% of them is added when the buff lands; they only come from gear
// and elixirs, which do not change mid-fight.
func (character *Character) newForeverBloodFuryAura(actionID ActionID) *Aura {
	var deps []*stats.StatDependency
	for _, stat := range []stats.Stat{stats.AttackPower, stats.RangedAttackPower, stats.SpellPower, stats.SpellDamage} {
		deps = append(deps, character.NewDynamicMultiplyStat(stat, 1.1))
	}
	schoolPowers := []stats.Stat{stats.ArcanePower, stats.FirePower, stats.FrostPower, stats.HolyPower, stats.NaturePower, stats.ShadowPower}

	var schoolBonus stats.Stats
	return character.RegisterAura(Aura{
		Label:    "Blood Fury",
		ActionID: actionID,
		Duration: time.Second * 15,
		OnGain: func(aura *Aura, sim *Simulation) {
			for _, dep := range deps {
				aura.Unit.EnableDynamicStatDep(sim, dep)
			}
			schoolBonus = stats.Stats{}
			for _, stat := range schoolPowers {
				schoolBonus[stat] = aura.Unit.GetStat(stat) * 0.1
			}
			aura.Unit.AddStatsDynamic(sim, schoolBonus)
		},
		OnExpire: func(aura *Aura, sim *Simulation) {
			for _, dep := range deps {
				aura.Unit.DisableDynamicStatDep(sim, dep)
			}
			aura.Unit.AddStatsDynamic(sim, schoolBonus.Invert())
		},
	})
}

// Troll Beast Slaying, and Dwarf Big Game Hunter under Forever.
func (character *Character) beastSlayingAura(multiplier float64) {
	character.mobTypeDamageAura(proto.MobType_MobTypeBeast, multiplier)
}

func (character *Character) mobTypeDamageAura(mobType proto.MobType, multiplier float64) {
	character.Env.RegisterPostFinalizeEffect(func() {
		for _, t := range character.Env.Encounter.Targets {
			if t.MobType == mobType {
				for _, at := range character.AttackTables[t.UnitIndex] {
					at.DamageDealtMultiplier *= multiplier
					at.CritMultiplier *= multiplier
				}
			}
		}
	})
}

// If customPercentage is 0, use the baseline Berserking calculations from health missing
// otherwise create a cooldown hard-coded to the custom percentage.
func makeBerserkingCooldown(character *Character, customPercentage float64, timer *Timer) {
	actionID := ActionID{SpellID: 26297, Tag: int32(customPercentage * 20)}

	label := "Berserking"
	if customPercentage != 0 {
		label = fmt.Sprintf("%s (%d)", label, int(customPercentage*100))
	}

	calcBerserkingPct := func() float64 {
		if customPercentage != 0 {
			return customPercentage
		}
		// from 10% at full health to 30% at 40% or less health
		switch hp := character.CurrentHealthPercent(); {
		case hp >= 1:
			return 0.1
		case hp <= 0.4:
			return 0.3
		default:
			return 0.1 + (1-hp)/3
		}
	}

	var berserkingAura *Aura
	var berserkingHaste float64
	if character.HasManaBar() {
		// Mana-using classes gain a flat % reduction in attack and cast speed
		berserkingAura = character.RegisterAura(Aura{
			Label:    label,
			ActionID: actionID,
			Duration: time.Second * 10,
			OnGain: func(aura *Aura, sim *Simulation) {
				berserkingHaste = 1 / (1 - calcBerserkingPct())

				character.MultiplyCastSpeed(berserkingHaste)
				character.MultiplyAttackSpeed(sim, berserkingHaste)

				if sim.Log != nil {
					character.Log(sim, "Berserking increased attack and casting speed by %.2f%% (%.2f%% hp)", berserkingHaste*100-100, character.CurrentHealthPercent()*100)
				}
			},
			OnExpire: func(aura *Aura, sim *Simulation) {
				character.MultiplyCastSpeed(1 / berserkingHaste)
				character.MultiplyAttackSpeed(sim, 1/berserkingHaste)
			},
		})
	} else {
		// Non-mana bar classes gain a flat % reduction in attack and cast speed
		berserkingAura = character.RegisterAura(Aura{
			Label:    label,
			ActionID: actionID,
			Duration: time.Second * 10,
			OnGain: func(aura *Aura, sim *Simulation) {
				berserkingHaste = 1 + calcBerserkingPct()

				character.MultiplyAttackSpeed(sim, berserkingHaste)

				if sim.Log != nil {
					character.Log(sim, "Berserking increased attack speed by %.2f%% (%.2f%% hp)", berserkingHaste*100-100, character.CurrentHealthPercent()*100)
				}
			},
			OnExpire: func(aura *Aura, sim *Simulation) {
				character.MultiplyAttackSpeed(sim, 1/berserkingHaste)
			},
		})
	}

	config := SpellConfig{
		ActionID: actionID,

		Cast: CastConfig{
			CD: Cooldown{
				Timer:    timer,
				Duration: time.Minute * 3,
			},
		},

		ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
			berserkingAura.Activate(sim)
		},
	}

	switch {
	case character.HasManaBar():
		config.ManaCost = ManaCostOptions{BaseCost: 0.07}
	case character.HasRageBar():
		config.RageCost = RageCostOptions{Cost: 5}
	case character.HasEnergyBar():
		config.EnergyCost = EnergyCostOptions{Cost: 10}
	}

	berserkingSpell := character.RegisterSpell(config)

	character.AddMajorCooldown(MajorCooldown{
		Spell: berserkingSpell,
		Type:  CooldownTypeDPS,
	})
}

// Berserking under Forever (client 20554): 10% melee haste, ranged haste and cast speed
// for 10 s on a three minute cooldown, whatever the troll's health. It costs nothing and is
// off the global cooldown. The action id is the old fixed 10% option's, so saved rotations
// still find it.
func makeForeverBerserkingCooldown(character *Character, timer *Timer) {
	actionID := ActionID{SpellID: 26297, Tag: 2}

	aura := character.RegisterAura(Aura{
		Label:    "Berserking (10)",
		ActionID: actionID,
		Duration: time.Second * 10,
		OnGain: func(aura *Aura, sim *Simulation) {
			character.MultiplyCastSpeed(1.1)
			character.MultiplyAttackSpeed(sim, 1.1)
		},
		OnExpire: func(aura *Aura, sim *Simulation) {
			character.MultiplyCastSpeed(1 / 1.1)
			character.MultiplyAttackSpeed(sim, 1/1.1)
		},
	})

	spell := character.RegisterSpell(SpellConfig{
		ActionID: actionID,
		Flags:    SpellFlagNoOnCastComplete,
		Cast: CastConfig{
			CD: Cooldown{
				Timer:    timer,
				Duration: time.Minute * 3,
			},
		},
		ApplyEffects: func(sim *Simulation, _ *Unit, _ *Spell) {
			aura.Activate(sim)
		},
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
}

func (character *Character) GetFaction() proto.Faction {
	if slices.Contains([]proto.Race{proto.Race_RaceHuman, proto.Race_RaceDwarf, proto.Race_RaceGnome, proto.Race_RaceNightElf, proto.Race_RaceSkyborneHighOrder}, character.Race) {
		return proto.Faction_Alliance
	} else if slices.Contains([]proto.Race{proto.Race_RaceOrc, proto.Race_RaceTroll, proto.Race_RaceTauren, proto.Race_RaceUndead, proto.Race_RaceSkyborneWindshaper}, character.Race) {
		return proto.Faction_Horde
	} else {
		return proto.Faction_Unknown
	}
}
