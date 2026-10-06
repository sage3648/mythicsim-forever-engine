//! Improved Stormstrike (talent 1223031), from Go sim/shaman/talents_enhancement.go
//! `applyImprovedStormstrike`: a completed Stormstrike may grant, one spell batch window later,
//! a buff that raises casting spirit regeneration. Go changes the pseudo stat without
//! refreshing the cached regeneration rates, so the buff only acts through a later refresh.
//! Its cooldown reset listens to hits the player takes, which never happen in scope.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImprovedStormstrike {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    proc_chance: f64,
    regen_rate: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    trigger: &str,
    aura: &str,
    proc_chance: f64,
    regen_rate: f64,
) -> Result<ImprovedStormstrike, String> {
    Ok(ImprovedStormstrike {
        trigger: fight.player_aura(trigger)?,
        aura: fight.player_aura(aura)?,
        proc_chance,
        regen_rate,
    })
}

impl ImprovedStormstrike {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spirit_regen_rate_casting += self.regen_rate;
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spirit_regen_rate_casting -= self.regen_rate;
    }

    /// The trigger's OnCastComplete for a Stormstrike cast.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        let result = SpellResult {
            armor_multiplier: 0.0,
            target: crate::core::fight::Side::Target,
            attacker: fight.spells[spell].caster,
            outcome: 0,
            damage: 0.0,
            threat: 0.0,
        };
        fight.schedule_delayed_proc(self.trigger, spell, result);
    }

    /// The delayed handler.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }
}
