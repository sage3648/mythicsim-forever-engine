//! Go sim/core/spelldata/item_proc.go: what a proc the client hangs on an item or an enchant does
//! not model.
//!
//! These are the generator's audits of a row: the sim reads the same rows to build a listener, so
//! a proc the generator emits is one the sim can build, and one it comments out is one nothing
//! would have heard correctly. Preparation does not call them; they are kept so the audits agree
//! with the sim's own decisions, and the tests below pin that agreement for the registered procs.

use crate::data::spells::Spell;

use super::aura_helpers::CallbackMask;
use super::proc_type_mask::{decode_proc_type_mask, ProcHint};
use super::resolve_proc::{row_class_flags, stated_chance};
use super::spell::ProcMask;

/// Go `ReasonStatesNoRate`: the reason given for a proc whose rows state no rate, which the
/// generator also gives where the rows' rate is a sentinel its tooltip contradicts.
pub(crate) const REASON_STATES_NO_RATE: &str = "states no rate";

/// Go `ReasonPPMHearsNoWeaponHits`.
pub(crate) const REASON_PPM_HEARS_NO_WEAPON_HITS: &str =
    "a procs-per-minute rate hears no weapon hits in this mask";

/// Go `ItemProcUnsupported`: what a proc the client hangs on an item, an enchant or a set bonus
/// does not model, as one reason per shape. An empty answer means the rows state enough to build
/// the listener the client describes.
pub(crate) fn item_proc_unsupported(trigger: &Spell, is_weapon_proc: bool) -> Vec<String> {
    unsupported(trigger, is_weapon_proc, false)
}

/// Go `EnchantAuraUnsupported`: the same for an enchant's equip aura, whose procs-per-minute rate
/// rolls on weapon hits only: the sim never procs one from a spell or a heal.
#[allow(dead_code)]
pub(crate) fn enchant_aura_unsupported(trigger: &Spell) -> Vec<String> {
    unsupported(trigger, false, true)
}

fn unsupported(trigger: &Spell, is_weapon_proc: bool, is_enchant: bool) -> Vec<String> {
    let mut unsupported = Vec::new();
    if trigger.id == 0 {
        return vec!["no row in the store".to_string()];
    }
    let hint = ProcHint(trigger.proc_hint);
    let decoded = decode_proc_type_mask(trigger.proc_flags, hint);

    // No ProcTypeMask can state either: a trigger restricted to one named ability, and one
    // restricted to an outcome the mask has no bit for.
    if hint.matches(ProcHint::NAMED_ABILITY) {
        unsupported.push("named ability".to_string());
    }
    if hint.matches(ProcHint::OUTCOME_TAKEN) {
        unsupported.push("an outcome the proc mask has no bit for".to_string());
    }
    if !row_class_flags(trigger).is_zero() {
        unsupported.push("the trigger carries a class mask".to_string());
    }
    for bits in &decoded.unsupported {
        unsupported.push(format!("ProcTypeMask {bits}"));
    }

    // A weapon proc hears the hits of whatever carries it, which is the one shape the row states
    // no listener for: the game casts a chance-on-hit effect and a combat enchant off the hit
    // itself.
    if is_weapon_proc {
        if !weapon_proc_rate_stated(trigger) {
            unsupported.push(REASON_STATES_NO_RATE.to_string());
        }
        return unsupported;
    }

    if decoded.callback == CallbackMask::EMPTY {
        unsupported.push("no callback in the proc mask".to_string());
    }

    if !proc_rate_stated(trigger) {
        unsupported.push(REASON_STATES_NO_RATE.to_string());
    } else if trigger.rppm > 0.0 && decoded.proc_mask == ProcMask::UNKNOWN {
        // Procs per minute are measured against the hits the listener hears, and an empty mask
        // counts none of them. On a weapon proc the weapon answers that; nothing else can.
        unsupported.push("no proc mask to measure its rate on".to_string());
    } else if is_enchant
        && trigger.rppm > 0.0
        && !decoded.proc_mask.matches(ProcMask::MELEE_OR_RANGED)
    {
        unsupported.push(REASON_PPM_HEARS_NO_WEAPON_HITS.to_string());
    }
    unsupported
}

/// Go `weaponProcRateStated`: the same for a weapon proc, where the client's 100 and 101 are no
/// answer. Those mean "fires whenever its own condition is met", and a chance-on-hit effect has no
/// condition: the game casts it off the weapon's hit without consulting the row at all.
fn weapon_proc_rate_stated(s: &Spell) -> bool {
    if s.proc_chance_source == super::spelldata::proc_chance_source::ALWAYS {
        return s.rppm > 0.0;
    }
    proc_rate_stated(s)
}

/// Go `procRateStated`: whether the row states a rate the trigger can be built from at all: a
/// roll of its own, or the procs-per-minute an override wrote onto it.
fn proc_rate_stated(s: &Spell) -> bool {
    s.rppm > 0.0 || stated_chance(s) != 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::must_find;

    #[test]
    fn a_row_that_states_a_chance_and_a_hit_is_supported() {
        // Draconic Infused Emblem's trigger.
        assert!(item_proc_unsupported(must_find(1318931), false).is_empty());
    }

    #[test]
    fn a_row_not_in_the_store_is_unsupported() {
        assert_eq!(
            item_proc_unsupported(crate::prepare::spelldata::nil(), false),
            vec!["no row in the store"]
        );
    }

    #[test]
    fn a_weapon_proc_whose_rate_is_a_sentinel_states_none() {
        // The client's 100 and 101 mean "fires on its own condition", which is no rate for a
        // chance-on-hit effect; a procs-per-minute rate is one.
        let sentinel = Spell {
            id: 1,
            proc_chance_source: crate::prepare::spelldata::proc_chance_source::ALWAYS,
            ..Spell::default()
        };
        assert_eq!(
            item_proc_unsupported(&sentinel, true),
            vec![REASON_STATES_NO_RATE]
        );
        let rated = Spell {
            rppm: 2.0,
            ..sentinel
        };
        assert!(item_proc_unsupported(&rated, true).is_empty());
    }

    #[test]
    fn the_registered_item_procs_are_supported() {
        use crate::prepare::forever_items_generated::ITEMS;
        use crate::prepare::shared_items::{expand_variants, registers, ProcKind};
        for registration in ITEMS {
            if let crate::prepare::forever_items::Registration::Proc(kind, cfg, variants) =
                registration
            {
                if matches!(kind, ProcKind::EquipAura) {
                    continue;
                }
                for expanded in expand_variants(*cfg, variants) {
                    if !registers(*kind, &expanded) || expanded.is_weapon_proc {
                        continue;
                    }
                    let (trigger, _) = expanded.rows();
                    let reasons = item_proc_unsupported(trigger, false);
                    assert!(
                        reasons.is_empty(),
                        "{} (trigger {}): {reasons:?}",
                        expanded.name,
                        trigger.id
                    );
                }
            }
        }
    }
}
