//! Power in Light (1309969), from Go sim/priest/talents_discipline.go `applyPowerInLight`: the
//! target's dynamic damage taken modifier multiplies this priest's Smite and Penance damage
//! after the outcome while any of its Holy Fire dots burns the target.

use crate::core::fight::{Agent, Fight, SpellDamageTakenModifier};

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    multiplier: f64,
    spells: &[usize],
    holy_fire_spells: &[usize],
) -> Result<(), String> {
    let len = fight.spells.len();
    let mut mask = vec![false; len];
    for &spell in spells {
        *mask
            .get_mut(spell)
            .ok_or("Power in Light names a spell outside the spellbook")? = true;
    }
    let mut auras = Vec::new();
    for &spell in holy_fire_spells {
        let dot = fight
            .spells
            .get(spell)
            .and_then(|state| state.dot)
            .ok_or("Power in Light names a Holy Fire without a dot")?;
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
