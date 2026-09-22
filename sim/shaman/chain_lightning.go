package shaman

import (
	"github.com/wowsims/classic/sim/core"
)

const ChainLightningRanks = 4
const ChainLightningTargetCount = int32(3)

// Forever beta client values. Spell ID, cost, cast time, cooldown, coefficient, bounce falloff, school and
// defense type come from the client table; damage stays here, scaled to level 60 the same way as Lightning
// Bolt's. Rank 3's 0.517 is what the client stores, against 0.571 on the ranks either side; it reads like a
// transposed digit but is kept as written.
var ChainLightningBaseDamage = [ChainLightningRanks + 1][]float64{{0}, {85, 97}, {97, 109}, {109, 122}, {119, 134}}
var ChainLightningLevel = [ChainLightningRanks + 1]int{0, 32, 40, 48, 56}

func (shaman *Shaman) registerChainLightningSpell() {
	shaman.ChainLightning = make([]*core.Spell, ChainLightningRanks+1)
	shaman.ChainLightningOverload = make([]*core.Spell, ChainLightningRanks+1)

	cdTimer := shaman.NewTimer()

	for rank := 1; rank <= ChainLightningRanks; rank++ {
		config := shaman.newChainLightningSpellConfig(rank, cdTimer)

		if config.RequiredLevel <= int(shaman.Level) {
			// The overload gets a config of its own so that the two casts share no bounce results.
			shaman.ChainLightningOverload[rank] = shaman.registerOverloadSpell(shaman.newChainLightningSpellConfig(rank, cdTimer))
			shaman.ChainLightning[rank] = shaman.RegisterSpell(config)
		}
	}
}

func (shaman *Shaman) newChainLightningSpellConfig(rank int, cdTimer *core.Timer) core.SpellConfig {
	row := spellData.ChainLightning.ByRank(int32(rank))
	baseDamageLow := ChainLightningBaseDamage[rank][0]
	baseDamageHigh := ChainLightningBaseDamage[rank][1]
	level := ChainLightningLevel[rank]

	shaman.ChainLightningBounceCoefficient = row.Effects[0].ChainAmplitude // 0.7: 30% reduction per bounce
	targetCount := ChainLightningTargetCount

	spell := shaman.newElectricSpellConfig(core.ActionID{SpellID: row.SpellID}, row)

	spell.SpellCode = SpellCode_ShamanChainLightning
	spell.ClassSpellMask = SpellMaskChainLightning
	spell.RequiredLevel = level
	spell.Rank = rank
	spell.Cast.CD = core.Cooldown{
		Timer:    cdTimer,
		Duration: row.Cooldown,
	}

	results := make([]*core.SpellResult, min(targetCount, shaman.Env.GetNumTargets()))

	spell.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		primaryTarget := target
		origMult := spell.DamageMultiplier
		for hitIndex := range results {
			baseDamage := sim.Roll(baseDamageLow, baseDamageHigh)
			results[hitIndex] = spell.CalcDamage(sim, target, baseDamage, spell.OutcomeMagicHitAndCrit)
			target = sim.Environment.NextTargetUnit(target)
			spell.DamageMultiplier *= shaman.ChainLightningBounceCoefficient
		}

		for _, result := range results {
			spell.DealDamage(sim, result)
		}

		spell.DamageMultiplier = origMult

		shaman.tryLightningOverload(sim, primaryTarget, spell, shaman.ChainLightningOverload[rank])
	}

	return spell
}
