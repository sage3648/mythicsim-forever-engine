//! Forever's rules for a physical row that deals school damage (README, Rust-only mechanics):
//! the client rows with a `SCHOOL_DAMAGE` effect on the Physical school, as Revenge, Hamstring,
//! Thunder Clap, Eviscerate and a pet's Claw are, against a weapon strike, whose weapon effects
//! take the caster's flat physical damage bonus whole and roll with the weapon.
//!
//! Two rules read such a row. Its hit rolls its base points a half Variance either side and lands
//! on a whole number, and the caster's flat physical damage bonus (from gear) counts at the
//! row's bonus coefficient column, never as spell power. Revenge and Hamstring state 1 and take
//! the bonus; Thunder Clap, Victory Rush and Shield Bash state 0 and do not (warrior damage
//! measurements at levels 20 to 24, 7 November 2026).

use crate::contracts::prepared_v2::ActionId;

use super::dbcenums;
use super::spell::school;
use super::spelldata::{find, Effect};

/// The school damage effect of the row a Physical spell is cast from, if its direct damage is
/// one. A row whose damage sits on a weapon effect answers none.
pub(crate) fn school_damage_effect(action: &ActionId, spell_school: u8) -> Option<&'static Effect> {
    if spell_school != school::PHYSICAL || action.spell_id == 0 {
        return None;
    }
    let row = find(action.spell_id);
    let effect = row.damage_effect();
    (effect.effect_type == dbcenums::E_SCHOOL_DAMAGE).then_some(effect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::all;

    fn action(spell_id: i32) -> ActionId {
        ActionId {
            spell_id,
            ..ActionId::default()
        }
    }

    #[test]
    fn a_school_damage_row_answers_its_bonus_coefficient() {
        // Revenge (25288) and Hamstring (7373) state 1; Thunder Clap (11581), Victory Rush
        // (402927) and Shield Bash (1672) state 0.
        for (id, coefficient) in [
            (25288, 1.0),
            (7373, 1.0),
            (11581, 0.0),
            (402927, 0.0),
            (1672, 0.0),
        ] {
            let effect = school_damage_effect(&action(id), school::PHYSICAL)
                .unwrap_or_else(|| panic!("{id} is a physical school damage row"));
            assert_eq!(effect.coeff(), coefficient, "spell {id}");
        }
    }

    #[test]
    fn a_weapon_strike_and_another_school_answer_none() {
        // Heroic Strike (25286) and Sinister Strike (11294) deal weapon damage.
        for id in [25286, 11294] {
            assert!(
                school_damage_effect(&action(id), school::PHYSICAL).is_none(),
                "spell {id}"
            );
        }
        assert!(school_damage_effect(&action(25288), school::HOLY).is_none());
        assert!(school_damage_effect(&action(0), school::PHYSICAL).is_none());
    }

    /// The whole roll adds a per level gain after rounding the base points
    /// (`Fight::physical_effect_roll`), which it leaves out because no physical school damage
    /// row of the client table has one, and its average is the base points.
    #[test]
    fn physical_rows_have_no_per_level_gain() {
        let mut rows = 0;
        for spell in all().filter(|spell| spell.spell_school() == school::PHYSICAL) {
            for effect in &spell.effects {
                if effect.effect_type == dbcenums::E_SCHOOL_DAMAGE && effect.variance != 0.0 {
                    rows += 1;
                    assert_eq!(effect.ppl, 0.0, "{} ({})", spell.name, spell.id);
                    assert_eq!(
                        effect.base_points.fract(),
                        0.0,
                        "{} ({})",
                        spell.name,
                        spell.id
                    );
                }
            }
        }
        assert!(rows > 50, "{rows} rows");
    }
}
