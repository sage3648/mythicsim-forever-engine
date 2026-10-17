//! Winter's Chill (talent 11180, aura 12579), from Go sim/mage/talents_frost.go
//! `registerWinterChill`: a landed Frost spell can add a stack, and each stack makes the mage's
//! Cone of Cold deal more damage (client 70425).

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct WintersChill {
    pub(crate) aura: AuraRef,
    pub(crate) trigger: AuraRef,
    proc_chance: f64,
    damage_per_stack: f64,
    damage_mod: ModId,
}

/// Bind the exported auras and register Go's dynamic damage modifier.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    trigger: &str,
    proc_chance: f64,
    damage_per_stack: f64,
) -> Result<WintersChill, String> {
    let find = |label: &str| {
        fight.trackers[Side::Player.index()]
            .find(label)
            .map(|index| AuraRef {
                side: Side::Player,
                index,
            })
            .ok_or_else(|| format!("Winter's Chill aura {label} is not registered"))
    };
    let (aura, trigger) = (find(aura)?, find(trigger)?);
    let affected = fight.spells_with_class(&["cone_of_cold"]);
    let damage_mod = fight.register_mod(ModKind::DamageDoneFlat, 0.0, 0, affected);
    Ok(WintersChill {
        aura,
        trigger,
        proc_chance,
        damage_per_stack,
        damage_mod,
    })
}

impl WintersChill {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(self.damage_mod, self.damage_per_stack * f64::from(stacks));
    }

    /// The "Winters Chill Talent" proc trigger: spell damage, landed, Frost school.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if state.flags.proc
            || !state.proc_spell_damage
            || !result.landed()
            || state.school & 16 == 0
        {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        fight.activate_aura(self.aura);
        fight.add_stack(self.aura);
    }
}
