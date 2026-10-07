//! Go sim/core/spelldata/resolve_proc.go: what the client states about a proc, as the fields
//! core registers a listener through.
//!
//! The listener a row describes is a [`ProcTrigger`]; its handler and extra condition only run in
//! a fight and are not carried. The exporter reads the resolved listener back (the callbacks it
//! hears, its chance and cooldown), so the resolution is exact.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::data::spells::{ClassFlags, Spell};

use super::aura_helpers::{CallbackMask, ProcTrigger};
use super::dbcenums;
use super::proc_type_mask::{decode_proc_type_mask, ProcHint};
use super::sim::{Sim, UnitId};
use super::spell::{ProcMask, SpellFlag};
use super::spelldata::{proc_chance_source, Effect};

/// Go `ProcOpt`: an addition to the resolved trigger for what the client does not state, which
/// on a proc is the rate. It reads the simulation for the procs-per-minute managers, which are
/// bound to the weapon carrying them.
pub(crate) type ProcOpt = Rc<dyn Fn(&Sim, Option<UnitId>, &mut ProcTrigger)>;

/// Go `core.ClassSpellFamilies`: the client's SpellClassSet per class.
pub(crate) fn class_spell_family(class: &str) -> Option<i32> {
    Some(match class {
        "ClassMage" => 3,
        "ClassWarrior" => 4,
        "ClassWarlock" => 5,
        "ClassPriest" => 6,
        "ClassDruid" => 7,
        "ClassRogue" => 8,
        "ClassHunter" => 9,
        "ClassPaladin" => 10,
        "ClassShaman" => 11,
        _ => return None,
    })
}

const CLASS_SPELL_FAMILIES: [i32; 9] = [3, 4, 5, 6, 7, 8, 9, 10, 11];

/// Go `ProcTrigger`: what the client states about a proc, as the fields core registers a
/// listener through: what it hears (the mask decoder, with the row's tooltip hint), how often
/// it fires, its internal cooldown and the attribute-driven gates. The caller adds what the row
/// does not carry (the name and action the sim keys the effect by) before registering it.
///
/// The character is needed for the procs-per-minute managers, which are bound to the weapon
/// carrying them.
pub(crate) fn proc_trigger(
    sim: &Sim,
    character: Option<UnitId>,
    s: &'static Spell,
    opts: &[ProcOpt],
) -> ProcTrigger {
    let decoded = decode_proc_type_mask(s.proc_flags, ProcHint(s.proc_hint));

    let mut trigger = ProcTrigger {
        name: s.name.clone(),
        action_id: ActionId {
            spell_id: s.id,
            ..ActionId::default()
        },
        callback: decoded.callback,
        proc_mask: decoded.proc_mask,
        outcome: decoded.outcome,
        require_damage_dealt: decoded.require_damage_dealt,
        icd: s.icd(),
        can_proc_from_procs: s.can_proc_from_procs(),
        class_spells_only: s.class_spells_only(),
        class_flags: proc_class_flags(sim, character, s),
        ..ProcTrigger::default()
    };

    if s.is_weapon_proc_aura() {
        trigger.spell_flags_exclude |= SpellFlag::SUPPRESS_WEAPON_PROCS;
    }

    fill_proc_chance(s, &mut trigger);

    // The row first and the caller's options on top, so an option sees the mask the row decoded
    // to and can override a rate the row states wrongly or not at all.
    for opt in opts {
        opt(sim, character, &mut trigger);
    }

    settle_proc_rate(sim, character, s, &mut trigger);
    trigger
}

/// Go `PPM`: a hand-supplied rate, for a proc whose real rate lives outside the spell data. It
/// reads the trigger's own proc mask, so an option that narrows the mask has to be given before
/// this one.
pub(crate) fn ppm(ppm: f64) -> ProcOpt {
    Rc::new(move |sim, character, trigger| {
        // The callback rolls the chance first and the manager only where the chance let it
        // through, so a column chance left next to a manager would gate the rate twice.
        trigger.proc_chance = 0.0;
        let character = character.expect("a procs-per-minute rate needs a character");
        trigger.dpm = Some(Rc::new(sim.new_ppm_manager(
            character,
            ppm,
            trigger.proc_mask,
        )));
    })
}

/// Go `ChanceFrom`: the chance an effect states.
#[allow(dead_code)]
pub(crate) fn chance_from(effect: &Effect) -> ProcOpt {
    chance(effect.percent())
}

/// Go `Chance`: a rate the caller states outright, which replaces whatever the row said,
/// manager included.
pub(crate) fn chance(chance: f64) -> ProcOpt {
    Rc::new(move |_, _, trigger| {
        trigger.proc_chance = chance;
        trigger.dpm = None;
    })
}

/// Go `WeaponProc`: a weapon proc's listener, which no row states: the game casts a "Chance on
/// hit" effect and a combat enchant off every eligible weapon hit, so the trigger hears them all
/// and the weapon it sits on decides which ones count.
pub(crate) fn weapon_proc() -> ProcOpt {
    Rc::new(|_, _, trigger| {
        trigger.callback = CallbackMask::ON_SPELL_HIT_DEALT;
        trigger.proc_mask = ProcMask::UNKNOWN;
        trigger.require_damage_dealt = false;
        trigger.can_proc_from_procs = false;
        trigger.spell_flags_exclude =
            SpellFlag(trigger.spell_flags_exclude.0 & !SpellFlag::SUPPRESS_WEAPON_PROCS.0);
        trigger.is_weapon_proc = true;
    })
}

/// Go `ItemProcChance`: the column's chance on an item or enchant proc
/// [`item_proc_rolls_the_column`] picks it for. A rate option that replaces the chance has to
/// come after this one.
pub(crate) fn item_proc_chance(s: &'static Spell) -> ProcOpt {
    Rc::new(move |_, _, trigger| {
        if item_proc_rolls_the_column(s) {
            trigger.proc_chance = f64::from(s.proc_chance) / 100.0;
        }
    })
}

/// Go `Spell.StatedChance`: the roll the row states, by the source that says where it is
/// stated. 0 where the row states no roll.
pub(crate) fn stated_chance(s: &Spell) -> f64 {
    match s.proc_chance_source {
        proc_chance_source::COLUMN => {
            // 101 is the client's "fires on its own condition" sentinel next to a real 100.
            (f64::from(s.proc_chance) / 100.0).min(1.0)
        }
        proc_chance_source::EFFECT_N => s.effect_n(i32::from(s.proc_chance_effect)).percent(),
        proc_chance_source::ALWAYS => 1.0,
        _ => 0.0,
    }
}

/// Go `Spell.ItemProcRollsTheColumn`: whether an item or enchant proc rolls the `ProcChance`
/// column rather than the chance its tooltip states on an effect.
pub(crate) fn item_proc_rolls_the_column(s: &Spell) -> bool {
    s.proc_chance_source == proc_chance_source::EFFECT_N
        && s.rppm == 0.0
        && s.proc_chance > 0
        && s.proc_chance < 100
        && f64::from(s.proc_chance) / 100.0 != stated_chance(s)
}

/// Go `procClassFlags`: the spells the listener fires on where the client names them, read
/// against the class wearing it.
fn proc_class_flags(sim: &Sim, character: Option<UnitId>, s: &Spell) -> ClassFlags {
    let flags = row_class_flags(s);
    if others_family(sim, character, &flags) {
        return ClassFlags::default();
    }
    flags
}

/// Go `rowClassFlags`: only the proc effect's flags.
pub(crate) fn row_class_flags(s: &Spell) -> ClassFlags {
    for e in &s.effects {
        if (dbcenums::is_proc_trigger(e.aura)
            || e.aura == dbcenums::A_PROC_TRIGGER_SPELL_COPY
            || e.aura == dbcenums::A_PROC_TRIGGER_DAMAGE
            || e.aura == dbcenums::A_DUMMY)
            && !e.class_flags.is_zero()
        {
            return e.class_flags;
        }
    }
    ClassFlags::default()
}

/// Go `othersFamily`: whether the mask names spells of a family this character's class never
/// casts, which is how an item shared by every class states the filter of the one class it was
/// written for.
pub(crate) fn others_family(sim: &Sim, character: Option<UnitId>, flags: &ClassFlags) -> bool {
    let Some(character) = character else {
        return false;
    };
    let Some(family) = class_spell_family(&sim.character(character).class) else {
        return false;
    };
    !flags.is_zero() && flags.family != family && CLASS_SPELL_FAMILIES.contains(&flags.family)
}

/// Go `fillProcChance`: a row whose rate is procs per minute states no roll.
fn fill_proc_chance(s: &Spell, trigger: &mut ProcTrigger) {
    if s.rppm > 0.0 {
        return;
    }
    trigger.proc_chance = stated_chance(s);
}

/// Go `settleProcRate`: the rate the trigger ends up with. A trigger left with neither a chance
/// nor a manager fires on every qualifying hit, which is never what a row with no stated rate
/// means.
fn settle_proc_rate(sim: &Sim, character: Option<UnitId>, s: &Spell, trigger: &mut ProcTrigger) {
    if trigger.proc_chance != 0.0 || trigger.dpm.is_some() {
        return;
    }
    if s.rppm > 0.0 {
        // A mask of nothing is the weapon-proc shape, where which hits count is the weapon's to
        // say and the manager has to be bound to it by whoever knows which weapon that is.
        if trigger.proc_mask == ProcMask::UNKNOWN {
            panic!(
                "spelldata: spell {} ({}) states {} procs per minute but no proc mask to measure them on; pass a manager bound to what carries it",
                s.id,
                s.name,
                go_float32(s.rppm)
            );
        }
        let character = character.expect("a procs-per-minute rate needs a character");
        trigger.dpm = Some(Rc::new(sim.new_ppm_manager(
            character,
            f64::from(s.rppm),
            trigger.proc_mask,
        )));
        return;
    }
    panic!(
        "spelldata: spell {} ({}) states no proc chance; pass PPM()",
        s.id, s.name
    );
}

/// Go's `%g` of a float32 value, which prints the shortest decimal that reads back.
fn go_float32(value: f32) -> String {
    format!("{value}")
}
