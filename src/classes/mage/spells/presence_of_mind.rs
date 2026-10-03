//! Presence of Mind (12043), from Go sim/mage/presence_of_mind.go: a major cooldown that
//! makes the next Mage spell with a cast time instant. The buff never expires on its
//! own; the spell that consumes it restarts the cooldown from that moment.

use crate::{
    classes::mage::masks::{except, is_class, ALL, INSTANT_CAST},
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId},
};

#[derive(Clone, Debug)]
pub(crate) struct PresenceOfMind {
    pub(crate) aura: AuraRef,
    spell: SpellId,
    cast_time_mod: ModId,
    has_cast_time: Vec<&'static str>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    aura: &str,
    cast_time_percent: f64,
) -> Result<PresenceOfMind, String> {
    let aura = fight.player_aura(aura)?;
    let mut excluded = INSTANT_CAST.to_vec();
    excluded.extend(["blizzard", "evocation"]);
    let has_cast_time = except(ALL, &excluded);
    let affected = fight.spells_with_class(&has_cast_time);
    let cast_time_mod =
        fight.register_mod(ModKind::CastTimePercent, cast_time_percent, 0, affected);
    Ok(PresenceOfMind {
        aura,
        spell,
        cast_time_mod,
        has_cast_time,
    })
}

impl PresenceOfMind {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cast_time_mod);
    }

    /// Go `pomSpell.CD.Use`: the cooldown restarts, without the cooldown multiplier.
    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_time_mod);
        if let Some((timer, duration)) = fight.spells[self.spell].cd {
            fight.timers[timer] = fight.now + duration;
        }
    }

    /// The buff's OnCastComplete: a spell with a default cast time consumes it.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let state = &fight.spells[spell];
        if is_class(state.class_spell.as_deref(), &self.has_cast_time)
            && state.default_cast.cast_time > 0
        {
            fight.deactivate_aura(self.aura);
        }
    }
}
