//! The prepared v2 build gate, shared by every class.
//!
//! An effect is required when an active aura that listens to combat events belongs to
//! it, or when the rotation can reach a spell it implements. A prepared input is
//! supported only when every required effect has a Rust implementation, every active
//! listener is claimed and every reachable spell stays inside the runtime's limits.
//! Each gap becomes one stable reason string. A class supplies its own effects, the
//! effects of its spells and the auras its effects claim through a [`ClassGate`].

use std::{collections::BTreeSet, sync::OnceLock};

use crate::{
    classes,
    contracts::prepared_v2::{ActionId, Effect, MajorCooldown, PreparedV2, Spell},
    core::fight::DIRECT_PROC_MASKS,
    rotation::{
        compile_bool_value, compile_condition, Action, FoundAura, Lookup, MissingAura, Rotation,
        Value,
    },
};

/// A class's part of the gate.
pub(crate) struct ClassGate {
    /// Go's `Class` enum name, as the exporter writes it.
    pub(crate) class: &'static str,
    /// The class's effect kinds implemented in Rust and validated against Go.
    pub(crate) effects: &'static [&'static str],
    /// The effect kind whose implementation executes a class spell, by its class spell name
    /// and shape.
    pub(crate) spell: fn(&Spell) -> Option<&'static str>,
    /// The aura labels a class effect takes responsibility for, as (unit, label).
    pub(crate) claims: for<'a> fn(&'a Effect) -> Vec<(&'static str, &'a str)>,
    /// Class limits on the input and the spells the rotation can reach.
    pub(crate) limits: fn(&PreparedV2, &[&Spell]) -> Vec<String>,
}

/// Effects of races, items and raid buffs, which every class shares.
const COMMON_EFFECTS: &[&str] = &[
    "aura_should_refresh",
    "basic_explosive",
    "berserking",
    "blood_fury",
    "chance_of_death",
    "conjured_energy",
    "conjured_mana",
    "crusader",
    "dragonbreath_chili",
    "energize_on_use",
    "emerald_dragon_whelp",
    "sulfuras_hand_of_ragnaros",
    "energize_proc",
    "eureka",
    "exclusive_category",
    "extra_attack_proc",
    "fixed_uptime_aura",
    "goblin_sapper",
    "inert_listener",
    "inert_pet",
    "judgement_of_wisdom",
    "parry_haste",
    "potion_mana",
    "player_damage_taken",
    "potion_resource",
    "pseudo_stat_auras",
    "pushback_trigger",
    "rage_bar",
    "redoubt",
    "shield_specialization",
    "reckoning",
    "holy_shield",
    "read_ley_line",
    "shatter_curse",
    "speed_on_use",
    "spell_cost_aura_on_use",
    "spell_data_damage_proc",
    "spell_data_stat_proc",
    "spell_data_heal_proc",
    "second_wind",
    "absorb_on_use",
    "heal_on_use",
    "stat_proc",
    "stat_auras",
    "health_rage_proc",
    "armor_debuff_proc",
    "damage_on_use",
    "stoneform",
    "sunder_armor_ramp",
    "temporary_stats",
    "touch_of_the_grave",
    "windfury_totem",
];

/// Every class with an implemented gate.
fn gates() -> [&'static ClassGate; 9] {
    [
        &classes::mage::prepared::GATE,
        &classes::druid::prepared::GATE,
        &classes::shaman::prepared::GATE,
        &classes::paladin::prepared::GATE,
        &classes::warlock::prepared::GATE,
        &classes::priest::prepared::GATE,
        &classes::rogue::prepared::GATE,
        &classes::warrior::prepared::GATE,
        &classes::hunter::prepared::GATE,
    ]
}

/// Every effect kind implemented in Rust, sorted.
pub(crate) fn implemented_effects() -> &'static [&'static str] {
    static EFFECTS: OnceLock<Vec<&'static str>> = OnceLock::new();
    EFFECTS.get_or_init(|| {
        let mut effects: BTreeSet<&'static str> = COMMON_EFFECTS.iter().copied().collect();
        for gate in gates() {
            effects.extend(gate.effects.iter().copied());
        }
        effects.into_iter().collect()
    })
}

/// The gate of the prepared player's class, if Rust implements it.
pub(crate) fn class_gate(class: &str) -> Option<&'static ClassGate> {
    gates().into_iter().find(|gate| gate.class == class)
}

/// The effect kind of a spell that is not a class spell: items and racials.
fn common_spell_capability(spell: &Spell, prepared: &PreparedV2) -> Option<&'static str> {
    let id = spell.action_id.clone().unwrap_or_default();
    let item = id.item_id;
    prepared.effects.iter().find_map(|effect| match effect {
        Effect::PotionMana { item_id, .. } if *item_id == item => Some("potion_mana"),
        Effect::PotionResource { item_id, .. } if *item_id == item => Some("potion_resource"),
        // Class spells Go registers without a class mask, named by their effect's spell.
        Effect::Bloodrage { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("bloodrage")
        }
        Effect::HeroicStrikeQueue { strikes, .. }
            if id.tag == 1 && strikes.iter().any(|strike| strike.spell_id == id.spell_id) =>
        {
            Some("heroic_strike_queue")
        }
        Effect::ConjuredMana { item_id, .. } if *item_id == item => Some("conjured_mana"),
        Effect::ConjuredEnergy { item_id, .. } if *item_id == item => Some("conjured_energy"),
        Effect::GoblinSapper { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("goblin_sapper")
        }
        Effect::BasicExplosive { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("basic_explosive")
        }
        Effect::EnergizeOnUse { item_id, .. } if *item_id == item => Some("energize_on_use"),
        Effect::SecondWind { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("second_wind")
        }
        Effect::AbsorbOnUse { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("absorb_on_use")
        }
        Effect::HealOnUse { item_id, .. } if *item_id == item && id.tag == 0 => Some("heal_on_use"),
        Effect::SpeedOnUse { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("speed_on_use")
        }
        Effect::SpellCostAuraOnUse { item_id, .. } if *item_id == item && id.tag == 0 => {
            Some("spell_cost_aura_on_use")
        }
        // A damage on-use item whose outcome appliers the runtime knows.
        Effect::DamageOnUse {
            item_id,
            direct,
            periodic,
            ..
        } if *item_id == item
            && id.tag == 0
            && crate::core::fight::on_use_damage_known(direct, periodic) =>
        {
            Some("damage_on_use")
        }
        Effect::Eureka { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("eureka")
        }
        Effect::Berserking { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("berserking")
        }
        Effect::BloodFury { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("blood_fury")
        }
        Effect::ShatterCurse { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("shatter_curse")
        }
        Effect::Stoneform { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("stoneform")
        }
        Effect::ReadLeyLine { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("read_ley_line")
        }
        Effect::TemporaryStats {
            spell_id, item_id, ..
        } if *spell_id == id.spell_id && *item_id == id.item_id && id.tag == 0 => {
            Some("temporary_stats")
        }
        // Class spells Go registers without a class mask, named by their effect.
        Effect::Prowl { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => Some("prowl"),
        Effect::Berserk { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("berserk")
        }
        Effect::Barkskin { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("barkskin")
        }
        Effect::Maul { queue_spell, .. }
            if prepared
                .player
                .spells
                .get(*queue_spell)
                .is_some_and(|queue| std::ptr::eq(queue, spell)) =>
        {
            Some("maul")
        }
        _ => None,
    })
}

/// The effect kind whose implementation executes a castable spell.
fn spell_capability(
    spell: &Spell,
    prepared: &PreparedV2,
    gate: &ClassGate,
) -> Option<&'static str> {
    // A class may also implement spells Go registers without a class mask.
    if let Some(kind) = (gate.spell)(spell) {
        return Some(kind);
    }
    if spell.class_spell.is_some() {
        return None;
    }
    common_spell_capability(spell, prepared)
}

/// Aura labels a race, item or raid buff effect takes responsibility for.
fn common_claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::TouchOfTheGrave { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::Eureka { aura, .. }
        | Effect::Berserking { aura, .. }
        | Effect::BloodFury { aura, .. }
        | Effect::ShatterCurse { aura, .. }
        | Effect::Stoneform { aura, .. }
        | Effect::ReadLeyLine { aura, .. }
        | Effect::TemporaryStats { aura, .. }
        | Effect::AbsorbOnUse { aura, .. }
        | Effect::SpeedOnUse { aura, .. } => vec![("player", aura)],
        Effect::JudgementOfWisdom { aura, .. } => vec![("target", aura)],
        Effect::RageBar { aura, .. } => vec![("player", aura)],
        Effect::ExtraAttackProc { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::ChanceOfDeath { aura } => vec![("player", aura)],
        Effect::ParryHaste { unit, aura, .. } => match unit.as_str() {
            "player" => vec![("player", aura)],
            "target" => vec![("target", aura)],
            _ => Vec::new(),
        },
        Effect::PushbackTrigger { aura, .. } => vec![("player", aura)],
        Effect::EnergizeProc { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::Crusader { trigger_aura, .. }
        | Effect::DragonbreathChili { trigger_aura, .. }
        | Effect::SpellDataDamageProc { trigger_aura, .. }
        | Effect::SpellDataHealProc { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::EmeraldDragonWhelp {
            trigger_aura, pet, ..
        } => vec![("player", trigger_aura), ("pet unit", pet)],
        Effect::SulfurasHandOfRagnaros {
            trigger_aura,
            immolation_aura,
            ..
        } => vec![("player", trigger_aura), ("player", immolation_aura)],
        Effect::StatProc {
            trigger_aura, aura, ..
        }
        | Effect::SpellDataStatProc {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::HealthRageProc { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::ArmorDebuffProc {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("target", aura)],
        Effect::SpellCostAuraOnUse { aura, .. } => vec![("player", aura)],
        Effect::WindfuryTotem {
            trigger_aura,
            proc_aura,
            ..
        } => vec![("player", trigger_aura), ("player", proc_aura)],
        Effect::InertListener { unit, aura, .. } => match unit.as_str() {
            "player" => vec![("player", aura)],
            "target" => vec![("target", aura)],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Proc triggers Rust matches as `ProcMaskDirect`; any other mask is unsupported.
fn undirected_procs(prepared: &PreparedV2) -> Vec<String> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::TouchOfTheGrave {
                trigger_aura,
                proc_mask,
                ..
            } => {
                let mut mask: Vec<&str> = proc_mask.iter().map(String::as_str).collect();
                let mut direct = DIRECT_PROC_MASKS.to_vec();
                mask.sort_unstable();
                direct.sort_unstable();
                (mask != direct)
                    .then(|| format!("{trigger_aura} procs from {proc_mask:?}, not direct hits"))
            }
            _ => None,
        })
        .collect()
}

/// Stats a temporary stat change may set: the runtime reads spell damage, attack powers, crit
/// and maximum mana during a fight, and nothing in scope reads healing power.
const DYNAMIC_STATS: &[&str] = &[
    "SpellDamage",
    "AttackPower",
    "RangedAttackPower",
    "HealingPower",
    "SpellCritPercent",
    "PhysicalCritPercent",
    "MP5",
    "Mana",
    // Spirit, which spirit regeneration, Life Tap and Dark Sacrifice read live.
    "Spirit",
    // Spell power by school, which spell power reads, and the resistances spells that hit
    // the player roll against: the combinations carry them when one changes them.
    "ArcaneDamage",
    "FireDamage",
    "FrostDamage",
    "HolyDamage",
    "NatureDamage",
    "ShadowDamage",
    "ArcaneResistance",
    "FireResistance",
    "FrostResistance",
    "NatureResistance",
    "ShadowResistance",
];

/// Whether a stat a stat aura or temporary stat change moves is one the runtime follows or
/// one nothing reads.
fn followed_stat(stat: &str) -> bool {
    DYNAMIC_STATS.contains(&stat) || INERT_STATS.contains(&stat)
}

/// Stats a stat aura may change without the runtime reading them: inputs to the stats it
/// reads, and stats nothing in scope reads.
const INERT_STATS: &[&str] = &[
    // Spell crit and maximum mana; mana regeneration reads only spirit and MP5.
    "Intellect",
    // Spirit regeneration, which the combinations carry; a class that reads Spirit itself
    // refuses a change of it.
    "Spirit",
    "Strength",
    "Agility",
    "Stamina",
    "Health",
    "Armor",
    "BonusArmor",
    "BlockValue",
    "BlockPercent",
    "DodgeRating",
    "DodgePercent",
    "ParryRating",
    "ParryPercent",
    "FeralAttackPower",
];

/// The stats the runtime follows on the player but does not pass on to a dynamic pet, whose
/// inheritance carries only its tracked powers.
const UNINHERITED_STATS: &[&str] = &[
    "Spirit",
    "ArcaneDamage",
    "FireDamage",
    "FrostDamage",
    "HolyDamage",
    "NatureDamage",
    "ShadowDamage",
    "ArcaneResistance",
    "FireResistance",
    "FrostResistance",
    "NatureResistance",
    "ShadowResistance",
];

/// A dynamic pet that would inherit a change of one of those stats.
fn uninherited_stat_changes(prepared: &PreparedV2) -> Vec<String> {
    let mut changed: BTreeSet<&str> = BTreeSet::new();
    for effect in &prepared.effects {
        match effect {
            Effect::StatAuras { changed: stats, .. } => {
                changed.extend(stats.iter().map(String::as_str))
            }
            Effect::BloodFury { active_stats, .. }
            | Effect::TemporaryStats { active_stats, .. } => {
                changed.extend(active_stats.keys().map(String::as_str))
            }
            _ => {}
        }
    }
    let mut reasons = Vec::new();
    for pet in &prepared.pets {
        for term in &pet.inheritance {
            if UNINHERITED_STATS.contains(&term.owner.as_str())
                && changed.contains(term.owner.as_str())
            {
                reasons.push(format!(
                    "pet {} inherits {}, which changes during the fight",
                    pet.label, term.owner
                ));
            }
        }
    }
    reasons
}

/// Stat aura combinations that change a stat the runtime holds fixed.
fn fixed_stat_aura_changes(prepared: &PreparedV2) -> Vec<String> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::StatAuras { auras, changed, .. } => Some((auras, changed)),
            _ => None,
        })
        .flat_map(|(auras, changed)| {
            changed
                .iter()
                .filter(|stat| !followed_stat(stat))
                .map(move |stat| {
                    format!("stat auras {auras:?} change {stat}, which the runtime holds fixed")
                })
        })
        .collect()
}

/// Temporary stat changes to stats the runtime holds fixed.
fn fixed_stat_changes(prepared: &PreparedV2) -> Vec<String> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::BloodFury {
                aura, active_stats, ..
            }
            | Effect::TemporaryStats {
                aura, active_stats, ..
            } => Some((aura, active_stats)),
            _ => None,
        })
        .flat_map(|(aura, stats)| {
            stats
                .keys()
                .filter(|stat| !followed_stat(stat))
                .map(move |stat| format!("{aura} changes {stat}, which the runtime holds fixed"))
        })
        .collect()
}

/// Stats that change the player's health, which stay fixed while the player tanks. The
/// target's swings carry their rolls for each stat aura combination.
const DEFENDER_STATS: &[&str] = &["Stamina", "Health"];

/// Effects whose behaviors act on the hits the player takes from the target's swings.
const HIT_TAKEN_EFFECTS: &[&str] = &[
    "chance_of_death",
    "sulfuras_hand_of_ragnaros",
    "natural_reaction",
    "natures_bounty",
    "parry_haste",
    "pushback_trigger",
    "rage_bar",
    "redoubt",
    "shield_specialization",
    "reckoning",
    "holy_shield",
    "eye_for_an_eye",
    "inert_listener",
    "revenge",
    "retaliation",
    "rage_on_avoid",
    "warrior_enrage",
    "blood_craze",
    "riposte",
    "spell_data_damage_proc",
    "battlegear_of_might_rage",
];

/// Effects that slow the target's melee speed through its live multiplier, with the target
/// auras they claim.
const ENEMY_SPEED_EFFECTS: &[&str] = &["thunder_clap"];

/// Callbacks the target's own swings fire on the target.
const TARGET_CASTER_CALLBACKS: &[&str] = &[
    "on_apply_effects",
    "on_cast_complete",
    "on_spell_hit_dealt",
    "on_periodic_damage_dealt",
];

/// Whether a spell is a survival major cooldown Go never fires: no timings, no defensive
/// health threshold and no rotation action that casts it.
fn survival_cooldown_never_fires(prepared: &PreparedV2, spell_id: i32) -> bool {
    let survival = prepared.player.hp_percent_for_defensives == 0.0
        && prepared.player.major_cooldowns.iter().any(|cooldown| {
            cooldown.action_id.spell_id == spell_id
                && cooldown.kind.iter().any(|kind| kind == "survival")
                && cooldown.timings_ns.is_empty()
        });
    let Ok(rotation) = crate::rotation::parse(&prepared.player.rotation) else {
        return false;
    };
    let cast = rotation
        .prepull
        .iter()
        .map(|prepull| &prepull.action)
        .chain(rotation.priority_list.iter().map(|item| &item.action))
        .any(|action| action.spells().iter().any(|id| id.spell_id == spell_id));
    survival && !cast
}

/// What the runtime cannot follow once the target swings at the player: listeners of the
/// swings it does not run, auras something in scope activates that change the swings, and
/// defender stats a stat aura changes.
fn tank_limits(
    prepared: &PreparedV2,
    enemy: &crate::contracts::prepared_v2::Enemy,
    claims: &dyn Fn(&Effect) -> Vec<(&'static str, String)>,
    rotation: Option<&Rotation>,
) -> Vec<String> {
    let mut reasons = Vec::new();
    // An aura only becomes active in the runtime through an effect that names it.
    let mut named: BTreeSet<String> = BTreeSet::new();
    // Labels an effect carries without claiming them, which a class may still activate.
    let mut carried: BTreeSet<String> = BTreeSet::new();
    for effect in &prepared.effects {
        // A racial survival cooldown that never fires leaves its aura inactive.
        if let Effect::Stoneform { spell_id, .. } = effect {
            if survival_cooldown_never_fires(prepared, *spell_id) {
                continue;
            }
        }
        for (unit, label) in claims(effect) {
            named.insert(format!("{unit}:{label}"));
        }
        // A class may activate any aura its effect carries without claiming it, so every label
        // an effect names on either unit counts.
        if let Ok(value) = serde_json::to_value(effect) {
            let mut strings = Vec::new();
            collect_strings(&value, &mut strings);
            for label in strings {
                carried.insert(format!("player:{label}"));
                carried.insert(format!("target:{label}"));
            }
        }
        match effect {
            Effect::StatAuras { auras, .. } => {
                named.extend(auras.iter().map(|label| format!("player:{label}")));
            }
            Effect::FixedUptimeAura { aura, .. } => {
                named.insert(format!("player:{aura}"));
            }
            _ => {}
        }
    }
    // A racial aura whose cast is an untimed survival cooldown the rotation never names never
    // activates at a zero defensive health threshold: Go fires such a cooldown only below it.
    let named_by_rotation = |spell_id: i32| {
        rotation.is_some_and(|rotation| {
            let casts = |action: &Action| {
                action
                    .spells()
                    .into_iter()
                    .any(|id| id.spell_id == spell_id && id.item_id == 0)
            };
            rotation
                .prepull
                .iter()
                .any(|prepull| casts(&prepull.action))
                || rotation
                    .priority_list
                    .iter()
                    .any(|item| casts(&item.action))
        })
    };
    for effect in &prepared.effects {
        if let Effect::Stoneform { spell_id, aura, .. }
        | Effect::ShatterCurse { spell_id, aura, .. } = effect
        {
            let never_fires = prepared.player.hp_percent_for_defensives == 0.0
                && prepared.player.major_cooldowns.iter().any(|cooldown| {
                    cooldown.action_id.spell_id == *spell_id
                        && cooldown.timings_ns.is_empty()
                        && cooldown.kind.iter().any(|kind| kind == "survival")
                });
            if never_fires && !named_by_rotation(*spell_id) {
                named.remove(&format!("player:{aura}"));
                carried.remove(&format!("player:{aura}"));
            }
        }
    }
    let can_be_active = |unit: &str, aura: &crate::contracts::prepared_v2::Aura| {
        aura.active || named.contains(&format!("{unit}:{}", aura.label))
    };
    for aura in &prepared.player.auras {
        if !can_be_active("player", aura)
            || !aura.callbacks.iter().any(|c| c == "on_spell_hit_taken")
        {
            continue;
        }
        let handled = prepared.effects.iter().any(|effect| {
            HIT_TAKEN_EFFECTS.contains(&effect.kind())
                && claims(effect)
                    .iter()
                    .any(|(unit, label)| *unit == "player" && *label == aura.label)
        });
        if !handled {
            reasons.push(format!(
                "player aura {:?} reacts to the target's swings",
                aura.label
            ));
        }
    }
    for aura in &prepared.target.auras {
        if can_be_active("target", aura)
            && aura
                .callbacks
                .iter()
                .any(|c| TARGET_CASTER_CALLBACKS.contains(&c.as_str()))
        {
            reasons.push(format!(
                "target aura {:?} reacts to the target's own swings",
                aura.label
            ));
        }
    }
    for aura in &enemy.changing_auras {
        if named.contains(aura) || carried.contains(aura) {
            reasons.push(format!("{aura} changes the target's swings at the player"));
        }
    }
    // The runtime reads the player's damage taken multiplier live, which only the pseudo stat
    // and damage taken effects multiply.
    let mut tracked: BTreeSet<String> = BTreeSet::new();
    for effect in &prepared.effects {
        match effect {
            Effect::PseudoStatAuras { auras } => tracked.extend(
                auras
                    .iter()
                    .filter(|entry| entry.stat == "damage_taken")
                    .map(|entry| format!("player:{}", entry.aura)),
            ),
            Effect::PlayerDamageTaken { auras } => {
                tracked.extend(auras.iter().map(|entry| format!("player:{}", entry.aura)))
            }
            _ => {}
        }
    }
    for aura in &enemy.damage_taken_auras {
        if named.contains(aura) && !tracked.contains(aura) {
            reasons.push(format!(
                "{aura} changes the player's damage taken, which nothing multiplies"
            ));
        }
    }
    // The runtime reads the player's physical damage taken multiplier live, which only the
    // racial survival effects multiply, while the rolls hold the reset's value.
    let live_school = {
        let physical = prepared
            .player
            .pseudo_stats
            .school_damage_taken_multiplier
            .physical;
        enemy
            .rolls
            .iter()
            .chain(&enemy.reduced_avoidance_rolls)
            .all(|rolls| {
                rolls.school_damage_taken_multiplier == Some(physical)
                    && rolls.table_damage_taken_multiplier.is_some()
            })
    };
    for aura in &enemy.school_damage_taken_auras {
        let multiplied = prepared.effects.iter().any(|effect| match effect {
            Effect::Stoneform { aura: label, .. } | Effect::ShatterCurse { aura: label, .. } => {
                format!("player:{label}") == *aura
            }
            _ => false,
        });
        if named.contains(aura) && !(multiplied && live_school) {
            reasons.push(format!(
                "{aura} changes the player's physical damage taken, which the runtime does not \
                 follow"
            ));
        }
    }
    for aura in &enemy.speed_auras {
        let slowed = prepared.effects.iter().any(|effect| {
            ENEMY_SPEED_EFFECTS.contains(&effect.kind())
                && claims(effect)
                    .iter()
                    .any(|(unit, label)| format!("{unit}:{label}") == *aura)
        });
        if named.contains(aura) && !slowed {
            reasons.push(format!(
                "{aura} slows the target's swings without an effect"
            ));
        }
    }
    // Each attack power aura's value holds while it is the only one active.
    let attack_power: Vec<&str> = enemy
        .attack_power_auras
        .iter()
        .filter(|entry| named.contains(&format!("target:{}", entry.aura)))
        .map(|entry| entry.aura.as_str())
        .collect();
    if attack_power.len() > 1 {
        reasons.push(format!(
            "target auras {attack_power:?} change the target's attack power together"
        ));
    }
    for effect in &prepared.effects {
        if let Effect::StatAuras {
            auras,
            combos,
            changed,
        } = effect
        {
            // Stamina moves only the maximum health the combinations carry.
            let health_tracked = combos.iter().all(|combo| combo.contains_key("Health"));
            for stat in changed {
                if DEFENDER_STATS.contains(&stat.as_str()) && !health_tracked {
                    reasons.push(format!(
                        "stat auras {auras:?} change {stat}, which the player's health holds fixed"
                    ));
                }
            }
        }
    }
    reasons
}

/// Every string in a JSON value, depth first.
fn collect_strings(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => out.push(text.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|item| collect_strings(item, out)),
        serde_json::Value::Object(map) => map.values().for_each(|item| collect_strings(item, out)),
        _ => {}
    }
}

/// Go `GetAPLSpell`: the first APL-flagged spell with the action ID, otherwise the first
/// registered one, as a spellbook position. The potion action names the first combat
/// potion. A missing spell drops the rotation action in Go.
pub(crate) fn rotation_spell_index(prepared: &PreparedV2, id: &ActionId) -> Option<usize> {
    let spells = &prepared.player.spells;
    if id.other_id == "OtherActionPotion" {
        return spells
            .iter()
            .position(|spell| spell.has_flag("SpellFlagCombatPotion"));
    }
    spells
        .iter()
        .position(|spell| spell.action_id.as_ref() == Some(id) && spell.has_flag("SpellFlagAPL"))
        .or_else(|| {
            spells
                .iter()
                .position(|spell| spell.action_id.as_ref() == Some(id))
        })
}

/// Go `Spell.Dot`: the spell holding the dot a spell names, its own or its related dot
/// spell's.
pub(crate) fn dot_owner(prepared: &PreparedV2, index: usize) -> Option<usize> {
    let spells = &prepared.player.spells;
    match &spells[index] {
        spell if spell.dot.is_some() => Some(index),
        spell => spell
            .related_dot_spell
            .filter(|&related| spells.get(related).is_some_and(|s| s.dot.is_some())),
    }
}

/// [`rotation_spell_index`] as the exported spell.
pub(crate) fn rotation_spell<'a>(prepared: &'a PreparedV2, id: &ActionId) -> Option<&'a Spell> {
    rotation_spell_index(prepared, id).map(|index| &prepared.player.spells[index])
}

/// The spell a major cooldown casts: its own when an earlier spell shares its action ID,
/// otherwise the first spell with the ID, as the runtime binds it.
fn cooldown_spell<'a>(prepared: &'a PreparedV2, cooldown: &MajorCooldown) -> Option<&'a Spell> {
    match cooldown.spell {
        Some(index) => prepared.player.spells.get(index),
        None => rotation_spell(prepared, &cooldown.action_id),
    }
}

/// Spell properties the fight runtime does not implement. A class spell computes its own
/// damage, so a physical damage roll is a limit only on other spells.
fn runtime_limits(spell: &Spell, class_spell: bool) -> Vec<&'static str> {
    let mut limits = Vec::new();
    if spell.max_charges != 0 {
        limits.push("charges");
    }
    if spell.has_cast_requirement {
        limits.push("cast requirements");
    }
    if spell.damage_effect.is_some() && spell.school & 1 != 0 && !class_spell {
        limits.push("physical damage");
    }
    if spell
        .dot
        .as_ref()
        .is_some_and(|dot| dot.affected_by_real_haste)
    {
        limits.push("periodic effects hasted by real haste");
    }
    limits
}

pub(crate) fn prepared_coverage(prepared: &PreparedV2, rotation: Option<&Rotation>) -> Vec<String> {
    let mut reasons = Vec::new();
    let player = &prepared.player;
    let Some(gate) = class_gate(&player.class) else {
        return vec![format!("class {} is not supported", player.class)];
    };
    if player.level != 60 {
        reasons.push(format!("player level {} is not supported", player.level));
    }
    if !(60..=63).contains(&prepared.target.level) {
        reasons.push(format!(
            "target level {} is not supported",
            prepared.target.level
        ));
    }

    let claims = |effect: &Effect| {
        let mut claimed = common_claims(effect);
        claimed.extend((gate.claims)(effect));
        claimed
            .into_iter()
            .map(|(unit, label)| (unit, label.to_string()))
            .collect::<Vec<_>>()
    };
    let mut required: BTreeSet<&str> = BTreeSet::new();
    let unit_auras = [
        ("player", &player.auras),
        ("target", &prepared.target.auras),
    ]
    .into_iter()
    .chain(prepared.pets.iter().map(|pet| ("pet", &pet.auras)));
    for (unit, auras) in unit_auras {
        for aura in auras {
            if !aura.active || !aura.has_event_callbacks() {
                continue;
            }
            let claimant = prepared.effects.iter().find(|effect| {
                claims(effect)
                    .iter()
                    .any(|(u, label)| *u == unit && *label == aura.label)
            });
            match claimant {
                Some(effect) => {
                    required.insert(effect.kind());
                }
                None => reasons.push(format!(
                    "{unit} aura {:?} listens to combat events without an effect",
                    aura.label
                )),
            }
        }
    }

    // A simulated pet needs a class effect that runs it, which claims the pet by its label.
    for pet in &prepared.pets {
        let claimant = prepared.effects.iter().find(|effect| {
            claims(effect)
                .iter()
                .any(|(u, label)| *u == "pet unit" && *label == pet.label)
        });
        match claimant {
            Some(effect) => {
                required.insert(effect.kind());
            }
            None => reasons.push(format!("pet {:?} has no behavior", pet.label)),
        }
        // Go passes the owner's stat changes to a dynamic pet at its next heartbeat; the
        // runtime follows the stats it tracks. Every owner stat change is a stat aura's.
        let changed: Vec<&String> = prepared
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::StatAuras { changed, .. } => Some(changed),
                _ => None,
            })
            .flatten()
            .collect();
        if pet.dynamic_stats {
            for term in &pet.inheritance {
                let tracked = crate::core::fight::pet::power_index(&term.owner).is_some();
                if changed.contains(&&term.owner) && !tracked {
                    reasons.push(format!(
                        "pet {:?} inherits changes of {} as {}, which is unsupported",
                        pet.label, term.owner, term.pet
                    ));
                }
            }
        }
    }
    if let Some(enemy) = &prepared.enemy {
        reasons.extend(tank_limits(prepared, enemy, &claims, rotation));
    }

    if let Some(rotation) = rotation {
        reasons.extend(unknown_aura_conditions(prepared, rotation));
        reasons.extend(aura_refresh_conditions(prepared, rotation));
        if player.class != "ClassShaman" {
            for item in &rotation.priority_list {
                let mut totems = false;
                if let Some(condition) = &item.condition {
                    condition.visit(&mut |value| {
                        totems |= matches!(value, Value::TotemRemainingTime { .. })
                    });
                }
                if totems {
                    reasons.push(format!(
                        "rotation item {}: totemRemainingTime needs a Shaman",
                        item.position
                    ));
                }
            }
        }
        reasons.extend(energy_without_bar(prepared, rotation));
        reasons.extend(rage_without_bar(prepared, rotation));
        let mut reachable = Vec::new();
        let mut registered_prepull = 0;
        // Prepull parsing accepts only casts.
        for prepull in &rotation.prepull {
            match prepull_pruned(prepared, prepull) {
                Some(true) => continue,
                Some(false) => {}
                None => {
                    reasons.push(format!(
                        "prepull action {}: the pinned reference and community #622 prune its \
                         condition differently",
                        prepull.position
                    ));
                    continue;
                }
            }
            if let Action::CastSpell(id) = &prepull.action {
                if let Some(spell) = rotation_spell(prepared, id) {
                    registered_prepull += 1;
                    reachable.push(spell);
                }
            }
            // Go GetAPLAura on the player: a known aura registers the action.
            if let Action::ActivateAura(id) = &prepull.action {
                if player
                    .auras
                    .iter()
                    .any(|aura| aura.action_id.as_ref() == Some(id))
                {
                    registered_prepull += 1;
                }
            }
        }
        if registered_prepull != player.prepull_actions {
            reasons.push(format!(
                "Go registered {} prepull actions and the rotation {registered_prepull}; \
                 prepull actions outside the rotation are unsupported",
                player.prepull_actions
            ));
        }
        let unreachable = unreachable_with_one_target(prepared, rotation);
        for item in &rotation.priority_list {
            if unreachable.contains(&item.position) {
                continue;
            }
            match &item.action {
                Action::AutocastOtherCooldowns => {
                    for cooldown in &player.major_cooldowns {
                        // A survival cooldown without timings waits for a health threshold Go
                        // never reaches at zero, so the autocast never casts it. Above zero
                        // it fires once health falls to it, also after its timings run out,
                        // which the runtime follows for class survival cooldowns only.
                        let survival = cooldown.kind.iter().any(|kind| kind == "survival");
                        let threshold = player.hp_percent_for_defensives != 0.0;
                        if survival && cooldown.timings_ns.is_empty() && !threshold {
                            continue;
                        }
                        // A survival cooldown the runtime implements, by its class, race,
                        // item or consumable effect; what its aura changes on the target's
                        // swings the tank limits check.
                        let known = cooldown_spell(prepared, cooldown).is_some_and(|spell| {
                            spell_capability(spell, prepared, gate).is_some_and(|kind| {
                                prepared.effects.iter().any(|effect| effect.kind() == kind)
                            })
                        });
                        if survival && threshold && !known {
                            reasons.push(format!(
                                "survival cooldown {} with a defensive health threshold is \
                                 unsupported",
                                cooldown.action_id.spell_id
                            ));
                            continue;
                        }
                        reachable.extend(cooldown_spell(prepared, cooldown));
                    }
                }
                action => {
                    for id in action.spells() {
                        reachable.extend(rotation_spell(prepared, id));
                    }
                }
            }
        }
        reasons.extend((gate.limits)(prepared, &reachable));
        reasons.extend(undirected_procs(prepared));
        reasons.extend(fixed_stat_changes(prepared));
        reasons.extend(fixed_stat_aura_changes(prepared));
        reasons.extend(uninherited_stat_changes(prepared));
        let mut unknown = BTreeSet::new();
        let mut limited = BTreeSet::new();
        for spell in reachable {
            let id = spell.action_id.clone().unwrap_or_default().to_string();
            match spell_capability(spell, prepared, gate) {
                Some(kind) if prepared.effects.iter().any(|effect| effect.kind() == kind) => {
                    required.insert(kind);
                }
                _ => {
                    unknown.insert(id.clone());
                }
            }
            // Go newHardcastAction: a tank's hardcast drops its avoidance, whose rolls the exporter
            // reads. A channel without a cast time sets no hardcast, so it keeps its avoidance.
            // Pushback of a channel with a cast time is not modeled. A cast with the pushback
            // flag is pushed back by every damaging hit, which needs the trigger the exporter
            // reads and a chance of one or none: no other chance has been compared with Go.
            if let Some(enemy) = &prepared.enemy {
                if spell.default_cast.cast_time_ns > 0 {
                    let pushback_chance = prepared.effects.iter().find_map(|effect| match effect {
                        Effect::PushbackTrigger { chance, .. } => Some(*chance),
                        _ => None,
                    });
                    let pushes_back = spell.has_flag("SpellFlagPushback");
                    if spell.has_flag("SpellFlagChanneled")
                        || enemy.reduced_avoidance_rolls.is_empty()
                        || (pushes_back && pushback_chance.is_none())
                    {
                        limited.insert(format!(
                            "rotation reaches {id}, a hardcast while the target swings at the player"
                        ));
                    } else if let Some(chance) = pushback_chance {
                        let rolled = chance - spell.pushback_resist;
                        if pushes_back && rolled > 0.0 && rolled < 1.0 {
                            limited.insert(format!(
                                "rotation reaches {id}, a hardcast the target's swings push back \
                                 with a chance that needs a roll"
                            ));
                        }
                    }
                }
            }
            for limit in runtime_limits(spell, (gate.spell)(spell).is_some()) {
                limited.insert(format!(
                    "rotation reaches {id}, which uses unsupported {limit}"
                ));
            }
        }
        reasons.extend(
            unknown
                .into_iter()
                .map(|id| format!("rotation reaches {id} without a known behavior")),
        );
        reasons.extend(limited);
    } else if player.prepull_actions != 0 {
        reasons.push("prepull actions outside the rotation are unsupported".into());
    }

    let implemented = implemented_effects();
    let missing: Vec<String> = required
        .into_iter()
        .filter(|kind| !implemented.contains(kind))
        .map(|kind| format!("effect {kind} is not implemented"))
        .collect();
    let mut all = missing;
    all.extend(reasons);
    all
}

/// Go gives `currentRage` no value on a unit without a rage bar, which drops the term; the
/// runtime reads the bar, so such a rotation is unsupported.
fn rage_without_bar(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let has_bar = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::RageBar { .. }));
    if has_bar {
        return Vec::new();
    }
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let mut uses = false;
        if let Some(condition) = &item.condition {
            condition.visit(&mut |value| uses |= matches!(value, Value::CurrentRage));
        }
        if uses {
            reasons.push(format!(
                "rotation item {} reads rage, which the player lacks",
                item.position
            ));
        }
    }
    reasons
}

/// Go gives energy and combo point values no value on a unit without an energy bar, which
/// drops the term; the runtime reads the bar, so such a rotation is unsupported.
fn energy_without_bar(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    if prepared.player.energy.is_some() {
        return Vec::new();
    }
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let mut uses = false;
        if let Some(condition) = &item.condition {
            condition.visit(&mut |value| {
                uses |= matches!(
                    value,
                    Value::CurrentEnergy
                        | Value::MaxEnergy
                        | Value::CurrentComboPoints
                        | Value::TimeToNextEnergyTick
                );
            });
        }
        if uses {
            reasons.push(format!(
                "rotation item {} reads energy or combo points, which the player lacks",
                item.position
            ));
        }
    }
    reasons
}

/// Whether Go prunes a prepull action for a constant false condition, which is the only use
/// Go makes of it; `None` when community #622 would prune it differently.
fn prepull_pruned(prepared: &PreparedV2, prepull: &crate::rotation::Prepull) -> Option<bool> {
    let aura = |id: &ActionId| find_aura(prepared, id);
    let target_aura = |id: &ActionId| find_unit_aura(&prepared.target.auras, id);
    let spell = |id: &ActionId| rotation_spell_index(prepared, id);
    let dot = |id: &ActionId| {
        rotation_spell_index(prepared, id).and_then(|index| dot_owner(prepared, index))
    };
    let pet_auras = crate::core::fight::pet::pet_agent_auras(prepared);
    let pet_aura_known =
        |pet: usize, id: &ActionId| pet_auras.get(pet).is_some_and(|auras| auras.contains(id));
    let lookup = Lookup {
        aura: &aura,
        target_aura: &target_aura,
        spell: &spell,
        dot: &dot,
        pet_aura_known: &pet_aura_known,
    };
    let pruned = |missing| {
        compile_condition(prepull.condition.as_ref(), &lookup, missing).never_holds()
            && compile_condition(prepull.condition.as_ref(), &lookup, missing)
                == crate::rotation::CompiledCondition::Pruned
    };
    let pinned = pruned(MissingAura::Dropped);
    (pinned == pruned(MissingAura::Inactive)).then_some(pinned)
}

/// Go `ShouldRefreshExclusiveEffects` depends on the other effects of each exclusive category,
/// which Rust reads only from an exported reading: an aura holding its category alone, or one
/// another aura holds for good. Any other reading, or an aura the unit lacks, is unsupported.
fn aura_refresh_conditions(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let mut reasons = Vec::new();
    let conditions = rotation.priority_list.iter().flat_map(|item| {
        let interrupt = match &item.action {
            Action::ChannelSpell { interrupt_if, .. } => interrupt_if.as_ref(),
            _ => None,
        };
        item.condition.iter().chain(interrupt)
    });
    for condition in conditions {
        condition.visit(&mut |value| {
            let Value::AuraShouldRefresh { id, target, .. } = value else {
                return;
            };
            let (unit, auras) = if *target {
                ("target", &prepared.target.auras)
            } else {
                ("player", &prepared.player.auras)
            };
            let Some(aura) = auras.iter().find(|aura| aura.action_id.as_ref() == Some(id)) else {
                reasons.push(format!("auraShouldRefresh names {id}, which the {unit} lacks"));
                return;
            };
            let read = prepared.effects.iter().any(|effect| {
                matches!(effect, Effect::AuraShouldRefresh { unit: u, aura: label, modes }
                    if u == unit && *label == aura.label
                        && modes.iter().all(|mode| mode == "own" || mode == "never"))
            });
            if !read {
                reasons.push(format!(
                    "auraShouldRefresh on {unit} aura {:?} has no supported exclusive effect reading",
                    aura.label
                ));
            }
        });
    }
    reasons
}

fn find_aura(prepared: &PreparedV2, id: &ActionId) -> Option<FoundAura<ActionId>> {
    find_unit_aura(&prepared.player.auras, id)
}

/// Go `GetAuraByID` on a unit's exported auras.
fn find_unit_aura(
    auras: &[crate::contracts::prepared_v2::Aura],
    id: &ActionId,
) -> Option<FoundAura<ActionId>> {
    auras
        .iter()
        .find(|aura| aura.action_id.as_ref() == Some(id))
        .map(|aura| FoundAura {
            aura: id.clone(),
            max_stacks: aura.max_stacks,
        })
}

/// Rotation items, by position, whose condition can never hold against the one target the
/// runtime supports, such as a `numberTargets` of two or more. Go still evaluates them, without
/// side effects, but never runs their action.
fn unreachable_with_one_target(prepared: &PreparedV2, rotation: &Rotation) -> BTreeSet<usize> {
    let aura = |id: &ActionId| find_aura(prepared, id);
    let target_aura = |id: &ActionId| find_unit_aura(&prepared.target.auras, id);
    let spell = |id: &ActionId| rotation_spell_index(prepared, id);
    let dot = |id: &ActionId| {
        rotation_spell_index(prepared, id).and_then(|index| dot_owner(prepared, index))
    };
    let pet_auras = crate::core::fight::pet::pet_agent_auras(prepared);
    let pet_aura_known =
        |pet: usize, id: &ActionId| pet_auras.get(pet).is_some_and(|auras| auras.contains(id));
    let lookup = Lookup {
        aura: &aura,
        target_aura: &target_aura,
        spell: &spell,
        dot: &dot,
        pet_aura_known: &pet_aura_known,
    };
    rotation
        .priority_list
        .iter()
        .filter(|item| {
            let condition = item.condition.as_ref().map(Value::with_one_target);
            compile_condition(condition.as_ref(), &lookup, MissingAura::Dropped).never_holds()
        })
        .map(|item| item.position)
        .collect()
}

/// Pinned Go gives `auraIsActive` and `auraNumStacks` on an aura the character cannot have
/// no value, which drops the term from its condition; community fix ElliotWood/Forever#622
/// (252f57aa8) reads the aura as inactive, with no stacks, instead. Where both compile to the
/// same action, as when an `auraIsKnown` guard already prunes it, the input is unaffected.
/// Otherwise reject it until the reference adopts the fix. See upstream/changes.json.
fn unknown_aura_conditions(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let known = |id: &ActionId| find_aura(prepared, id).is_some();
    // Go resolves rotation names on the casting player: `GetAuraByID` finds the first aura
    // with the same action ID, tag included; `GetAPLSpell` and `GetAPLDot` find spells.
    let aura = |id: &ActionId| find_aura(prepared, id);
    let target_aura = |id: &ActionId| find_unit_aura(&prepared.target.auras, id);
    let target_known = |id: &ActionId| target_aura(id).is_some();
    let spell = |id: &ActionId| rotation_spell_index(prepared, id);
    let dot = |id: &ActionId| {
        rotation_spell_index(prepared, id).and_then(|index| dot_owner(prepared, index))
    };
    let pet_auras = crate::core::fight::pet::pet_agent_auras(prepared);
    let pet_aura_known =
        |pet: usize, id: &ActionId| pet_auras.get(pet).is_some_and(|auras| auras.contains(id));
    let lookup = Lookup {
        aura: &aura,
        target_aura: &target_aura,
        spell: &spell,
        dot: &dot,
        pet_aura_known: &pet_aura_known,
    };
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let pinned = compile_condition(item.condition.as_ref(), &lookup, MissingAura::Dropped);
        let fixed = compile_condition(item.condition.as_ref(), &lookup, MissingAura::Inactive);
        // A channel's interrupt condition compiles the same way; any difference counts.
        let interrupt = match &item.action {
            Action::ChannelSpell { interrupt_if, .. } => interrupt_if.as_ref(),
            _ => None,
        };
        let interrupt_same = compile_bool_value(interrupt, &lookup, MissingAura::Dropped)
            == compile_bool_value(interrupt, &lookup, MissingAura::Inactive);
        if pinned.same_meaning(&fixed) && interrupt_same {
            continue;
        }
        let mut unknown = Vec::new();
        for condition in item.condition.iter().chain(interrupt) {
            condition.visit(&mut |value| match value {
                Value::AuraIsActive(id) if !known(id) => {
                    unknown.push(("auraIsActive", id.to_string()))
                }
                Value::TargetAuraIsActive(id) if !target_known(id) => {
                    unknown.push(("auraIsActive on the target", id.to_string()))
                }
                Value::TargetAuraNumStacks(id) if !target_known(id) => {
                    unknown.push(("auraNumStacks on the target", id.to_string()))
                }
                Value::AuraNumStacks(id) if !known(id) => {
                    unknown.push(("auraNumStacks", id.to_string()))
                }
                Value::AuraRemainingTime(id) if !known(id) => {
                    unknown.push(("auraRemainingTime", id.to_string()))
                }
                Value::TargetAuraRemainingTime(id) if !target_known(id) => {
                    unknown.push(("auraRemainingTime on the target", id.to_string()))
                }
                _ => {}
            });
        }
        for (operator, id) in unknown {
            reasons.push(format!(
                "rotation item {}: {operator} names {id}, which the character lacks; \
                 the pinned reference drops the condition and community #622 reads it as inactive",
                item.position
            ));
        }
    }
    reasons
}
