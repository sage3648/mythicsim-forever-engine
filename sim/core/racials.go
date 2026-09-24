package core

import (
	"slices"
	"time"

	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/stats"
)

// Forever's racials. Where the client carries the racial (client 1.60.1.69977: Blood Fury,
// Eureka!, Berserking, Touch of the Grave and the Skyborne line) the figures are the
// client's; the rest follow the published racials guide. Against Classic, Forever drops
// every +10 resistance racial, turns the weapon skill racials into critical strike while
// the matching weapon is held, and gives each race a new passive or cooldown. See
// docs/forever_rules.md.
//
// Blood Elves and Draenei are not Forever races. They keep the racials this engine
// already had for them.
func applyRaceEffects(agent Agent) {
	character := agent.GetCharacter()

	switch character.Race {
	case proto.Race_RaceBloodElf:
		character.stats[stats.ArcaneResistance] += 5
		character.stats[stats.FireResistance] += 5
		character.stats[stats.FrostResistance] += 5
		character.stats[stats.NatureResistance] += 5
		character.stats[stats.ShadowResistance] += 5

		var actionID ActionID

		var resourceMetrics *ResourceMetrics = nil
		if resourceMetrics == nil {
			if character.HasEnergyBar() {
				actionID = ActionID{SpellID: 25046}
				resourceMetrics = character.NewEnergyMetrics(actionID)
			} else if character.HasManaBar() {
				actionID = ActionID{SpellID: 28730}
				resourceMetrics = character.NewManaMetrics(actionID)
			}
		}

		spell := character.RegisterSpell(SpellConfig{
			ActionID: actionID,
			Flags:    SpellFlagNoOnCastComplete,
			Cast: CastConfig{
				CD: Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Minute * 2,
				},
			},
			ApplyEffects: func(sim *Simulation, _ *Unit, spell *Spell) {
				if spell.Unit.HasEnergyBar() {
					spell.Unit.AddEnergy(sim, 10, resourceMetrics)
				} else if spell.Unit.HasManaBar() {
					spell.Unit.AddMana(sim, 10, resourceMetrics)
				}
			},
		})

		character.AddMajorCooldown(MajorCooldown{
			Spell:    spell,
			Type:     CooldownTypeDPS,
			Priority: CooldownPriorityLow,
			ShouldActivate: func(sim *Simulation, character *Character) bool {
				if spell.Unit.HasEnergyBar() {
					return character.CurrentEnergy() <= character.maxEnergy-10
				}
				return true
			},
		})
	case proto.Race_RaceDraenei:
		character.stats[stats.ShadowResistance] += 10

		switch character.Class {
		case proto.Class_ClassHunter, proto.Class_ClassPaladin, proto.Class_ClassWarrior:
			MakePermanent(DraneiRacialAura(character, false))
		case proto.Class_ClassMage, proto.Class_ClassPriest, proto.Class_ClassShaman:
			MakePermanent(DraneiRacialAura(character, true))
		}

		character.RegisterSpell(SpellConfig{
			ActionID:    ActionID{SpellID: 28880},
			Flags:       SpellFlagAPL | SpellFlagHelpful | SpellFlagIgnoreModifiers,
			ProcMask:    ProcMaskSpellHealing,
			SpellSchool: SpellSchoolHoly,
			DefenseType: DefenseTypeMagic,

			MaxRange: 40,

			Cast: CastConfig{
				DefaultCast: Cast{
					CastTime: time.Millisecond * 1500,
				},
				CD: Cooldown{
					Timer:    character.NewTimer(),
					Duration: time.Second * 15,
				},
			},

			DamageMultiplier: 1.0,
			ThreatMultiplier: 1.0,

			Hot: DotConfig{
				Aura: Aura{
					Label: "Gift of the Naaru" + character.Label,
				},
				NumberOfTicks:       5,
				TickLength:          time.Second * 3,
				AffectedByCastSpeed: false,
				OnTick: func(sim *Simulation, target *Unit, dot *Dot) {
					healValue := float64((35.0 + 15*CharacterLevel) / dot.ExpectedTickCount())
					dot.Spell.CalcAndDealPeriodicHealing(sim, target, healValue, dot.OutcomeTick)
				},
			},

			ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
				spell.Hot(target).Activate(sim)
			},
		})
	case proto.Race_RaceDwarf:
		// Mace Specialization moved to the Dwarves, and pays critical strike now.
		character.applyWeaponSpecializationCrit("Mace Specialization", 20864, 1, proto.WeaponType_WeaponTypeMace)
		// Big Game Hunter: Beast Slaying's +5% against Beasts.
		character.mobTypeDamageAura(proto.MobType_MobTypeBeast, 1.05)
		character.registerStoneform()
	case proto.Race_RaceGnome:
		// Expansive Mind raises the resource pool itself now rather than Intellect. Only the
		// mana half is modelled; rage and energy have no max stat here.
		character.MultiplyStat(stats.Mana, 1.05)
		character.registerEureka()
	case proto.Race_RaceHuman:
		character.MultiplyStat(stats.Spirit, 1.05)
		// Mace Specialization moved to the Dwarves.
		character.applyWeaponSpecializationCrit("Sword Specialization", 20597, 2, proto.WeaponType_WeaponTypeSword)
	case proto.Race_RaceNightElf:
		// Quickness
		character.PseudoStats.BaseDodgeChance += 0.01
		character.registerElunesLight()
	case proto.Race_RaceOrc:
		// Command is not one of the orc's four: Shatter Curse took its place, and dispelling a
		// curse is nothing the sim measures.
		character.applyWeaponSpecializationCrit("Axe Specialization", 20574, 1, proto.WeaponType_WeaponTypeAxe)
		character.registerBloodFury()
	case proto.Race_RaceTauren:
		// Endurance carries a point of hit alongside the health.
		character.MultiplyStat(stats.Health, 1.05)
		character.AddStat(stats.PhysicalHitPercent, 1)
		character.AddStat(stats.SpellHitPercent, 1)
	case proto.Race_RaceTroll:
		// Forever's troll keeps neither ranged weapon specialization; Rapid Regeneration and
		// Regeneration took their place.
		character.mobTypeDamageAura(proto.MobType_MobTypeBeast, 1.05)
		character.registerBerserking()
	case proto.Race_RaceUndead:
		character.registerTouchOfTheGrave()
	case proto.Race_RaceSkyborneHighOrder, proto.Race_RaceSkyborneWindshaper:
		// Both halves share one racial skill line in the client (2980), so they sim alike. Its
		// only combat effects are the two passives below. Read Ley Line (regeneration), Walk on
		// Air and Skysight (movement) do nothing the sim measures.

		// Wind Blessed (1259710)
		character.PseudoStats.MeleeSpeedMultiplier *= 1.01
		character.PseudoStats.RangedSpeedMultiplier *= 1.01
		character.PseudoStats.CastSpeedMultiplier *= 1.01

		// Elemental Insight (1259707)
		character.mobTypeDamageAura(proto.MobType_MobTypeElemental, 1.05)
	}
}

// Under Forever the weapon skill racials pay critical strike instead, and only while the
// matching weapon is held in either hand. The tooltips read "spell and ability critical
// strike chance", so it lands on both pools.
func (character *Character) applyWeaponSpecializationCrit(label string, spellID int32, critPercent float64, weaponTypes ...proto.WeaponType) {
	holdsWeapon := func() bool {
		for _, weapon := range []*Item{character.MainHand(), character.OffHand()} {
			if weapon != nil && weapon.ID != 0 && slices.Contains(weaponTypes, weapon.WeaponType) {
				return true
			}
		}
		return false
	}

	aura := character.RegisterAura(Aura{
		Label:      label,
		ActionID:   ActionID{SpellID: spellID},
		Duration:   NeverExpires,
		BuildPhase: Ternary(holdsWeapon(), CharacterBuildPhaseBase, CharacterBuildPhaseNone),
	}).AttachStatsBuff(stats.Stats{
		stats.PhysicalCritPercent: critPercent,
		stats.SpellCritPercent:    critPercent,
	})

	if holdsWeapon() {
		MakePermanent(aura)
	}

	character.RegisterItemSwapCallback(AllWeaponSlots(), func(sim *Simulation, _ proto.ItemSlot) {
		if holdsWeapon() {
			aura.Activate(sim)
		} else {
			aura.Deactivate(sim)
		}
	})
}

// Stoneform, the dwarf's cooldown: 10% armor for 8 sec on a 3 min cooldown, as in Classic. It
// is a survival cooldown that is only ever cast from the rotation, so a dwarf DPS does not
// spend a global cooldown on it.
func (character *Character) registerStoneform() {
	actionID := ActionID{SpellID: 20594}

	stoneFormAura := character.NewTemporaryStatsAuraWrapped("Stoneform", actionID, stats.Stats{}, time.Second*8, func(aura *Aura) {
		aura.ApplyOnGain(func(aura *Aura, sim *Simulation) {
			character.PseudoStats.ArmorMultiplier *= 1.1
		})
		aura.ApplyOnExpire(func(aura *Aura, sim *Simulation) {
			character.PseudoStats.ArmorMultiplier /= 1.1
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

		RelatedSelfBuff: stoneFormAura.Aura,
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeSurvival,
		ShouldActivate: func(_ *Simulation, _ *Character) bool {
			// Only castable with a manual APL action.
			return false
		},
	})
}

// Blood Fury under Forever (client 20572): 10% more melee attack power, ranged attack power
// and spell power for 15 s on a two minute cooldown, off the global cooldown. The three
// effects are percentage modifiers (auras 166, 167 and 317), so they scale whatever the orc
// gains while the buff is up rather than a snapshot taken when it is cast.
//
// Spell power here is split into generic and school stats. The school stats cannot carry a
// stat dependency, so 10% of them is added when the buff lands; they only come from gear and
// elixirs, which do not change mid-fight.
func (character *Character) registerBloodFury() {
	actionID := ActionID{SpellID: 20572}

	aura := character.RegisterAura(Aura{
		Label:    "Blood Fury",
		ActionID: actionID,
		Duration: time.Second * 15,
	})
	for _, stat := range []stats.Stat{stats.AttackPower, stats.RangedAttackPower, stats.SpellDamage, stats.HealingPower} {
		aura.AttachStatDependency(character.NewDynamicMultiplyStat(stat, 1.1))
	}

	schoolPowers := []stats.Stat{stats.ArcaneDamage, stats.FireDamage, stats.FrostDamage, stats.HolyDamage, stats.NatureDamage, stats.ShadowDamage}
	var schoolBonus stats.Stats
	aura.ApplyOnGain(func(aura *Aura, sim *Simulation) {
		schoolBonus = stats.Stats{}
		for _, stat := range schoolPowers {
			schoolBonus[stat] = aura.Unit.GetStat(stat) * 0.1
		}
		aura.Unit.AddStatsDynamic(sim, schoolBonus)
	})
	aura.ApplyOnExpire(func(aura *Aura, sim *Simulation) {
		aura.Unit.AddStatsDynamic(sim, schoolBonus.Invert())
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
		},

		RelatedSelfBuff: aura,
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
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
		// OnCastComplete runs after the cast's effects, so the ability that spends the last
		// charge still gets the bonus.
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

		RelatedSelfBuff: aura,
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
}

// Elune's Light, the night elf's Forever racial cooldown: 10% critical strike with spells
// and attacks for 15 seconds on a three minute cooldown.
func (character *Character) registerElunesLight() {
	actionID := ActionID{SpellID: 460520}

	aura := character.RegisterAura(Aura{
		Label:    "Elune's Light",
		ActionID: actionID,
		Duration: time.Second * 15,
	}).AttachStatsBuff(stats.Stats{
		stats.PhysicalCritPercent: 10,
		stats.SpellCritPercent:    10,
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

		RelatedSelfBuff: aura,
	})

	character.AddMajorCooldown(MajorCooldown{
		Spell: spell,
		Type:  CooldownTypeDPS,
	})
}

// Berserking under Forever (client 20554): 10% melee haste, ranged haste and cast speed for
// 10 s on a three minute cooldown, whatever the troll's health. It costs nothing and is off
// the global cooldown. The action id is the one the previous engine line gave its fixed 10%
// option, so rotations saved against that line still find it.
func (character *Character) registerBerserking() {
	actionID := ActionID{SpellID: 26297, Tag: 2}

	aura := character.RegisterAura(Aura{
		Label:    "Berserking (10)",
		ActionID: actionID,
		Duration: time.Second * 10,
	}).AttachMultiplyAttackSpeed(1.1).AttachMultiplyCastSpeed(1.1)

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

		RelatedSelfBuff: aura,
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
		// The drain is a proc off another hit, so it must not feed the procs that spawned it
		// or two undead attacks would chain into each other.
		Flags: SpellFlagNoOnCastComplete | SpellFlagPassiveSpell,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *Simulation, target *Unit, spell *Spell) {
			result := spell.CalcAndDealDamage(sim, target, character.GetStat(stats.Health)*0.05, spell.OutcomeMagicHit)

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

// Troll Beast Slaying, Dwarf Big Game Hunter and Skyborne Elemental Insight: more damage, and
// a larger critical strike bonus, against one creature type.
func (character *Character) mobTypeDamageAura(mobType proto.MobType, multiplier float64) {
	character.Env.RegisterPostFinalizeEffect(func() {
		for _, at := range character.AttackTables {
			if at.Defender.MobType == mobType {
				at.DamageDealtMultiplier *= multiplier
				at.CritMultiplier *= multiplier
			}
		}
	})
}

// GetFaction derives the character's faction from its race. The Skyborne choose a side at
// character creation, so each half has its own race: High Order is Alliance, Windshaper Horde.
func (character *Character) GetFaction() proto.Faction {
	switch character.Race {
	case proto.Race_RaceHuman, proto.Race_RaceDwarf, proto.Race_RaceGnome, proto.Race_RaceNightElf,
		proto.Race_RaceDraenei, proto.Race_RaceSkyborneHighOrder:
		return proto.Faction_Alliance
	case proto.Race_RaceOrc, proto.Race_RaceTroll, proto.Race_RaceTauren, proto.Race_RaceUndead,
		proto.Race_RaceBloodElf, proto.Race_RaceSkyborneWindshaper:
		return proto.Faction_Horde
	default:
		return proto.Faction_Unknown
	}
}
