package shaman

import (
	"fmt"

	"github.com/wowsims/classic/sim/common/shared"
	"github.com/wowsims/classic/sim/core"
)

const FlameShockRanks = 6

// Forever beta client values. Spell ID, cost, cooldown, school, defense type, the 0.214 direct coefficient and
// the dot (per tick damage, 4 ticks of 3 sec, 0.1 per tick) come from the client table (see shocks.go). The
// direct hit stays here: the table reads 136 for rank 5 against our 137 (spell_damage_test.go).
var FlameShockBaseDamage = [FlameShockRanks + 1]float64{0, 24, 38, 49, 89, 137, 166}
var FlameShockLevel = [FlameShockRanks + 1]int{0, 10, 18, 28, 40, 52, 60}

func (shaman *Shaman) registerFlameShockSpell(shockTimer *core.Timer) {
	shaman.FlameShock = make([]*core.Spell, FlameShockRanks+1)

	for rank := 1; rank <= FlameShockRanks; rank++ {
		if FlameShockLevel[rank] <= int(shaman.Level) {
			shaman.FlameShock[rank] = shaman.RegisterSpell(shaman.newFlameShockSpell(rank, shockTimer))
		}
	}
}

func (shaman *Shaman) newFlameShockSpell(rank int, shockTimer *core.Timer) core.SpellConfig {
	row := spellData.FlameShock.ByRank(int32(rank))
	periodic := row.Periodic.(shared.SpellDataPeriodic)
	baseDamage := FlameShockBaseDamage[rank]
	level := FlameShockLevel[rank]

	spell := shaman.newShockSpellConfig(core.ActionID{SpellID: row.SpellID}, row, shockTimer)

	spell.SpellCode = SpellCode_ShamanFlameShock
	spell.ClassSpellMask = SpellMaskFlameShock
	spell.RequiredLevel = level
	spell.Rank = rank
	// Call of Flame names Flame Shock alongside the fire totems under Forever.
	spell.DamageMultiplier *= shaman.callOfFlameMultiplier()

	spell.Cast.IgnoreHaste = true

	spell.Dot = core.DotConfig{
		Aura: core.Aura{
			Label: fmt.Sprintf("Flame Shock (Rank %d)", rank),
		},

		NumberOfTicks:    periodic.NumberOfTicks,
		TickLength:       periodic.TickLength,
		BonusCoefficient: roundCoef(periodic.Coef),

		OnSnapshot: func(sim *core.Simulation, target *core.Unit, dot *core.Dot, isRollover bool) {
			dot.Snapshot(target, periodic.Tick, isRollover)
		},

		OnTick: func(sim *core.Simulation, target *core.Unit, dot *core.Dot) {
			dot.CalcAndDealPeriodicSnapshotDamage(sim, target, dot.OutcomeTick)
		},
	}

	spell.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		result := spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
		if result.Landed() {
			spell.Dot(result.Target).Apply(sim)
		}
	}

	return spell
}
