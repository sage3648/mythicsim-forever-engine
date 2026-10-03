//! Decimation (440870), from Go sim/warlock/talents_demonology.go `applyDecimation`: a landed
//! Shadow Bolt or Searing Pain inside the 35% execute phase grants Decimation (440873), whose
//! modifiers raise both spells' damage and cut Soul Fire's cast time. Its cooldown cut on Soul
//! Fire is a static modifier already on the exported spell.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct Decimation {
    aura: AuraRef,
    execute_phase: i32,
    trigger_spells: Vec<SpellId>,
    damage: ModId,
    cast_time: ModId,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    execute_phase: i32,
    trigger_spells: &[usize],
    damage_spells: &[usize],
    damage_done_flat: f64,
    cast_spells: &[usize],
    cast_time_percent: f64,
) -> Result<Decimation, String> {
    if execute_phase != 35 {
        return Err(format!(
            "Decimation's execute phase {execute_phase} is unsupported"
        ));
    }
    let aura = fight.player_aura(aura)?;
    let damage = fight.register_mod(
        ModKind::DamageDoneFlat,
        damage_done_flat,
        0,
        damage_spells.to_vec(),
    );
    let cast_time = fight.register_mod(
        ModKind::CastTimePercent,
        cast_time_percent,
        0,
        cast_spells.to_vec(),
    );
    Ok(Decimation {
        aura,
        execute_phase,
        trigger_spells: trigger_spells.to_vec(),
        damage,
        cast_time,
    })
}

impl Decimation {
    /// The trigger's OnSpellHitDealt: Go `AttachProcTriggerCallback` with `OutcomeLanded`, the
    /// execute phase as its extra condition and `TriggerImmediately`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells.contains(&spell) || !result.landed() {
            return;
        }
        debug_assert_eq!(self.execute_phase, 35);
        if !fight.is_execute_phase_35() {
            return;
        }
        fight.activate_aura(self.aura);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage);
        fight.activate_mod(self.cast_time);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage);
        fight.deactivate_mod(self.cast_time);
    }
}
