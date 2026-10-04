//! Go common/classic/items_weapons.go Sulfuras, Hand of Ragnaros: a weapon proc at one proc a
//! minute of the weapon's speed that casts its Fireball on the unit hit at once, a magic hit
//! whose landing applies a burn of a fixed amount every two seconds; and, while equipped,
//! Immolation, which hits every melee attacker for a fixed amount of Fire, never missing.

use crate::contracts::prepared_v2::Effect;

use super::{Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_LANDED};

/// The bound proc and its spells.
#[derive(Clone, Debug)]
pub(crate) struct Sulfuras {
    /// The proc chance of each spell, from the weapon's proc manager.
    pub(crate) chances: Vec<Option<f64>>,
    pub(crate) fireball: SpellId,
    pub(crate) immolation: SpellId,
}

impl<A: Agent> Fight<A> {
    /// Bind Sulfuras: the Fireball's burn takes its fixed base and ticks without crits, as Go's
    /// `OnSnapshot` and `OutcomeTick` give it.
    pub(crate) fn bind_sulfuras(&mut self, effect: &Effect) -> Result<(), String> {
        let Effect::SulfurasHandOfRagnaros {
            chances,
            fireball_spell,
            dot_base,
            immolation_spell,
            ..
        } = effect
        else {
            return Ok(());
        };
        if *fireball_spell >= self.spells.len() || *immolation_spell >= self.spells.len() {
            return Err("Sulfuras names spells the player does not have".into());
        }
        let dot = self.spells[*fireball_spell]
            .dot
            .ok_or("Sulfuras's Fireball has no burn")?;
        self.dots[dot].tick_base = Some(*dot_base);
        self.dots[dot].tick_can_crit = false;
        let mut by_spell = vec![None; self.spells.len()];
        for entry in chances {
            if let Some(slot) = by_spell.get_mut(entry.spell) {
                *slot = Some(entry.chance);
            }
        }
        self.sulfuras = Some(Sulfuras {
            chances: by_spell,
            fireball: *fireball_spell,
            immolation: *immolation_spell,
        });
        Ok(())
    }

    /// Go `CreateWeaponProcTrigger` for Sulfuras: landed hits roll the spell's chance, then the
    /// Fireball casts at once on the unit hit.
    pub(crate) fn sulfuras_callback(
        &mut self,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        let sulfuras = self.sulfuras.as_ref().expect("Sulfuras is bound");
        let (Some(chance), fireball) = (sulfuras.chances[spell], sulfuras.fireball) else {
            return;
        };
        if !self.proc_for_aura(chance, aura) {
            return;
        }
        self.cast(fireball, result.target);
    }

    /// The Fireball's `ApplyEffects`: the rolled magic hit, then the burn when it lands.
    pub(crate) fn sulfuras_fireball(&mut self, spell: SpellId, target: Side, min: f64, max: f64) {
        // Go sim.Roll: min + (max - min) * RandomFloat("Damage Roll").
        let base = min + (max - min) * self.random("Damage Roll");
        let result = self.calc_damage(spell, target, base);
        self.deal_damage(spell, result, false);
        if result.landed() {
            let dot = self.spells[spell].dot.expect("bound with a burn");
            self.apply_dot(dot);
        }
    }

    /// Immolation's trigger: a melee hit the target lands on the wearer casts the hit back on
    /// it a spell batch window later, as the trigger does not fire immediately.
    pub(crate) fn sulfuras_immolation(&mut self, aura: AuraRef, result: &SpellResult) {
        if result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        let immolation = self
            .sulfuras
            .as_ref()
            .expect("Sulfuras is bound")
            .immolation;
        self.schedule_delayed_proc(aura, immolation, *result);
    }

    /// Immolation's delayed handler: the hit on the attacker, the target, the only unit that
    /// swings at the wearer in scope.
    pub(crate) fn sulfuras_immolation_hit(&mut self) {
        let immolation = self
            .sulfuras
            .as_ref()
            .expect("Sulfuras is bound")
            .immolation;
        self.cast(immolation, Side::Target);
    }
}
