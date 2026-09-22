package paladin

import (
	"math"
	"strconv"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

// The client stores .095 as a float32; the table widens it. Rounding back to the stated value keeps
// the sim's numbers where they were (sim/mage/frostbolt.go). Every coefficient read from the table
// goes through this.
func roundCoef(coef float64) float64 {
	return math.Round(coef*1e6) / 1e6
}

// Consecration is baseline in the beta client (1.60.1.69893): every paladin trains all five ranks, and
// the Forever tree builds on top of it through Consecrated Ground and Holy Conduit.
//
// Each tick casts a separate damage spell (1280345-1280349) with two parts: a flat amount every enemy
// in the area takes, and a larger amount with the spell power coefficient that only the first
// $s3 = 4 enemies take. Classic's 48 a tick at 0.042 becomes 12 + 27 at 0.095 on the capped part.
//
// Cost, cooldown, school, defense type, tick, tick schedule, coefficient and the capped target count
// come from the client table; the ids stay ours, as every rank is registered (see sim/rogue).
func (paladin *Paladin) registerConsecration() {
	cd := core.Cooldown{
		Timer:    paladin.NewTimer(),
		Duration: spellData.Consecration.ByRank(1).Cooldown,
	}

	for i, level := range []int32{20, 30, 40, 50, 60} {
		spellID := []int32{26573, 20116, 20922, 20923, 20924}[i]
		if paladin.Level < level {
			break
		}

		row := spellData.Consecration.BySpellID(spellID)
		periodic := row.Periodic.(shared.SpellDataPeriodic)        // every enemy, per tick
		capped := row.SecondaryPeriodic.(shared.SpellDataPeriodic) // first $s3 enemies, per tick, scales with spell power
		cappedTargets := int(row.Effects[2].Value)

		paladin.RegisterSpell(core.SpellConfig{
			ActionID:    core.ActionID{SpellID: spellID},
			SpellSchool: row.SpellSchool,
			DefenseType: row.DefenseType,
			ProcMask:    core.ProcMaskSpellDamage,
			Flags:       core.SpellFlagPureDot | core.SpellFlagAPL,

			RequiredLevel: int(level),
			Rank:          i + 1,

			SpellCode:      SpellCode_PaladinConsecration,
			ClassSpellMask: SpellMaskConsecration,
			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.benediction() * paladin.holyConduit() / 100,
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
				CD: cd,
			},
			DamageMultiplier: 1,
			ThreatMultiplier: 1,
			Dot: core.DotConfig{
				IsAOE: true,
				Aura: core.Aura{
					Label: "Consecration" + paladin.Label + strconv.Itoa(i+1),
				},
				NumberOfTicks: periodic.NumberOfTicks,
				TickLength:    periodic.TickLength,

				BonusCoefficient: roundCoef(capped.Coef),

				OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
					dot.Snapshot(target, periodic.Tick+capped.Tick, isRollover)
				},
				OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
					// Consecration can miss, showing up as either a resist in logs or a
					// silent failure (missing damage tick).
					// ponytail: "first 4 to enter" is read as the first 4 targets in the encounter.
					for j, aoeTarget := range sim.Encounter.TargetUnits {
						if j < cappedTargets {
							dot.CalcAndDealPeriodicSnapshotDamage(sim, aoeTarget, dot.OutcomeMagicHitAndTick)
						} else {
							// The flat part alone, which carries no coefficient; the spell's own
							// BonusCoefficient stays zero so this does not pick one up.
							dot.Spell.CalcAndDealDamage(sim, aoeTarget, periodic.Tick, dot.OutcomeMagicHitAndTick)
						}
					}
				},
			},

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				spell.AOEDot().Apply(sim)
			},
		})
	}
}
