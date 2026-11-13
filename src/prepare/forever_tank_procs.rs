//! Under Forever's rules: the equip effects and set bonuses of the live client that act when the
//! wearer is struck or blocks, whose rows state a ProcChance of 100 beside a tooltip that says
//! "chance". The owner's decision of 13 November 2026 reads those rows literally: a ProcChance of
//! 100 is the rate, gated by each row's own recovery or cooldown. Krol'dok Battlegear's rows are in
//! the pinned store (the generator files the (4) as stating no rate, ProcChanceSource 3, which the
//! decision overrides with a chance of 1); Zealot Plate's and Girdle of Extracted Answers' are
//! restated in `data/forever_client_spells.rs`.
//!
//! - Krol'dok Battlegear (4), Krol'dok Resolve (1324298, ItemSetSpell 8249): "Chance on block to
//!   heal for 14 to 23. Healing is doubled in Strongholds and Cities." Every block heals the
//!   wearer by a roll of the heal row (1324296: 18 with a variance of 0.5), through a heal spell
//!   cast on the wearer as an enchant's heal proc is (`NewSpellDataHealProc`). No fight is in a
//!   Stronghold or City, so the doubling never applies.
//! - Krol'dok Battlegear (5), Ogre Dominance (1324309, ItemSetSpell 8250): "Taking damage has a
//!   high chance to increase your Stamina by 1 for 30 sec, stacking up to 20 times. (3s
//!   cooldown)" Each landed hit taken that deals damage, at most once in the row's 3 sec proc
//!   recovery, refreshes the buff (1324308) and adds a stack, as a stacking stat proc does.
//! - Zealot Plate (274651), Proc Self Energize 88 (1294668): "When struck in combat, has a chance
//!   to energize you for 66 to 110 mana. This effect cannot occur more than once every 15 sec."
//!   A landed melee hit taken that deals damage, at most once in the row's 15 sec proc recovery,
//!   restores a roll of Zealotry's Energize (1294666: 88 with a variance of 0.5) to a wearer
//!   with a mana bar.
//! - Girdle of Extracted Answers (276742), Shield Spike (1308777): "Deals 20 to 30 Physical
//!   damage every time you block." Every block deals a roll of the row's Proc Trigger Damage (25
//!   with a variance of 0.4) to the attacker, at most once in the row's 5 sec cooldown. That
//!   cooldown is the spell's own (SpellCooldowns 100881, ItemEffect 236443 states none: -1), not
//!   a proc recovery (SpellAuraOptions 247236 has none), and the tooltip reads it as "5 sec
//!   cooldown"; it gates the proc as a recovery would. The damage never misses and cannot crit,
//!   as the damage of a proc trigger damage aura does not roll an attack table.
//!
//! A character no target attacks is never struck and never blocks: it wears each as a plain item,
//! and the listeners are inert. A tank with no mana bar gains nothing from Zealot Plate, which
//! registers no listener for it, and one with no shield never blocks.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::{HitOutcome, StackingStatAura};
use super::env::Environment;
use super::export_items::{aura_named, outcome_names, spell_position};
use super::resolve_aura::{aura_config, label};
use super::resolve_proc::{chance, proc_trigger};
use super::shared_items::{spell_data_proc_damage_spell, spell_data_proc_heal_spell, EffectSource};
use super::sim::AuraId;
use super::spelldata::must_find;
use super::stats::{Stat, Stats};

/// Zealot Plate.
pub(crate) const ZEALOT_PLATE: i32 = 274_651;
/// Girdle of Extracted Answers.
pub(crate) const GIRDLE_OF_EXTRACTED_ANSWERS: i32 = 276_742;

/// Krol'dok Resolve and the heal it casts.
const KROLDOK_RESOLVE: i32 = 1_324_298;
const KROLDOK_RESOLVE_HEAL: i32 = 1_324_296;
/// Ogre Dominance and the buff it applies.
const OGRE_DOMINANCE: i32 = 1_324_309;
const OGRE_DOMINANCE_BUFF: i32 = 1_324_308;
/// Proc Self Energize 88 and Zealotry.
const ZEALOT_TRIGGER: i32 = 1_294_668;
const ZEALOTRY: i32 = 1_294_666;
/// Shield Spike.
const SHIELD_SPIKE: i32 = 1_308_777;

/// The labels the listeners go by. A set bonus's is the permanent aura its piece count registers
/// (`item_sets`), an item's the item's name.
const KROLDOK_4_LABEL: &str = "Krol'dok Battlegear 4P";
const KROLDOK_5_LABEL: &str = "Krol'dok Battlegear 5P";
const ZEALOT_PLATE_LABEL: &str = "Zealot Plate";
const GIRDLE_LABEL: &str = "Girdle of Extracted Answers";
/// The stacking buff Ogre Dominance applies.
const OGRE_DOMINANCE_AURA: &str = "Ogre Dominance";

/// The item effects of this module, applied under Forever's rules.
pub(crate) fn has_item_effect(item: i32) -> bool {
    matches!(item, ZEALOT_PLATE | GIRDLE_OF_EXTRACTED_ANSWERS)
}

/// Applies the equip effect of one of [`has_item_effect`]'s items, answering whether it is one.
pub(crate) fn apply_item(env: &mut Environment, item: i32) -> bool {
    match item {
        ZEALOT_PLATE => zealot_plate(env),
        GIRDLE_OF_EXTRACTED_ANSWERS => shield_spike(env),
        _ => return false,
    }
    true
}

/// Krol'dok Battlegear (4): a heal spell cast on the wearer, and the listener on the set's aura
/// that casts it on every block.
pub(crate) fn kroldok_battlegear_4(env: &mut Environment, set_aura: AuraId) {
    let unit = env.player;
    let config = spell_data_proc_heal_spell(env, must_find(KROLDOK_RESOLVE_HEAL), true);
    env.sim.register_spell(unit, config);
    // The store files the row's 100 beside "chance on block" as no stated rate; it is the rate.
    let rate = [chance(1.0)];
    let mut listener = proc_trigger(&env.sim, Some(unit), must_find(KROLDOK_RESOLVE), &rate);
    listener.name = KROLDOK_4_LABEL.to_string();
    // The row names a block: the mask alone hears every landed hit, and a block that takes the
    // whole hit deals no damage.
    listener.outcome = HitOutcome::BLOCK;
    listener.require_damage_dealt = false;
    listener.trigger_immediately = true;
    env.sim.attach_proc_trigger(set_aura, &listener);
}

/// Krol'dok Battlegear (5): the stacking Stamina buff, and the listener on the set's aura that
/// refreshes it and adds a stack.
pub(crate) fn kroldok_battlegear_5(env: &mut Environment, set_aura: AuraId) {
    let unit = env.player;
    let buff = must_find(OGRE_DOMINANCE_BUFF);
    let stamina = buff
        .effects
        .iter()
        .find(|effect| effect.aura == super::dbcenums::A_MOD_STAT && effect.misc == STAMINA_STAT)
        .map_or(0.0, |effect| effect.base_points);
    env.sim.make_stacking_aura(
        unit,
        StackingStatAura {
            aura: aura_config(buff, &[label(OGRE_DOMINANCE_AURA)]),
            bonus_per_stack: Stats::from_pairs(&[(Stat::Stamina, stamina)]),
        },
    );
    let mut listener = proc_trigger(&env.sim, Some(unit), must_find(OGRE_DOMINANCE), &[]);
    listener.name = KROLDOK_5_LABEL.to_string();
    env.sim.attach_proc_trigger(set_aura, &listener);
}

/// The client's stat index of Stamina in a Mod Stat aura's Misc.
const STAMINA_STAT: i32 = 2;

/// Zealot Plate: the listener on the item that restores mana when the wearer is struck. A wearer
/// with no mana bar gains nothing, so the plate is a plain item and no listener is registered.
fn zealot_plate(env: &mut Environment) {
    let unit = env.player;
    if !env.sim.unit(unit).mana_bar.enabled {
        return;
    }
    let source = EffectSource {
        id: ZEALOT_PLATE,
        is_enchant: false,
    };
    let mut listener = proc_trigger(&env.sim, Some(unit), must_find(ZEALOT_TRIGGER), &[]);
    listener.name = ZEALOT_PLATE_LABEL.to_string();
    listener.action_id = source.action_id();
    listener.trigger_immediately = true;
    source.register_trigger(env, &listener);
}

/// Girdle of Extracted Answers: the spell that deals Shield Spike's damage, and the listener on
/// the item that casts it on the attacker on a block.
fn shield_spike(env: &mut Environment) {
    let unit = env.player;
    let row = must_find(SHIELD_SPIKE);
    let source = EffectSource {
        id: GIRDLE_OF_EXTRACTED_ANSWERS,
        is_enchant: false,
    };
    let config = spell_data_proc_damage_spell(env, row, true);
    env.sim.register_spell(unit, config);
    let mut listener = proc_trigger(&env.sim, Some(unit), row, &[]);
    listener.name = GIRDLE_LABEL.to_string();
    listener.action_id = source.action_id();
    listener.outcome = HitOutcome::BLOCK;
    listener.require_damage_dealt = false;
    // The cooldown is the spell's own and gates the proc like a recovery.
    listener.icd = row.cooldown();
    listener.trigger_immediately = true;
    source.register_trigger(env, &listener);
}

/// The stat auras of the effects above, for the combinations `statAurasEffect` reads: Ogre
/// Dominance's stacks, when the wearer is struck at all.
pub(crate) fn stat_auras(env: &Environment) -> Vec<String> {
    if aura_named(env, KROLDOK_5_LABEL).is_some() && env.tanking() {
        vec![OGRE_DOMINANCE_AURA.to_string()]
    } else {
        Vec::new()
    }
}

/// A listener nothing in the fight reaches: the wearer is struck by no target.
fn inert(label: &str) -> Value {
    json!({
        "kind": "inert_listener", "unit": "player", "aura": label,
        "reason": "hears only melee hits the player takes",
    })
}

/// The exported effects of this module's listeners the character wears.
pub(crate) fn export_effects(env: &Environment) -> Vec<Value> {
    let tanking = env.tanking();
    let mut effects = Vec::new();
    for label in [
        KROLDOK_4_LABEL,
        KROLDOK_5_LABEL,
        ZEALOT_PLATE_LABEL,
        GIRDLE_LABEL,
    ] {
        let Some(aura) = aura_named(env, label) else {
            continue;
        };
        let described = if tanking {
            match label {
                KROLDOK_4_LABEL => kroldok_heal_effect(env, aura),
                KROLDOK_5_LABEL => Some(ogre_dominance_effect(env, aura)),
                ZEALOT_PLATE_LABEL => Some(zealot_plate_effect(env, aura)),
                GIRDLE_LABEL => shield_spike_effect(env, aura),
                _ => None,
            }
        } else {
            None
        };
        effects.push(described.unwrap_or_else(|| inert(label)));
    }
    effects
}

/// The spellbook position of the spell cast for a row.
fn position_of(env: &Environment, row: i32) -> Option<usize> {
    spell_position(env, &ActionId::spell(row))
}

/// `spell_data_heal_proc` for Krol'dok Resolve: a block casts the heal on the wearer at once.
fn kroldok_heal_effect(env: &Environment, aura: AuraId) -> Option<Value> {
    let heal = must_find(KROLDOK_RESOLVE_HEAL);
    let effect = heal.proc_heal_effect();
    let position = position_of(env, KROLDOK_RESOLVE_HEAL)?;
    let player = env.player;
    let pseudo = &env.sim.unit(player).pseudo_stats;
    let level = env.sim.unit(player).level;
    Some(json!({
        "kind": "spell_data_heal_proc", "trigger_aura": env.sim.aura(aura).label,
        "trigger_spells": Vec::<usize>::new(), "struck": true,
        "outcome": outcome_names(HitOutcome::BLOCK),
        "require_damage": false, "proc_chance": 1.0, "spell": position,
        "can_crit": !heal.cannot_crit(),
        "healing_dealt_multiplier": pseudo.healing_dealt_multiplier,
        "healing_taken_multiplier": pseudo.healing_taken_multiplier,
        "table_healing_dealt_multiplier": env.attack_table(player, player).healing_dealt_multiplier,
        "bonus_healing_taken": pseudo.bonus_healing_taken,
        "average": effect.average(level), "variance": effect.variance,
    }))
}

/// `spell_data_stat_proc` for Ogre Dominance: a landed hit taken refreshes the buff and adds a
/// stack, a spell batch window later.
fn ogre_dominance_effect(env: &Environment, aura: AuraId) -> Value {
    json!({
        "kind": "spell_data_stat_proc", "trigger_aura": env.sim.aura(aura).label,
        "aura": OGRE_DOMINANCE_AURA, "trigger_spells": Vec::<usize>::new(),
        "callbacks": ["on_spell_hit_taken"], "struck": true, "landed_only": true,
        "require_damage": true, "proc_chance": 1.0, "add_stack": true,
    })
}

/// `mana_proc` for Zealot Plate: a landed hit taken restores a roll of Zealotry's Energize.
fn zealot_plate_effect(env: &Environment, aura: AuraId) -> Value {
    let energize = must_find(ZEALOTRY).energize_effect();
    let level = env.sim.unit(env.player).level;
    json!({
        "kind": "mana_proc", "trigger_aura": env.sim.aura(aura).label,
        "rng_label": env.sim.aura(aura).label, "chances": Vec::<Value>::new(),
        "mana": energize.average(level), "variance": energize.variance,
        "metrics_action_id": json!({"spell_id": ZEALOTRY}), "delay_ns": 0,
        "struck": true, "proc_chance": 1.0, "landed_only": true, "require_damage": true,
    })
}

/// `spell_data_damage_proc` for Shield Spike: a block deals the row's roll to the attacker.
fn shield_spike_effect(env: &Environment, aura: AuraId) -> Option<Value> {
    let position = position_of(env, SHIELD_SPIKE)?;
    let damage = must_find(SHIELD_SPIKE)
        .effects
        .iter()
        .find(|effect| effect.aura == super::dbcenums::A_PROC_TRIGGER_DAMAGE)?;
    let level = env.sim.unit(env.player).level;
    Some(json!({
        "kind": "spell_data_damage_proc", "trigger_aura": env.sim.aura(aura).label,
        "trigger_spells": Vec::<usize>::new(), "struck": true, "landed_only": true,
        "require_damage": false, "proc_chance": 1.0, "spell": position,
        "average": damage.average(level), "variance": damage.variance, "can_crit": false,
        "outcome": "always_hit", "taken_outcome": outcome_names(HitOutcome::BLOCK),
    }))
}
