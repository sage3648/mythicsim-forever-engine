//! Improved Stormstrike (talent 1223031), from Go sim/shaman/talents_enhancement.go
//! `applyImprovedStormstrike`: a completed Stormstrike may grant, one spell batch window later,
//! a buff that raises casting spirit regeneration. Go changes the pseudo stat without
//! refreshing the cached regeneration rates, so the buff only acts through a later refresh;
//! under Forever's rule it acts at once (see [`CastingRegen`]).
//! Its reset listens to the hits the shaman takes: a dodge or parry of a target's swing rolls the
//! talent's chance to finish Stormstrike's cooldown (Go `Stormstrike.CD.Reset`).

use crate::core::{
    fight::{
        casting_regen::CastingRegen, Agent, AuraRef, Fight, SpellId, SpellResult, OUTCOME_DODGE,
        OUTCOME_PARRY,
    },
    time::STARTING_CD_TIME,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImprovedStormstrike {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    /// The reset's trigger, a hit taken listener.
    reset: AuraRef,
    /// Stormstrike's cast, whose cooldown the reset finishes.
    stormstrike: Option<SpellId>,
    proc_chance: f64,
    regen: CastingRegen,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    aura: &str,
    reset: &str,
    proc_chance: f64,
    regen_rate: f64,
) -> Result<ImprovedStormstrike, String> {
    let aura = fight.player_aura(aura)?;
    let stormstrike = (0..fight.spells.len())
        .find(|&spell| fight.spells[spell].class_spell.as_deref() == Some("stormstrike_cast"));
    Ok(ImprovedStormstrike {
        trigger: fight.player_aura(trigger)?,
        aura,
        reset: fight.player_aura(reset)?,
        stormstrike,
        proc_chance,
        regen: CastingRegen::bind(fight, aura, regen_rate)?,
    })
}

impl ImprovedStormstrike {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        self.regen.changed(fight, true);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        self.regen.changed(fight, false);
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

    /// The reset's OnSpellHitTaken for a target's swing: a dodge or parry rolls the chance and
    /// finishes Stormstrike's cooldown.
    pub(crate) fn on_enemy_hit_taken<A: Agent>(&self, fight: &mut Fight<A>, result: &SpellResult) {
        if result.outcome & (OUTCOME_DODGE | OUTCOME_PARRY) == 0 {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.reset) > self.proc_chance {
            return;
        }
        if let Some((timer, _)) = self.stormstrike.and_then(|spell| fight.spells[spell].cd) {
            fight.timers[timer] = STARTING_CD_TIME;
        }
    }

    /// The delayed handler.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }
}
