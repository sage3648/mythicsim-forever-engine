package paladin

import (
	"strconv"
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/proto"
)

// Values come from client 1.60.1.69913. See docs/mythicsim-seal-of-fury.md
// for source rows, server-side assumptions and upstream convergence notes.
func (paladin *Paladin) registerSealOfFury() {
	if paladin.Env.Ruleset != proto.Ruleset_RulesetForever {
		return
	}
	shield := paladin.registerFuryAbsorb()
	ranks := []struct {
		level, maxLevel                                 int32
		sealID, procID, judgementID                     int32
		mana, damage, judgeMin, judgeMax, judgePerLevel float64
	}{
		{level: 10, maxLevel: 16, sealID: 1311649, procID: 1311647, judgementID: 1311650, mana: 40, damage: 6, judgeMin: 22, judgeMax: 24, judgePerLevel: 1.71},
		{level: 18, maxLevel: 24, sealID: 1311656, procID: 1311654, judgementID: 1311655, mana: 60, damage: 9, judgeMin: 35, judgeMax: 39, judgePerLevel: 2.16},
		{level: 25, maxLevel: 31, sealID: 20163, procID: 20231, judgementID: 20183, mana: 90, damage: 14, judgeMin: 51, judgeMax: 57, judgePerLevel: 2.52},
		{level: 34, maxLevel: 40, sealID: 20419, procID: 20415, judgementID: 20411, mana: 120, damage: 19, judgeMin: 70, judgeMax: 78, judgePerLevel: 2.79},
		{level: 42, maxLevel: 48, sealID: 20421, procID: 20416, judgementID: 20412, mana: 140, damage: 25, judgeMin: 91, judgeMax: 101, judgePerLevel: 3.42},
		{level: 50, maxLevel: 56, sealID: 20422, procID: 20417, judgementID: 20413, mana: 170, damage: 32, judgeMin: 118, judgeMax: 128, judgePerLevel: 3.69},
		{level: 58, maxLevel: 64, sealID: 20423, procID: 20418, judgementID: 20414, mana: 200, damage: 35, judgeMin: 146, judgeMax: 160, judgePerLevel: 3.69},
	}
	for i, rank := range ranks {
		if paladin.Level < rank.level {
			break
		}
		scaling := rank.judgePerLevel * float64(min(paladin.Level, rank.maxLevel)-rank.level)
		judgement := paladin.RegisterSpell(core.SpellConfig{
			ActionID:         core.ActionID{SpellID: rank.judgementID},
			SpellCode:        SpellCode_PaladinJudgementOfFury,
			SpellSchool:      core.SpellSchoolHoly,
			DefenseType:      core.DefenseTypeMelee,
			ProcMask:         core.ProcMaskSpellDamage,
			Flags:            core.SpellFlagMeleeMetrics | core.SpellFlagSuppressWeaponProcs | core.SpellFlagSuppressEquipProcs | core.SpellFlagBinary,
			DamageMultiplier: paladin.improvedSeals(),
			ThreatMultiplier: 1,
			BonusCoefficient: 0.45,
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				// The encounter already assigns the tank. The engine does not model taunt swaps.
				spell.CalcAndDealDamage(sim, target, sim.Roll(rank.judgeMin+scaling, rank.judgeMax+scaling), spell.OutcomeMeleeSpecialNoBlockDodgeParry)
			},
		})
		proc := paladin.RegisterSpell(core.SpellConfig{
			ActionID:         core.ActionID{SpellID: rank.procID},
			SpellSchool:      core.SpellSchoolHoly,
			DefenseType:      core.DefenseTypeMelee,
			ProcMask:         core.ProcMaskEmpty,
			Flags:            core.SpellFlagMeleeMetrics | core.SpellFlagPassiveSpell | core.SpellFlagSuppressWeaponProcs | core.SpellFlagSuppressEquipProcs,
			DamageMultiplier: paladin.improvedSeals(),
			ThreatMultiplier: 1,
			BonusCoefficient: 0.1,
			ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
				result := spell.CalcDamage(sim, target, rank.damage, spell.OutcomeMeleeSpecialCritOnly)
				damage := result.Damage
				spell.DealDamage(sim, result)
				if paladin.OffHand().WeaponType == proto.WeaponType_WeaponTypeShield && damage > 0 {
					shield.apply(sim, damage*0.5)
				}
			},
		})
		aura := paladin.RegisterAura(core.Aura{
			Label:    "Seal of Fury " + strconv.Itoa(i+1),
			ActionID: core.ActionID{SpellID: rank.sealID},
			Duration: 30 * time.Second,
			OnSpellHitDealt: func(_ *core.Aura, sim *core.Simulation, spell *core.Spell, result *core.SpellResult) {
				if result.Landed() && spell.ProcMask.Matches(core.ProcMaskMeleeWhiteHit) {
					proc.Cast(sim, result.Target)
				}
			},
		})
		paladin.registerSealProc(aura, func(sim *core.Simulation, target *core.Unit) { proc.Cast(sim, target) })
		paladin.aurasSoF = append(paladin.aurasSoF, aura)
		paladin.spellsJoF = append(paladin.spellsJoF, judgement)
		paladin.sealOfFury = paladin.RegisterSpell(core.SpellConfig{
			ActionID:      aura.ActionID,
			SpellSchool:   core.SpellSchoolHoly,
			Flags:         core.SpellFlagAPL,
			RequiredLevel: int(rank.level), Rank: i + 1,
			ManaCost: core.ManaCostOptions{FlatCost: rank.mana - paladin.getLibramSealCostReduction(), Multiplier: paladin.benediction()},
			Cast:     core.CastConfig{DefaultCast: core.Cast{GCD: core.GCDDefault}},
			ApplyEffects: func(sim *core.Simulation, _ *core.Unit, spell *core.Spell) {
				paladin.applySeal(aura, spell, judgement, sim)
			},
		})
	}
}

// The core Shield records shielding but does not consume it. Keep Fury's
// consumption local so this patch can converge without changing other shields.
type furyAbsorb struct {
	shield    *core.Shield
	remaining float64
}

func (f *furyAbsorb) apply(sim *core.Simulation, amount float64) {
	f.shield.Apply(sim, amount)
	f.remaining = amount * f.shield.Spell.DamageMultiplier * f.shield.Spell.Unit.PseudoStats.ShieldDealtMultiplier
}

func (paladin *Paladin) registerFuryAbsorb() *furyAbsorb {
	f := &furyAbsorb{}
	spell := paladin.RegisterSpell(core.SpellConfig{
		ActionID:    core.ActionID{SpellID: 1310927},
		SpellSchool: core.SpellSchoolHoly, ProcMask: core.ProcMaskEmpty,
		Flags: core.SpellFlagPassiveSpell, DamageMultiplier: 1,
		Shield: core.ShieldConfig{SelfOnly: true, Aura: core.Aura{
			Label: "Light's Fury", Duration: 10 * time.Second,
			OnExpire: func(_ *core.Aura, _ *core.Simulation) { f.remaining = 0 },
			OnReset:  func(_ *core.Aura, _ *core.Simulation) { f.remaining = 0 },
		}},
	})
	f.shield = spell.SelfShield()
	manaMetrics := paladin.NewManaMetrics(core.ActionID{SpellID: 1314104})
	paladin.AddDynamicDamageTakenModifier(func(sim *core.Simulation, incoming *core.Spell, result *core.SpellResult) {
		if !f.shield.IsActive() || result.Damage <= 0 || f.remaining <= 0 {
			return
		}
		absorbed := min(f.remaining, result.Damage)
		result.Damage -= absorbed
		f.remaining -= absorbed
		if f.remaining == 0 {
			f.shield.Deactivate(sim)
			if paladin.Talents.ImprovedSealOfFury {
				levelsAbove := min(int32(3), max(int32(0), incoming.Unit.Level-paladin.Level))
				paladin.AddMana(sim, float64(paladin.Level)*(1+0.15*float64(levelsAbove)), manaMetrics)
			}
		}
	})
	return f
}
