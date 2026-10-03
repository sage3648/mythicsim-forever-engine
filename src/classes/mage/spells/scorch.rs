//! Scorch (every rank, 2948 to 10207) with Improved Scorch (11095), from Go
//! sim/mage/scorch.go. A landed Scorch can stack Fire Vulnerability (22959), which in
//! Forever raises only the Fire damage of the mage who applied it, so it is a buff on
//! the mage rather than a debuff on the target.

use crate::{
    classes::mage::masks::ALL,
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId},
};

/// Go `SpellSchoolFire`.
const FIRE: u8 = 4;

#[derive(Clone, Debug)]
pub(crate) struct ImprovedScorch {
    pub(crate) aura: AuraRef,
    proc_chance: f64,
    damage_per_stack: f64,
    damage_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    proc_chance: f64,
    damage_per_stack: f64,
) -> Result<ImprovedScorch, String> {
    let aura = fight.player_aura(aura)?;
    let fire = fight.spells_with_class_and_school(ALL, FIRE);
    let damage_mod = fight.register_mod(ModKind::DamageDonePercent, 0.0, 0, fire);
    Ok(ImprovedScorch {
        aura,
        proc_chance,
        damage_per_stack,
        damage_mod,
    })
}

/// Go Scorch `ApplyEffects`: the hit lands at once; without the talent nothing is rolled.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    improved: Option<&ImprovedScorch>,
) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
    if let Some(improved) = improved {
        if result.landed() && fight.proc(improved.proc_chance, "Improved Scorch") {
            fight.activate_aura(improved.aura);
            fight.add_stack(improved.aura);
        }
    }
}

impl ImprovedScorch {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(self.damage_mod, self.damage_per_stack * f64::from(stacks));
    }
}
