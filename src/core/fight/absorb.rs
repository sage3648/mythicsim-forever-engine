//! Go common/shared/shared_utils.go `NewSpellDataAbsorbOnUse` and core aura_helpers.go
//! `NewDamageAbsorptionAura`: an item use whose aura shields the wearer, for the amount its
//! absorb effect rolls, against the damage of the schools the effect masks. The shield is one of
//! the player's dynamic damage taken modifiers: while up it takes what it can of a hit that deals
//! damage, its stacks follow the strength left, and it fades when spent. A second use replaces
//! the strength left.

use super::{Agent, AuraRef, Fight, SpellResult};

/// One item's shield.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ItemAbsorb {
    pub(crate) aura: AuraRef,
    /// Go `SpellSchool` bits of the hits it takes.
    pub(crate) schools: u8,
    pub(crate) average: f64,
    pub(crate) variance: f64,
    strength: f64,
}

impl ItemAbsorb {
    pub(crate) fn new(aura: AuraRef, schools: u8, average: f64, variance: f64) -> Self {
        ItemAbsorb {
            aura,
            schools,
            average,
            variance,
            strength: 0.0,
        }
    }
}

impl<A: Agent> Fight<A> {
    /// The use's `ApplyEffects`: the roll, then `DamageAbsorptionAura.Activate`, which sets the
    /// strength and stacks it as whole points.
    pub(crate) fn apply_item_absorb(&mut self, index: usize) {
        let shield = self.item_absorbs[index];
        let amount = self.effect_roll(shield.average, shield.variance);
        self.activate_aura(shield.aura);
        self.item_absorbs[index].strength = amount;
        let stacks = (amount as i32).max(1);
        self.aura_mut(shield.aura).max_stacks = stacks;
        self.set_stacks(shield.aura, stacks);
    }

    /// The shields' damage taken modifiers on a hit of the school bits, in registration order.
    pub(crate) fn item_absorbs_taken(&mut self, school: u8, result: &mut SpellResult) {
        for index in 0..self.item_absorbs.len() {
            let shield = self.item_absorbs[index];
            if !self.aura(shield.aura).active
                || result.damage <= 0.0
                || school & shield.schools == 0
            {
                continue;
            }
            let absorbed = shield.strength.min(result.damage);
            result.damage -= absorbed;
            let strength = shield.strength - absorbed;
            self.item_absorbs[index].strength = strength;
            if self.log.is_some() {
                let line = format!(
                    "{} absorbed {absorbed:.1} damage, new shield strength: {strength:.1}",
                    self.aura(shield.aura).label
                );
                self.player_log(&line);
            }
            if strength <= 0.0 {
                self.deactivate_aura(shield.aura);
                continue;
            }
            self.set_stacks(shield.aura, strength as i32);
        }
    }
}
