//! Missile Barrage (44404), from Go sim/mage/talents_arcane.go `registerMissileBarrage`.
//! Casting Frostbolt, Fireball or Frostfire Bolt (20%) or Arcane Blast (40%) can make the
//! next Arcane Missiles free and fire its missiles twice as often. The chances, duration
//! and modifiers are Go literals because the client tables lack the rows.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct MissileBarrage {
    pub(crate) aura: AuraRef,
    arcane_blast_chance: f64,
    bolt_chance: f64,
    label: String,
    cost_mod: ModId,
    tick_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    arcane_blast_chance: f64,
    bolt_chance: f64,
    label: &str,
    cost_percent_add: f64,
    tick_length_delta: i64,
) -> Result<MissileBarrage, String> {
    let aura = fight.player_aura(aura)?;
    let channels = fight.spells_with_class(&["arcane_missiles_cast"]);
    // Go AttachSpellMod order: the cost modifier, then the tick length modifier.
    let cost_mod = fight.register_mod(
        ModKind::PowerCostPercentAdd,
        cost_percent_add,
        0,
        channels.clone(),
    );
    let tick_mod = fight.register_mod(ModKind::DotTickLengthFlat, 0.0, tick_length_delta, channels);
    Ok(MissileBarrage {
        aura,
        arcane_blast_chance,
        bolt_chance,
        label: label.to_string(),
        cost_mod,
        tick_mod,
    })
}

impl MissileBarrage {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
        fight.activate_mod(self.tick_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cost_mod);
        fight.deactivate_mod(self.tick_mod);
    }

    /// The Missile Barrage aura's OnCastComplete: Arcane Missiles consumes it.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if fight.spells[spell].class_spell.as_deref() == Some("arcane_missiles_cast") {
            fight.deactivate_aura(self.aura);
        }
    }

    /// The permanent trigger aura's OnCastComplete.
    pub(crate) fn trigger<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let chance = match fight.spells[spell].class_spell.as_deref() {
            Some("arcane_blast") => self.arcane_blast_chance,
            Some("fireball" | "frostbolt" | "frostfire_bolt") => self.bolt_chance,
            _ => return,
        };
        if fight.proc(chance, &self.label) {
            fight.activate_aura(self.aura);
        }
    }
}
