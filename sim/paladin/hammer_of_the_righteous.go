package paladin

import (
	"github.com/wowsims/classic/sim/core"
)

// Hammer of the Righteous is trained on the Protection line at level 40 in Forever
// (SkillLineAbility AcquireMethod 0) and this sim did not have it. Beta client 1.60.1.69893,
// spell 407632: Holy damage equal to 3x main hand weapon DPS, 6 sec cooldown, 6% of base mana,
// on the melee table (DefenseType 2), no spell power coefficient in any effect.
//
// Its cooldown is not its own. Client category 2404 holds both it and Holy Strike, so casting
// either puts both on cooldown for the cast spell's duration - which makes them a choice, not
// two buttons. The tooltip says "shared with Crusader Strike", a leftover: Forever has no
// Crusader Strike, and the category is what the client actually enforces.
//
// Not modelled: the other three targets it can hit. The arena and the tests are single target.
//
// Id, cost, cooldown, school, defense type and the weapon DPS multiple come from the client table.
// The client's missile speed (35) is not applied, as before.
func (paladin *Paladin) registerHammerOfTheRighteous() {
	if !paladin.Env.IsForever() || paladin.Level < 40 {
		return
	}

	row := spellData.HammerOfTheRighteous.ByRank(1)
	weaponDPS := row.Effects[2].Value

	paladin.RegisterSpell(core.SpellConfig{
		ActionID:       core.ActionID{SpellID: row.SpellID},
		SpellCode:      SpellCode_PaladinHammerOfTheRighteous,
		ClassSpellMask: SpellMaskHammerOfTheRighteous,
		SpellSchool:    row.SpellSchool,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskMeleeMHSpecial,
		Flags:          core.SpellFlagMeleeMetrics | core.SpellFlagAPL,

		RequiredLevel: 40,

		ManaCost: core.ManaCostOptions{
			BaseCost:   row.PowerCostPct / 100,
			Multiplier: paladin.benediction(),
		},
		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
			IgnoreHaste: true,
			CD: core.Cooldown{
				Timer:    paladin.strikeTimer(),
				Duration: row.Cooldown,
			},
		},

		DamageMultiplier: 1,
		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			baseDamage := weaponDPS * paladin.AutoAttacks.MH().DPS()
			spell.CalcAndDealDamage(sim, target, baseDamage, spell.OutcomeMeleeSpecialHitAndCrit)
		},
	})
}
