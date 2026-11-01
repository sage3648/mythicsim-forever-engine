//! Oil of Immolation (MythicSim Rust only, client 70707 item 8956): the character uses the oil at
//! the pull and again each time its Fire Shield ends, and every period of the shield casts the
//! trigger spell, a flat Fire hit on each target.
//!
//! The use has no global cooldown and shares no cooldown with the potions, so the cycle is a
//! chain of actions that needs nothing of the rotation. The shield's last tick falls on the
//! moment it ends, before the next use. The fight models one distance between the player and the
//! targets, so the shield reaches all of them or none: a character beyond the radius gets no
//! tick.

use crate::contracts::prepared_v2::Effect;

use super::{Action, Agent, AuraRef, Fight, Side, SpellId, PRIORITY_AUTO, PRIORITY_PREPULL};

/// The bound oil: its aura, the tick spell and the shield's shape.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OilOfImmolation {
    aura: AuraRef,
    spell: SpellId,
    period: i64,
    ticks: i32,
    radius: f64,
}

impl<A: Agent> Fight<A> {
    /// Bind the oil's aura and its tick spell.
    pub(crate) fn bind_oil_of_immolation(&mut self, effects: &[Effect]) -> Result<(), String> {
        for effect in effects {
            if let Effect::OilOfImmolation {
                aura,
                spell,
                period_ns,
                ticks,
                radius,
                ..
            } = effect
            {
                self.oil = Some(OilOfImmolation {
                    aura: self.player_aura(aura)?,
                    spell: *spell,
                    period: *period_ns,
                    ticks: *ticks,
                    radius: *radius,
                });
            }
        }
        Ok(())
    }

    /// The first use, at the pull: the shield is up from 0 s.
    pub(crate) fn oil_of_immolation_reset(&mut self) {
        if self.oil.is_some() {
            self.schedule(0, PRIORITY_PREPULL, Action::OilUse);
        }
    }

    /// The character uses the oil: the shield goes up, or is refreshed, and its first period
    /// starts.
    pub(crate) fn oil_use(&mut self) {
        let Some(oil) = self.oil else { return };
        self.activate_aura(oil.aura);
        self.schedule(self.now + oil.period, PRIORITY_AUTO, Action::OilTick(1));
    }

    /// One period of the shield: the trigger spell on every target while the player stands within
    /// the radius, then the next period or, after the last, the next use.
    pub(crate) fn oil_tick(&mut self, tick: i32) {
        let Some(oil) = self.oil else { return };
        if self.distance_now(Side::Player) <= oil.radius {
            self.cast(oil.spell, Side::Player);
        }
        if tick < oil.ticks {
            self.schedule(
                self.now + oil.period,
                PRIORITY_AUTO,
                Action::OilTick(tick + 1),
            );
        } else {
            self.oil_use();
        }
    }
}
