//! Elemental Mastery (16166), from Go sim/shaman/talents_elemental.go `applyElementalMastery`: a
//! major cooldown of client 70425 whose 15 sec aura makes Lightning Bolt and Lava Burst cast
//! 0.5 sec faster and cancels Chain Lightning's cooldown with a flat mod of its own length.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ElementalMastery {
    pub(crate) aura: AuraRef,
    cast_mod: ModId,
    cooldown_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    cast_time_delta: i64,
    chain_lightning_cooldown: i64,
) -> Result<ElementalMastery, String> {
    let aura = fight.player_aura(aura)?;
    // Go AddDynamicMod order: the cast time, then the cooldown.
    let casts = fight.spells_with_class(&["lightning_bolt", "lava_burst"]);
    let cast_mod = fight.register_mod(ModKind::CastTimeFlat, 0.0, cast_time_delta, casts);
    let chains = fight.spells_with_class(&["chain_lightning"]);
    let cooldown_mod = fight.register_mod(
        ModKind::CooldownFlat,
        0.0,
        -chain_lightning_cooldown,
        chains,
    );
    Ok(ElementalMastery {
        aura,
        cast_mod,
        cooldown_mod,
    })
}

impl ElementalMastery {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cast_mod);
        fight.activate_mod(self.cooldown_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_mod);
        fight.deactivate_mod(self.cooldown_mod);
    }
}
