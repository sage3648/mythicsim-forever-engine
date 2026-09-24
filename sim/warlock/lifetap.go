package warlock

import (
	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

const LifeTapRanks = 6

// Spell ID and school come from the client table; it reads no health or mana value (a dummy effect),
// so the rank's $m1 stays ours: the effect's base points at the rank's max level.
var LifeTapBaseDamage = [LifeTapRanks + 1]float64{0, 30, 75, 140, 220, 310, 424}

// Life Tap is a plain mana gain, not a damage roll against the warlock. The client's 11689 converts
// $s1 health into ($m1 + Spirit) * (1 + Improved Life Tap 18182, 10/20%) mana, so no damage done or
// taken modifier touches either side: Shadow Mastery, Burning Shadow, Master Demonologist, Soul Link
// and Molten Skin no longer scale it, and the 0.8 spell power term is gone. Demonic Energies hands
// the pet a share of the restore.
func (warlock *Warlock) getLifeTapBaseConfig(rank int) core.SpellConfig {
	row := spellData.LifeTap.ByRank(int32(rank))
	spellId := row.SpellID
	healthCost := LifeTapBaseDamage[rank]
	manaMultiplier := 1 + 0.1*float64(warlock.Talents.ImprovedLifeTap)

	level := [LifeTapRanks + 1]int{0, 6, 16, 26, 36, 46, 56}[rank]

	actionID := core.ActionID{SpellID: spellId}
	petManaShare := 0.5 * float64(warlock.Talents.DemonicEnergies)

	manaMetrics := warlock.NewManaMetrics(actionID)
	for _, pet := range warlock.BasePets {
		pet.LifeTapManaMetrics = pet.NewManaMetrics(actionID)
	}

	return core.SpellConfig{
		ActionID:       actionID,
		SpellSchool:    row.SpellSchool,
		SpellCode:      SpellCode_WarlockLifeTap,
		ClassSpellMask: SpellMaskLifeTap,
		DefenseType:    row.DefenseType,
		ProcMask:       core.ProcMaskSpellDamage,
		Flags:          core.SpellFlagAPL | core.SpellFlagResetAttackSwing | WarlockFlagAffliction,
		RequiredLevel:  level,
		Rank:           rank,

		Cast: core.CastConfig{
			DefaultCast: core.Cast{
				GCD: core.GCDDefault,
			},
		},

		ThreatMultiplier: 1,

		ApplyEffects: func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
			restore := (healthCost + warlock.GetStat(stats.Spirit)) * manaMultiplier

			if warlock.IsTanking() {
				warlock.RemoveHealth(sim, healthCost)
			}

			warlock.AddMana(sim, restore, manaMetrics)

			if petManaShare > 0 && warlock.ActivePet != nil {
				warlock.ActivePet.AddMana(sim, restore*petManaShare, warlock.ActivePet.LifeTapManaMetrics)
			}
		},
	}
}

func (warlock *Warlock) registerLifeTapSpell() {
	warlock.LifeTap = make([]*core.Spell, 0)
	for i := 1; i <= LifeTapRanks; i++ {
		config := warlock.getLifeTapBaseConfig(i)

		if config.RequiredLevel <= int(warlock.Level) {
			warlock.LifeTap = append(warlock.LifeTap, warlock.GetOrRegisterSpell(config))
		}
	}
}
