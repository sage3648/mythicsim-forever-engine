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
    pub attack_table: AttackTable,
    pub spells: Vec<Spell>,
    /// Go's initial major cooldown order after the rotation claimed its spells.
    pub major_cooldowns: Vec<MajorCooldown>,
    /// The request's APL in protojson form. Interpreted by the rotation module.
    pub rotation: serde_json::Value,
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

/// Behavior Rust must execute, with the parameters Go keeps in closures. Each variant
/// names the Go source that defines it in docs/prepared-v2.md.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    Frostbolt {},
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
}

impl Effect {
    /// The stable kind name used in capability reports.
    pub fn kind(&self) -> &'static str {
        match self {
            Effect::Frostbolt {} => "frostbolt",
            Effect::ArcaneBlast { .. } => "arcane_blast",
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
        }
    }
}
