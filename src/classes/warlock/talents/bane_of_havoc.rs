//! Bane of Havoc (1225228), from Go sim/warlock/talents_destruction.go `applyBaneOfHavoc`: a
//! bane on one target at a time that copies a share of the warlock's damage to every other
//! target onto the baned one. The copy is the share of the damage already dealt, so nothing
//! on either side modifies it again, and only the warlock's own damage copies, not the
//! demon's.

use crate::{
    classes::warlock::{agent::WarlockAgent, spells::find_spell},
    core::fight::{AuraRef, Fight, Outcome, Side, SpellId, SpellResult},
};

use super::super::spells::take_bane_slot;

/// Bane of Havoc's bound aura, copy spell and share.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BaneOfHavoc {
    /// The bane's aura on the first target; each target past it holds a copy.
    aura: AuraRef,
    /// The spell of the same ID with tag 1 that deals the copies.
    copy: SpellId,
    share: f64,
}

pub(crate) fn bind(
    fight: &Fight<WarlockAgent>,
    spell_id: i32,
    aura: &str,
    share: f64,
) -> Result<BaneOfHavoc, String> {
    let index = fight.trackers[Side::Target.index()]
        .find(aura)
        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
    find_spell(fight, spell_id)?;
    let copy = fight
        .spells
        .iter()
        .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == 1)
        .ok_or_else(|| format!("Bane of Havoc's copy spell {spell_id} is not registered"))?;
    Ok(BaneOfHavoc {
        aura: AuraRef {
            side: Side::Target,
            index,
        },
        copy,
        share,
    })
}

fn state(fight: &Fight<WarlockAgent>) -> BaneOfHavoc {
    fight.agent.bane_of_havoc.expect("Bane of Havoc is bound")
}

/// `ApplyEffects`: a landed hit moves the bane to the target, fading it from the target that
/// held it, and takes the bane slot.
pub(crate) fn apply(fight: &mut Fight<WarlockAgent>, spell: SpellId, target: Side) {
    let havoc = state(fight);
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    if result.landed() {
        if let Some(held) = fight.agent.havoc_target {
            if held != target {
                let aura = fight.aura_on(havoc.aura, held);
                fight.deactivate_aura(aura);
            }
        }
        let aura = fight.aura_on(havoc.aura, target);
        take_bane_slot(fight, aura);
        fight.activate_aura(aura);
    }
    fight.deal_damage(spell, result, false);
}

/// The bane's `OnGain`: this target holds it.
pub(crate) fn on_gain(fight: &mut Fight<WarlockAgent>, aura: AuraRef) {
    fight.agent.havoc_target = Some(aura.side);
}

/// The bane's `OnExpire`: nobody holds it when the target that did loses it.
pub(crate) fn on_expire(fight: &mut Fight<WarlockAgent>, aura: AuraRef) {
    if fight.agent.havoc_target == Some(aura.side) {
        fight.agent.havoc_target = None;
    }
}

/// The copy aura's `OnSpellHitDealt` and `OnPeriodicDamageDealt`: damage to a target other
/// than the baned one is dealt again to the baned one for the share, which always hits.
pub(crate) fn copy_damage(fight: &mut Fight<WarlockAgent>, result: &SpellResult) {
    let Some(held) = fight.agent.havoc_target else {
        return;
    };
    if result.target == held || result.damage <= 0.0 {
        return;
    }
    let havoc = state(fight);
    let copied = fight.calc_damage_with(
        havoc.copy,
        held,
        result.damage * havoc.share,
        Outcome::AlwaysHit,
    );
    fight.deal_damage(havoc.copy, copied, false);
}
