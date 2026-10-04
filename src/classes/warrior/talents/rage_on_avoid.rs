//! Shield Specialization (12298) and Master of Defense (1310316), from Go
//! sim/warrior/talents_protection.go `registerRageOnAvoid`: proc triggers on hits taken that
//! act at once, granting rage at a chance on a block, or on a dodge or parry while the warrior
//! can block.

use crate::core::fight::{
    Agent, AuraRef, Fight, SpellResult, OUTCOME_BLOCK, OUTCOME_DODGE, OUTCOME_PARRY,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AvoidRage {
    pub(crate) aura: AuraRef,
    /// The outcome bits the trigger hears.
    pub(crate) outcomes: u16,
    /// Go `ExtraCondition`: the warrior can block, which nothing changes in a fight.
    pub(crate) allowed: bool,
    pub(crate) chance: f64,
    pub(crate) rage: f64,
    /// Go `NewRageMetrics` of the energize spell.
    pub(crate) metrics: usize,
}

/// The outcome bits of the exported outcome names.
pub(crate) fn outcome_bits(outcomes: &[String]) -> u16 {
    outcomes
        .iter()
        .map(|outcome| match outcome.as_str() {
            "block" => OUTCOME_BLOCK,
            "dodge" => OUTCOME_DODGE,
            "parry" => OUTCOME_PARRY,
            _ => 0,
        })
        .fold(0, |bits, bit| bits | bit)
}

/// The trigger's OnSpellHitTaken.
pub(crate) fn on_hit_taken<A: Agent>(
    fight: &mut Fight<A>,
    params: AvoidRage,
    result: &SpellResult,
) {
    if result.outcome & params.outcomes == 0 || !params.allowed {
        return;
    }
    if params.chance != 1.0 && fight.random_for_aura(params.aura) > params.chance {
        return;
    }
    fight.add_rage(params.rage, params.metrics);
}
