//! Go sim/core/proc_types.go: the listener shape a client `ProcTypeMask` describes.

use super::aura_helpers::{CallbackMask, HitOutcome};
use super::dbcenums as d;
use super::spell::ProcMask;

/// What the client's mask alone cannot say, read off the spell's tooltip by the generator and
/// stored in the row's `ProcHint` column. Go `ProcHint`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ProcHint(pub u8);

#[allow(dead_code)]
impl ProcHint {
    /// The tooltip names the cast itself as the trigger.
    pub const CAST_TRIGGER: ProcHint = ProcHint(1 << 0);
    /// The tooltip names a critical strike as the trigger.
    pub const CRIT: ProcHint = ProcHint(1 << 1);
    /// The tooltip's trigger clause names healing, or an unrestricted "a spell".
    pub const HEALS: ProcHint = ProcHint(1 << 2);
    /// The tooltip restricts the trigger to healing spells.
    pub const PURE_HEAL: ProcHint = ProcHint(1 << 3);
    /// The trigger clause names one ability.
    pub const NAMED_ABILITY: ProcHint = ProcHint(1 << 4);
    /// The trigger clause states an attack outcome the mask has no bit for.
    pub const OUTCOME_TAKEN: ProcHint = ProcHint(1 << 5);
    /// The wearer's own attack being dodged.
    pub const ATTACK_DODGED: ProcHint = ProcHint(1 << 6);
    /// The wearer's own attack being parried.
    pub const ATTACK_PARRIED: ProcHint = ProcHint(1 << 7);
    /// The wearer's own attack landing on nothing.
    pub const ATTACK_AVOIDED: ProcHint = ProcHint(Self::ATTACK_DODGED.0 | Self::ATTACK_PARRIED.0);

    /// Whether any bit of `other` is set.
    pub(crate) fn matches(self, other: ProcHint) -> bool {
        self.0 & other.0 != 0
    }
}

/// Go `ProcTypeInfo`.
#[derive(Clone, Debug, Default)]
pub(crate) struct ProcTypeInfo {
    pub callback: CallbackMask,
    pub proc_mask: ProcMask,
    pub outcome: HitOutcome,
    pub require_damage_dealt: bool,
    /// Named bits the decoder does not model; empty when supported.
    pub unsupported: Vec<String>,
}

/// The taken bits that name a direct hit arriving on the character.
const PROC_FLAG_ANY_DIRECT_TAKEN: u32 = d::PROC_FLAG_TAKE_MELEE_SWING
    | d::PROC_FLAG_TAKE_MELEE_ABILITY
    | d::PROC_FLAG_TAKE_RANGED_ATTACK
    | d::PROC_FLAG_TAKE_RANGED_ABILITY
    | d::PROC_FLAG_TAKE_HARMFUL_ABILITY
    | d::PROC_FLAG_TAKE_HARMFUL_SPELL;

/// Every direct hit dealt.
const PROC_FLAG_ANY_DIRECT_DEALT: u32 = d::PROC_FLAG_DEAL_MELEE_SWING
    | d::PROC_FLAG_DEAL_MELEE_ABILITY
    | d::PROC_FLAG_DEAL_RANGED_ATTACK
    | d::PROC_FLAG_DEAL_RANGED_ABILITY
    | d::PROC_FLAG_DEAL_HARMFUL_SPELL;

/// Go `DecodeProcTypeMask`: reads `SpellAuraOptions.ProcTypeMask` (two 32-bit words).
pub(crate) fn decode_proc_type_mask(mask: [u32; 2], hint: ProcHint) -> ProcTypeInfo {
    let mut info = ProcTypeInfo {
        require_damage_dealt: true,
        ..ProcTypeInfo::default()
    };
    let word = mask[0];
    let mut proc_mask = 0u32;
    let mut callback = CallbackMask::EMPTY;

    if word & d::PROC_FLAG_DEAL_MELEE_SWING != 0 {
        proc_mask |= ProcMask::MELEE_WHITE_HIT.0;
    }
    if word & d::PROC_FLAG_DEAL_MELEE_ABILITY != 0 {
        proc_mask |= ProcMask::MELEE_SPECIAL.0;
    }
    if word & d::PROC_FLAG_DEAL_RANGED_ATTACK != 0 {
        proc_mask |= ProcMask::RANGED_AUTO.0;
    }
    if word & d::PROC_FLAG_DEAL_RANGED_ABILITY != 0 {
        proc_mask |= ProcMask::RANGED_SPECIAL.0;
    }
    if word & (d::PROC_FLAG_DEAL_HARMFUL_PERIODIC | d::PROC_FLAG_DEAL_HARMFUL_SPELL) != 0 {
        proc_mask |= ProcMask::SPELL_DAMAGE.0;
    }

    if word & PROC_FLAG_ANY_DIRECT_TAKEN != 0 {
        callback = callback | CallbackMask::ON_SPELL_HIT_TAKEN;
        if word & d::PROC_FLAG_TAKE_MELEE_SWING != 0 {
            proc_mask |= ProcMask::MELEE_WHITE_HIT.0;
        }
        if word & d::PROC_FLAG_TAKE_MELEE_ABILITY != 0 {
            proc_mask |= ProcMask::MELEE_SPECIAL.0;
        }
        if word & d::PROC_FLAG_TAKE_RANGED_ATTACK != 0 {
            proc_mask |= ProcMask::RANGED_AUTO.0;
        }
        if word & d::PROC_FLAG_TAKE_RANGED_ABILITY != 0 {
            proc_mask |= ProcMask::RANGED_SPECIAL.0;
        }
        if word & d::PROC_FLAG_TAKE_HARMFUL_SPELL != 0 {
            proc_mask |= ProcMask::SPELL_DAMAGE.0;
        }
    }

    if word & d::PROC_FLAG_TAKE_HARMFUL_PERIODIC != 0 {
        callback = callback | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN;
    }

    // Damage of any kind, however it arrived, which is both of the taken callbacks.
    if word & d::PROC_FLAG_TAKE_ANY_DAMAGE != 0 {
        callback =
            callback | CallbackMask::ON_SPELL_HIT_TAKEN | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN;
    }

    // The damage-class-none bit beside the magic one names the same casts a second way and says
    // nothing about the trigger, so the cast question is asked of the mask without it.
    let cast_word = word & !d::PROC_FLAG_DEAL_HARMFUL_ABILITY;

    // A mask made of nothing but the spell-cast bits. The harmful one has to be present.
    let spell_cast_mask = cast_word & d::PROC_FLAG_DEAL_HARMFUL_SPELL != 0
        && cast_word & !(d::PROC_FLAG_DEAL_HARMFUL_SPELL | d::PROC_FLAG_DEAL_HELPFUL_SPELL) == 0;

    // Whether the cast itself is the trigger.
    let cast_only = spell_cast_mask
        && (cast_word == d::PROC_FLAG_DEAL_HARMFUL_SPELL || hint.matches(ProcHint::CAST_TRIGGER));

    // A tooltip naming an outcome is the exception to all of it: a crit is only known once the
    // hit resolves, so those stay on hit-dealt.
    if cast_only && !hint.matches(ProcHint::CRIT) {
        callback = callback | CallbackMask::ON_CAST_COMPLETE;
        info.require_damage_dealt = false;
    } else if word & PROC_FLAG_ANY_DIRECT_DEALT != 0 {
        callback = callback | CallbackMask::ON_SPELL_HIT_DEALT;
        if word & d::PROC_FLAG_DEAL_HARMFUL_SPELL != 0 {
            info.require_damage_dealt = false;
        }
    }

    if word & d::PROC_FLAG_DEAL_HARMFUL_PERIODIC != 0 {
        callback = callback | CallbackMask::ON_PERIODIC_DAMAGE_DEALT;
    }

    // The helpful-spell bit beside the harmful one is the client's "any spell", heals included.
    let helpful_spells = word & d::PROC_FLAG_DEAL_HELPFUL_SPELL != 0
        && (word & d::PROC_FLAG_DEAL_HARMFUL_SPELL != 0 || hint.matches(ProcHint::HEALS));

    if helpful_spells {
        info.require_damage_dealt = false;
        proc_mask |= ProcMask::SPELL_HEALING.0;

        // Casting the heal is already the trigger above, so adding heal-dealt on top would
        // proc twice for one heal.
        if !callback.matches(CallbackMask::ON_CAST_COMPLETE) {
            callback = callback | CallbackMask::ON_HEAL_DEALT;
            if word & d::PROC_FLAG_DEAL_HELPFUL_PERIODIC != 0 {
                callback = callback | CallbackMask::ON_PERIODIC_HEAL_DEALT;
            }
            // Periodic damage paired with only a heal usually indicates a pure heal mask.
            if word & PROC_FLAG_ANY_DIRECT_DEALT == 0 {
                callback.0 &= !CallbackMask::ON_PERIODIC_DAMAGE_DEALT.0;
                callback.0 &= !CallbackMask::ON_SPELL_HIT_DEALT.0;
                proc_mask &= !ProcMask::SPELL_DAMAGE.0;
            }
        }
    }

    // A mask naming one hand hears that hand's melee hits only. Naming both hands is the same as
    // naming neither.
    match word & (d::PROC_FLAG_MAIN_HAND_WEAPON_SWING | d::PROC_FLAG_OFF_HAND_WEAPON_SWING) {
        d::PROC_FLAG_MAIN_HAND_WEAPON_SWING => proc_mask &= !ProcMask::MELEE_OH.0,
        d::PROC_FLAG_OFF_HAND_WEAPON_SWING => proc_mask &= !ProcMask::MELEE_MH.0,
        _ => {}
    }
    if mask[1] & d::PROC_FLAG_2_MAIN_HAND_ONLY != 0 {
        proc_mask &= !ProcMask::MELEE_OH.0;
    }

    // An avoidance outcome is a hit that landed on nothing, so a listener whose trigger is one
    // hears a hit that dealt no damage.
    if hint.matches(ProcHint(
        ProcHint::OUTCOME_TAKEN.0 | ProcHint::ATTACK_AVOIDED.0,
    )) {
        info.require_damage_dealt = false;
    }

    // An outcome the listener can be given.
    info.outcome = if callback.matches(CallbackMask::ON_CAST_COMPLETE) {
        HitOutcome::EMPTY
    } else if hint.matches(ProcHint::ATTACK_AVOIDED) {
        let mut outcome = HitOutcome::EMPTY;
        if hint.matches(ProcHint::ATTACK_DODGED) {
            outcome = outcome | HitOutcome::DODGE;
        }
        if hint.matches(ProcHint::ATTACK_PARRIED) {
            outcome = outcome | HitOutcome::PARRY;
        }
        outcome
    } else if hint.matches(ProcHint::CRIT) {
        HitOutcome::CRIT
    } else {
        HitOutcome::LANDED
    };

    if hint.matches(ProcHint::PURE_HEAL) {
        callback.0 &= !CallbackMask::ON_SPELL_HIT_DEALT.0;
        callback.0 &= !CallbackMask::ON_PERIODIC_DAMAGE_DEALT.0;
    }

    info.callback = callback;
    info.proc_mask = ProcMask(proc_mask);
    info.unsupported = unsupported_proc_flags(mask);
    info
}

/// The bits the shape above says nothing about: everything from the death bit up is a state
/// change rather than a hit, and of word 1 the sim models the main-hand restriction alone.
fn unsupported_proc_flags(mask: [u32; 2]) -> Vec<String> {
    let unsupported = mask[0]
        & (d::PROC_FLAG_HEARTBEAT
            | d::PROC_FLAG_KILL
            | d::PROC_FLAG_TAKE_HELPFUL_SPELL
            | !(d::PROC_FLAG_DEATH - 1));
    let name_of = |bit: u32| -> Option<&'static str> {
        Some(match 1u32 << bit {
            d::PROC_FLAG_HEARTBEAT => "HEARTBEAT",
            d::PROC_FLAG_KILL => "KILL",
            d::PROC_FLAG_TAKE_HELPFUL_SPELL => "TAKE_HELPFUL_SPELL",
            d::PROC_FLAG_DEATH => "DEATH",
            d::PROC_FLAG_JUMP => "JUMP",
            d::PROC_FLAG_ENTER_COMBAT => "ENTER_COMBAT",
            d::PROC_FLAG_ENCOUNTER_START => "ENCOUNTER_START",
            d::PROC_FLAG_CAST_ENDED => "CAST_ENDED",
            d::PROC_FLAG_LOOTED => "LOOTED",
            _ => return None,
        })
    };
    let mut names = Vec::new();
    for bit in 0..32 {
        if unsupported & (1 << bit) == 0 {
            continue;
        }
        match name_of(bit) {
            Some(name) => names.push(name.to_string()),
            None => names.push(format!("bit {bit}")),
        }
    }
    let word1 = mask[1] & !d::PROC_FLAG_2_MAIN_HAND_ONLY;
    for bit in 0..32 {
        if word1 & (1 << bit) != 0 {
            names.push(format!("bit {}", 32 + bit));
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_melee_swing_mask_hears_landed_white_hits() {
        let info = decode_proc_type_mask([d::PROC_FLAG_DEAL_MELEE_SWING, 0], ProcHint::default());
        assert_eq!(info.proc_mask, ProcMask::MELEE_WHITE_HIT);
        assert_eq!(info.callback, CallbackMask::ON_SPELL_HIT_DEALT);
        assert_eq!(info.outcome, HitOutcome::LANDED);
        assert!(info.require_damage_dealt);
        assert!(info.unsupported.is_empty());
    }

    #[test]
    fn a_lone_harmful_spell_bit_is_a_cast() {
        let info = decode_proc_type_mask([d::PROC_FLAG_DEAL_HARMFUL_SPELL, 0], ProcHint::default());
        assert_eq!(info.callback, CallbackMask::ON_CAST_COMPLETE);
        assert_eq!(info.outcome, HitOutcome::EMPTY);
        assert!(!info.require_damage_dealt);
    }

    #[test]
    fn a_crit_hint_keeps_a_spell_on_hit_dealt() {
        let info = decode_proc_type_mask([d::PROC_FLAG_DEAL_HARMFUL_SPELL, 0], ProcHint::CRIT);
        assert_eq!(info.callback, CallbackMask::ON_SPELL_HIT_DEALT);
        assert_eq!(info.outcome, HitOutcome::CRIT);
    }

    #[test]
    fn a_taken_melee_mask_listens_to_hits_taken() {
        let info = decode_proc_type_mask([d::PROC_FLAG_TAKE_MELEE_SWING, 0], ProcHint::default());
        assert_eq!(info.callback, CallbackMask::ON_SPELL_HIT_TAKEN);
        assert_eq!(info.proc_mask, ProcMask::MELEE_WHITE_HIT);
    }

    #[test]
    fn unsupported_bits_are_named() {
        let info = decode_proc_type_mask(
            [d::PROC_FLAG_KILL | d::PROC_FLAG_DEATH, 0x2],
            ProcHint::default(),
        );
        assert_eq!(info.unsupported, vec!["KILL", "DEATH", "bit 33"]);
    }

    #[test]
    fn one_hand_drops_the_other_hands_melee() {
        let mask = d::PROC_FLAG_DEAL_MELEE_SWING | d::PROC_FLAG_MAIN_HAND_WEAPON_SWING;
        let info = decode_proc_type_mask([mask, 0], ProcHint::default());
        assert_eq!(info.proc_mask, ProcMask::MELEE_MH_AUTO);
    }
}
