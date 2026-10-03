//! Arcane Power (12042), from Go sim/mage/arcane_power.go: a major cooldown whose aura
//! raises the damage and mana cost of every Mage spell. Frostfire Bolt's damage bonus
//! applies to its hit only, through the direct damage modifier.

use crate::{
    classes::mage::masks::{except, ALL},
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind},
};

#[derive(Clone, Debug)]
pub(crate) struct ArcanePower {
    pub(crate) aura: AuraRef,
    mods: [ModId; 3],
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    damage: f64,
    cost_percent_add: f64,
) -> Result<ArcanePower, String> {
    let aura = fight.player_aura(aura)?;
    // Go AttachSpellMod order.
    let others = fight.spells_with_class(&except(ALL, &["frostfire_bolt"]));
    let damage_mod = fight.register_mod(ModKind::DamageDoneFlat, damage, 0, others);
    let frostfire = fight.spells_with_class(&["frostfire_bolt"]);
    let direct_mod = fight.register_mod(ModKind::DirectDamageDoneFlat, damage, 0, frostfire);
    let all = fight.spells_with_class(ALL);
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, cost_percent_add, 0, all);
    Ok(ArcanePower {
        aura,
        mods: [damage_mod, direct_mod, cost_mod],
    })
}

impl ArcanePower {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        for modifier in self.mods {
            fight.activate_mod(modifier);
        }
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        for modifier in self.mods {
            fight.deactivate_mod(modifier);
        }
    }
}
