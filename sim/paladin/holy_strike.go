package paladin

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

// Holy Strike is new in Forever and baseline from level 6. Three talents hang off it - Improved Holy
// Strike shortens its cooldown, Iron Creed sharpens its threat and Sacred Arbiter its damage.
//
// Beta client 1.60.1.69893 gives it eight ranks on ids Classic used for NPC spells. Each is a
// normalized weapon strike plus a flat amount (effect 121) and a weapon damage percentage (effect 31),
// with a 12 sec cooldown and the 0.429 coefficient the sim had guessed. The client multiplies the
// flat amount by the percentage the same way it does Backstab's, so rank 8 is 40% of (weapon + 81 to
// 105): about 32 to 42 on top of the weapon share, close to the 36 to 46 the BlizzCon tooltip showed.
// Damage is each rank's at its max level; it stays ours, as the client table holds the centre of the
// flat roll, truncated (spell_damage_test.go checks it). Cost, cooldown, school, defense type, weapon
// percentage and coefficient come from the table; the ids stay ours, as every rank is registered.
// TODO: beta will confirm - Holy damage on the melee hit table, so it rolls partial resists the
// way every other Holy ability here does. Whether a melee-table Holy strike actually partial
// resists is unknown; if it does not, it wants SpellFlagIgnoreResists.
var holyStrikeRanks = []struct {
	level     int32
	minDamage float64
	maxDamage float64
}{
	{level: 6, minDamage: 11, maxDamage: 14},
	{level: 12, minDamage: 15, maxDamage: 20},
	{level: 20, minDamage: 17, maxDamage: 23},
	{level: 28, minDamage: 22, maxDamage: 29},
	{level: 36, minDamage: 32, maxDamage: 40},
	{level: 44, minDamage: 53, maxDamage: 68},
	{level: 52, minDamage: 73, maxDamage: 91},
	{level: 60, minDamage: 81, maxDamage: 105},
}

func (paladin *Paladin) registerHolyStrike() {
	// Rank 2 takes off 2 sec, so the linear reading was right. Confirmed on the beta.
	cd := core.Cooldown{
		Timer:    paladin.strikeTimer(),
		Duration: spellData.HolyStrike.ByRank(1).Cooldown - time.Second*time.Duration(paladin.Talents.ImprovedHolyStrike),
	}

	// Sacred Arbiter also refreshes the paladin's Judgement effects. Judgement of the Crusader
	// is the only Judgement that leaves anything behind, and it already refreshes off every
	// melee attack the paladin lands, so that half of the talent needs nothing here.
	damageMultiplier := paladin.getWeaponSpecializationModifier()
	if paladin.Talents.SacredArbiter {
		damageMultiplier *= 1.1
	}

	// 5% per rank, confirmed on the beta at ranks 2, 3 and 4: 10%, 15% and 20%.
	threatMultiplier := 1 + 0.05*float64(paladin.Talents.IronCreed)

	ironCreedAura := paladin.registerIronCreedAura()

	for i, rank := range holyStrikeRanks {
		spellID := []int32{679, 678, 1866, 680, 2495, 5569, 10332, 10333}[i]
		if paladin.Level < rank.level {
			break
		}
		row := spellData.HolyStrike.BySpellID(spellID)
		weapon := row.Effects[1].Value / 100

		paladin.RegisterSpell(core.SpellConfig{
			ActionID:       core.ActionID{SpellID: spellID},
			SpellCode:      SpellCode_PaladinHolyStrike,
			ClassSpellMask: SpellMaskHolyStrike,
			SpellSchool:    row.SpellSchool,
			DefenseType:    row.DefenseType,
			ProcMask:       core.ProcMaskMeleeMHSpecial,
			Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

			RequiredLevel: int(rank.level),
			Rank:          i + 1,

			ManaCost: core.ManaCostOptions{
				FlatCost:   float64(row.Cost),
				Multiplier: paladin.benediction(),
			},
			Cast: core.CastConfig{
				DefaultCast: core.Cast{
					GCD: core.GCDDefault,
				},
				IgnoreHaste: true,
				CD:          cd,
			},

			DamageMultiplier: damageMultiplier,
			ThreatMultiplier: threatMultiplier,
			// Holy damage, so spell power feeds it on top of the weapon share and the flat roll.
			BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				if ironCreedAura != nil {
					ironCreedAura.Activate(sim)
				}

				// A share of weapon damage, so it takes the normalized swing the way every other
				// percentage-of-weapon strike in the sim does.
				baseDamage := weapon * (spell.Unit.MHNormalizedWeaponDamage(sim, spell.MeleeAttackPower(target)) +
					sim.Roll(rank.minDamage, rank.maxDamage))
				spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
			},
		})
	}
}

// The half of Iron Creed that is not threat: Holy Strike shaves the damage the paladin takes,
// but only while Righteous Fury is up. Ardent Defender's spell id stands in for the buff.
func (paladin *Paladin) registerIronCreedAura() *core.Aura {
	if paladin.Talents.IronCreed == 0 || !paladin.Options.RighteousFury {
		return nil
	}

	// 2% per rank, confirmed on the beta at ranks 2, 3 and 4: 4%, 6% and 8%. The 6 seconds
	// is flat at every rank, which those same tooltips show.
	damageTaken := 1 - 0.02*float64(paladin.Talents.IronCreed)

	return paladin.RegisterAura(core.Aura{
		Label:    "Iron Creed",
		ActionID: core.ActionID{SpellID: 31850},
		Duration: time.Second * 6,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			paladin.PseudoStats.DamageTakenMultiplier *= damageTaken
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			paladin.PseudoStats.DamageTakenMultiplier /= damageTaken
		},
	})
}
