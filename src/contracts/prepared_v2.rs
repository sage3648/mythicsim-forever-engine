//! Strict prepared v2 wire types: a reset Go simulation described as data.
//!
//! The pinned Go engine prepares the character, gear, buffs and talents, resets one
//! simulation and exports the resulting state with `tools/oracle-v2`. Static modifiers
//! are already applied to stats and spells. Behavior Rust must execute is named in
//! `effects` with the parameters Go keeps in closures. See docs/prepared-v2.md.
//!
//! Every struct rejects unknown fields and every effect kind is an explicit variant, so
//! a new Go effect or field fails deserialization instead of disappearing.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The `schema_version` that identifies this contract.
pub const SCHEMA_VERSION: u32 = 2;
/// The `contract` name that distinguishes prepared inputs from other v2 documents.
pub const CONTRACT: &str = "forever-prepared";
/// Go's `NeverExpires` duration, used for permanent auras.
pub const NEVER_EXPIRES_NS: i64 = i64::MAX;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedV2 {
    pub schema_version: u32,
    pub contract: String,
    pub reference: Reference,
    /// SHA-256 of the deterministic protobuf encoding of the original request.
    pub request_sha256: String,
    pub scenario_id: String,
    pub sim: SimOptions,
    pub encounter: Encounter,
    pub target: Target,
    pub player: Player,
    pub melee: Melee,
    pub effects: Vec<Effect>,
    /// Request features the exporter could not describe. Must be empty to simulate.
    pub unrepresented: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub engine_revision: String,
    pub client_build: String,
    pub exporter: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SimOptions {
    pub iterations: u32,
    pub seed: i64,
    /// Go `useLabeledRands`: one random stream per label instead of one shared stream.
    pub labeled_rng: bool,
    pub debug_first_iteration: bool,
    /// Go `debug`: log every iteration into one buffer.
    pub debug: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Encounter {
    pub duration_ns: i64,
    pub duration_variation_ns: i64,
    pub execute_proportion_20: f64,
    pub execute_proportion_25: f64,
    pub execute_proportion_35: f64,
    pub execute_proportion_45: f64,
    pub execute_proportion_90: f64,
}

/// Per-school values in Go's `SchoolIndex` order.
#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Schools {
    pub none: f64,
    pub physical: f64,
    pub arcane: f64,
    pub fire: f64,
    pub frost: f64,
    pub holy: f64,
    pub nature: f64,
    pub shadow: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PseudoStats {
    pub spell_cost_percent_modifier: i32,
    pub cast_speed_multiplier: f64,
    pub spirit_regen_rate_casting: f64,
    pub force_full_spirit_regen: bool,
    pub spirit_regen_multiplier: f64,
    pub threat_multiplier: f64,
    pub damage_dealt_multiplier: f64,
    pub school_damage_dealt_multiplier: Schools,
    pub dot_damage_multiplier_additive: f64,
    pub crit_damage_multiplier: f64,
    pub damage_taken_multiplier: f64,
    pub school_damage_taken_multiplier: Schools,
    pub school_bonus_spell_damage: Schools,
    pub school_bonus_hit_chance: Schools,
    pub bonus_spell_damage_taken: f64,
    pub bonus_spell_crit_percent_taken: f64,
    pub reduced_crit_taken_percent: f64,
    pub incapacitated: bool,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(deny_unknown_fields)]
pub struct ActionId {
    #[serde(default, skip_serializing_if = "is_zero")]
    pub spell_id: i32,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub item_id: i32,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub other_id: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub tag: i32,
}

fn is_zero(value: &i32) -> bool {
    *value == 0
}

fn is_zero_usize(value: &usize) -> bool {
    *value == 0
}

fn is_zero_f64(value: &f64) -> bool {
    *value == 0.0
}

impl ActionId {
    pub fn spell(spell_id: i32) -> Self {
        Self {
            spell_id,
            ..Self::default()
        }
    }

    pub fn item(item_id: i32) -> Self {
        Self {
            item_id,
            ..Self::default()
        }
    }

    /// Go `SameActionIgnoreTag`.
    pub fn same_action_ignore_tag(&self, other: &ActionId) -> bool {
        self.spell_id == other.spell_id
            && self.item_id == other.item_id
            && self.other_id == other.other_id
    }
}

impl std::fmt::Display for ActionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.spell_id != 0 {
            write!(f, "spell {}", self.spell_id)?;
        } else if self.item_id != 0 {
            write!(f, "item {}", self.item_id)?;
        } else {
            write!(f, "{}", self.other_id)?;
        }
        if self.tag != 0 {
            write!(f, " tag {}", self.tag)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cooldown {
    /// Shared Go timer identity: spells naming the same timer share one cooldown.
    pub timer: String,
    pub duration_ns: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Aura {
    pub label: String,
    pub tag: Option<String>,
    pub action_id: Option<ActionId>,
    pub action_id_for_proc: Option<ActionId>,
    pub duration_ns: i64,
    pub max_stacks: i32,
    /// Active after the reset, which for an `on_reset` aura means permanent.
    pub active: bool,
    pub stacks: i32,
    pub callbacks: Vec<String>,
    pub icd: Option<Cooldown>,
    pub exclusive_effects: u32,
}

/// Callbacks that react to combat events rather than an aura's own lifetime.
pub const EVENT_CALLBACKS: &[&str] = &[
    "on_apply_effects",
    "on_cast_complete",
    "on_spell_hit_dealt",
    "on_spell_hit_taken",
    "on_periodic_damage_dealt",
    "on_periodic_damage_taken",
    "on_heal_dealt",
    "on_heal_taken",
    "on_periodic_heal_dealt",
    "on_periodic_heal_taken",
    "on_encounter_start",
];

impl Aura {
    pub fn has_event_callbacks(&self) -> bool {
        self.callbacks
            .iter()
            .any(|callback| EVENT_CALLBACKS.contains(&callback.as_str()))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub index: i32,
    pub label: String,
    pub level: i32,
    pub mob_type: Option<String>,
    pub stats: BTreeMap<String, f64>,
    pub pseudo_stats: PseudoStats,
    pub auras: Vec<Aura>,
    /// Go rolls an opening swing offset for an enemy with a melee swing at every reset,
    /// even when the enemy never swings.
    pub auto_swing_melee: bool,
    pub auto_swing_ranged: bool,
    /// Registered actions the target reports with zero metrics, since it never acts.
    pub metrics_actions: Vec<MetricsAction>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Mana {
    pub max: f64,
    pub base: f64,
    pub spirit_regen_per_second: f64,
    /// Go's computed rates at the start of the fight, for preparation checks.
    pub regen_per_second_casting: f64,
    pub regen_per_second_not_casting: f64,
    /// The lowest maximum mana while Go deactivates every aura at the end of a fight.
    /// Each Mana change clamps current mana, and time to OOM reads it afterwards.
    pub teardown_max: f64,
}

/// Go `energyBar` for a player that has one, and its combo points.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Energy {
    pub max_energy: f64,
    pub max_combo_points: i32,
    pub tick_duration_ns: i64,
    pub energy_per_tick: f64,
}

/// A Fireball rank's dot: the base amount its ticks snapshot and whether they can crit.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FireballRank {
    pub spell_id: i32,
    pub tick_base: f64,
    pub tick_can_crit: bool,
}

/// Improved Scorch's stacking Fire Vulnerability buff on the mage.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ImprovedScorch {
    pub aura: String,
    pub proc_chance: f64,
    pub damage_per_stack: f64,
}

/// An action a unit's metrics list, from Go `Spell.doneIteration`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MetricsAction {
    pub action_id: ActionId,
    pub melee_metrics: bool,
    pub school: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AttackTable {
    pub base_spell_miss_chance: f64,
    pub spell_crit_suppression: f64,
    pub bonus_spell_crit_percent: f64,
    pub crit_multiplier: f64,
    pub damage_dealt_multiplier: f64,
    pub damage_taken_multiplier: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cost {
    pub resource: String,
    pub base_cost: i32,
    pub flat_modifier: i32,
    pub percent_modifier: f64,
    pub additive_percent_modifier: f64,
    /// Go `EnergyCost.Refund`: the share of an energy cost a missed strike gives back.
    #[serde(default, skip_serializing_if = "is_zero_f64")]
    pub refund: f64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Cast {
    pub cost: f64,
    pub gcd_ns: i64,
    pub gcd_min_ns: i64,
    pub cast_time_ns: i64,
    pub non_empty: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CastKind {
    /// Go `makeCastFunc`: cost, GCD, hardcast and haste handling.
    Full,
    /// Go `makeCastFuncSimple`: cooldown checks without cost, GCD or hardcast.
    Simple,
    /// Go `makeCastFuncAutosOrProcs`: effects only.
    AutosOrProcs,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Dot {
    /// "target" for a per-target dot, "self" for a self-only or AoE aura.
    pub unit: String,
    pub aura_label: String,
    pub base_tick_count: i32,
    pub base_tick_length_ns: i64,
    pub bonus_coefficient: f64,
    pub periodic_damage_multiplier: f64,
    pub base_duration_multiplier: f64,
    pub base_duration_flat_ns: i64,
    pub affected_by_cast_speed: bool,
    pub affected_by_real_haste: bool,
    pub haste_reduces_duration: bool,
    pub channeled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DamageEffect {
    /// Client amount at the caster level, before variance.
    pub average: f64,
    /// Client variance: the roll is uniform over average x (1 +- variance / 2).
    pub variance: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    pub action_id: Option<ActionId>,
    pub rank: i32,
    /// Go's client school bit mask.
    pub school: u8,
    pub defense_type: String,
    pub proc_mask: Vec<String>,
    pub flags: Vec<String>,
    /// Stable name for the Go class spell mask, absent for non-class spells.
    pub class_spell: Option<String>,
    pub missile_speed: f64,
    pub cost: Option<Cost>,
    pub default_cast: Cast,
    pub cast_kind: CastKind,
    pub ignore_haste: bool,
    pub has_extra_cast_condition: bool,
    pub has_cast_requirement: bool,
    pub min_range: f64,
    pub max_range: f64,
    pub max_charges: i32,
    pub cd: Option<Cooldown>,
    pub shared_cd: Option<Cooldown>,
    pub bonus_hit_percent: f64,
    pub bonus_crit_percent: f64,
    pub bonus_spell_damage: f64,
    pub bonus_expertise_percent: f64,
    pub cast_time_multiplier: f64,
    pub cd_multiplier: f64,
    pub damage_multiplier: f64,
    pub damage_multiplier_additive: f64,
    pub direct_damage_multiplier_additive: f64,
    pub crit_multiplier_pct: f64,
    pub crit_multiplier_additive: f64,
    pub bonus_base_damage: f64,
    pub bonus_coefficient: f64,
    pub threat_multiplier: f64,
    pub flat_threat_bonus: f64,
    pub pushback_resist: f64,
    pub dot: Option<Dot>,
    pub damage_effect: Option<DamageEffect>,
    /// The spellbook position of the spell whose dot Go `Spell.Dot` resolves to when this
    /// spell has none of its own.
    #[serde(default)]
    pub related_dot_spell: Option<usize>,
    /// Go `MetricSplits`: the spell reports one tagged metric entry per split.
    #[serde(default, skip_serializing_if = "is_zero_usize")]
    pub metric_splits: usize,
}

impl Spell {
    pub fn has_flag(&self, flag: &str) -> bool {
        self.flags.iter().any(|f| f == flag)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MajorCooldown {
    pub action_id: ActionId,
    pub priority: i32,
    #[serde(rename = "type")]
    pub kind: Vec<String>,
    pub timings_ns: Vec<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Player {
    pub index: i32,
    pub label: String,
    pub level: i32,
    pub mob_type: Option<String>,
    pub stats: BTreeMap<String, f64>,
    pub pseudo_stats: PseudoStats,
    pub auras: Vec<Aura>,
    pub name: String,
    pub class: String,
    pub race: String,
    pub professions: Vec<String>,
    pub talents_string: String,
    pub talents: BTreeMap<String, i32>,
    pub class_options: serde_json::Value,
    pub reaction_ns: i64,
    pub channel_clip_delay_ns: i64,
    pub distance_yards: f64,
    /// Go `Unit.CastSpeed`, the factor applied to hasted durations.
    pub cast_speed: f64,
    pub mana: Mana,
    /// Go `energyBar`, absent for a player without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<Energy>,
    pub attack_table: AttackTable,
    pub spells: Vec<Spell>,
    /// Go's initial major cooldown order after the rotation claimed its spells.
    pub major_cooldowns: Vec<MajorCooldown>,
    /// The request's APL in protojson form. Interpreted by the rotation module.
    pub rotation: serde_json::Value,
    /// Every prepull action Go registered: the rotation's, and any a class or item adds.
    pub prepull_actions: usize,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManaGain {
    pub min: f64,
    pub spread: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MissileRank {
    pub channel_spell_id: i32,
    pub tick_spell_id: i32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManaGem {
    pub item_id: i32,
    pub mana: f64,
}

/// A Go `Weapon`.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Weapon {
    pub base_damage_min: f64,
    pub base_damage_max: f64,
    pub attack_power_per_dps: f64,
    pub swing_speed: f64,
    pub normalized_swing_speed: f64,
    pub school: u8,
    pub min_range: f64,
    pub max_range: f64,
}

/// The player's weapon attacks and the physical attack table against the target, with the
/// defender's static chances resolved.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Melee {
    pub auto_swing_melee: bool,
    pub auto_swing_ranged: bool,
    pub dual_wielding: bool,
    pub main_hand: Weapon,
    pub off_hand: Weapon,
    pub ranged: Weapon,
    pub base_miss_chance: f64,
    pub base_glance_chance: f64,
    pub glance_multiplier: f64,
    pub glance_spread: f64,
    pub hit_suppression: f64,
    pub melee_crit_suppression: f64,
    pub ignore_armor: bool,
    pub armor_ignore_factor: f64,
    pub in_front_of_target: bool,
    pub attack_speed_multiplier: f64,
    pub melee_speed_multiplier: f64,
    pub dodge_reduction: f64,
    pub disable_dw_miss_penalty: bool,
    pub defender_dodge: f64,
    pub defender_parry: f64,
    pub defender_block: f64,
    pub defender_armor: f64,
    pub defender_block_reduction: f64,
    pub defender_bonus_attack_power: f64,
    pub defender_bonus_physical_damage_taken: f64,
    pub defender_reduced_physical_hit_taken: f64,
}

/// A spell a dynamic proc manager hears, by spellbook position, with the chance it rolls.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SpellChance {
    pub spell: usize,
    pub chance: f64,
}

/// A spell druid.RegisterSpell registered, by spellbook position, with the forms it may be
/// cast in.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DruidFormSpell {
    pub spell: usize,
    pub forms: Vec<String>,
}

/// Behavior Rust must execute, with the parameters Go keeps in closures. Each variant
/// names the Go source that defines it in docs/prepared-v2.md.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Frostbolt {},
    /// The Gnome racial Eureka!: a major cooldown whose three stacks lower the cost and raise
    /// the damage of the spells the class names, resolved into spell positions.
    Eureka {
        spell_id: i32,
        aura: String,
        cost_percent: f64,
        damage_percent: f64,
        tick_cancel_percent: f64,
        cost_spells: Vec<usize>,
        damage_spells: Vec<usize>,
        tick_cancel_spells: Vec<usize>,
        spending_spells: Vec<usize>,
    },
    /// The Undead racial Touch of the Grave: landed hits can drain the target.
    TouchOfTheGrave {
        trigger_aura: String,
        drain_spell_id: i32,
        proc_chance: f64,
        proc_mask: Vec<String>,
        health_fraction: f64,
        delay_ns: i64,
    },
    /// The Troll racial Berserking: a major cooldown whose aura multiplies cast speed. Its
    /// attack speed share has no effect in scope, where player auto attacks are unrepresented.
    Berserking {
        spell_id: i32,
        aura: String,
        cast_speed_multiplier: f64,
    },
    /// The Orc racial Blood Fury: a major cooldown whose aura multiplies stats through Go's
    /// dynamic stat dependencies. `active_stats` holds every stat the aura changes, at the
    /// value Go computes while it is active.
    BloodFury {
        spell_id: i32,
        aura: String,
        active_stats: BTreeMap<String, f64>,
    },
    /// An on-use cooldown whose aura adds flat stats: Go `RegisterTemporaryStatsOnUseCD`, as
    /// the Night Elf racial Elune's Light. `active_stats` holds every stat the aura changes,
    /// at the value Go computes while it is active; `gain_log` and `expire_log` are the lines
    /// its gain and expiry log.
    TemporaryStats {
        spell_id: i32,
        aura: String,
        active_stats: BTreeMap<String, f64>,
        gain_log: String,
        expire_log: String,
    },
    /// The forms the druid starts in and each druid spell may be cast in.
    DruidForms {
        starting_form: Vec<String>,
        spells: Vec<DruidFormSpell>,
    },
    /// Moonkin Form's cast, which activates its aura.
    MoonkinForm {
        spell_id: i32,
        aura: String,
    },
    /// Every Starfire rank's direct hit.
    Starfire {},
    /// Every Wrath rank's hit after travel.
    Wrath {},
    /// Moonfire's hit, which casts its snapshotting dot spell when it lands.
    Moonfire {
        rank: FireballRank,
    },
    /// Insect Swarm's binary hit roll, then its snapshotting dot and the target debuff.
    InsectSwarm {
        rank: FireballRank,
        debuff_aura: String,
    },
    /// Innervate's cast and aura: full spirit regeneration at a multiple, attributed to its own
    /// regeneration metrics.
    Innervate {
        spell_id: i32,
        aura: String,
        spirit_regen_multiplier: f64,
        regen_metrics_action_id: ActionId,
    },
    /// Omen of Clarity: landed spells can grant Clearcasting at a chance from their cast time,
    /// doubled with a halved cooldown in Moonkin Form.
    OmenOfClarity {
        trigger_aura: String,
        aura: String,
        callbacks: Vec<String>,
        outcome: Vec<String>,
        require_damage_dealt: bool,
        trigger_immediately: bool,
        proc_chance: f64,
        trigger_spells: Vec<usize>,
        icd_ns: i64,
        ppm: f64,
        gcd_ns: i64,
        moonkin_chance_multiplier: f64,
        moonkin_cooldown_multiplier: f64,
        cost_spells: Vec<usize>,
        cost_percent_add: f64,
    },
    /// Nature's Grace: a damaging spell crit grants cast speed and a shorter GCD.
    NaturesGrace {
        trigger_aura: String,
        aura: String,
        haste_multiplier: f64,
        gcd_reduction_ns: i64,
        gcd_spells: Vec<usize>,
        trigger_spells: Vec<usize>,
    },
    /// Eclipse: each Wrath banks charges that shorten Starfire's cast.
    Eclipse {
        trigger_aura: String,
        aura: String,
        cast_time_reduction_ns: i64,
        charges_per_wrath: i32,
        duration_ns: i64,
    },
    /// Auras whose gain and expiry change stats through Go's AddStatsDynamic, and the player's
    /// stats Rust reads for every combination of them: entry i has aura j active when bit j
    /// of i is set. `changed` names every stat any combination changes.
    StatAuras {
        auras: Vec<String>,
        combos: Vec<BTreeMap<String, f64>>,
        changed: Vec<String>,
    },
    /// The Crusader weapon enchant: a weapon proc at a per-spell chance that activates the
    /// hand's Holy Strength and heals.
    Crusader {
        trigger_aura: String,
        mh_aura: String,
        oh_aura: String,
        chances: Vec<SpellChance>,
        heal_min: f64,
        heal_max: f64,
        heal_metrics_action_id: ActionId,
        mh_gain_log: String,
        mh_expire_log: String,
        oh_gain_log: String,
        oh_expire_log: String,
    },
    /// Dragonbreath Chili: a chance on landed melee hits to cast a rolled Fire hit, after a
    /// spell batch window.
    DragonbreathChili {
        trigger_aura: String,
        spell_id: i32,
        proc_chance: f64,
        trigger_spells: Vec<usize>,
        roll_min: f64,
        roll_max: f64,
        delay_ns: i64,
    },
    /// The party Windfury Totem: the totem aura refreshed every period holds a trigger that can
    /// grant charges of attack power and cast an extra main hand attack; landed autos spend the
    /// charges.
    WindfuryTotem {
        totem_aura: String,
        period_ns: i64,
        trigger_aura: String,
        trigger_spells: Vec<usize>,
        trigger_outcome: Vec<String>,
        trigger_proc_chance: f64,
        proc_aura: String,
        spend_spells: Vec<usize>,
        spend_outcome: Vec<String>,
        extra_attack_spell: usize,
    },
    /// The raid's Sunder Armor, ramped one stack a period from the pull; target armor at
    /// each stack count, as Go computes it.
    SunderArmorRamp {
        aura: String,
        period_ns: i64,
        ticks: i32,
        armor_by_stacks: Vec<f64>,
        /// A stronger permanent member of the aura's exclusive category, such as the raid's
        /// Expose Armor, blocks every activation, which Go still counts as a proc.
        #[serde(default)]
        blocked: bool,
    },
    /// Paladin judgement.go: a landed melee strike refreshes the active judgement debuffs.
    JudgementRefresh {
        trigger_aura: String,
        proc_mask: Vec<String>,
        judgement_auras: Vec<String>,
    },
    /// Every Mind Blast rank's direct hit.
    MindBlast {},
    /// Every Shadow Word: Death rank's direct hit; Early Demise adds crit in the 20% execute
    /// phase.
    ShadowWordDeath {
        early_demise_crit: f64,
    },
    /// Every Shadow Word: Pain rank: a hit roll without a hit count, then a snapshotting dot
    /// whose ticks roll only a crit.
    ShadowWordPain {
        ranks: Vec<FireballRank>,
    },
    /// Every Devouring Plague rank: Shadow Word: Pain's shape, each tick healing the priest for
    /// its damage under the rank's action ID with this tag.
    DevouringPlague {
        ranks: Vec<FireballRank>,
        heal_metrics_tag: i32,
    },
    /// Every Mind Flay rank: a binary hit roll, then a channel.
    MindFlay {
        ranks: Vec<FireballRank>,
    },
    /// Shadowform's cast and aura: Shadow damage and cost modifiers on `school_spells`, a crit
    /// damage bonus on `crit_spells`, and helpful Holy casts in `cancel_spells` end it.
    Shadowform {
        spell_id: i32,
        aura: String,
        damage_percent: f64,
        cost_percent: f64,
        crit_multiplier: f64,
        school_spells: Vec<usize>,
        crit_spells: Vec<usize>,
        cancel_spells: Vec<usize>,
    },
    /// Inner Focus: the next priest spell is free and gains crit; the cooldown restarts when
    /// the aura ends.
    InnerFocus {
        spell_id: i32,
        aura: String,
        cost_percent: i32,
        crit_percent: f64,
        crit_spells: Vec<usize>,
        spender_spells: Vec<usize>,
    },
    /// Shadow Weaving: landed Shadow spells stack a Shadow damage bonus.
    ShadowWeaving {
        trigger_aura: String,
        aura: String,
        callbacks: Vec<String>,
        outcome: Vec<String>,
        trigger_immediately: bool,
        proc_chance: f64,
        trigger_spells: Vec<usize>,
        damage_per_stack: f64,
        damage_spells: Vec<usize>,
    },
    /// Dark Sacrifice: a self-only periodic mana gain of the client base plus Spirit over a
    /// divisor, a major cooldown used once the whole gain fits.
    DarkSacrifice {
        spell_id: i32,
        aura: String,
        tick_base: f64,
        spirit_divisor: f64,
        metrics_action_id: ActionId,
    },
    /// A registered pet nothing summons: Go resets and dismisses it each fight, logging its
    /// stats, and reports its zero metrics.
    InertPet {
        name: String,
        label: String,
        unit_index: i32,
        metrics_actions: Vec<MetricsAction>,
        auras: Vec<ActionId>,
        dismissed_log: String,
        reason: String,
    },
    /// The Orc racial Shatter Curse: a survival cooldown whose aura lowers the player's
    /// spell damage taken, which has no effect in scope. Go never autocasts it at the
    /// default defensive health threshold; configured timings still cast it.
    ShatterCurse {
        spell_id: i32,
        aura: String,
    },
    /// The Dwarf racial Stoneform: a survival cooldown whose aura lowers the player's
    /// physical damage taken, which has no effect in scope. Go never autocasts it at the
    /// default defensive health threshold; configured timings still cast it.
    Stoneform {
        spell_id: i32,
        aura: String,
    },
    /// The High Order Skyborne racial Read Ley Line: a cast that only a rotation action
    /// uses, whose aura Energized multiplies mana regeneration.
    ReadLeyLine {
        spell_id: i32,
        aura: String,
        regen_multiplier: f64,
    },
    /// Master of Elements: Fire and Frost crits refund part of the base cost.
    MasterOfElements {
        trigger_aura: String,
        refund: f64,
        metrics_action_id: ActionId,
    },
    /// Pyroblast, Fireball's shape at its highest rank.
    Pyroblast {
        spell_id: i32,
        tick_base: f64,
        tick_can_crit: bool,
    },
    /// Heating Up: crits stack a Pyroblast cast time cut.
    HeatingUp {
        aura: String,
        trigger_aura: String,
        cast_time_per_stack: f64,
    },
    /// Combustion's major cooldown and stacking Fire crit buff.
    Combustion {
        spell_id: i32,
        aura: String,
        crit_per_stack: f64,
        max_crits: i32,
    },
    /// Fire Blast's instant hit.
    FireBlast {},
    /// Every Fireball rank: a hit after travel, then a dot that snapshots.
    Fireball {
        ranks: Vec<FireballRank>,
    },
    /// Every Frostfire Bolt rank: Fireball's shape with the Frostfire school.
    FrostfireBolt {
        ranks: Vec<FireballRank>,
    },
    /// Every Scorch rank, with Improved Scorch when talented.
    Scorch {
        #[serde(default)]
        improved_scorch: Option<ImprovedScorch>,
    },
    /// Ignite: crits of fire spells feed a fire dot.
    Ignite {
        trigger_aura: String,
        spell_id: i32,
        share: f64,
        num_ticks: i32,
    },
    /// Arcane Power's major cooldown and aura.
    ArcanePower {
        spell_id: i32,
        aura: String,
        damage: f64,
        cost_percent_add: f64,
    },
    /// Presence of Mind's major cooldown and aura.
    PresenceOfMind {
        spell_id: i32,
        aura: String,
        cast_time_percent: f64,
    },
    /// Arcane Blast and its stacking buff.
    ArcaneBlast {
        spell_id: i32,
        aura: String,
        damage_per_stack: f64,
        cost_per_stack: f64,
    },
    IceLance {
        spell_id: i32,
        frozen_multiplier: f64,
    },
    ArcaneMissiles {
        ranks: Vec<MissileRank>,
    },
    ColdSnap {
        spell_id: i32,
    },
    Evocation {
        spell_id: i32,
        regen_aura: String,
        channel_aura: String,
        regen_multiplier: f64,
    },
    ManaGems {
        gems: Vec<ManaGem>,
        regen_window_seconds: f64,
    },
    MageArmor {
        aura: String,
    },
    ArcaneConcentration {
        talent_rank: i32,
        proc_chance: f64,
        icd_ns: i64,
        trigger_aura: String,
        aura: String,
        aura_duration_ns: i64,
    },
    MissileBarrage {
        trigger_aura: String,
        aura: String,
        arcane_blast_chance: f64,
        bolt_chance: f64,
        rng_label: String,
        cost_percent_add: f64,
        tick_length_delta_ns: i64,
    },
    FingersOfFrost {
        talent_rank: i32,
        shatter_rank: i32,
        proc_chance: f64,
        max_stacks: i32,
        shatter_crit: f64,
        duration_ns: i64,
        trigger_aura: String,
        aura: String,
    },
    WintersChill {
        talent_rank: i32,
        proc_chance: f64,
        max_stacks: i32,
        crit_per_stack: f64,
        duration_ns: i64,
        trigger_aura: String,
        aura: String,
    },
    JudgementOfWisdom {
        aura: String,
        proc_chance: f64,
        proc_mask: Vec<String>,
        mana: f64,
        metrics_action_id: ActionId,
        delay_ns: i64,
    },
    PotionMana {
        item_id: i32,
        rng_label: String,
        gains: Vec<ManaGain>,
        stone_multiplier: f64,
        regen_window_seconds: f64,
    },
    ConjuredMana {
        item_id: i32,
        rng_label: String,
        gains: Vec<ManaGain>,
        selected: bool,
        regen_window_seconds: f64,
    },
    EnergizeOnUse {
        item_id: i32,
        spell_id: i32,
        average: f64,
        variance: f64,
        whole: f64,
    },
    InertListener {
        unit: String,
        aura: String,
        reason: String,
    },
    /// Every Shadow Bolt rank: a hit after travel.
    ShadowBolt {},
    /// Immolate's hit and the snapshot dot on its related spell.
    Immolate {
        spell_id: i32,
        tick_base: f64,
        tick_can_crit: bool,
    },
    /// Corruption's snapshot dot.
    Corruption {
        spell_id: i32,
        tick_base: f64,
        tick_can_crit: bool,
    },
    /// Bane of Agony's ramping snapshot dot: the snapshot pays `ramp_share` of the tick and
    /// every `ramp_every_ticks` ticks adds that share back.
    BaneOfAgony {
        spell_id: i32,
        tick_base: f64,
        tick_can_crit: bool,
        ramp_share: f64,
        ramp_every_ticks: i32,
        #[serde(default)]
        amplify: Option<f64>,
    },
    /// Amplify Curse's major cooldown and aura, spent by Bane of Agony.
    AmplifyCurse {
        spell_id: i32,
        aura: String,
    },
    /// Curse of the Elements' debuff on the target: flat resistance changes and school
    /// damage taken multipliers while it is active.
    CurseOfTheElements {
        spell_id: i32,
        aura: String,
        resistance_delta: BTreeMap<String, f64>,
        school_damage_taken_multiplier: BTreeMap<String, f64>,
    },
    /// Life Tap: (base + Spirit) times the multiplier, as health spent and mana gained.
    LifeTap {
        spell_id: i32,
        base_amount: f64,
        mana_multiplier: f64,
    },
    /// Conflagrate's hit, which consumes Immolate unless Shadow and Flame spares it.
    Conflagrate {
        spell_id: i32,
        keep_immolate_chance: f64,
        rng_label: String,
    },
    /// Shadowburn's instant binary hit.
    Shadowburn {},
    /// Searing Pain's hit.
    SearingPain {},
    /// Soul Fire's hit after travel.
    SoulFire {},
    /// Improved Shadow Bolt: Shadow Bolt crits leave a target debuff that multiplies the
    /// warlock's shadow damage after the outcome.
    ImprovedShadowBolt {
        trigger_aura: String,
        aura: String,
        spell_id: i32,
        multiplier: f64,
        /// Spellbook positions of the spells the trigger listens to.
        trigger_spells: Vec<usize>,
    },
    /// Shadow and Flame: Conflagrate and Shadowburn hits multiply the warlock's shadow or
    /// fire damage dealt for a while.
    ShadowAndFlame {
        trigger_aura: String,
        shadow_aura: String,
        fire_aura: String,
        shadow_spell_id: i32,
        fire_spell_id: i32,
        multiplier: f64,
        /// Spellbook positions of the spells the trigger listens to.
        trigger_spells: Vec<usize>,
        /// The trigger spells that raise shadow damage; the others raise fire damage.
        shadow_spells: Vec<usize>,
    },
    /// Every Lightning Bolt rank: an overload may roll when the bolt lands.
    LightningBolt {
        overload_chance: f64,
        overload_tag: i32,
        rng_label: String,
    },
    /// Every Chain Lightning rank: a third of the overload chance per hit and a bounce
    /// reduction on later targets.
    ChainLightning {
        overload_chance: f64,
        overload_tag: i32,
        rng_label: String,
        bounce_reduction: f64,
        bounce_bonus: f64,
    },
    /// Flame Shock's hit and the dot it applies when it lands.
    FlameShock {
        spell_id: i32,
        tick_base: f64,
        tick_can_crit: bool,
    },
    /// Lava Burst, stronger against a target burning with Flame Shock.
    LavaBurst {
        spell_id: i32,
        flame_shock_bonus: f64,
    },
    /// Fire Nova: one hit on each target from a fixed base.
    FireNova {
        spell_id: i32,
        base_damage: f64,
    },
    /// Searing Totem: a target dot whose ticks cast the totem's attack.
    SearingTotem {
        spell_id: i32,
        attack_spell_id: i32,
        attack_damage: f64,
        magma_totem_aura: String,
        flametongue_totem_aura: String,
    },
    /// Elemental Focus: a completed elemental cast may grant Clearcasting.
    ElementalFocus {
        trigger_aura: String,
        aura: String,
        proc_chance: f64,
        cost_percent_add: f64,
        max_stacks: i32,
    },
    /// Go consumes.go conjured item that restores energy, such as Thistle Tea.
    ConjuredEnergy {
        item_id: i32,
        rng_label: String,
        gains: Vec<ManaGain>,
        selected: bool,
        level_reduction: f64,
    },
    SinisterStrike {
        spell_id: i32,
        base_damage: f64,
    },
    Backstab {
        spell_id: i32,
        base_damage: f64,
        main_hand_dagger: bool,
        extra_combo_point_chance: f64,
        extra_combo_point_action: ActionId,
    },
    Eviscerate {
        spell_id: i32,
        damage_average: f64,
        damage_variance: f64,
        combo_point_damage: f64,
        attack_power_per_combo_point: f64,
    },
    SliceAndDice {
        spell_id: i32,
        aura: String,
        durations_ns: Vec<i64>,
        melee_speed_multiplier: f64,
    },
    BladeFlurry {
        spell_id: i32,
        aura: String,
        attack_speed_multiplier: f64,
    },
    AdrenalineRush {
        spell_id: i32,
        aura: String,
        regen_multiplier: f64,
        energy_threshold: f64,
    },
    RogueFinisher {
        relentless_strikes: bool,
        relentless_strikes_chance_per_point: f64,
        relentless_strikes_energy: f64,
        relentless_strikes_action: ActionId,
        ruthlessness_chance: f64,
        ruthlessness_action: ActionId,
    },
    InstantPoison {
        trigger_aura: String,
        spell_id: i32,
        proc_mask: Vec<String>,
        proc_chance: f64,
        min_damage: f64,
        max_damage: f64,
    },
    DeadlyPoison {
        trigger_aura: String,
        spell_id: i32,
        tag: i32,
        proc_mask: Vec<String>,
        proc_chance: f64,
        tick_damage: f64,
    },
    /// Go consumes.go Goblin Sapper Charge: a rolled Fire hit on the target and a second roll
    /// on the player, which rolls on the player's attack table against itself.
    GoblinSapper {
        item_id: i32,
        self_tag: i32,
        min_damage: f64,
        max_damage: f64,
        aoe_cap_multiplier: f64,
        self_attack_table: AttackTable,
    },
    /// Go health.go trackChanceOfDeath once a spell can hit the player.
    ChanceOfDeath {
        aura: String,
    },
    /// An item proc trigger that restores energy a spell batch window after a landed hit, such
    /// as Shadowcraft Armor's: the chance each spell rolls, by spellbook position.
    EnergizeProc {
        trigger_aura: String,
        rng_label: String,
        chances: Vec<SpellChance>,
        energy: f64,
        metrics_action_id: ActionId,
        delay_ns: i64,
    },
    /// Go aura_helpers.go ApplyFixedUptimeAura: a periodic roll that activates the aura and a
    /// first roll with a random duration.
    FixedUptimeAura {
        aura: String,
        uptime: f64,
        tick_length_ns: i64,
        start_time_ns: i64,
    },
}

impl Effect {
    /// The stable kind name used in capability reports.
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::Frostbolt {} => "frostbolt",
            Effect::ArcaneBlast { .. } => "arcane_blast",
            Effect::ArcanePower { .. } => "arcane_power",
            Effect::Ignite { .. } => "ignite",
            Effect::FireBlast {} => "fire_blast",
            Effect::Combustion { .. } => "combustion",
            Effect::Pyroblast { .. } => "pyroblast",
            Effect::HeatingUp { .. } => "heating_up",
            Effect::MasterOfElements { .. } => "master_of_elements",
            Effect::Fireball { .. } => "fireball",
            Effect::FrostfireBolt { .. } => "frostfire_bolt",
            Effect::Scorch { .. } => "scorch",
            Effect::TouchOfTheGrave { .. } => "touch_of_the_grave",
            Effect::Eureka { .. } => "eureka",
            Effect::Berserking { .. } => "berserking",
            Effect::BloodFury { .. } => "blood_fury",
            Effect::TemporaryStats { .. } => "temporary_stats",
            Effect::DruidForms { .. } => "druid_forms",
            Effect::MoonkinForm { .. } => "moonkin_form",
            Effect::Starfire {} => "starfire",
            Effect::Wrath {} => "wrath",
            Effect::Moonfire { .. } => "moonfire",
            Effect::InsectSwarm { .. } => "insect_swarm",
            Effect::Innervate { .. } => "innervate",
            Effect::OmenOfClarity { .. } => "omen_of_clarity",
            Effect::NaturesGrace { .. } => "natures_grace",
            Effect::Eclipse { .. } => "eclipse",
            Effect::MindBlast {} => "mind_blast",
            Effect::ShadowWordDeath { .. } => "shadow_word_death",
            Effect::ShadowWordPain { .. } => "shadow_word_pain",
            Effect::DevouringPlague { .. } => "devouring_plague",
            Effect::MindFlay { .. } => "mind_flay",
            Effect::Shadowform { .. } => "shadowform",
            Effect::InnerFocus { .. } => "inner_focus",
            Effect::ShadowWeaving { .. } => "shadow_weaving",
            Effect::DarkSacrifice { .. } => "dark_sacrifice",
            Effect::InertPet { .. } => "inert_pet",
            Effect::JudgementRefresh { .. } => "judgement_refresh",
            Effect::SunderArmorRamp { .. } => "sunder_armor_ramp",
            Effect::StatAuras { .. } => "stat_auras",
            Effect::WindfuryTotem { .. } => "windfury_totem",
            Effect::Crusader { .. } => "crusader",
            Effect::DragonbreathChili { .. } => "dragonbreath_chili",
            Effect::ShatterCurse { .. } => "shatter_curse",
            Effect::Stoneform { .. } => "stoneform",
            Effect::ReadLeyLine { .. } => "read_ley_line",
            Effect::PresenceOfMind { .. } => "presence_of_mind",
            Effect::IceLance { .. } => "ice_lance",
            Effect::ArcaneMissiles { .. } => "arcane_missiles",
            Effect::ColdSnap { .. } => "cold_snap",
            Effect::Evocation { .. } => "evocation",
            Effect::ManaGems { .. } => "mana_gems",
            Effect::MageArmor { .. } => "mage_armor",
            Effect::ArcaneConcentration { .. } => "arcane_concentration",
            Effect::MissileBarrage { .. } => "missile_barrage",
            Effect::FingersOfFrost { .. } => "fingers_of_frost",
            Effect::WintersChill { .. } => "winters_chill",
            Effect::JudgementOfWisdom { .. } => "judgement_of_wisdom",
            Effect::PotionMana { .. } => "potion_mana",
            Effect::ConjuredMana { .. } => "conjured_mana",
            Effect::EnergizeOnUse { .. } => "energize_on_use",
            Effect::InertListener { .. } => "inert_listener",
            Effect::ShadowBolt {} => "shadow_bolt",
            Effect::Immolate { .. } => "immolate",
            Effect::Corruption { .. } => "corruption",
            Effect::BaneOfAgony { .. } => "bane_of_agony",
            Effect::AmplifyCurse { .. } => "amplify_curse",
            Effect::CurseOfTheElements { .. } => "curse_of_the_elements",
            Effect::LifeTap { .. } => "life_tap",
            Effect::Conflagrate { .. } => "conflagrate",
            Effect::Shadowburn {} => "shadowburn",
            Effect::SearingPain {} => "searing_pain",
            Effect::SoulFire {} => "soul_fire",
            Effect::ImprovedShadowBolt { .. } => "improved_shadow_bolt",
            Effect::ShadowAndFlame { .. } => "shadow_and_flame",
            Effect::LightningBolt { .. } => "lightning_bolt",
            Effect::ChainLightning { .. } => "chain_lightning",
            Effect::FlameShock { .. } => "flame_shock",
            Effect::LavaBurst { .. } => "lava_burst",
            Effect::FireNova { .. } => "fire_nova",
            Effect::SearingTotem { .. } => "searing_totem",
            Effect::ElementalFocus { .. } => "elemental_focus",
            Effect::ConjuredEnergy { .. } => "conjured_energy",
            Effect::SinisterStrike { .. } => "sinister_strike",
            Effect::Backstab { .. } => "backstab",
            Effect::Eviscerate { .. } => "eviscerate",
            Effect::SliceAndDice { .. } => "slice_and_dice",
            Effect::BladeFlurry { .. } => "blade_flurry",
            Effect::AdrenalineRush { .. } => "adrenaline_rush",
            Effect::RogueFinisher { .. } => "rogue_finisher",
            Effect::InstantPoison { .. } => "instant_poison",
            Effect::DeadlyPoison { .. } => "deadly_poison",
            Effect::GoblinSapper { .. } => "goblin_sapper",
            Effect::ChanceOfDeath { .. } => "chance_of_death",
            Effect::FixedUptimeAura { .. } => "fixed_uptime_aura",
            Effect::EnergizeProc { .. } => "energize_proc",
        }
    }
}
