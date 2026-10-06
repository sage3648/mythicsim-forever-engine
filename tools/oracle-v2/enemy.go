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
	// unit.go TotalMeleeHasteMultiplier's factors on the target: its attack speed and melee
	// speed multipliers and the haste rating term, which a slow multiplies through
	// MultiplyMeleeSpeed.
	AttackSpeedMultiplier      float64 `json:"attack_speed_multiplier"`
	MeleeSpeedMultiplier       float64 `json:"melee_speed_multiplier"`
	MeleeHasteRatingMultiplier float64 `json:"melee_haste_rating_multiplier"`
	// Player auras inactive at reset whose activation changes only the player's damage taken
	// multiplier, which the runtime reads live, as "player:label".
	DamageTakenAuras []string `json:"damage_taken_auras,omitempty"`
	// Target auras inactive at reset whose activation changes only the target's melee speed
	// multiplier, as "target:label".
	SpeedAuras []string `json:"speed_auras,omitempty"`
	// Player auras inactive at reset whose activation changes only the swing's school damage
	// taken multiplier and so its target multiplier, as "player:label".
	SchoolDamageTakenAuras []string `json:"school_damage_taken_auras,omitempty"`
	// The rolls while a hardcast holds the tank's reduced avoidance aura (gcd.go
	// newHardcastAction), by stat aura combination.
	ReducedAvoidanceRolls []EnemyRolls `json:"reduced_avoidance_rolls,omitempty"`
	// Target auras inactive at reset whose activation changes only the target's attack power,
	// with the swing's attack power and the debug line's MAP while each is active alone.
	AttackPowerAuras []EnemyAttackPowerAura `json:"attack_power_auras,omitempty"`
}

// A target aura that changes only the target's attack power, as a debuff a player's talent puts
// on it does.
type EnemyAttackPowerAura struct {
	Aura           string  `json:"aura"`
	AttackPower    float64 `json:"attack_power"`
	LogAttackPower float64 `json:"log_attack_power"`
}

// The steps of the target's swing that read the player's defenses.
type EnemyRolls struct {
	ArmorMultiplier  float64 `json:"armor_multiplier"`
	BonusDamageTaken float64 `json:"bonus_damage_taken"`
	TargetMultiplier float64 `json:"target_multiplier"`
	// TargetDamageMultiplier's factors besides the player's damage taken multiplier: the
	// physical school's and the attack table's. Present when the runtime may read the
	// player's damage taken multiplier live: every stat aura that changes it is one an
	// effect multiplies it with.
	SchoolDamageTakenMultiplier *float64 `json:"school_damage_taken_multiplier,omitempty"`
	TableDamageTakenMultiplier  *float64 `json:"table_damage_taken_multiplier,omitempty"`
	// The amount each table step adds to the running chance, zero where a step is skipped.
	MissChance     float64 `json:"miss_chance"`
	DodgeChance    float64 `json:"dodge_chance"`
	ParryChance    float64 `json:"parry_chance"`
	BlockChance    float64 `json:"block_chance"`
	CritChance     float64 `json:"crit_chance"`
	CrushChance    float64 `json:"crush_chance"`
	BlockReduction float64 `json:"block_reduction"`
	// BlockDamageReduction's factors, the block value and its multiplier, whose product
	// spell_outcome.go applyEnemyAttackTableBlock fuses into the subtraction.
	BlockValue           float64 `json:"block_value"`
	BlockValueMultiplier float64 `json:"block_value_multiplier"`
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
	schoolTaken := 1.0
	if physical {
		schoolTaken = pseudo.SchoolDamageTakenMultiplier[stats.SchoolIndexPhysical]
	}
	// A copy, so the rolls do not keep the simulation's attack table, and with it the whole
	// simulation, alive for every stat aura combination.
	tableTaken := table.DamageTakenMultiplier
	return Enemy{
		ActionID: actionID(spell.ActionID), School: uint8(spell.SpellSchool),
		AttackSpeedMultiplier: target.PseudoStats.AttackSpeedMultiplier, MeleeSpeedMultiplier: target.PseudoStats.MeleeSpeedMultiplier,
		MeleeHasteRatingMultiplier: 1 + target.GetStat(stats.MeleeHasteRating)/(core.PhysicalHasteRatingPerHastePercent*100),
		SwingSpeed:                 weapon.SwingSpeed, MeleeHasteMultiplier: target.TotalMeleeHasteMultiplier(),
		BaseDamageMin: weapon.BaseDamageMin, DamageSpread: target.PseudoStats.DamageSpread,
		AttackPower: max(0, target.GetStat(stats.AttackPower)), AttackPowerCoefficient: core.EnemyAutoAttackAPCoefficient,
		BonusDamage: bonus, AttackerMultiplier: spell.AttackerDamageMultiplier(table, false),
		Rolls: []EnemyRolls{{ArmorMultiplier: armor, BonusDamageTaken: bonusTaken,
			TargetMultiplier:            spell.TargetDamageMultiplier(simulation, table, false),
			SchoolDamageTakenMultiplier: &schoolTaken, TableDamageTakenMultiplier: &tableTaken,
			MissChance: max(0, miss), DodgeChance: dodge, ParryChance: parry, BlockChance: block,
			CritChance: crit, CrushChance: crush, BlockReduction: player.BlockDamageReduction(),
			BlockValue: player.GetStat(stats.BlockValue), BlockValueMultiplier: pseudo.BlockValueMultiplier}},
		ThreatMultiplier: spell.ThreatMultiplier, FlatThreatBonus: spell.FlatThreatBonus,
		UnitThreatMultiplier: target.PseudoStats.ThreatMultiplier,
		LogAttackPower:       target.GetStat(stats.AttackPower), LogRangedAttackPower: target.GetStat(stats.RangedAttackPower),
		LogSpellPower: spell.SpellDamage(player),
	}
}

func exportEnemy(request *proto.RaidSimRequest, statAuras []string, combos *enemyCombos, trackedDamageTaken []string, character *core.Character, target *core.Unit, unrepresented *[]string) *Enemy {
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
	var values Enemy
	var beyondRolls string
	if combos != nil {
		values, beyondRolls = combos.values, combos.beyondRolls
	} else {
		values, beyondRolls = enemyAtResetValues(request, statAuras)
	}
	// The runtime computes the target multiplier from the player's live damage taken multiplier.
	for _, rolls := range values.Rolls {
		live := character.PseudoStats.DamageTakenMultiplier * *rolls.SchoolDamageTakenMultiplier * *rolls.TableDamageTakenMultiplier
		note(spell.Flags.Matches(core.SpellFlagIgnoreTargetModifiers) || live != rolls.TargetMultiplier,
			"the target's swing has a target multiplier beyond its factors")
	}
	// The runtime reads the damage taken multiplier live only while every stat aura that
	// changes it is one an effect multiplies it with; otherwise the rolls carry it.
	live := true
	baseDamageTaken := character.PseudoStats.DamageTakenMultiplier
	note(values.AttackSpeedMultiplier*values.MeleeSpeedMultiplier*values.MeleeHasteRatingMultiplier != values.MeleeHasteMultiplier,
		"the target's melee haste is beyond its factors")
	if combos != nil {
		// The rolls under every stat aura combination, and under each with the hardcast's
		// reduced avoidance aura, which the stat auras effect read from the same simulations;
		// nothing else of the swing may change.
		values.Rolls = combos.rolls
		for bit, label := range statAuras {
			if combos.aloneDamageTaken[bit] != baseDamageTaken && !slices.Contains(trackedDamageTaken, label) {
				live = false
			}
		}
		for range combos.changed {
			note(true, "stat auras change the target's swing beyond its rolls")
		}
		values.ReducedAvoidanceRolls = combos.reducedRolls
		for range combos.reducedChanged {
			note(true, "reduced avoidance changes the target's swing beyond its rolls")
		}
	} else if character.HardcastAvoidanceAura != nil {
		// gcd.go newHardcastAction: a tank's hardcast holds the reduced avoidance aura until the cast
		// completes; read the rolls with it active.
		simulation := core.NewSim(request, simsignals.CreateSignals())
		simulation.Reset()
		character := simulation.Raid.Parties[0].Players[0].GetCharacter()
		character.HardcastAvoidanceAura.Activate(simulation)
		combo := enemyValues(simulation, character, simulation.Encounter.ActiveTargetUnits[0])
		values.ReducedAvoidanceRolls = append(values.ReducedAvoidanceRolls, combo.Rolls[0])
		note(encodeBeyondRolls(combo) != beyondRolls, "reduced avoidance changes the target's swing beyond its rolls")
	}
	if !live {
		for _, table := range [][]EnemyRolls{values.Rolls, values.ReducedAvoidanceRolls} {
			for i := range table {
				table[i].SchoolDamageTakenMultiplier, table[i].TableDamageTakenMultiplier = nil, nil
			}
		}
		values.ChangingAuras = append(values.ChangingAuras, values.DamageTakenAuras...)
		values.ChangingAuras = append(values.ChangingAuras, values.SchoolDamageTakenAuras...)
		values.DamageTakenAuras, values.SchoolDamageTakenAuras = nil, nil
	}
	return &values
}

// The target's swing at reset with the aura lists enemyAtReset found, and the encoding of
// everything in it besides the rolls and the aura lists, which every combination must keep.
func enemyAtResetValues(request *proto.RaidSimRequest, statAuras []string) (Enemy, string) {
	values, changing, damageTaken, speed, attackPower, schoolDamageTaken := enemyAtReset(request, statAuras)
	values.ChangingAuras = changing
	values.DamageTakenAuras, values.SpeedAuras = damageTaken, speed
	values.SchoolDamageTakenAuras = schoolDamageTaken
	values.AttackPowerAuras = attackPower
	return values, encodeBeyondRolls(values)
}

// What one pass over the stat aura combinations reads of the target's swing. The stat auras
// effect already builds a reset simulation for each combination; reading the swing from the
// same simulation instead of building two more per combination, one for the rolls and one for
// the rolls under a hardcast, gives the same values, since reading them changes nothing.
type enemyCombos struct {
	request   *proto.RaidSimRequest
	statAuras []string
	hardcast  bool
	// The swing at reset and the encoding of its values beyond the rolls and aura lists.
	values      Enemy
	beyondRolls string
	// The rolls by combination, and with the reduced avoidance aura, and how many
	// combinations changed the swing beyond them.
	rolls, reducedRolls     []EnemyRolls
	changed, reducedChanged int
	// The player's damage taken multiplier with only the aura of each bit active.
	aloneDamageTaken []float64
}

func newEnemyCombos(request *proto.RaidSimRequest, statAuras []string, character *core.Character) *enemyCombos {
	values, beyondRolls := enemyAtResetValues(request, statAuras)
	return &enemyCombos{request: request, statAuras: statAuras, hardcast: character.HardcastAvoidanceAura != nil,
		values: values, beyondRolls: beyondRolls, aloneDamageTaken: make([]float64, len(statAuras))}
}

// Reads one combination: the stat auras effect's simulation, or when its own setup left a
// different state than setStatAuras does, one set up here.
func (c *enemyCombos) read(mask int, simulation *core.Simulation, player *core.Character, exact bool) {
	if !exact {
		simulation = core.NewSim(c.request, simsignals.CreateSignals())
		simulation.Reset()
		player = simulation.Raid.Parties[0].Players[0].GetCharacter()
		setStatAuras(simulation, player, c.statAuras, mask)
	}
	for bit := range c.statAuras {
		if mask == 1<<bit {
			c.aloneDamageTaken[bit] = player.PseudoStats.DamageTakenMultiplier
		}
	}
	combo := enemyValues(simulation, player, simulation.Encounter.ActiveTargetUnits[0])
	c.rolls = append(c.rolls, combo.Rolls[0])
	if encodeBeyondRolls(combo) != c.beyondRolls {
		c.changed++
	}
	// gcd.go newHardcastAction: a tank's hardcast holds the reduced avoidance aura until the cast
	// completes; read the rolls with it active under every stat aura combination.
	if c.hardcast {
		player.HardcastAvoidanceAura.Activate(simulation)
		combo = enemyValues(simulation, player, simulation.Encounter.ActiveTargetUnits[0])
		c.reducedRolls = append(c.reducedRolls, combo.Rolls[0])
		if encodeBeyondRolls(combo) != c.beyondRolls {
			c.reducedChanged++
		}
	}
}

// The stat auras of a combination, as the stat_auras effect sets them: an aura up from the reset,
// such as Bear Form, is down where its bit is clear.
func setStatAuras(simulation *core.Simulation, character *core.Character, statAuras []string, mask int) {
	for bit, label := range statAuras {
		if aura := character.GetAura(label); mask&(1<<bit) == 0 && aura.IsActive() {
			aura.Deactivate(simulation)
		}
	}
	for bit, label := range statAuras {
		if mask&(1<<bit) != 0 && !character.GetAura(label).IsActive() {
			character.GetAura(label).Activate(simulation)
		}
	}
}

// The values with the rolls and the aura lists left out: what a stat aura combination or a
// hardcast must leave unchanged, since the rolls are what they change.
func encodeBeyondRolls(values Enemy) string {
	values.Rolls, values.ReducedAvoidanceRolls = nil, nil
	values.ChangingAuras, values.AttackPowerAuras = nil, nil
	values.DamageTakenAuras, values.SpeedAuras, values.SchoolDamageTakenAuras = nil, nil, nil
	return encodeEnemy(values)
}

func encodeEnemy(values Enemy) string {
	out, err := json.Marshal(values)
	if err != nil {
		fail(err)
	}
	return string(out)
}

// The values with the target multiplier, which the runtime reads from the player's live
// damage taken multiplier, left out.
func withoutTargetMultiplier(values Enemy) Enemy {
	values.Rolls = append([]EnemyRolls{}, values.Rolls...)
	for i := range values.Rolls {
		values.Rolls[i].TargetMultiplier = 0
	}
	return values
}

// The values with the swing's school damage taken and target multipliers, which a racial
// survival aura changes, left out.
func withoutSchoolDamageTaken(values Enemy) Enemy {
	values.Rolls = append([]EnemyRolls{}, values.Rolls...)
	for i := range values.Rolls {
		values.Rolls[i].TargetMultiplier = 0
		values.Rolls[i].SchoolDamageTakenMultiplier = nil
	}
	return values
}

// The values with the target's melee speed, which a slow changes, left out.
func withoutMeleeSpeed(values Enemy) Enemy {
	values.MeleeHasteMultiplier, values.MeleeSpeedMultiplier = 0, 0
	return values
}

// The target's swings at reset, and the player and target auras inactive at reset whose
// activation, at one stack or at most, changes one of their values: those that change only the
// player's damage taken multiplier, those that change only the target's melee speed, target
// auras without stacks that change only its attack power, player auras that change only the
// swing's school damage taken multiplier, and the rest. Each aura is checked in a separate
// reset simulation.
func enemyAtReset(request *proto.RaidSimRequest, statAuras []string) (Enemy, []string, []string, []string, []EnemyAttackPowerAura, []string) {
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
	changed, damageTaken, speed, schoolDamageTaken := []string{}, []string{}, []string{}, []string{}
	attackPower := []EnemyAttackPowerAura{}
	reference := base
	base.ChangingAuras = []string{}
	// How an aura's activation changes the values: 0 not at all, 1 the player's damage taken
	// multiplier only, 2 the target's melee speed only, 4 the swing's school damage taken
	// multiplier only, 3 otherwise.
	classify := func(values Enemy, player bool) int {
		switch {
		case encode(values) == baseline:
			return 0
		case player && encode(withoutTargetMultiplier(values)) == encode(withoutTargetMultiplier(reference)):
			return 1
		case !player && encode(withoutMeleeSpeed(values)) == encode(withoutMeleeSpeed(reference)):
			return 2
		case player && encode(withoutSchoolDamageTaken(values)) == encode(withoutSchoolDamageTaken(reference)):
			return 4
		}
		return 3
	}
	for _, c := range candidates {
		kind, ok := func() (kind int, ok bool) {
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
			values := enemyValues(simulation, character, target)
			kind = classify(values, c.player)
			// A target aura without stacks that changes nothing but the attack power.
			onlyPower := values
			onlyPower.AttackPower, onlyPower.LogAttackPower = base.AttackPower, base.LogAttackPower
			if kind == 3 && !c.player && aura.MaxStacks == 0 && encode(onlyPower) == baseline {
				attackPower = append(attackPower, EnemyAttackPowerAura{Aura: c.label,
					AttackPower: values.AttackPower, LogAttackPower: values.LogAttackPower})
				return 0, true
			}
			// An exclusive category can refuse the activation; such an aura changes nothing.
			if aura.MaxStacks > 1 && aura.IsActive() {
				aura.SetStacks(simulation, aura.MaxStacks)
				switch stacked := classify(enemyValues(simulation, character, target), c.player); {
				case kind == 0:
					kind = stacked
				case stacked != 0 && stacked != kind:
					kind = 3
				}
			}
			return kind, true
		}()
		switch {
		case !ok || kind == 3:
			changed = append(changed, name(c))
		case kind == 1:
			damageTaken = append(damageTaken, name(c))
		case kind == 2:
			speed = append(speed, name(c))
		case kind == 4:
			schoolDamageTaken = append(schoolDamageTaken, name(c))
		}
	}
	return base, changed, damageTaken, speed, attackPower, schoolDamageTaken
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
