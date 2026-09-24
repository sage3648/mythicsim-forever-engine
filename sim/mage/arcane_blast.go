package mage

import (
	"time"

	"github.com/wowsims/classic/sim/core"
)

const ArcaneBlastMaxStacks = 4

func (mage *Mage) registerArcaneBlastSpell() {
	if !mage.Talents.ArcaneBlast {
		return
	}

	// Beta client 1.60.1.69893: rank 5 (1239700), the level 60 rank of five. Forever gives it its own
	// spell ids; 30451 is kept as the action id the APLs already name. Cost, cast time, school and
	// coefficient come from the client table; the damage is ours (the table has the centre, 394).
	row := spellData.ArcaneBlast.ByRank(5)
	baseDamage := []float64{364, 424}

	actionID := core.ActionID{SpellID: 30451}

	// Arcane Blast buffs the mage's other spells rather than itself, and the next one of them
	// spends the stacks. Client 1.60.1.69977: the buff's (400573) damage mask, [12718707, 4096],
	// names every mage damage spell but Arcane Blast, Arcane Missiles, Blizzard and Flamestrike,
	// whatever its tooltip says. Those three still spend the stacks without gaining from them.
	damageMod := mage.AddDynamicMod(core.SpellModConfig{
		Kind: core.SpellMod_DamageDone_Flat,
		ClassMask: SpellMaskArcaneExplosion | SpellMaskBlastWave | SpellMaskFireball | SpellMaskFireBlast |
			SpellMaskFrostbolt | SpellMaskIceLance | SpellMaskPyroblast | SpellMaskScorch,
	})

	mage.ArcaneBlastAura = mage.RegisterAura(core.Aura{
		Label:     "Arcane Blast",
		ActionID:  actionID,
		Duration:  time.Second * 8,
		MaxStacks: ArcaneBlastMaxStacks,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Activate()
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			damageMod.Deactivate()
		},
		OnStacksChange: func(aura *core.Aura, sim *core.Simulation, oldStacks int32, newStacks int32) {
			damageMod.UpdateFloatValue(.10 * float64(newStacks))
			mage.ArcaneBlast.Cost.Multiplier += 175 * (newStacks - oldStacks)
		},
		OnCastComplete: func(aura *core.Aura, sim *core.Simulation, spell *core.Spell) {
			if !spell.Flags.Matches(SpellFlagMage) || !spell.ProcMask.Matches(core.ProcMaskSpellDamage) {
				return
			}

			// OnCastComplete runs after the damage is rolled, so the spell that drops the stacks is
			// still buffed by them. Arcane Missiles spends them when the channel ends instead, so
			// that every missile benefits.
			switch spell.SpellCode {
			case SpellCode_MageArcaneBlast, SpellCode_MageArcaneMissiles, SpellCode_MageArcaneMissilesTick:
				return
			}

			aura.Deactivate(sim)
		},
	})

	mage.ArcaneBlast = mage.RegisterSpell(core.SpellConfig{
		SpellCode:      SpellCode_MageArcaneBlast,
		ClassSpellMask: SpellMaskArcaneBlast,
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          SpellFlagMage | core.SpellFlagAPL,

		RequiredLevel: 60,
		Rank:          1,

		ManaCost: core.ManaCostOptions{
			BaseCost: row.PowerCostPct / 100,
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD:      core.GCDDefault,
				CastTime: row.CastTime,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,
		BonusCoefficient: roundCoef(row.Direct.BonusCoefficient()),

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			damage := sim.Roll(baseDamage[0], baseDamage[1])
			spell.CalcAndDealDamage(sim, target, damage, spell.OutcomeMagicHitAndCrit)

			mage.ArcaneBlastAura.Activate(sim)
			mage.ArcaneBlastAura.AddStack(sim)
		},
	})
}

// Arcane Missiles holds on to the Arcane Blast stacks for the whole channel and spends them when
// the last missile lands.
func (mage *Mage) spendArcaneBlastStacks(sim *core.Simulation) {
	if mage.ArcaneBlastAura != nil {
		mage.ArcaneBlastAura.Deactivate(sim)
	}
}
