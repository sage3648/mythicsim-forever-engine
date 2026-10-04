//! Rend and Tear (1223246), from Go sim/druid/talents_feral_combat.go `applyRendAndTear`: the
//! target's dynamic modifier on the druid's special attacks while one of its bleeds is up.

use crate::core::fight::{Agent, Fight, SpellDamageTakenModifier};

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    multiplier: f64,
    spells: &[usize],
    bleed_spells: &[usize],
) -> Result<(), String> {
    let mut mask = vec![false; fight.spells.len()];
    for &spell in spells {
        if let Some(slot) = mask.get_mut(spell) {
            *slot = true;
        }
    }
    let mut auras = Vec::new();
    for &spell in bleed_spells {
        let dot = fight
            .spells
            .get(spell)
            .and_then(|state| state.dot)
            .ok_or_else(|| format!("Rend and Tear's bleed {spell} has no dot"))?;
        auras.push(fight.dots[dot].aura);
    }
    fight
        .spell_damage_taken_modifiers
        .push(SpellDamageTakenModifier {
            spells: mask,
            auras,
            multiplier,
        });
    Ok(())
}
