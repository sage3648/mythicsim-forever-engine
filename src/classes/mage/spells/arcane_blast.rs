//! Arcane Blast (1239700) and its Arcane Blast buff (400573), from Go sim/mage/arcane_blast.go
//! and arcane_charge.go. Each cast stacks the buff up to four times; every stack raises the
//! damage of the mage's other damaging spells and the cost of Arcane Blast. The next other
//! damaging cast spends the stacks after its damage is rolled; Arcane Missiles holds them
//! until its channel ends.

use crate::{
    classes::mage::masks::{damaging_except, is_class},
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId},
};

/// The stacks' damage mask: every damaging spell but these, whatever the tooltip says.
const UNBUFFED: &[&str] = &[
    "arcane_blast",
    "arcane_missiles_tick",
    "blizzard",
    "flamestrike",
];
/// Casts that keep the stacks: Arcane Blast and Arcane Missiles.
const KEEP: &[&str] = &["arcane_blast", "arcane_missiles_tick"];

#[derive(Clone, Debug)]
pub(crate) struct ArcaneCharges {
    pub(crate) aura: AuraRef,
    damage_per_stack: f64,
    cost_per_stack: f64,
    damage_mod: ModId,
    cost_mod: ModId,
    spenders: Vec<&'static str>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    damage_per_stack: f64,
    cost_per_stack: f64,
) -> Result<ArcaneCharges, String> {
    let aura = fight.player_aura(aura)?;
    // Go AddDynamicMod order: damage, then cost.
    let buffed = fight.spells_with_class(&damaging_except(UNBUFFED));
    let damage_mod = fight.register_mod(ModKind::DamageDoneFlat, 0.0, 0, buffed);
    let blasts = fight.spells_with_class(&["arcane_blast"]);
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, 0.0, 0, blasts);
    Ok(ArcaneCharges {
        aura,
        damage_per_stack,
        cost_per_stack,
        damage_mod,
        cost_mod,
        spenders: damaging_except(KEEP),
    })
}

/// Go Arcane Blast `ApplyEffects`: the hit lands at once, then a stack is added.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    charges: &ArcaneCharges,
) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
    fight.activate_aura(charges.aura);
    fight.add_stack(charges.aura);
}

impl ArcaneCharges {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage_mod);
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage_mod);
        fight.deactivate_mod(self.cost_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(self.damage_mod, self.damage_per_stack * f64::from(stacks));
        fight.update_mod_value(self.cost_mod, self.cost_per_stack * f64::from(stacks));
    }

    /// The buff's OnCastComplete, after the spending cast's damage is rolled.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if is_class(fight.spells[spell].class_spell.as_deref(), &self.spenders) {
            fight.deactivate_aura(self.aura);
        }
    }

    /// The Arcane Missiles channel aura's OnExpire: the held stacks go.
    pub(crate) fn on_channel_end<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_aura(self.aura);
    }
}
