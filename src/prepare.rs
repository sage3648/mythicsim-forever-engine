//! Rust preparation: the application's request built into a reset simulation and described
//! as prepared v2, as the pinned Go engine and tools/oracle-v2 do.
//!
//! The modules mirror the Go files they port, under sim/core. A request feature preparation
//! does not cover yet is refused with a stable code, so the caller can prepare it in Go; a
//! request it prepares must give exactly the prepared state the Go exporter writes.

pub(crate) mod agent;
pub(crate) mod attack;
pub(crate) mod aura_helpers;
pub(crate) mod battle_squawk;
pub(crate) mod buffs;
pub(crate) mod burst_hps;
pub(crate) mod character;
pub(crate) mod classic_enchants;
pub(crate) mod classic_export;
pub(crate) mod classic_items;
pub(crate) mod classic_weapons;
pub(crate) mod classic_whelp;
pub(crate) mod common_effects;
pub(crate) mod consumable_effects;
pub(crate) mod consumes;
pub(crate) mod damage_taken;
pub(crate) mod dbcenums;
pub(crate) mod debuffs;
pub(crate) mod enchant_speed;
pub(crate) mod enemy;
pub(crate) mod energy;
pub(crate) mod env;
pub(crate) mod export;
pub(crate) mod export_items;
pub(crate) mod external_cooldowns;
pub(crate) mod forever_item_sets;
pub(crate) mod forever_items;
pub(crate) mod forever_items_generated;
pub(crate) mod forever_set_spells_generated;
pub(crate) mod healer_gear;
pub(crate) mod heals;
pub(crate) mod hotfix_113554;
pub(crate) mod incapacitate;
pub(crate) mod item_aura;
pub(crate) mod item_effects;
pub(crate) mod item_proc;
pub(crate) mod item_sets;
pub(crate) mod item_swap;
#[cfg(test)]
pub(crate) mod item_test_support;
pub(crate) mod itemhelpers;
pub(crate) mod items;
pub(crate) mod items_registry;
pub(crate) mod levels;
pub(crate) mod major_cooldown;
pub(crate) mod mp5_sources;
pub(crate) mod oil_of_immolation;
pub(crate) mod overheal;
pub(crate) mod parse_effects;
pub(crate) mod party_damage;
pub(crate) mod periodic_action;
pub(crate) mod pet;
pub(crate) mod physical_damage;
pub(crate) mod power_infusion;
pub(crate) mod presets;
pub(crate) mod proc_oil;
pub(crate) mod proc_type_mask;
pub(crate) mod procs;
pub(crate) mod professions;
pub(crate) mod racials;
pub(crate) mod rage;
pub(crate) mod resolve_aura;
pub(crate) mod resolve_proc;
pub(crate) mod resolve_spell;
pub(crate) mod revelation;
pub(crate) mod rotation;
pub(crate) mod shared_auras;
pub(crate) mod shared_items;
pub(crate) mod shared_on_use;
pub(crate) mod shared_procs;
pub(crate) mod sim;
pub(crate) mod spell;
pub(crate) mod spell_mod;
pub(crate) mod spelldata;
pub(crate) mod stat_auras;
pub(crate) mod stats;
pub(crate) mod talent_layout;
pub(crate) mod target;
pub(crate) mod weapon_skill;

use crate::contracts::prepared_v2::PreparedV2;
use crate::contracts::request::Request;

/// Why preparation refuses a request: a stable code and the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub reason: String,
}

impl Refusal {
    pub(crate) fn new(code: &'static str, reason: String) -> Refusal {
        Refusal { code, reason }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.reason)
    }
}

/// The outcome of preparing a request in Rust.
#[derive(Debug)]
pub enum PrepareError {
    /// The request is not valid protojson for the reference's schema.
    Invalid(String),
    /// Rust preparation does not cover the request; Go can prepare it.
    Refused(Refusal),
    /// Preparation failed inside Rust, a defect; Go can prepare the request.
    Fault(String),
}

impl std::fmt::Display for PrepareError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepareError::Invalid(reason) => write!(f, "invalid request: {reason}"),
            PrepareError::Refused(refusal) => write!(f, "preparation refused: {refusal}"),
            PrepareError::Fault(reason) => write!(f, "preparation failed: {reason}"),
        }
    }
}

pub use sim::Rules;

/// Prepares a request as JSON, the document tools/oracle-v2 writes, with Forever's rules.
pub fn prepare_json(request: &[u8], scenario: &str) -> Result<serde_json::Value, PrepareError> {
    prepare_json_with(request, scenario, Rules::Forever)
}

/// Prepares a request as JSON with the given rules: [`Rules::GoReference`] gives exactly what the
/// pinned exporter writes, for the comparisons with Go.
pub fn prepare_json_with(
    request: &[u8],
    scenario: &str,
    rules: Rules,
) -> Result<serde_json::Value, PrepareError> {
    let bytes = request;
    let mut request = Request::from_json(bytes).map_err(PrepareError::Invalid)?;
    let options = request.message().message("sim_options");
    if options.is_none_or(|options| options.i32("iterations") <= 0) {
        return Err(PrepareError::Invalid("missing iterations".to_string()));
    }
    let digest = request.sha256();
    if rules == Rules::Forever {
        if let Some(known) = known_ranks(bytes)? {
            request = known;
        }
    }
    let mut character = levels::Character::from_request(request.message());
    if let Some(reason) = levels::level_refusal(
        &first_player_class(request.message()),
        character.level,
        character.sheet.is_some(),
    ) {
        return Err(PrepareError::Refused(Refusal::new(
            "level_unsupported",
            reason,
        )));
    }
    if character.level != levels::MAX_LEVEL {
        if let Some(sheet) = character.sheet.clone() {
            let (calibration, first) = calibrate_level(bytes, &character, &sheet, scenario, rules)?;
            character.calibration = calibration;
            // Every unlearned spell is known from the start of the real pass, so each export of
            // a spell, the item swap's included, flags it alike.
            character.unlearned = first.unlearned.clone();
            // The rotation names the ranks the character knows before preparation registers
            // its actions (levels::rewrite_rotations), as known_ranks does for book ranks.
            let mut value: serde_json::Value = serde_json::from_slice(bytes)
                .map_err(|err| PrepareError::Invalid(err.to_string()))?;
            if rules == Rules::Forever {
                spelldata::book_ranks::rewrite_request(&mut value);
            }
            levels::rewrite_rotations(&mut value, &first);
            let bytes =
                serde_json::to_vec(&value).map_err(|err| PrepareError::Fault(err.to_string()))?;
            request = Request::from_json(&bytes).map_err(PrepareError::Invalid)?;
        }
    }
    levels::with_character(character, || {
        let mut env =
            env::Environment::with_rules(request.message(), crate::classes::prepare_agent, rules)
                .map_err(PrepareError::Refused)?;
        export::export(&mut env, &digest, scenario).map_err(PrepareError::Refused)
    })
}

/// A levelling character's calibration ([`levels::calibrate`]): the request prepared once with
/// no buffs or consumables, in the gear the sheet was read in, its player's stats read against
/// the sheet.
fn calibrate_level(
    bytes: &[u8],
    character: &levels::Character,
    sheet: &levels::Sheet,
    scenario: &str,
    rules: Rules,
) -> Result<(levels::Calibration, levels::Character), PrepareError> {
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|err| PrepareError::Invalid(err.to_string()))?;
    without_buffs(&mut value);
    in_sheet_gear(&mut value);
    let bare = serde_json::to_vec(&value).map_err(|err| PrepareError::Fault(err.to_string()))?;
    let mut request = Request::from_json(&bare).map_err(PrepareError::Invalid)?;
    if rules == Rules::Forever {
        if let Some(known) = known_ranks(&bare)? {
            request = known;
        }
    }
    let digest = request.sha256();
    let (prepared, first, slopes) = levels::with_character(character.clone(), || {
        let mut env =
            env::Environment::with_rules(request.message(), crate::classes::prepare_agent, rules)
                .map_err(PrepareError::Refused)?;
        let prepared =
            export::export(&mut env, &digest, scenario).map_err(PrepareError::Refused)?;
        // How much of a point of base health or mana reaches the maximum: a talent or racial that
        // raises maximum health or mana by a share (Arcane Mind, Tauren Endurance) scales the base
        // too, and the solve divides by it.
        let unit = env.sim.unit(env.player);
        let slope = |stat: stats::Stat| {
            let before = unit.sdm.apply_stat_dependencies(unit.stats_without_deps);
            let mut plus = unit.stats_without_deps;
            plus[stat] += 1000.0;
            let slope = (unit.sdm.apply_stat_dependencies(plus)[stat] - before[stat]) / 1000.0;
            if slope > 0.0 {
                slope
            } else {
                1.0
            }
        };
        let slopes = levels::Slopes {
            health: slope(stats::Stat::Health),
            mana: slope(stats::Stat::Mana),
        };
        Ok::<_, PrepareError>((prepared, levels::current(), slopes))
    })?;
    let player = &prepared["player"];
    let mut first = first;
    // Every spell the first pass registered, unlearned ones included, with its class spell:
    // levels::rewrite_rotations points a rotation only at a known one of the same kind.
    first.registered = player["spells"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|spell| {
            let id = i32::try_from(spell["action_id"]["spell_id"].as_i64()?).ok()?;
            let kind = spell["class_spell"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            Some((id, kind))
        })
        .collect();
    let stats_of = |value: &serde_json::Value| {
        let mut stats = stats::Stats::default();
        for stat in stats::Stat::ALL {
            stats[stat] = value[stat.name()].as_f64().unwrap_or(0.0);
        }
        stats
    };
    let engine = stats_of(&player["stats"]);
    let class = player["class"].as_str().unwrap_or_default();
    let race = player["race"].as_str().unwrap_or_default();
    let used = levels::with_character(character.clone(), || character::base_stats(class, race))
        .unwrap_or_default();
    let tables = crate::data::tables::tables();
    let k_agi = tables
        .crit_per_agi_max_level
        .get(class)
        .copied()
        .unwrap_or(0.0);
    let k_int = tables
        .crit_per_int_max_level
        .get(class)
        .copied()
        .unwrap_or(0.0);
    Ok((
        levels::calibrate(sheet, &engine, &used, k_agi, k_int, slopes),
        first,
    ))
}

/// The request with each player in the gear its sheet was read in, where the sheet names it: a
/// Best Gear or Droptimizer set swaps other gear in, and the sheet is solved against its own
/// (levels::calibrate), so a candidate's own crit is not solved away.
fn in_sheet_gear(request: &mut serde_json::Value) {
    let Some(parties) = request
        .pointer_mut("/raid/parties")
        .and_then(serde_json::Value::as_array_mut)
    else {
        return;
    };
    for party in parties {
        let Some(players) = party
            .get_mut("players")
            .and_then(serde_json::Value::as_array_mut)
        else {
            continue;
        };
        for player in players {
            let gear = player
                .pointer("/characterSheet/equipment")
                .or_else(|| player.pointer("/character_sheet/equipment"))
                .cloned();
            if let (Some(gear), Some(player)) = (gear, player.as_object_mut()) {
                player.remove("equipment");
                player.insert("equipment".to_string(), gear);
                // The swap set belongs to the simmed gear: swapped over the sheet's own weapons
                // (a two-hander over a one-hander and shield) it can be one the engine refuses,
                // and the sheet was read with nothing swapped.
                for key in [
                    "itemSwap",
                    "item_swap",
                    "enableItemSwap",
                    "enable_item_swap",
                ] {
                    player.remove(key);
                }
            }
        }
    }
}

/// The class of the request's first player, as the schema names it (`ClassMage`).
fn first_player_class(request: &crate::contracts::request::Message) -> String {
    request
        .message("raid")
        .and_then(|raid| raid.messages("parties").first().copied())
        .and_then(|party| party.messages("players").first().copied())
        .map(|player| player.enum_name("class"))
        .unwrap_or_default()
}

/// The request with no raid, party or player buffs, no debuffs and no consumables, the way a
/// character sheet reads with nothing up.
fn without_buffs(request: &mut serde_json::Value) {
    fn clear(object: &mut serde_json::Value, keys: &[&str]) {
        if let Some(map) = object.as_object_mut() {
            for key in keys {
                map.remove(*key);
            }
        }
    }
    let Some(raid) = request.get_mut("raid") else {
        return;
    };
    clear(raid, &["buffs", "debuffs"]);
    if let Some(parties) = raid.get_mut("parties").and_then(|p| p.as_array_mut()) {
        for party in parties {
            clear(party, &["buffs"]);
            if let Some(players) = party.get_mut("players").and_then(|p| p.as_array_mut()) {
                for player in players {
                    clear(player, &["buffs", "consumables", "consumes"]);
                }
            }
        }
    }
}

/// The request with every Ahn'Qiraj 20 book rank it names read as the rank below, which is what
/// a player knows under Forever's rules (`spelldata::book_ranks`), or None when it names none.
/// The digest stays the one of the request as sent.
fn known_ranks(bytes: &[u8]) -> Result<Option<Request>, PrepareError> {
    let mut value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|err| PrepareError::Invalid(err.to_string()))?;
    if !spelldata::book_ranks::rewrite_request(&mut value) {
        return Ok(None);
    }
    let bytes = serde_json::to_vec(&value).map_err(|err| PrepareError::Fault(err.to_string()))?;
    Request::from_json(&bytes)
        .map(Some)
        .map_err(PrepareError::Invalid)
}

/// Prepares a request into the prepared v2 contract.
pub fn prepare(request: &[u8], scenario: &str) -> Result<PreparedV2, PrepareError> {
    let value = prepare_json(request, scenario)?;
    serde_json::from_value(value)
        .map_err(|err| PrepareError::Invalid(format!("prepared output: {err}")))
}
