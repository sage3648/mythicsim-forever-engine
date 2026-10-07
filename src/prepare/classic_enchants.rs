//! Go sim/common/classic/enchants.go: Enchant Gloves - Threat, Enchant Cloak - Subtlety and
//! Crusader. Item swapping is refused in preparation, so the `ItemSwap` registrations after each
//! aura have nothing to toggle.

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use super::classic_items::tagged;
use super::env::Environment;
use super::shared_items::dynamic_legacy_proc_for_enchant;
use super::sim::{AuraConfig, SECOND};
use super::stats::{Stat, Stats};

/// Crusader's effect ID.
pub(crate) const CRUSADER: i32 = 1900;

/// Go `NewEnchantEffect` for the IDs sim/common/classic registers: applies the effect to the
/// player and answers true, or answers false for an ID it registers nothing for.
pub(crate) fn apply_enchant_effect(env: &mut Environment, enchant: i32) -> bool {
    match enchant {
        2613 => threat_aura(env, "Increase Threat", 1.02),
        2621 => threat_aura(env, "Decrease Threat", 0.98),
        CRUSADER => crusader(env),
        _ => return false,
    }
    true
}

/// Enchant Gloves - Threat and Enchant Cloak - Subtlety: a permanent aura that scales the
/// wearer's threat.
fn threat_aura(env: &mut Environment, label: &str, multiplier: f64) {
    let unit = env.player;
    let aura = env.sim.register_aura(
        unit,
        AuraConfig {
            label: label.to_string(),
            ..AuraConfig::default()
        },
    );
    env.sim.make_permanent(aura);
    env.sim
        .attach_multiplicative_pseudo_stat_buff(aura, PseudoStatField::ThreatMultiplier, multiplier);
}

/// Crusader (spell 20007): a weapon proc at 1 PPM on landed hits that heals 75 to 125 and gives
/// the hand that struck 100 Strength for 15 sec, in a buff of its own for each hand.
fn crusader(env: &mut Environment) {
    let unit = env.player;
    let duration = 15 * SECOND;
    let action_id = ActionId::spell(20007);

    for (tag, suffix) in [(1, "(MH)"), (2, "(OH)")] {
        env.sim.new_temporary_stats_aura(
            unit,
            &format!("Holy Strength {suffix}"),
            &tagged(&action_id, tag),
            Stats::from_pairs(&[(Stat::Strength, 100.0)]),
            duration,
        );
    }

    let dpm = dynamic_legacy_proc_for_enchant(&env.sim, unit, CRUSADER, 1.0, 0.0);
    env.sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: "Enchant Weapon - Crusader".to_string(),
            action_id,
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            dpm: Some(dpm),
            is_weapon_proc: true,
            ..ProcTrigger::default()
        },
    );
}
