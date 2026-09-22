package warlock

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const BaneOfAgonyRanks = 6

func (warlock *Warlock) getBaneOfAgonyBaseConfig(rank int) core.SpellConfig {
	// Beta client 1.60.1: 0.133 per tick at every rank, and the average tick roughly halved. Spell ID,
	// cost, school, tick, tick count and coefficient come from the client table.
	row := spellData.BaneOfAgony.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := periodic.Tick * (1 + .05*float64(warlock.Talents.ImprovedBaneOfAgony))
	level := [BaneOfAgonyRanks + 1]int{0, 8, 18, 28, 38, 48, 58}[rank]

	snapshotBaseDmgNoBonus := 0.0

	return core.SpellConfig{
		SpellCode:      SpellCode_WarlockBaneOfAgony,
		ClassSpellMask: SpellMaskBaneOfAgony,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | core.SpellFlagPureDot | WarlockFlagAffliction,
		ProcMask:       core.ProcMaskSpellDamage,
		RequiredLevel:  level,
		Rank:           rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		CritDamageBonus: 0,

		DamageMultiplierAdditive: 1,
		DamageMultiplier:         1,
		ThreatMultiplier:         1,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "BaneofAgony-" + warlock.Label + strconv.Itoa(rank),
			},
			NumberOfTicks:    periodic.NumberOfTicks,
			TickLength:       periodic.TickLength,
			BonusCoefficient: roundCoef(periodic.Coef),

			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				baseDmg := baseDamage

				if warlock.AmplifyCurseAura.IsActive() {
					baseDmg *= 1.5
					warlock.AmplifyCurseAura.Deactivate(sim)
				}

				// BoA starts with 50% base damage, but bonus from spell power is not changed.
				// Every 4 ticks this base damage is added again, resulting in 150% base damage for the last 4 ticks
				snapshotBaseDmgNoBonus = baseDmg * 0.5

				dot.Snapshot(target, snapshotBaseDmgNoBonus, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
				if dot.TickCount%4 == 0 { // BoA ramp up
					dot.SnapshotBaseDamage += snapshotBaseDmgNoBonus
				}
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)

				if activeBane := warlock.ActiveBaneAura.Get(target); activeBane != nil && activeBane != dot.Aura {
					activeBane.Deactivate(sim)
				}

				dot.Apply(sim)
				warlock.ActiveBaneAura[target.UnitIndex] = dot.Aura
			}
			spell.DealOutcome(sim, result)
		},
	}
}

func (warlock *Warlock) registerBaneOfAgonySpell() {
	warlock.BaneOfAgony = make([]*core.Spell, 0)
	for rank := 1; rank <= BaneOfAgonyRanks; rank++ {
		config := warlock.getBaneOfAgonyBaseConfig(rank)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.BaneOfAgony = append(warlock.BaneOfAgony, warlock.GetOrRegisterSpell(config))
		}
	}
}

func (warlock *Warlock) registerCurseOfRecklessnessSpell() {
	playerLevel := warlock.Level

	warlock.CurseOfRecklessnessAuras = warlock.NewEnemyAuraArray(core.CurseOfRecklessnessAura)

	rank := map[int32]int{
		25: 1,
		40: 2,
		50: 3,
		60: 4,
	}[playerLevel]
	if rank == 0 {
		return
	}
	// Spell ID, cost and school from the client table
	row := spellData.CurseOfRecklessness.ByRank(int32(rank))

	warlock.CurseOfRecklessness = warlock.RegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: row.SpellID},
		SpellSchool: row.SpellSchool,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       core.SpellFlagAPL | WarlockFlagAffliction,
		Rank:        rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		ThreatMultiplier: 1,
		FlatThreatBonus:  156,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				aura := warlock.CurseOfRecklessnessAuras.Get(target)
				if activeCurse := warlock.ActiveCurseAura.Get(target); activeCurse != nil && activeCurse != aura {
					activeCurse.Deactivate(sim)
				}

				warlock.ActiveCurseAura[target.UnitIndex] = aura
				warlock.ActiveCurseAura.Get(target).Activate(sim)
			}
		},

		RelatedAuras: []core.AuraArray{warlock.CurseOfRecklessnessAuras},
	})
}

func (warlock *Warlock) registerCurseOfElementsSpell() {
	playerLevel := warlock.Level
	if playerLevel < 40 {
		return
	}

	// Beta client 1.60.1: Curse of Shadow is gone from the spellbook and Curse of the Elements took it
	// over, reducing every magic resistance and raising all magic damage taken (school mask 126 against
	// Classic's Fire and Frost). Its ranks are new ids learned at 30, 40 and 50, the last at Classic's
	// top rank values of 75 resistance and 10%. The raid debuffs in core only split that into Fire and
	// Frost plus Shadow and Arcane, so the curse applies both; Nature and Holy are not covered.
	rank := map[int32]int{
		40: 3,
		50: 4,
		60: 4,
	}[playerLevel]
	if rank == 0 {
		return
	}
	// Cost, school and duration from the client table. The id stays spelled out: the table's ranks 1
	// and 2 (440892, 1311676) are never registered, and spell_sources_test.go would file them as ours.
	row := spellData.CurseOfTheElements.ByRank(int32(rank))
	spellID := map[int]int32{3: 1311677, 4: 1311680}[rank]

	elementsAuras := warlock.NewEnemyAuraArray(core.CurseOfElementsAura)
	shadowAuras := warlock.NewEnemyAuraArray(core.CurseOfShadowAura)
	warlock.CurseOfElementsAuras = warlock.NewEnemyAuraArray(func(unit *core.Unit) *core.Aura {
		debuffs := []*core.Aura{elementsAuras.Get(unit), shadowAuras.Get(unit)}
		return unit.RegisterAura(core.Aura{
			Label:    "Curse of the Elements-" + warlock.Label,
			ActionID: core.ActionID{SpellID: spellID},
			Duration: row.Duration,
			OnGain: func(aura *core.Aura, sim *core.Simulation) {
				for _, debuff := range debuffs {
					debuff.Activate(sim)
				}
			},
			OnExpire: func(aura *core.Aura, sim *core.Simulation) {
				// A raid debuff made permanent by the encounter settings is not this curse's to remove
				for _, debuff := range debuffs {
					if debuff.Duration != core.NeverExpires {
						debuff.Deactivate(sim)
					}
				}
			},
		})
	})

	warlock.CurseOfElements = warlock.RegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: spellID},
		SpellSchool: row.SpellSchool,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       core.SpellFlagAPL | WarlockFlagAffliction,
		Rank:        rank,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		ThreatMultiplier: 1,
		FlatThreatBonus:  156,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				aura := warlock.CurseOfElementsAuras.Get(target)
				if activeCurse := warlock.ActiveCurseAura.Get(target); activeCurse != nil && activeCurse != aura {
					activeCurse.Deactivate(sim)
				}

				warlock.ActiveCurseAura[target.UnitIndex] = aura
				warlock.ActiveCurseAura.Get(target).Activate(sim)
			}
		},

		RelatedAuras: []core.AuraArray{warlock.CurseOfElementsAuras},
	})
}

func (warlock *Warlock) registerAmplifyCurseSpell() {
	if !warlock.Talents.AmplifyCurse {
		return
	}

	row := spellData.AmplifyCurse.ByRank(1)
	actionID := core.ActionID{SpellID: row.SpellID}

	warlock.AmplifyCurseAura = warlock.GetOrRegisterAura(core.Aura{
		Label:    "Amplify Curse",
		ActionID: actionID,
		Duration: row.Duration,
	})

	warlock.AmplifyCurse = warlock.GetOrRegisterSpell(core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: core.SpellSchoolShadow,
		Flags:       core.SpellFlagAPL | WarlockFlagAffliction,

		Cast: core.CastConfig{
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			warlock.AmplifyCurseAura.Activate(sim)
		},
	})
}

func (warlock *Warlock) registerBaneOfDoomSpell() {
	if warlock.Level < 60 {
		return
	}

	// Spell ID, cost, cooldown, school and the dot (1742 once after 60 sec, 4.0 coefficient) from the
	// client table
	row := spellData.BaneOfDoom.ByRank(1)
	periodic := row.Periodic.(shared.SpellDataPeriodic)

	warlock.BaneOfDoom = warlock.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_WarlockBaneOfDoom,
		ClassSpellMask: SpellMaskBaneOfDoom,
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | WarlockFlagAffliction,

		RequiredLevel: 60,

		ManaCost: core.ManaCostOptions{
			FlatCost: float64(row.Cost),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			CD: core.Cooldown{
				Timer:    warlock.NewTimer(),
				Duration: row.Cooldown,
			},
		},

		CritDamageBonus: 0,

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		FlatThreatBonus:  160,

		Dot: core.DotConfig{
			Aura: core.Aura{
				Label: "BaneofDoom",
			},
			NumberOfTicks: periodic.NumberOfTicks,
			TickLength:    periodic.TickLength,
			// Beta client 1.60.1: 1742 damage with a 4.0 spell power coefficient. Classic's 3200 carried
			// no coefficient of its own, and the spell level 1 this file used to set never reached the dot.
			BonusCoefficient: roundCoef(periodic.Coef),
			OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
				dot.Snapshot(target, periodic.Tick, isRollover)
			},
			OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
				dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				dot := spell.Dot(target)
				if activeBane := warlock.ActiveBaneAura.Get(target); activeBane != nil && activeBane != dot.Aura {
					activeBane.Deactivate(sim)
				}

				dot.Apply(sim)
				warlock.ActiveBaneAura[target.UnitIndex] = dot.Aura
			}
		},
	})
}

func (warlock *Warlock) registerBaneOfHavocSpell() {
	if !warlock.Talents.BaneOfHavoc {
		return
	}

	actionID := core.ActionID{SpellID: 80240}

	warlock.BaneOfHavocAuras = warlock.NewEnemyAuraArray(func(unit *core.Unit) *core.Aura {
		return unit.RegisterAura(core.Aura{
			Label:    "Bane of Havoc-" + warlock.Label,
			ActionID: actionID,
			Duration: time.Minute * 5,
		})
	})

	// Only marks the target for now, the 15% damage copy needs a second target to matter
	warlock.BaneOfHavoc = warlock.RegisterSpell(core.SpellConfig{
		ActionID:    actionID,
		SpellSchool: core.SpellSchoolShadow,
		ProcMask:    core.ProcMaskEmpty,
		Flags:       core.SpellFlagAPL | WarlockFlagDestruction,

		// 5% of base mana in the beta client (1225228), not a flat 300
		ManaCost: core.ManaCostOptions{
			BaseCost: spellData.BaneOfHavoc.ByRank(1).PowerCostPct / 100,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			result := spell.CalcOutcome(sim, target, spell.OutcomeMagicHitNoHitCounter)
			if result.Landed() {
				aura := warlock.BaneOfHavocAuras.Get(target)
				if activeBane := warlock.ActiveBaneAura.Get(target); activeBane != nil && activeBane != aura {
					activeBane.Deactivate(sim)
				}

				warlock.ActiveBaneAura[target.UnitIndex] = aura
				aura.Activate(sim)
			}
		},

		RelatedAuras: []core.AuraArray{warlock.BaneOfHavocAuras},
	})
}
