//! The warlock's demons, from Go sim/warlock/pets.go: the summoned demon's ability loop and
//! the Succubus's Lash of Pain. The demon itself, its stats, swings and mana, is the runtime's
//! simulated pet.

use crate::core::{
    fight::{Agent, Fight, Side, SpellId},
    time::NS_PER_SECOND,
};

/// Go `WarlockPet.ExecuteCustomRotation`'s inputs.
#[derive(Clone, Debug)]
pub(crate) struct DemonAi {
    autocast: Vec<SpellId>,
    min_mana: f64,
    wait: i64,
}

/// Resolve the demon's abilities, given as positions in its spellbook.
pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    autocast: &[usize],
    min_mana: f64,
    wait_ns: i64,
) -> Result<DemonAi, String> {
    let pet_spells: Vec<SpellId> = (0..fight.spells.len())
        .filter(|&spell| fight.spells[spell].caster == Side::Pet)
        .collect();
    let autocast = autocast
        .iter()
        .map(|&position| {
            pet_spells
                .get(position)
                .copied()
                .ok_or_else(|| format!("the demon has no spell at {position}"))
        })
        .collect::<Result<_, String>>()?;
    Ok(DemonAi {
        autocast,
        min_mana,
        wait: wait_ns,
    })
}

impl DemonAi {
    /// Go `ExecuteCustomRotation`: cast the first ability that can be cast while mana stays
    /// above `MinMana`, otherwise wait for mana, at least the fixed delay.
    pub(crate) fn rotation<A: Agent>(&self, fight: &mut Fight<A>) {
        let mut wait_until = 0i64;
        for &spell in &self.autocast {
            if fight.can_cast(spell) && fight.unit(Side::Pet).mana > self.min_mana {
                fight.cast(spell, Side::Target);
                return;
            }
            let cost = self.min_mana.max(fight.current_cost(spell));
            let regen = fight
                .unit_config(Side::Pet)
                .fixed_regen
                .map_or(0.0, |(casting, _)| casting);
            if regen > 0.0 {
                let time_till_mana = ((cost - fight.unit(Side::Pet).mana) / regen).max(0.0);
                wait_until = wait_until.min((NS_PER_SECOND as f64 * time_till_mana) as i64);
            }
        }
        let ready = fight.now + wait_until + self.wait;
        fight.wait_until_of(Side::Pet, ready);
    }
}

/// Go `registerLashOfPainSpell`'s `ApplyEffects`: a fixed base with the spell power share on
/// the magic hit and crit table, dealt at once.
pub(crate) fn lash_of_pain<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    base: f64,
) {
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}
