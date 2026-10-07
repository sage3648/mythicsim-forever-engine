//! Rust preparation: the application's request built into a reset simulation and described
//! as prepared v2, as the pinned Go engine and tools/oracle-v2 do.
//!
//! The modules mirror the Go files they port, under sim/core. A request feature preparation
//! does not cover yet is refused with a stable code, so the caller can prepare it in Go; a
//! request it prepares must give exactly the prepared state the Go exporter writes.

pub(crate) mod agent;
pub(crate) mod attack;
pub(crate) mod aura_helpers;
pub(crate) mod buffs;
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
pub(crate) mod forever_item_sets;
pub(crate) mod forever_items;
pub(crate) mod forever_items_generated;
pub(crate) mod incapacitate;
pub(crate) mod item_aura;
pub(crate) mod item_effects;
pub(crate) mod item_proc;
pub(crate) mod item_sets;
#[cfg(test)]
pub(crate) mod item_test_support;
pub(crate) mod itemhelpers;
pub(crate) mod items;
pub(crate) mod items_registry;
pub(crate) mod major_cooldown;
pub(crate) mod parse_effects;
pub(crate) mod periodic_action;
pub(crate) mod pet;
pub(crate) mod presets;
pub(crate) mod proc_type_mask;
pub(crate) mod procs;
pub(crate) mod professions;
pub(crate) mod racials;
pub(crate) mod rage;
pub(crate) mod resolve_aura;
pub(crate) mod resolve_proc;
pub(crate) mod resolve_spell;
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
pub(crate) mod target;

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

/// Prepares a request as JSON, the document tools/oracle-v2 writes.
pub fn prepare_json(request: &[u8], scenario: &str) -> Result<serde_json::Value, PrepareError> {
    let request = Request::from_json(request).map_err(PrepareError::Invalid)?;
    let options = request.message().message("sim_options");
    if options.is_none_or(|options| options.i32("iterations") <= 0) {
        return Err(PrepareError::Invalid("missing iterations".to_string()));
    }
    let digest = request.sha256();
    let mut env = env::Environment::new(request.message(), crate::classes::prepare_agent)
        .map_err(PrepareError::Refused)?;
    export::export(&mut env, &digest, scenario).map_err(PrepareError::Refused)
}

/// Prepares a request into the prepared v2 contract.
pub fn prepare(request: &[u8], scenario: &str) -> Result<PreparedV2, PrepareError> {
    let value = prepare_json(request, scenario)?;
    serde_json::from_value(value)
        .map_err(|err| PrepareError::Invalid(format!("prepared output: {err}")))
}
