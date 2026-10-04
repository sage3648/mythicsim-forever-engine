//! Quietus (1310728), from Go sim/rogue/talents_subtlety.go `registerQuietus`: an execute
//! phase callback activates its aura once the target reaches 35%, and the aura's attached
//! modifier raises Sinister Strike, Ghostly Strike and Hemorrhage for the rest of the fight.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Quietus {
    pub(crate) aura: AuraRef,
    execute_phase: i32,
    damage_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    execute_phase: i32,
    damage_bonus: f64,
    class_spells: &[String],
) -> Result<Quietus, String> {
    let aura = fight.player_aura(aura)?;
    let names: Vec<&str> = class_spells.iter().map(String::as_str).collect();
    let damage_mod = fight.register_mod(
        ModKind::DamageDoneFlat,
        damage_bonus,
        0,
        fight.spells_with_class(&names),
    );
    Ok(Quietus {
        aura,
        execute_phase,
        damage_mod,
    })
}

impl Quietus {
    /// The execute phase callback.
    pub(crate) fn on_execute_phase<A: Agent>(&self, fight: &mut Fight<A>, phase: i32) {
        if phase == self.execute_phase {
            fight.activate_aura(self.aura);
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage_mod);
    }
}
