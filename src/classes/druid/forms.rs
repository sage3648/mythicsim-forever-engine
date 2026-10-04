//! Druid forms, from Go sim/druid/druid.go `RegisterSpell` and forms.go: the form the druid
//! is in, the forms each spell may be cast in, and the movement speed the forms change.
//!
//! A spell whose forms exclude the current one and humanoid fails its cast check and logs
//! why. A spell castable in humanoid form that the current form excludes clears the form
//! when cast instead.

use crate::{
    contracts::prepared_v2::DruidFormSpell,
    core::fight::{Fight, SpellId},
};

use super::agent::DruidAgent;

/// Go `DruidForm` bits.
pub(crate) const HUMANOID: u8 = 1;
pub(crate) const BEAR: u8 = 1 << 1;
pub(crate) const CAT: u8 = 1 << 2;
pub(crate) const MOONKIN: u8 = 1 << 3;
pub(crate) const TREE: u8 = 1 << 4;
/// Go `Any`.
pub(crate) const ANY: u8 = HUMANOID | BEAR | CAT | MOONKIN | TREE;

/// Go `GetMovementSpeed` for a player: seven yards a second.
const PLAYER_SPEED: f64 = 7.0;

/// Form bits from exported form names.
pub(crate) fn parse(names: &[String]) -> u8 {
    names.iter().fold(0, |mask, name| {
        mask | match name.as_str() {
            "humanoid" => HUMANOID,
            "bear" => BEAR,
            "cat" => CAT,
            "moonkin" => MOONKIN,
            "tree" => TREE,
            _ => 0,
        }
    })
}

/// The form masks of the druid's spells, by spell position, and the starting form.
#[derive(Clone, Debug)]
pub(crate) struct Forms {
    masks: Vec<Option<u8>>,
    pub(crate) starting: u8,
}

impl Forms {
    pub(crate) fn new(spell_count: usize, starting: &[String], spells: &[DruidFormSpell]) -> Self {
        let mut masks = vec![None; spell_count];
        for entry in spells {
            if let Some(slot) = masks.get_mut(entry.spell) {
                *slot = Some(parse(&entry.forms));
            }
        }
        Forms {
            masks,
            starting: parse(starting),
        }
    }

    /// The form check of `RegisterSpell`'s cast condition: outside its forms, unless it
    /// allows humanoid form.
    pub(crate) fn wrong_form(&self, form: u8, spell: SpellId) -> bool {
        self.masks[spell]
            .is_some_and(|mask| mask != ANY && mask & form == 0 && mask & HUMANOID == 0)
    }

    /// The form check of `RegisterSpell`'s `ModifyCast`: a humanoid spell cast outside its
    /// forms clears the form first.
    pub(crate) fn clears_form(&self, form: u8, spell: SpellId) -> bool {
        self.masks[spell].is_some_and(|mask| mask & form == 0 && mask & HUMANOID != 0)
    }
}

/// Go `Unit.MultiplyMovementSpeed`, which only logs in scope.
pub(crate) fn multiply_movement_speed(fight: &mut Fight<DruidAgent>, amount: f64) {
    let old = fight.agent.movement_speed;
    fight.agent.movement_speed *= amount;
    if fight.log.is_some() {
        let new = fight.agent.movement_speed;
        let line = format!(
            "[DEBUG] Movement speed changed from {:.2} ({:.2}%) to {:.2} ({:.2}%)",
            PLAYER_SPEED * old,
            (old - 1.0) * 100.0,
            PLAYER_SPEED * new,
            (new - 1.0) * 100.0
        );
        fight.player_log(&line);
    }
}
