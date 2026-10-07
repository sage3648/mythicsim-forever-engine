//! Go sim/rogue/poisons.go: the three imbues as weapon proc auras and the spells they cast.
//!
//! Poisons are consumable imbues, not class spells, so the client rows hold no ladder for them:
//! the numbers are Go literals.

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::sim::{AuraConfig, Sim, UnitId, SECOND};
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

/// Go `getPoisonProcMask`: the hands the imbue is on.
pub(super) fn poison_proc_mask(sim: &Sim, unit: UnitId, imbue_id: i32) -> ProcMask {
    let consumables = &sim.character(unit).consumables;
    let mut mask = ProcMask::UNKNOWN;
    if consumables.i32("mhImbue_id") == imbue_id {
        mask = mask | ProcMask::MELEE_MH;
    }
    if consumables.i32("ohImbue_id") == imbue_id {
        mask = mask | ProcMask::MELEE_OH;
    }
    mask
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
    let proc_mask = poison_proc_mask(sim, unit, imbue_id);
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
        if poison_proc_mask(sim, unit, DEADLY_IMBUE_ID) == ProcMask::UNKNOWN {
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
        if poison_proc_mask(sim, unit, WOUND_IMBUE_ID) == ProcMask::UNKNOWN {
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
        if poison_proc_mask(sim, unit, INSTANT_IMBUE_ID) == ProcMask::UNKNOWN {
            return;
        }
        let spell = sim.register_spell(
            unit,
            poison_config(spell_action(INSTANT_POISON_SPELL_ID), masks::INSTANT_POISON),
        );
        self.spells.instant_poison = Some(spell);
    }
}
