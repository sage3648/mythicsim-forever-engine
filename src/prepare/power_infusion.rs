//! Power Infusion: tools/oracle-v2/power_infusion.go. The generated aura `buffs.PowerInfusionsAura`
//! builds is the priest's own (talents_discipline.go `applyPowerInfusion`) and the external
//! caster's copy that any class receives from priests in the raid (buffs/drivers.go
//! `drivePowerInfusions`, an external cooldown that `external_cooldowns.rs` describes). Both bid in
//! the same exclusive categories.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::dbcenums;
use super::env::Environment;
use super::sim::{AuraId, Sim, UnitId};
use super::spelldata::must_find;
use super::stats::SCHOOL_LEN;

/// The Power Infusion row every copy of the aura reads.
const SPELL: i32 = 10060;

fn find_aura(sim: &Sim, unit: UnitId, action: &ActionId) -> Option<AuraId> {
    sim.unit(unit)
        .auras
        .iter()
        .copied()
        .find(|aura| sim.aura(*aura).action_id.as_ref() == Some(action))
}

/// `powerInfusionEffect`: the aura with the action ID multiplies the damage of the schools its
/// mask names and healing dealt. The multipliers come from client data; a separate reset
/// simulation checks they are all the aura changes. No aura outside Power Infusion shares its
/// categories, which hold one effect for each copy of the aura.
pub(crate) fn power_infusion_effect(
    env: &Environment,
    action: &ActionId,
    notes: &mut Vec<String>,
) -> Option<Value> {
    let mut note = |condition: bool, message: &str| {
        if condition {
            notes.push(message.to_string());
        }
    };
    let player = env.player;
    let row = must_find(SPELL);
    let damage = 1.0
        + row
            .effect(dbcenums::A_MOD_DAMAGE_PERCENT_DONE, 126)
            .percent();
    let healing = 1.0
        + row
            .effect(dbcenums::A_MOD_HEALING_DONE_PERCENT, 126)
            .percent();
    let Some(aura) = find_aura(&env.sim, player, action) else {
        note(true, "Power Infusion has no aura");
        return None;
    };
    for effect in &env.sim.aura(aura).exclusive_effects {
        let category = env.sim.effects[effect.0].category;
        for member in &env.sim.categories[category.0].effects {
            let member_aura = env.sim.effects[member.0].aura;
            let spell_id = env
                .sim
                .aura(member_aura)
                .action_id
                .as_ref()
                .map_or(0, |id| id.spell_id);
            note(spell_id != SPELL, "Power Infusion shares its category");
        }
    }
    let label = env.sim.aura(aura).label.clone();
    let mut fresh = env.fresh();
    let player = fresh.player;
    let before = fresh.sim.unit(player).pseudo_stats.clone();
    let before_stats = fresh.sim.unit(player).stats;
    let fresh_aura =
        find_aura(&fresh.sim, player, action).expect("the fresh simulation has the same auras");
    fresh.sim.activate(fresh_aura);
    let mut after = fresh.sim.unit(player).pseudo_stats.clone();
    let mut schools = Vec::new();
    for school in 0..SCHOOL_LEN {
        if after.school_damage_dealt_multiplier[school]
            != before.school_damage_dealt_multiplier[school]
        {
            schools.push(school);
            note(
                after.school_damage_dealt_multiplier[school]
                    != before.school_damage_dealt_multiplier[school] * damage,
                "Power Infusion's school damage is not its client multiplier",
            );
        }
    }
    note(
        after.healing_dealt_multiplier != before.healing_dealt_multiplier * healing,
        "Power Infusion's healing is not its client multiplier",
    );
    after.school_damage_dealt_multiplier = before.school_damage_dealt_multiplier;
    after.healing_dealt_multiplier = before.healing_dealt_multiplier;
    note(
        after != before || fresh.sim.unit(player).stats != before_stats,
        "Power Infusion changes more than school damage and healing",
    );
    Some(json!({
        "kind": "power_infusion", "spell_id": SPELL, "aura": label,
        "damage_multiplier": damage, "schools": schools, "healing_multiplier": healing,
    }))
}
