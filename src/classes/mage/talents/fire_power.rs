//! Fire Power (talent 11124), from Go sim/mage/talents_fire.go `registerFirePower` as client
//! 70425 redesigned it: 1/2/3% more damage for Fire spells per Fire effect the mage has on the
//! target, up to four, through a damage done by caster handler on every attack table. The
//! effects are the Fire-only dots (Fireball, Flamestrike's area dot, Ignite, Pyroblast), each
//! family counted once whatever rank is up, and Improved Scorch's Fire Vulnerability on the
//! current target. Ignite takes no bonus itself; Frostfire Bolt takes it but does not count.

use crate::core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId};

/// Go `SpellSchoolFire`.
const FIRE: u8 = 4;

#[derive(Clone, Debug)]
pub(crate) struct FirePower {
    per_effect: f64,
    max_effects: i32,
    /// Each family's dots: Fireball, Flamestrike, Ignite and Pyroblast, in Go's order.
    families: Vec<Vec<DotId>>,
    /// Whether each spell, by spellbook position, is Ignite.
    ignite: Vec<bool>,
}

pub(crate) fn bind<A: Agent>(fight: &Fight<A>, per_effect: f64, max_effects: i32) -> FirePower {
    let families = [
        &["fireball"][..],
        &["flamestrike", "flamestrike_dot"][..],
        &["ignite"][..],
        &["pyroblast", "pyroblast_dot"][..],
    ]
    .iter()
    .map(|names| {
        // By class name alone: Ignite carries NoSpellMods, which a modifier's spell list skips.
        fight
            .spells
            .iter()
            .filter(|spell| {
                spell.school == FIRE
                    && spell
                        .class_spell
                        .as_deref()
                        .is_some_and(|class| names.contains(&class))
            })
            .filter_map(|spell| spell.dot)
            .collect()
    })
    .collect();
    let ignite = fight
        .spells
        .iter()
        .map(|spell| spell.class_spell.as_deref() == Some("ignite"))
        .collect();
    FirePower {
        per_effect,
        max_effects,
        families,
        ignite,
    }
}

impl FirePower {
    fn fire_effects<A: Agent>(
        &self,
        fight: &Fight<A>,
        target: Side,
        fire_vulnerability: Option<AuraRef>,
    ) -> i32 {
        let mut count = 0;
        for family in &self.families {
            if family.iter().any(|&dot| {
                let dot = fight.dot_on(dot, target);
                fight.aura(fight.dots[dot].aura).active
            }) {
                count += 1;
            }
        }
        if target == Side::Target && fire_vulnerability.is_some_and(|aura| fight.aura(aura).active)
        {
            count += 1;
        }
        count.min(self.max_effects)
    }

    /// The handler: a Fire spell of the mage, Ignite aside, against `target`.
    pub(crate) fn multiplier<A: Agent>(
        &self,
        fight: &Fight<A>,
        spell: SpellId,
        target: Side,
        fire_vulnerability: Option<AuraRef>,
    ) -> f64 {
        if fight.spells[spell].school & FIRE == 0 || self.ignite[spell] {
            return 1.0;
        }
        let count = self.fire_effects(fight, target, fire_vulnerability);
        1.0 + self.per_effect * f64::from(count)
    }
}
