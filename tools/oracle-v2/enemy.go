package main

import (
	"encoding/json"
	"fmt"
	"slices"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/stats"
)

// The target's main hand swings at the player when the player tanks it: attack.go's enemy
// ApplyEffects, CalcDamage and spell_outcome.go outcomeEnemyMeleeWhite. Every value they read
// is static in scope, so each is exported resolved, as Go computes it at reset; the exporter
// rejects any aura whose activation would change one.
type Enemy struct {
	ActionID             *ActionID `json:"action_id"`
	School               uint8     `json:"school"`
	SwingSpeed           float64   `json:"swing_speed"`
	MeleeHasteMultiplier float64   `json:"melee_haste_multiplier"`
	// EnemyWeaponDamage: BaseDamageMin * (1 + spread * roll + max(0, AP * coefficient)).
	BaseDamageMin          float64 `json:"base_damage_min"`
	DamageSpread           float64 `json:"damage_spread"`
	AttackPower            float64 `json:"attack_power"`
	AttackPowerCoefficient float64 `json:"attack_power_coefficient"`
	// CalcDamage's attacker steps.
	BonusDamage        float64 `json:"bonus_damage"`
	AttackerMultiplier float64 `json:"attacker_multiplier"`
	// The steps that read the player's defenses, by stat aura combination as the stat_auras
	// effect numbers them; one entry without stat auras.
	Rolls []EnemyRolls `json:"rolls"`
	// ThreatFromDamage.
	ThreatMultiplier     float64 `json:"threat_multiplier"`
	FlatThreatBonus      float64 `json:"flat_threat_bonus"`
	UnitThreatMultiplier float64 `json:"unit_threat_multiplier"`
	// The debug line's MAP, RAP and SP.
	LogAttackPower       float64 `json:"log_attack_power"`
	LogRangedAttackPower float64 `json:"log_ranged_attack_power"`
	LogSpellPower        float64 `json:"log_spell_power"`
	// Auras inactive at reset whose activation changes a value above, as "player:label" or
	// "target:label". The gate rejects a build where something in scope activates one.
	ChangingAuras []string `json:"changing_auras"`
}

// The steps of the target's swing that read the player's defenses.
type EnemyRolls struct {
	ArmorMultiplier  float64 `json:"armor_multiplier"`
	BonusDamageTaken float64 `json:"bonus_damage_taken"`
	TargetMultiplier float64 `json:"target_multiplier"`
	// The amount each table step adds to the running chance, zero where a step is skipped.
	MissChance     float64 `json:"miss_chance"`
	DodgeChance    float64 `json:"dodge_chance"`
	ParryChance    float64 `json:"parry_chance"`
	BlockChance    float64 `json:"block_chance"`
	CritChance     float64 `json:"crit_chance"`
	CrushChance    float64 `json:"crush_chance"`
	BlockReduction float64 `json:"block_reduction"`
}

// The target's swings as the simulation's current state resolves them.
func enemyValues(simulation *core.Simulation, character *core.Character, target *core.Unit) Enemy {
	player := &character.Unit
	table := target.AttackTables[player.UnitIndex]
	spell := target.AutoAttacks.MHAuto()
	weapon := target.AutoAttacks.MH()
	pseudo := &player.PseudoStats
	physical := spell.SpellSchool.Matches(core.SpellSchoolPhysical)

	bonus := 0.0
	if spell.BonusCoefficient > 0 {
		bonus = spell.BonusCoefficient * spell.BonusDamage(table)
	} else if physical {
		bonus = spell.BonusDamage(table)
	}
	armor, _ := spell.ResistanceMultiplier(simulation, false, table)
	bonusTaken := 0.0
	if !spell.Flags.Matches(core.SpellFlagIgnoreTargetModifiers) && physical {
		bonusTaken = pseudo.BonusPhysicalDamageTaken
	}
	miss := player.GetTotalChanceToBeMissedAsDefender(table) - target.GetStat(stats.PhysicalHitPercent)/100
	dodge, parry, block := 0.0, 0.0, 0.0
	if !pseudo.Stunned {
		dodge = max(player.GetTotalDodgeChanceAsDefender(spell, table), 0.0)
		if pseudo.CanParry {
			parry = player.GetTotalParryChanceAsDefender(spell, table)
		}
		if pseudo.CanBlock && physical {
			block = player.GetTotalBlockChanceAsDefender(table)
		}
	}
	critPercent := target.GetStat(stats.PhysicalCritPercent) + spell.BonusCritPercent
	crit := max(critPercent/100-table.MeleeCritSuppression-pseudo.ReducedCritTakenPercent, 0)
	crush := 0.0
	if target.PseudoStats.CanCrush {
		crush = max(0, table.BaseCrushChance)
	}
	return Enemy{
		ActionID: actionID(spell.ActionID), School: uint8(spell.SpellSchool),
		SwingSpeed: weapon.SwingSpeed, MeleeHasteMultiplier: target.TotalMeleeHasteMultiplier(),
		BaseDamageMin: weapon.BaseDamageMin, DamageSpread: target.PseudoStats.DamageSpread,
		AttackPower: max(0, target.GetStat(stats.AttackPower)), AttackPowerCoefficient: core.EnemyAutoAttackAPCoefficient,
		BonusDamage: bonus, AttackerMultiplier: spell.AttackerDamageMultiplier(table, false),
		Rolls: []EnemyRolls{{ArmorMultiplier: armor, BonusDamageTaken: bonusTaken,
			TargetMultiplier: spell.TargetDamageMultiplier(simulation, table, false),
			MissChance:       max(0, miss), DodgeChance: dodge, ParryChance: parry, BlockChance: block,
			CritChance: crit, CrushChance: crush, BlockReduction: player.BlockDamageReduction()}},
		ThreatMultiplier: spell.ThreatMultiplier, FlatThreatBonus: spell.FlatThreatBonus,
		UnitThreatMultiplier: target.PseudoStats.ThreatMultiplier,
		LogAttackPower:       target.GetStat(stats.AttackPower), LogRangedAttackPower: target.GetStat(stats.RangedAttackPower),
		LogSpellPower: spell.SpellDamage(player),
	}
}

func exportEnemy(request *proto.RaidSimRequest, statAuras []string, character *core.Character, target *core.Unit, unrepresented *[]string) *Enemy {
	note := func(condition bool, reason string) {
		if condition {
			*unrepresented = append(*unrepresented, reason)
		}
	}
	player := &character.Unit
	table := target.AttackTables[player.UnitIndex]
	spell := target.AutoAttacks.MHAuto()
	note(target.AutoAttacks.IsDualWielding, "a target that dual wields is unsupported")
	note(player.Blockhandler != nil, "a block handler is unsupported")
	note(table.DamageDoneByCasterMultiplier != nil || len(table.DamageDoneByCasterExtraMultiplier) != 0,
		"caster damage callbacks on the target's attacks are unsupported")
	note(spell.Flags.Matches(core.SpellFlagNoLogs|core.SpellFlagNoOnDamageDealt), "the target's swing has unsupported flags")
	weapon := target.AutoAttacks.MH()
	distance := target.DistanceFromTarget
	note(!((weapon.MinRange == 0 || weapon.MinRange < distance) && (weapon.MaxRange == 0 || weapon.MaxRange >= distance)),
		"a target out of melee range is unsupported")
	// Absorb shields register a damage taken modifier that acts only while their aura is up.
	// Each modifier must leave a hit unchanged at reset; what activates the shields is a spell or
	// listener the gate checks.
	if acting := actingDamageTakenModifiers(request); acting > 0 {
		*unrepresented = append(*unrepresented, fmt.Sprintf("%d damage taken modifiers on the player act at reset", acting))
	}
	values, changing := enemyAtReset(request, statAuras)
	values.ChangingAuras = changing
	// The rolls under every stat aura combination; nothing else of the swing may change.
	if len(statAuras) > 0 {
		values.Rolls = nil
		for mask := 0; mask < 1<<len(statAuras); mask++ {
			simulation := core.NewSim(request, simsignals.CreateSignals())
			simulation.Reset()
			character := simulation.Raid.Parties[0].Players[0].GetCharacter()
			for bit, label := range statAuras {
				if mask&(1<<bit) != 0 {
					character.GetAura(label).Activate(simulation)
				}
			}
			combo := enemyValues(simulation, character, simulation.Encounter.ActiveTargetUnits[0])
			values.Rolls = append(values.Rolls, combo.Rolls[0])
			combo.Rolls, combo.ChangingAuras = values.Rolls, values.ChangingAuras
			note(encodeEnemy(combo) != encodeEnemy(values), "stat auras change the target's swing beyond its rolls")
		}
	}
	return &values
}

func encodeEnemy(values Enemy) string {
	out, err := json.Marshal(values)
	if err != nil {
		fail(err)
	}
	return string(out)
}

// The target's swings at reset, and the player and target auras inactive at reset whose
// activation, at one stack or at most, changes one of their values. Each aura is checked in a
// separate reset simulation.
func enemyAtReset(request *proto.RaidSimRequest, statAuras []string) (Enemy, []string) {
	encode := encodeEnemy
	fresh := func() (*core.Simulation, *core.Character, *core.Unit) {
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		return simulation, simulation.Raid.Parties[0].Players[0].GetCharacter(), simulation.Encounter.ActiveTargetUnits[0]
	}
	simulation, character, target := fresh()
	base := enemyValues(simulation, character, target)
	baseline := encode(base)
	type candidate struct {
		player bool
		label  string
	}
	name := func(c candidate) string {
		if c.player {
			return "player:" + c.label
		}
		return "target:" + c.label
	}
	candidates := []candidate{}
	for _, unit := range []*core.Unit{&character.Unit, target} {
		for _, aura := range unit.GetAuras() {
			if !aura.IsActive() && !(unit == &character.Unit && slices.Contains(statAuras, aura.Label)) {
				candidates = append(candidates, candidate{unit == &character.Unit, aura.Label})
			}
		}
	}
	changed := []string{}
	base.ChangingAuras = []string{}
	for _, c := range candidates {
		differs, ok := func() (differs bool, ok bool) {
			defer func() {
				if recover() != nil {
					ok = false
				}
			}()
			simulation, character, target := fresh()
			unit := target
			if c.player {
				unit = &character.Unit
			}
			aura := unit.GetAura(c.label)
			aura.Activate(simulation)
			if encode(enemyValues(simulation, character, target)) != baseline {
				return true, true
			}
			if aura.MaxStacks > 1 {
				aura.SetStacks(simulation, aura.MaxStacks)
				if encode(enemyValues(simulation, character, target)) != baseline {
					return true, true
				}
			}
			return false, true
		}()
		if differs || !ok {
			changed = append(changed, name(c))
		}
	}
	return base, changed
}

// How many of the player's dynamic damage taken modifiers change a target melee hit in a
// separate reset simulation.
func actingDamageTakenModifiers(request *proto.RaidSimRequest) int {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	player := simulation.Raid.Parties[0].Players[0].GetCharacter()
	target := simulation.Encounter.ActiveTargetUnits[0]
	spell := target.AutoAttacks.MHAuto()
	acting := 0
	for _, modifier := range player.DynamicDamageTakenModifiers {
		result := &core.SpellResult{Target: &player.Unit, Damage: 1000, Outcome: core.OutcomeHit}
		modifier(simulation, spell, result, false)
		if result.Damage != 1000 {
			acting++
		}
	}
	return acting
}
