//! Reckless Swings (1329555, proc 1329554), from the development notes of 2 November 2026 and
//! wowsims/forever #841's `registerRecklessSwings`: a cast for a share of base health gives an
//! aura of four charges for 8 sec, and each landed white swing that dealt damage gives 5 Rage and
//! spends a charge a spell batch window later, as a proc handler's effects do. Forever's rules
//! only. Since the hotfixes of 16 November 2026 ("Fixed an issue where Reckless Swings was not
//! activated by Cleave or Heroic Strike"; the row's ProcFlags went from 0x4 to 0x14) Heroic
//! Strike and Cleave count as swings too; no other melee ability does. Since push 114115 a Heroic
//! Strike or Cleave gives its Rage and spends a charge once a cast, however many targets it hits,
//! and a grant that comes after the last charge went gives nothing: a combat log of a level 60
//! Fury warrior in Upper Blackrock Spire on 16 November 2026 (client 1.60.1.70938) shows 19
//! Cleaves on one or two targets and 3 Heroic Strikes spending one charge each, 106 of 106
//! landed white swings one each and Whirlwind, Bloodthirst and Hamstring none, as Flurry's 327
//! of 328 two target Cleaves spend one (flurry.rs).

use crate::classes::warrior::agent::WarriorAgent;

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RecklessSwings {
    pub(crate) aura: AuraRef,
    pub(crate) trigger: AuraRef,
    pub(crate) rage: f64,
    pub(crate) charges: i32,
    pub(crate) health_cost: f64,
    /// Go `NewRageMetrics(actionID)`, the proc's.
    pub(crate) metrics: usize,
    /// Forever's rules since push 114115: a Heroic Strike or Cleave grants once a cast, and a
    /// grant with no charge left gives nothing.
    pub(crate) strikes_spend_once: bool,
}

/// The cast: the health goes, the aura comes up at full charges.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: RecklessSwings) {
    fight.remove_health(params.health_cost);
    fight.activate_aura(params.aura);
    fight.set_stacks(params.aura, params.charges);
}

/// The trigger's OnSpellHitDealt: a white swing that landed and dealt damage while the aura is up.
pub(crate) fn on_hit(
    fight: &mut Fight<WarriorAgent>,
    params: RecklessSwings,
    spell: SpellId,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    let strike = matches!(
        state.class_spell.as_deref(),
        Some("heroic_strike" | "cleave")
    );
    if state.flags.proc
        || !state.white_hit
        || (state.melee_special && !strike)
        || !result.landed()
        || result.damage == 0.0
        || !fight.aura(params.aura).active
    {
        return;
    }
    if params.strikes_spend_once && strike {
        // One grant a cast: a Cleave deals each target's hit at the same moment.
        let cast = Some((spell, fight.now));
        if fight.agent.reckless_strike_spent == cast {
            return;
        }
        fight.agent.reckless_strike_spent = cast;
    }
    fight.schedule_delayed_proc(params.trigger, spell, *result);
}

/// The delayed handler: the Rage, and one charge.
pub(crate) fn grant<A: Agent>(fight: &mut Fight<A>, params: RecklessSwings) {
    if params.strikes_spend_once && !fight.aura(params.aura).active {
        // Two hits of one moment found the last charge: the second gives nothing.
        return;
    }
    fight.add_rage(params.rage, params.metrics);
    fight.remove_stack(params.aura);
}
