package shaman

import (
	"time"

	"github.com/wowsims/classic/sim/core"
	"github.com/wowsims/classic/sim/core/stats"
)

const RockbiterWeaponRanks = 7

var RockbiterWeaponEnchantId = [RockbiterWeaponRanks + 1]int32{0, 29, 6, 1, 503, 1663, 683, 1664}

// Each rank at the bracket that uses it (25 -> r4, 40 -> r5, 50 -> r6, 60 -> r7), from the passives'
// aura 99 base + per level (15569 129+8.1, 16312 393+16.1, ...), floored as the server floors:
// r4 137 and r6 489, not the rounded 138 and 490.
var RockbiterWeaponBonusAP = [RockbiterWeaponRanks + 1]float64{0, 50, 79, 118, 137, 319, 489, 653}
var RockbiterWeaponBonusTPS = [RockbiterWeaponRanks + 1]float64{0, 6, 10, 16, 27, 41, 55, 72}
var RockbiterWeaponLevel = [RockbiterWeaponRanks + 1]int32{0, 1, 8, 16, 24, 34, 44, 54}

var RockbiterWeaponRankByLevel = map[int32]int32{
	25: 4,
	40: 5,
	50: 6,
	60: 7,
}

func (shaman *Shaman) RegisterRockbiterImbue(procMask core.ProcMask) {
	if procMask == core.ProcMaskUnknown {
		return
	}

	rank := RockbiterWeaponRankByLevel[shaman.Level]
	enchantId := RockbiterWeaponEnchantId[rank]
	bonusThreat := RockbiterWeaponBonusTPS[rank]

	duration := time.Minute * 5

	hasMHImbue := procMask.Matches(core.ProcMaskMeleeMH)
	hasOHImbue := procMask.Matches(core.ProcMaskMeleeOH)

	if hasMHImbue {
		shaman.MainHand().TempEnchant = enchantId
		shaman.AutoAttacks.MHConfig().FlatThreatBonus += bonusThreat * shaman.AutoAttacks.MH().SwingSpeed
	}
	if hasOHImbue {
		shaman.OffHand().TempEnchant = enchantId
		shaman.AutoAttacks.MHConfig().FlatThreatBonus += bonusThreat * shaman.AutoAttacks.OH().SwingSpeed
	}

	shaman.OnSpellRegistered(func(spell *core.Spell) {
		if spell.ProcMask.Matches(procMask) {
			spell.FlatThreatBonus += bonusThreat
		}
	})

	aura := shaman.RegisterAura(core.Aura{
		Label:    "Rockbiter Imbue",
		Duration: duration,
	})

	shaman.RegisterOnItemSwapWithImbue(enchantId, &procMask, aura)
}

func (shaman *Shaman) ApplyRockbiterImbue(procMask core.ProcMask) {
	if procMask.Matches(core.ProcMaskMeleeMH) && shaman.HasMHWeapon() {
		shaman.ApplyRockbiterImbueToItem(shaman.MainHand())
	}

	if procMask.Matches(core.ProcMaskMeleeOH) && shaman.HasOHWeapon() {
		shaman.ApplyRockbiterImbueToItem(shaman.OffHand())
	}
}

func (shaman *Shaman) ApplyRockbiterImbueToItem(item *core.Item) {
	if item == nil {
		return
	}

	rank := RockbiterWeaponRankByLevel[shaman.Level]
	enchantId := RockbiterWeaponEnchantId[rank]

	bonusAP := RockbiterWeaponBonusAP[rank] * []float64{1, 1.07, 1.13, 1.2}[shaman.Talents.ElementalWeapons]

	newStats := stats.Stats{stats.AttackPower: bonusAP}

	item.Stats = item.Stats.Add(newStats)
	item.TempEnchant = enchantId
}
