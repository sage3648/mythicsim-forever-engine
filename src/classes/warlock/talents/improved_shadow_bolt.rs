//! Improved Shadow Bolt (17793), from Go sim/warlock/talents_destruction.go
//! `applyImprovedShadowBolt`: a Shadow Bolt crit activates a debuff (17794) on the target,
//! and while it is up the target's dynamic damage taken modifier multiplies the warlock's
//! shadow damage after the outcome. The debuff has no charges, so hits do not consume it.

use crate::core::fight::{Agent, AuraRef, DamageTakenModifier, Fight, Side, SpellId, SpellResult};

/// Go `SpellSchoolShadow`.
const SHADOW: u8 = 32;

#[derive(Clone, Debug)]
pub(crate) struct ImprovedShadowBolt {
    debuff: AuraRef,
    trigger_spells: Vec<SpellId>,
}

/// Register the target's damage taken modifier and resolve the debuff.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    multiplier: f64,
    trigger_spells: &[usize],
) -> Result<ImprovedShadowBolt, String> {
    let index = fight.trackers[Side::Target.index()]
        .find(aura)
        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
    let debuff = AuraRef {
        side: Side::Target,
        index,
    };
    fight.damage_taken_modifiers.push(DamageTakenModifier {
        school_mask: SHADOW,
        aura: debuff,
        multiplier,
    });
    Ok(ImprovedShadowBolt {
        debuff,
        trigger_spells: trigger_spells.to_vec(),
    })
}

impl ImprovedShadowBolt {
    /// The trigger's OnSpellHitDealt: Go `AttachProcTriggerCallback` with the Shadow Bolt
    /// class mask, `OutcomeCrit`, no proc roll and `TriggerImmediately`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if self.trigger_spells.contains(&spell) && result.crit() {
            fight.activate_aura(self.debuff);
        }
    }
}
