//! tools/oracle-v2/enemy.go: the target's main hand swing at a player it tanks, as Go resolves
//! it at reset (attack.go's enemy `ApplyEffects`, `CalcDamage` and spell_outcome.go
//! `outcomeEnemyMeleeWhite`), and the player and target auras whose activation would change it.
//!
//! Every value the swing reads is static in scope, so each is exported resolved. The values
//! that depend on which stat auras are up are read from a separate reset simulation per stat
//! aura combination, and each aura that could change a value is checked in a simulation of its
//! own, as the exporter does.

use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::contracts::prepared_v2::{Enemy, EnemyAttackPowerAura, EnemyRolls};

use super::env::Environment;
use super::sim::{school_array_index, UnitId, PHYSICAL_HASTE_RATING_PER_HASTE_PERCENT};
use super::spell::{school, Spell, SpellFlag};
use super::stats::{Stat, SCHOOL_LEN};
use super::Refusal;

/// Go `EnemyAutoAttackAPCoefficient`.
const ENEMY_AUTO_ATTACK_AP_COEFFICIENT: f64 = 0.00052;
/// Go `DefenseRatingPerDefenseLevel` and `MissDodgeParryBlockCritChancePerDefense`.
use super::character::constants::{
    DEFENSE_RATING_PER_DEFENSE_LEVEL, MISS_DODGE_PARRY_BLOCK_CRIT_CHANCE_PER_DEFENSE,
};

fn refuse(reason: impl Into<String>) -> Refusal {
    Refusal::new("tanking", reason.into())
}

/// Go `spell.schoolValue`.
fn school_value(spell: &Spell, values: &[f64; SCHOOL_LEN]) -> f64 {
    if spell.spell_school == school::FROSTFIRE {
        return values[school_array_index(super::stats::SchoolIndex::Fire)]
            .max(values[school_array_index(super::stats::SchoolIndex::Frost)]);
    }
    values[school_array_index(spell.school_index)]
}

/// The steps of the target's swing that read the player's defenses, and everything else it
/// reads, as the simulation's current state resolves them: `enemyValues` for one target.
///
/// A swing of a school other than Physical rolls a partial resist from the simulation's random
/// stream, which preparation does not carry, so it is refused rather than approximated.
pub(crate) fn enemy_values(env: &Environment, target: UnitId) -> Result<Enemy, Refusal> {
    let sim = &env.sim;
    let player = env.player;
    let table = env.attack_table(target, player);
    let attacker = sim.unit(target);
    let defender = sim.unit(player);
    let spell_id = attacker
        .auto_attacks
        .mh_spell
        .ok_or_else(|| refuse("the tanked target has no main hand swing"))?;
    let spell = sim.spell(spell_id);
    let weapon = &attacker.auto_attacks.mh;
    let pseudo = &defender.pseudo_stats;
    let physical = spell.spell_school & school::PHYSICAL != 0;
    if !physical
        && !spell
            .flags
            .matches(SpellFlag::IGNORE_RESISTS | SpellFlag::BINARY)
    {
        return Err(refuse(
            "a target swing that is not Physical rolls partial resists, which are not prepared",
        ));
    }

    // spell_result.go BonusDamage: the physical branch only, which the guard above leaves.
    let bonus_damage =
        |spell: &Spell| spell.bonus_base_damage + attacker.stats[Stat::PhysicalDamage];
    let mut bonus = 0.0;
    if spell.bonus_coefficient > 0.0 {
        if !physical {
            return Err(refuse(
                "a target swing that is not Physical has bonus damage",
            ));
        }
        bonus = spell.bonus_coefficient * bonus_damage(spell);
    } else if physical {
        bonus = bonus_damage(spell);
    }
    let armor_multiplier = resistance_multiplier(env, target, spell);
    let bonus_taken = if !spell.flags.matches(SpellFlag::IGNORE_TARGET_MODIFIERS) && physical {
        pseudo.bonus_physical_damage_taken
    } else {
        0.0
    };

    // unit.go GetTotalChanceToBeMissedAsDefender, less the attacker's hit.
    let miss_total = (table.base_miss_chance
        + pseudo.reduced_physical_hit_taken_chance / 100.0
        + (defender.stats[Stat::DefenseRating] / DEFENSE_RATING_PER_DEFENSE_LEVEL).floor()
            * MISS_DODGE_PARRY_BLOCK_CRIT_CHANCE_PER_DEFENSE
            / 100.0)
        .max(0.0);
    let miss = miss_total - attacker.stats[Stat::PhysicalHitPercent] / 100.0;
    let (mut dodge, mut parry, mut block) = (0.0, 0.0, 0.0);
    if !pseudo.stunned {
        // spell_result.go DodgeParrySuppression.
        let suppression =
            (attacker.stats[Stat::ExpertisePercent] + spell.bonus_expertise_percent) / 100.0;
        let dodge_chance = (pseudo.base_dodge_chance
            + table.base_dodge_chance
            + defender.stats[Stat::DodgePercent] / 100.0
            - suppression
            - attacker.pseudo_stats.dodge_reduction)
            .max(0.0);
        dodge = dodge_chance.max(0.0);
        if pseudo.can_parry {
            parry = (pseudo.base_parry_chance
                + table.base_parry_chance
                + defender.stats[Stat::ParryPercent] / 100.0
                - suppression)
                .max(0.0);
        }
        if pseudo.can_block && physical {
            block = (pseudo.base_block_chance
                + table.base_block_chance
                + defender.stats[Stat::BlockPercent] / 100.0)
                .max(0.0);
        }
    }
    let crit_percent = attacker.stats[Stat::PhysicalCritPercent] + spell.bonus_crit_percent;
    let crit =
        (crit_percent / 100.0 - table.melee_crit_suppression - pseudo.reduced_crit_taken_percent)
            .max(0.0);
    let crush = if attacker.pseudo_stats.can_crush {
        table.base_crush_chance.max(0.0)
    } else {
        0.0
    };
    let school_taken = if physical {
        pseudo.school_damage_taken_multiplier
            [school_array_index(super::stats::SchoolIndex::Physical)]
    } else {
        1.0
    };
    // spell_result.go TargetDamageMultiplier.
    let target_multiplier = if spell.flags.matches(SpellFlag::IGNORE_TARGET_MODIFIERS) {
        1.0
    } else {
        let mut multiplier = pseudo.damage_taken_multiplier
            * school_value(spell, &pseudo.school_damage_taken_multiplier)
            * table.damage_taken_multiplier;
        if spell.flags.matches(SpellFlag::DISEASE) {
            multiplier *= pseudo.disease_damage_taken_multiplier;
        }
        multiplier
    };
    // spell_result.go AttackerDamageMultiplier.
    let attacker_multiplier = if spell.flags.matches(SpellFlag::IGNORE_ATTACKER_MODIFIERS) {
        1.0
    } else {
        attacker.pseudo_stats.damage_dealt_multiplier
            * school_value(spell, &attacker.pseudo_stats.school_damage_dealt_multiplier)
            * table.damage_dealt_multiplier
            * spell.damage_multiplier
            * (spell.damage_multiplier_additive + spell.direct_damage_multiplier_additive)
    };
    let rolls = EnemyRolls {
        armor_multiplier,
        bonus_damage_taken: bonus_taken,
        target_multiplier,
        school_damage_taken_multiplier: Some(school_taken),
        table_damage_taken_multiplier: Some(table.damage_taken_multiplier),
        miss_chance: miss.max(0.0),
        dodge_chance: dodge,
        parry_chance: parry,
        block_chance: block,
        crit_chance: crit,
        crush_chance: crush,
        block_reduction: defender.stats[Stat::BlockValue] * pseudo.block_value_multiplier,
        block_value: Some(defender.stats[Stat::BlockValue]),
        block_value_multiplier: Some(pseudo.block_value_multiplier),
    };
    Ok(Enemy {
        action_id: spell.action_id.clone(),
        school: spell.spell_school,
        swing_speed: weapon.swing_speed,
        melee_haste_multiplier: sim.total_melee_haste_multiplier(target),
        base_damage_min: weapon.base_damage_min,
        damage_spread: attacker.pseudo_stats.damage_spread,
        attack_power: attacker.stats[Stat::AttackPower].max(0.0),
        attack_power_coefficient: ENEMY_AUTO_ATTACK_AP_COEFFICIENT,
        bonus_damage: bonus,
        attacker_multiplier,
        rolls: vec![rolls],
        threat_multiplier: spell.threat_multiplier,
        flat_threat_bonus: spell.flat_threat_bonus,
        unit_threat_multiplier: attacker.pseudo_stats.threat_multiplier,
        log_attack_power: attacker.stats[Stat::AttackPower],
        log_ranged_attack_power: attacker.stats[Stat::RangedAttackPower],
        // spell.SpellDamage(player): the target's spell damage, the spell's own and the swing's
        // school bonus, which the Physical school has none of.
        log_spell_power: attacker.stats[Stat::SpellDamage] + spell.bonus_spell_damage + 0.0,
        changing_auras: Vec::new(),
        reduced_avoidance_rolls: Vec::new(),
        attack_speed_multiplier: Some(attacker.pseudo_stats.attack_speed_multiplier),
        melee_speed_multiplier: Some(attacker.pseudo_stats.melee_speed_multiplier),
        melee_haste_rating_multiplier: Some(
            1.0 + attacker.stats[Stat::MeleeHasteRating]
                / (PHYSICAL_HASTE_RATING_PER_HASTE_PERCENT * 100.0),
        ),
        damage_taken_auras: Vec::new(),
        speed_auras: Vec::new(),
        school_damage_taken_auras: Vec::new(),
        attack_power_auras: Vec::new(),
    })
}

/// spell_resistances.go `ResistanceMultiplier` for a swing that is not periodic: the armor
/// reduction of a Physical swing, or one for a school that ignores resists.
fn resistance_multiplier(env: &Environment, target: UnitId, spell: &Spell) -> f64 {
    if spell.flags.matches(SpellFlag::IGNORE_RESISTS) || spell.spell_school & school::PHYSICAL == 0
    {
        return 1.0;
    }
    let sim = &env.sim;
    let table = env.attack_table(target, env.player);
    // spell_resistances.go GetArmorDamageModifier.
    if table.ignore_armor {
        return 1.0;
    }
    let ignore_factor = table.armor_ignore_factor.clamp(0.0, 1.0);
    let attacker = sim.unit(target);
    let defender = sim.unit(env.player);
    // Go fuses the level term (line 141) and the ignored share's subtraction (line 142).
    let armor_constant = 85.0_f64.mul_add(f64::from(attacker.level), 400.0);
    let armor = defender.pseudo_stats.armor_multiplier * defender.stats[Stat::Armor];
    let defender_armor = (-armor).mul_add(ignore_factor, armor);
    let defender_armor = (defender_armor - attacker.stats[Stat::ArmorPenetration]).max(0.0);
    (1.0 - defender_armor / (defender_armor + armor_constant)).max(0.25)
}

/// Go's JSON encoding of the values, which the exporter compares to tell whether an aura
/// changes any: equal encodings are equal values, down to the sign of a zero.
pub(crate) fn encode(values: &Enemy) -> String {
    serde_json::to_string(values).expect("the values serialize")
}

/// The values with the rolls and the aura lists left out: what a stat aura combination or a
/// hardcast must leave unchanged, since the rolls are what they change.
pub(crate) fn encode_beyond_rolls(values: &Enemy) -> String {
    let mut values = values.clone();
    values.rolls.clear();
    values.reduced_avoidance_rolls.clear();
    values.changing_auras.clear();
    values.attack_power_auras.clear();
    values.damage_taken_auras.clear();
    values.speed_auras.clear();
    values.school_damage_taken_auras.clear();
    encode(&values)
}

/// The values with the target multiplier, which the runtime reads from the player's live
/// damage taken multiplier, left out.
fn without_target_multiplier(values: &Enemy) -> Enemy {
    let mut values = values.clone();
    for rolls in &mut values.rolls {
        rolls.target_multiplier = 0.0;
    }
    values
}

/// The values with the swing's school damage taken and target multipliers, which a racial
/// survival aura changes, left out.
fn without_school_damage_taken(values: &Enemy) -> Enemy {
    let mut values = values.clone();
    for rolls in &mut values.rolls {
        rolls.target_multiplier = 0.0;
        rolls.school_damage_taken_multiplier = None;
    }
    values
}

/// The values with the target's melee speed, which a slow changes, left out.
fn without_melee_speed(values: &Enemy) -> Enemy {
    let mut values = values.clone();
    values.melee_haste_multiplier = 0.0;
    values.melee_speed_multiplier = Some(0.0);
    values
}

/// What `enemyAtReset` finds: the swing at reset and the auras inactive at reset that change
/// it, sorted by how.
pub(crate) struct AtReset {
    pub values: Enemy,
    pub changing: Vec<String>,
    pub damage_taken: Vec<String>,
    pub speed: Vec<String>,
    pub attack_power: Vec<EnemyAttackPowerAura>,
    pub school_damage_taken: Vec<String>,
}

/// How an aura's activation changes the values: not at all, the player's damage taken
/// multiplier only, the target's melee speed only, the swing's school damage taken multiplier
/// only, or otherwise.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Change {
    None,
    DamageTaken,
    Speed,
    SchoolDamageTaken,
    Other,
}

/// The target's swings at reset, and the player and target auras inactive at reset whose
/// activation, at one stack or at most, changes one of their values. Each aura is checked in a
/// separate reset simulation.
pub(crate) fn enemy_at_reset(env: &Environment, stat_auras: &[String]) -> Result<AtReset, Refusal> {
    let target = env.encounter.targets[0];
    let base = enemy_values(env, target)?;
    let baseline = encode(&base);
    // The player's auras first, then the target's, in their registration order.
    let mut candidates: Vec<(bool, String)> = Vec::new();
    for (player, unit) in [(true, env.player), (false, target)] {
        for aura in &env.sim.unit(unit).auras {
            let aura = env.sim.aura(*aura);
            if !(aura.active || player && stat_auras.contains(&aura.label)) {
                candidates.push((player, aura.label.clone()));
            }
        }
    }
    let classify = |values: &Enemy, player: bool| -> Change {
        if encode(values) == baseline {
            Change::None
        } else if player
            && encode(&without_target_multiplier(values))
                == encode(&without_target_multiplier(&base))
        {
            Change::DamageTaken
        } else if !player
            && encode(&without_melee_speed(values)) == encode(&without_melee_speed(&base))
        {
            Change::Speed
        } else if player
            && encode(&without_school_damage_taken(values))
                == encode(&without_school_damage_taken(&base))
        {
            Change::SchoolDamageTaken
        } else {
            Change::Other
        }
    };
    let mut out = AtReset {
        values: base.clone(),
        changing: Vec::new(),
        damage_taken: Vec::new(),
        speed: Vec::new(),
        attack_power: Vec::new(),
        school_damage_taken: Vec::new(),
    };
    for (player, label) in candidates {
        let name = format!("{}:{label}", if player { "player" } else { "target" });
        // A panic while activating an aura, as Go recovers from, counts as a change.
        let checked = catch_unwind(AssertUnwindSafe(|| {
            check_candidate(env, &base, &baseline, player, &label, &classify)
        }));
        match checked {
            Ok(Ok((Change::Other, _))) | Err(_) => out.changing.push(name),
            Ok(Ok((Change::DamageTaken, _))) => out.damage_taken.push(name),
            Ok(Ok((Change::Speed, _))) => out.speed.push(name),
            Ok(Ok((Change::SchoolDamageTaken, _))) => out.school_damage_taken.push(name),
            Ok(Ok((Change::None, Some(power)))) => out.attack_power.push(power),
            Ok(Ok((Change::None, None))) => {}
            Ok(Err(refusal)) => return Err(refusal),
        }
    }
    Ok(out)
}

type Classifier<'a> = dyn Fn(&Enemy, bool) -> Change + 'a;

/// One candidate's check in a reset simulation of its own. The attack power case is a target
/// aura without stacks that changes nothing but the attack power.
fn check_candidate(
    env: &Environment,
    base: &Enemy,
    baseline: &str,
    player: bool,
    label: &str,
    classify: &Classifier,
) -> Result<(Change, Option<EnemyAttackPowerAura>), Refusal> {
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    let unit = if player { fresh.player } else { target };
    let Some(aura) = fresh.sim.get_aura(unit, label) else {
        return Ok((Change::None, None));
    };
    fresh.sim.activate(aura);
    let values = enemy_values(&fresh, target)?;
    let mut kind = classify(&values, player);
    let mut only_power = values.clone();
    only_power.attack_power = base.attack_power;
    only_power.log_attack_power = base.log_attack_power;
    if kind == Change::Other
        && !player
        && fresh.sim.aura(aura).max_stacks == 0
        && encode(&only_power) == baseline
    {
        return Ok((
            Change::None,
            Some(EnemyAttackPowerAura {
                aura: label.to_string(),
                attack_power: values.attack_power,
                log_attack_power: values.log_attack_power,
            }),
        ));
    }
    // An exclusive category can refuse the activation; such an aura changes nothing.
    let max_stacks = fresh.sim.aura(aura).max_stacks;
    if max_stacks > 1 && fresh.sim.aura(aura).active {
        fresh.sim.set_stacks(aura, max_stacks);
        let stacked = classify(&enemy_values(&fresh, target)?, player);
        if kind == Change::None {
            kind = stacked;
        } else if stacked != Change::None && stacked != kind {
            kind = Change::Other;
        }
    }
    Ok((kind, None))
}

/// The swing at reset with the aura lists `enemy_at_reset` found, and the encoding of
/// everything in it besides the rolls and the aura lists, which every combination must keep.
pub(crate) fn enemy_at_reset_values(
    env: &Environment,
    stat_auras: &[String],
) -> Result<(Enemy, String), Refusal> {
    let found = enemy_at_reset(env, stat_auras)?;
    let mut values = found.values;
    values.changing_auras = found.changing;
    values.damage_taken_auras = found.damage_taken;
    values.speed_auras = found.speed;
    values.school_damage_taken_auras = found.school_damage_taken;
    values.attack_power_auras = found.attack_power;
    let beyond = encode_beyond_rolls(&values);
    Ok((values, beyond))
}

/// The stat auras of a combination, as the stat_auras effect sets them: an aura up from the
/// reset, such as Bear Form, is down where its bit is clear.
pub(crate) fn set_stat_auras(env: &mut Environment, stat_auras: &[String], mask: usize) {
    let player = env.player;
    for (bit, label) in stat_auras.iter().enumerate() {
        if let Some(aura) = env.sim.get_aura(player, label) {
            if mask & (1 << bit) == 0 && env.sim.aura(aura).active {
                env.sim.deactivate(aura);
            }
        }
    }
    for (bit, label) in stat_auras.iter().enumerate() {
        if let Some(aura) = env.sim.get_aura(player, label) {
            if mask & (1 << bit) != 0 && !env.sim.aura(aura).active {
                env.sim.activate(aura);
            }
        }
    }
}

/// What one pass over the stat aura combinations reads of the target's swing. The stat auras
/// effect builds a reset simulation for each combination; reading the swing from the same
/// simulation instead of building two more per combination gives the same values, since
/// reading them changes nothing.
///
/// The stat auras effect calls `read` for each combination, in mask order, with the simulation
/// it set up.
pub(crate) struct EnemyCombos {
    request: crate::contracts::request::Message,
    factory: super::env::AgentFactory,
    stat_auras: Vec<String>,
    hardcast: bool,
    /// The swing at reset and the encoding of its values beyond the rolls and aura lists.
    pub values: Enemy,
    pub beyond_rolls: String,
    /// The rolls by combination, and with the reduced avoidance aura, and how many
    /// combinations changed the swing beyond them.
    pub rolls: Vec<EnemyRolls>,
    pub reduced_rolls: Vec<EnemyRolls>,
    pub changed: usize,
    pub reduced_changed: usize,
    /// The player's damage taken multiplier with only the aura of each bit active.
    pub alone_damage_taken: Vec<f64>,
}

impl EnemyCombos {
    pub(crate) fn new(env: &Environment, stat_auras: &[String]) -> Result<EnemyCombos, Refusal> {
        let (values, beyond_rolls) = enemy_at_reset_values(env, stat_auras)?;
        Ok(EnemyCombos {
            request: env.request.clone(),
            factory: env.factory,
            stat_auras: stat_auras.to_vec(),
            hardcast: env
                .sim
                .character(env.player)
                .hardcast_avoidance_aura
                .is_some(),
            values,
            beyond_rolls,
            rolls: Vec::new(),
            reduced_rolls: Vec::new(),
            changed: 0,
            reduced_changed: 0,
            alone_damage_taken: vec![0.0; stat_auras.len()],
        })
    }

    /// Reads one combination: the stat auras effect's simulation, or when its own setup left a
    /// different state than `set_stat_auras` does, one set up here.
    pub(crate) fn read(
        &mut self,
        mask: usize,
        env: &mut Environment,
        exact: bool,
    ) -> Result<(), Refusal> {
        let mut own: Option<Environment> = None;
        if !exact {
            let mut fresh = Environment::new(&self.request, self.factory)?;
            set_stat_auras(&mut fresh, &self.stat_auras, mask);
            own = Some(fresh);
        }
        let env = own.as_mut().unwrap_or(env);
        for bit in 0..self.stat_auras.len() {
            if mask == 1 << bit {
                self.alone_damage_taken[bit] = env
                    .sim
                    .unit(env.player)
                    .pseudo_stats
                    .damage_taken_multiplier;
            }
        }
        let target = env.encounter.targets[0];
        let combo = enemy_values(env, target)?;
        self.rolls.push(combo.rolls[0].clone());
        if encode_beyond_rolls(&combo) != self.beyond_rolls {
            self.changed += 1;
        }
        // gcd.go newHardcastAction: a tank's hardcast holds the reduced avoidance aura until
        // the cast completes; read the rolls with it active under every stat aura combination.
        if self.hardcast {
            if let Some(aura) = env.sim.character(env.player).hardcast_avoidance_aura {
                env.sim.activate(aura);
            }
            let combo = enemy_values(env, target)?;
            self.reduced_rolls.push(combo.rolls[0].clone());
            if encode_beyond_rolls(&combo) != self.beyond_rolls {
                self.reduced_changed += 1;
            }
        }
        Ok(())
    }
}

/// How many of the player's dynamic damage taken modifiers change a target melee hit at
/// reset. The only modifiers a player registers are the absorption shields', which act only
/// while their aura is active; an absorbing aura that is up at reset acts.
pub(crate) fn acting_damage_taken_modifiers(env: &Environment) -> usize {
    let sim = &env.sim;
    sim.unit(env.player)
        .absorption_auras
        .iter()
        .filter(|aura| sim.aura(**aura).active)
        .count()
}

/// Go `exportEnemy`: the target's swings at the player when the player tanks it, with the
/// refusals that keep the runtime's reading of them sound. `tracked` are the auras whose
/// damage taken multiplier an effect multiplies, which the runtime tracks live.
/// Go's `note` of the exporter: records why a request is unrepresented.
fn note(unrepresented: &mut Vec<String>, condition: bool, reason: &str) {
    if condition {
        unrepresented.push(reason.to_string());
    }
}

pub(crate) fn export_enemy(
    env: &Environment,
    stat_auras: &[String],
    combos: Option<EnemyCombos>,
    tracked: &[String],
    unrepresented: &mut Vec<String>,
) -> Result<Enemy, Refusal> {
    let sim = &env.sim;
    let player = env.player;
    let target = env.encounter.targets[0];
    let table = env.attack_table(target, player);
    let aa = &sim.unit(target).auto_attacks;
    let spell = sim.spell(
        aa.mh_spell
            .ok_or_else(|| refuse("the tanked target has no main hand swing"))?,
    );
    note(
        unrepresented,
        aa.is_dual_wielding,
        "a target that dual wields is unsupported",
    );
    note(
        unrepresented,
        table.damage_done_by_caster,
        "caster damage callbacks on the target's attacks are unsupported",
    );
    note(
        unrepresented,
        spell
            .flags
            .matches(SpellFlag::NO_LOGS | SpellFlag::NO_ON_DAMAGE_DEALT),
        "the target's swing has unsupported flags",
    );
    let distance = sim.unit(target).distance_from_target;
    let weapon = &aa.mh;
    note(
        unrepresented,
        !((weapon.min_range == 0.0 || weapon.min_range < distance)
            && (weapon.max_range == 0.0 || weapon.max_range >= distance)),
        "a target out of melee range is unsupported",
    );
    // Absorb shields register a damage taken modifier that acts only while their aura is up.
    // Each modifier must leave a hit unchanged at reset; what activates the shields is a spell
    // or listener the gate checks.
    let acting = acting_damage_taken_modifiers(env);
    if acting > 0 {
        unrepresented.push(format!(
            "{acting} damage taken modifiers on the player act at reset"
        ));
    }
    let (mut values, beyond_rolls) = match &combos {
        Some(combos) => (combos.values.clone(), combos.beyond_rolls.clone()),
        None => enemy_at_reset_values(env, stat_auras)?,
    };
    // The runtime computes the target multiplier from the player's live damage taken
    // multiplier.
    let base_damage_taken = sim.unit(player).pseudo_stats.damage_taken_multiplier;
    for rolls in &values.rolls {
        let live = base_damage_taken
            * rolls.school_damage_taken_multiplier.unwrap_or(1.0)
            * rolls.table_damage_taken_multiplier.unwrap_or(1.0);
        note(
            unrepresented,
            spell.flags.matches(SpellFlag::IGNORE_TARGET_MODIFIERS)
                || live != rolls.target_multiplier,
            "the target's swing has a target multiplier beyond its factors",
        );
    }
    // The runtime reads the damage taken multiplier live only while every stat aura that
    // changes it is one an effect multiplies it with; otherwise the rolls carry it.
    let mut live = true;
    let factors = values.attack_speed_multiplier.unwrap_or(1.0)
        * values.melee_speed_multiplier.unwrap_or(1.0)
        * values.melee_haste_rating_multiplier.unwrap_or(1.0);
    note(
        unrepresented,
        factors != values.melee_haste_multiplier,
        "the target's melee haste is beyond its factors",
    );
    if let Some(combos) = combos {
        // The rolls under every stat aura combination, and under each with the hardcast's
        // reduced avoidance aura, which the stat auras effect read from the same simulations;
        // nothing else of the swing may change.
        values.rolls = combos.rolls;
        for (bit, label) in stat_auras.iter().enumerate() {
            if combos.alone_damage_taken[bit] != base_damage_taken
                && !tracked.iter().any(|tracked| tracked == label)
            {
                live = false;
            }
        }
        for _ in 0..combos.changed {
            note(
                unrepresented,
                true,
                "stat auras change the target's swing beyond its rolls",
            );
        }
        values.reduced_avoidance_rolls = combos.reduced_rolls;
        for _ in 0..combos.reduced_changed {
            note(
                unrepresented,
                true,
                "reduced avoidance changes the target's swing beyond its rolls",
            );
        }
    } else if let Some(aura) = sim.character(player).hardcast_avoidance_aura {
        // gcd.go newHardcastAction: a tank's hardcast holds the reduced avoidance aura until
        // the cast completes; read the rolls with it active.
        let mut fresh = env.fresh();
        let _ = aura;
        if let Some(reduced) = fresh.sim.character(fresh.player).hardcast_avoidance_aura {
            fresh.sim.activate(reduced);
        }
        let combo = enemy_values(&fresh, fresh.encounter.targets[0])?;
        values.reduced_avoidance_rolls.push(combo.rolls[0].clone());
        note(
            unrepresented,
            encode_beyond_rolls(&combo) != beyond_rolls,
            "reduced avoidance changes the target's swing beyond its rolls",
        );
    }
    if !live {
        for rolls in values
            .rolls
            .iter_mut()
            .chain(values.reduced_avoidance_rolls.iter_mut())
        {
            rolls.school_damage_taken_multiplier = None;
            rolls.table_damage_taken_multiplier = None;
        }
        let damage_taken = std::mem::take(&mut values.damage_taken_auras);
        let school_taken = std::mem::take(&mut values.school_damage_taken_auras);
        values.changing_auras.extend(damage_taken);
        values.changing_auras.extend(school_taken);
    }
    Ok(values)
}

/// Auras of races, items and raid buffs whose gain and expiry change stats through
/// `AddStatsDynamic`: melee_procs.go `commonStatAuraLabels`.
const COMMON_STAT_AURA_LABELS: [&str; 9] = [
    "Blood Fury",
    "Elune's Light",
    "Holy Strength (MH)",
    "Holy Strength (OH)",
    "Windfury Totem (External)",
    "Battle Shout (External)",
    "Headmaster's Charge",
    "Crusader's Wrath",
    "Diamond Flask",
];

/// STUB for melee_procs.go `characterStatAuras` until the Hunter agent's `stat_auras.rs` lands:
/// the shared labels the character has, in the exporter's order. The potion, class, spell data
/// proc, Lion Horn and on-use item labels it also lists are not here yet, so a tank with one of
/// those is described without that stat aura until the real function replaces this one.
pub(crate) fn stat_aura_labels_stub(env: &Environment) -> Vec<String> {
    COMMON_STAT_AURA_LABELS
        .iter()
        .filter(|label| env.sim.get_aura(env.player, label).is_some())
        .map(|label| label.to_string())
        .collect()
}

/// STUB for the reader half of melee_procs.go `statAurasEffect` until the Hunter agent's
/// `stat_auras.rs` lands. The real function builds a reset simulation per combination in mask
/// order, sets the stat auras of the mask up, reads their stats and calls
/// [`EnemyCombos::read`] with that simulation; this one does the setting up and the reading.
pub(crate) fn read_stat_aura_combinations_stub(
    env: &Environment,
    labels: &[String],
    combos: &mut EnemyCombos,
) -> Result<(), Refusal> {
    if labels.len() > 10 {
        return Err(refuse(format!(
            "{} stat auras exceed the combination limit",
            labels.len()
        )));
    }
    for mask in 0..1usize << labels.len() {
        let mut fresh = env.fresh();
        set_stat_auras(&mut fresh, labels, mask);
        combos.read(mask, &mut fresh, true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
