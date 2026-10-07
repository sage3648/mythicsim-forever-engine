//! tools/oracle-v2/main.go `hitTakenItemListeners`: the item procs whose listener hears only the
//! melee hits the player takes.
//!
//! Go literal triggers in common/classic (Essence of the Pure Flame's damage shield, The Lion
//! Horn of Stormwind) and the trigger rows spelldata decodes (Uther's Strength, the chest
//! absorption enchants) hear nothing the Goblin Sapper Charge's spell hit on the thrower is.
//! Only the target's swings land melee hits on the player, so without a tank the listener is
//! inert. A tank's proc is a described effect, which is not ported yet: it is refused.

use serde_json::{json, Value};

use super::aura_helpers::CallbackMask;
use super::env::Environment;
use super::resolve_proc::proc_trigger;
use super::spell::ProcMask;
use super::spelldata;
use super::Refusal;

/// The item procs and the client rows their listener and absorb spell come from, if any.
const HIT_TAKEN_ITEM_PROCS: [(&str, i32, i32); 6] = [
    ("Essence of the Pure Flame", 0, 0),
    ("The Lion Horn of Stormwind", 0, 0),
    ("Uther's Strength", 8397, 10368),
    ("Enchant Chest - Minor Absorption", 7445, 7423),
    ("Enchant Chest - Lesser Absorption", 7446, 7447),
    ("Enchant Chest - Absorption", 1249072, 1249073),
];

/// Go `callbackNames`.
fn callback_names(callback: CallbackMask) -> Vec<&'static str> {
    [
        (CallbackMask::ON_SPELL_HIT_DEALT, "on_spell_hit_dealt"),
        (CallbackMask::ON_SPELL_HIT_TAKEN, "on_spell_hit_taken"),
        (
            CallbackMask::ON_PERIODIC_DAMAGE_DEALT,
            "on_periodic_damage_dealt",
        ),
        (CallbackMask::ON_HEAL_DEALT, "on_heal_dealt"),
        (
            CallbackMask::ON_PERIODIC_HEAL_DEALT,
            "on_periodic_heal_dealt",
        ),
        (CallbackMask::ON_CAST_COMPLETE, "on_cast_complete"),
        (CallbackMask::ON_APPLY_EFFECTS, "on_apply_effects"),
        (
            CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
            "on_periodic_damage_taken",
        ),
    ]
    .into_iter()
    .filter(|(mask, _)| callback.matches(*mask))
    .map(|(_, name)| name)
    .collect()
}

/// Go `hitTakenItemListeners`. A tank's proc is refused with the code `tanking`, which the
/// items that register these auras describe once they are ported.
pub(crate) fn hit_taken_item_listeners(
    env: &Environment,
    tanking: bool,
) -> Result<Vec<Value>, Refusal> {
    const LIFECYCLE: [&str; 7] = [
        "on_init",
        "on_reset",
        "on_done_iteration",
        "on_gain",
        "on_expire",
        "on_stacks_change",
        "on_encounter_start",
    ];
    let sim = &env.sim;
    let player = env.player;
    let mut effects = Vec::new();
    for (label, trigger, _absorb) in HIT_TAKEN_ITEM_PROCS {
        let Some(aura) = sim.get_aura(player, label) else {
            continue;
        };
        let mut heard = sim
            .aura(aura)
            .callback_names()
            .iter()
            .all(|name| LIFECYCLE.contains(&name.as_str()) || name == "on_spell_hit_taken");
        if trigger != 0 {
            let listener = proc_trigger(sim, Some(player), spelldata::find(trigger), &[]);
            let names = callback_names(listener.callback);
            heard = heard
                && names == ["on_spell_hit_taken"]
                && listener.proc_mask != ProcMask::UNKNOWN
                && listener.proc_mask.0 & !ProcMask::MELEE.0 == 0;
        }
        // Every item in the list is one of the three tank cases: the damage shield, the Lion Horn's
        // stat proc or an absorb shield.
        if heard && tanking {
            return Err(Refusal::new(
                "tanking",
                format!("{label}'s proc on the melee hits a tank takes is not prepared yet"),
            ));
        }
        if heard {
            effects.push(
                json!({"kind": "inert_listener", "unit": "player", "aura": label,
                "reason": "hears only melee hits the player takes"}),
            );
        }
    }
    Ok(effects)
}
