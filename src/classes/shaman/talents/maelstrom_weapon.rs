//! Maelstrom Weapon (talent 408498), from Go sim/shaman/talents_enhancement.go
//! `applyMaelstromWeapon`: landed melee hits stack a buff, one spell batch window later, whose
//! stacks cut Lightning Bolt's cast time and cost; a completed Lightning Bolt spends it.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct MaelstromWeapon {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    per_stack: f64,
    cast_mod: ModId,
    cost_mod: ModId,
}

/// The trigger's chance for each spellbook position, absent for a spell it never hears.
pub(crate) type Chances = Vec<Option<f64>>;

/// `bolts` are the Lightning Bolt ranks the two modifiers name.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    aura: &str,
    per_stack: f64,
    bolts: Vec<SpellId>,
) -> Result<MaelstromWeapon, String> {
    let trigger = fight.player_aura(trigger)?;
    let aura = fight.player_aura(aura)?;
    // Go AddDynamicMod order: cast time, then cost.
    let cast_mod = fight.register_mod(ModKind::CastTimePercent, 0.0, 0, bolts.clone());
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, 0.0, 0, bolts);
    Ok(MaelstromWeapon {
        trigger,
        aura,
        per_stack,
        cast_mod,
        cost_mod,
    })
}

impl MaelstromWeapon {
    /// OnStacksChange: both modifiers take the new stacks' value and stay active.
    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, new: i32) {
        let value = self.per_stack * f64::from(new);
        fight.update_mod_value(self.cast_mod, value);
        fight.update_mod_value(self.cost_mod, value);
        fight.activate_mod(self.cast_mod);
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_mod);
        fight.deactivate_mod(self.cost_mod);
    }

    /// OnCastComplete: a Lightning Bolt spends the buff.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, bolt: bool) {
        if bolt {
            fight.deactivate_aura(self.aura);
        }
    }

    /// The trigger: a landed hit the proc manager hears rolls its chance and schedules a stack.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
        chance: Option<f64>,
    ) {
        let Some(chance) = chance else {
            return;
        };
        if !result.landed() {
            return;
        }
        let label = fight.aura(self.trigger).label.clone();
        if fight.proc(chance, &label) {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The delayed handler: one more stack.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
        fight.add_stack(self.aura);
    }
}
