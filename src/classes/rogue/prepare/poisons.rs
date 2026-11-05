//! Go sim/rogue/poisons.go: the three imbues as weapon proc auras and the spells they cast.
//!
//! Poisons are consumable imbues, not class spells, so the client rows hold no ladder for them:
//! the numbers are Go literals.

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::items::slot as item_slot;
use crate::prepare::sim::{AuraConfig, Rules, Sim, UnitId, SECOND};
use crate::prepare::spell::{school, DefenseType, DotConfig, ProcMask, SpellConfig, SpellFlag};

use super::masks;
use super::util::{aura_array_map, new_enemy_aura_array, spell_action, tagged_action};
use super::Rogue;

const INSTANT_IMBUE_ID: i32 = 26891;
const WOUND_IMBUE_ID: i32 = 27188;
const DEADLY_IMBUE_ID: i32 = 27186;

const INSTANT_POISON_SPELL_ID: i32 = 11340;
const DEADLY_POISON_SPELL_ID: i32 = 25347;
const WOUND_POISON_SPELL_ID: i32 = 13227;

/// Go `getPoisonProcMask`: the hands the imbue is on, in the set worn. With a Rogue's swap set
/// worn, the hands follow the swap set's poisons (see [`hand_imbues`]).
pub(super) fn poison_proc_mask(sim: &Sim, unit: UnitId, imbue_id: i32) -> ProcMask {
    let swapped = sim.item_swap(unit).is_some_and(|swap| swap.swapped);
    hand_mask(hand_imbues(sim, unit, swapped), imbue_id)
}

/// Whether the imbue is on a hand in either set: the poison's spell and trigger are registered
/// once for both.
fn in_either_set(sim: &Sim, unit: UnitId, imbue_id: i32) -> bool {
    poison_hands(sim, unit, imbue_id) != ProcMask::UNKNOWN
}

/// The hands the imbue is on in either set.
fn poison_hands(sim: &Sim, unit: UnitId, imbue_id: i32) -> ProcMask {
    hand_mask(hand_imbues(sim, unit, false), imbue_id)
        | hand_mask(hand_imbues(sim, unit, true), imbue_id)
}

fn hand_mask([main_hand, off_hand]: [i32; 2], imbue_id: i32) -> ProcMask {
    let mut mask = ProcMask::UNKNOWN;
    if main_hand == imbue_id {
        mask = mask | ProcMask::MELEE_MH;
    }
    if off_hand == imbue_id {
        mask = mask | ProcMask::MELEE_OH;
    }
    mask
}

/// The imbue on the main hand and the off hand in a set: the request's `mhImbueId` and
/// `ohImbueId`, and with the swap set worn the swap set's `mhImbueSwapId` and `ohImbueSwapId`
/// on each hand the swap changes (0 is no poison).
///
/// MythicSim Rust-only, under Forever's rules: a Rogue's swap set holds the same weapons with
/// poisons of their own, as a Rogue carries a second copy of a weapon with another poison on
/// it. The reference's Rogue reads the request's imbues alone, and refuses an item swap.
pub(super) fn hand_imbues(sim: &Sim, unit: UnitId, swapped: bool) -> [i32; 2] {
    let consumables = &sim.character(unit).consumables;
    let mut imbues = [consumables.i32("mhImbue_id"), consumables.i32("ohImbue_id")];
    let swap = sim.character(unit).item_swap.as_ref();
    if !swapped || sim.rules != Rules::Forever {
        return imbues;
    }
    let Some(swap) = swap else {
        return imbues;
    };
    for (hand, (slot, field)) in [
        (item_slot::MAIN_HAND, "mh_imbue_swap_id"),
        (item_slot::OFF_HAND, "oh_imbue_swap_id"),
    ]
    .into_iter()
    .enumerate()
    {
        if swap.slots.contains(&slot) {
            imbues[hand] = consumables.i32(field);
        }
    }
    imbues
}

/// What a Rogue's swap set may change, noted for the item swap to refuse otherwise: under
/// Forever's rules each hand the swap changes holds the same item with the same enchant in both
/// sets, so the swings, the weapon's procs and the weapon talents read as the reset's and only
/// the poisons follow the swap. A stone or oil the sets differ in would stay on all fight, and
/// another weapon would need the off hand's swing and procs to follow the swap. The reference's
/// Rogue refuses an item swap.
pub(super) fn note_item_swap_limits(sim: &mut Sim, unit: UnitId) {
    let Some(swap) = sim.item_swap(unit) else {
        return;
    };
    let mut unported = Vec::new();
    if sim.rules != Rules::Forever {
        unported.push("a Rogue under the reference's rules".to_string());
    }
    let [worn_imbues, swap_imbues] = [hand_imbues(sim, unit, false), hand_imbues(sim, unit, true)];
    for (hand, slot) in [item_slot::MAIN_HAND, item_slot::OFF_HAND]
        .into_iter()
        .enumerate()
    {
        if !swap.slots.contains(&slot) {
            continue;
        }
        let (worn, swapped) = (&swap.original_equip[slot], &swap.swap_equip[slot]);
        let same = worn.id == swapped.id
            && worn.random_suffix.id == swapped.random_suffix.id
            && worn.enchant.effect_id == swapped.enchant.effect_id;
        if !same {
            unported.push(format!(
                "item {} (enchant {}) in slot {slot} for the Rogue's item {} (enchant {}): a \
                 Rogue's swap set holds the same weapons with other poisons",
                swapped.id, swapped.enchant.effect_id, worn.id, worn.enchant.effect_id
            ));
        }
        let (from, to) = (worn_imbues[hand], swap_imbues[hand]);
        let poison_or_none = |id: i32| id == 0 || is_poison(id);
        if from != to && !(poison_or_none(from) && poison_or_none(to)) {
            unported.push(format!(
                "a stone or oil the Rogue's sets differ in, {from} and {to} in slot {slot}"
            ));
        }
    }
    for what in unported {
        sim.item_swap_unported(unit, what);
    }
}

/// Whether an imbue is one of the three poisons.
fn is_poison(imbue_id: i32) -> bool {
    [INSTANT_IMBUE_ID, WOUND_IMBUE_ID, DEADLY_IMBUE_ID].contains(&imbue_id)
}

/// The hands each imbue is on, for the exporter.
pub(super) const IMBUES: Imbues = Imbues {
    instant: INSTANT_IMBUE_ID,
    wound: WOUND_IMBUE_ID,
    deadly: DEADLY_IMBUE_ID,
};

pub(super) struct Imbues {
    pub instant: i32,
    pub wound: i32,
    pub deadly: i32,
}

/// Go `applyPoisons`.
pub(super) fn apply_poisons(sim: &mut Sim, unit: UnitId) {
    apply_poison_proc(sim, unit, "Deadly Poison", DEADLY_IMBUE_ID);
    apply_poison_proc(sim, unit, "Wound Poison", WOUND_IMBUE_ID);
    apply_poison_proc(sim, unit, "Instant Poison", INSTANT_IMBUE_ID);
}

/// Go `applyPoisonProc`: a weapon proc on the hand the imbue is on, whose chance the handler
/// rolls at proc time.
fn apply_poison_proc(sim: &mut Sim, unit: UnitId, name: &str, imbue_id: i32) {
    let proc_mask = poison_hands(sim, unit, imbue_id);
    if proc_mask == ProcMask::UNKNOWN {
        return;
    }
    sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: name.to_string(),
            outcome: HitOutcome::LANDED,
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            trigger_immediately: true,
            proc_mask,
            is_weapon_proc: true,
            ..ProcTrigger::default()
        },
    );
}

/// The fields the three poison spells share.
fn poison_config(action_id: crate::contracts::prepared_v2::ActionId, mask: i64) -> SpellConfig {
    SpellConfig {
        action_id,
        spell_school: school::NATURE,
        defense_type: DefenseType::Magic,
        proc_mask: ProcMask::SPELL_DAMAGE_PROC,
        class_spell_mask: mask,
        flags: SpellFlag::POISON | SpellFlag::PASSIVE_SPELL | SpellFlag::PROC,
        damage_multiplier: 1.0,
        damage_multiplier_additive: 1.0,
        threat_multiplier: 1.0,
        ..SpellConfig::default()
    }
}

impl Rogue {
    /// Go `registerDeadlyPoisonSpell`.
    pub(super) fn register_deadly_poison_spell(&mut self, sim: &mut Sim, unit: UnitId) {
        if !in_either_set(sim, unit, DEADLY_IMBUE_ID) {
            return;
        }
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                dot: DotConfig {
                    aura: AuraConfig {
                        label: "Deadly Poison".to_string(),
                        max_stacks: 5,
                        duration: 12 * SECOND,
                        ..AuraConfig::default()
                    },
                    number_of_ticks: 4,
                    tick_length: 3 * SECOND,
                    ..DotConfig::default()
                },
                ..poison_config(
                    tagged_action(DEADLY_POISON_SPELL_ID, 100),
                    masks::DEADLY_POISON,
                )
            },
        );
        self.spells.deadly_poison = Some(spell);
    }

    /// Go `registerWoundPoisonSpell`.
    pub(super) fn register_wound_poison_spell(&mut self, sim: &mut Sim, unit: UnitId) {
        if !in_either_set(sim, unit, WOUND_IMBUE_ID) {
            return;
        }
        // The healing debuff has no effect on a DPS sim.
        let debuff = AuraConfig {
            label: "Wound Poison".to_string(),
            action_id: Some(spell_action(WOUND_POISON_SPELL_ID)),
            duration: 15 * SECOND,
            max_stacks: 5,
            ..AuraConfig::default()
        };
        let auras =
            new_enemy_aura_array(sim, |sim, target| sim.register_aura(target, debuff.clone()));
        let related = aura_array_map(sim, &auras);
        self.auras.wound_poison_debuff = auras;
        let spell = sim.register_spell(
            unit,
            SpellConfig {
                related_aura_arrays: related,
                ..poison_config(spell_action(WOUND_POISON_SPELL_ID), masks::WOUND_POISON)
            },
        );
        self.spells.wound_poison = Some(spell);
    }

    /// Go `registerInstantPoisonSpell`.
    pub(super) fn register_instant_poison_spell(&mut self, sim: &mut Sim, unit: UnitId) {
        if !in_either_set(sim, unit, INSTANT_IMBUE_ID) {
            return;
        }
        let spell = sim.register_spell(
            unit,
            poison_config(spell_action(INSTANT_POISON_SPELL_ID), masks::INSTANT_POISON),
        );
        self.spells.instant_poison = Some(spell);
    }
}
