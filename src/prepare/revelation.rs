//! Go sim/common/forever/revelation.go: Enchant Weapon - Revelation (enchant 8217, MythicSim
//! patches 157 and 170). Two triggers, one on hits and heals and one on ticks, roll a chance on
//! each eligible landed hit that does not crit, and a batch window later activate the class's
//! buff, which gives the spells its mask names 100% more crit until the next cast of one of them
//! spends it, or a channel it names ends.

use std::rc::Rc;

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::consumable_effects::proc_trigger_spells;
use super::dbcenums;
use super::env::Environment;
use super::sim::{AuraCallback, AuraConfig, EventCallbacks, Rules, Sim, SECOND};
use super::spell::{DefenseType, ProcMask, Spell, SpellFlag};
use super::spelldata::{store, ClassFlags};

pub(crate) const REVELATION_ENCHANT: i32 = 8217;

/// Go `RevelationChanceAtNoCrit`, the patch 157 log fit.
const CHANCE_AT_NO_CRIT: f64 = 0.098;

/// The equip aura the enchant applies (1248806), whose action the triggers proc as.
const TRIGGER_SPELL_ID: i32 = 1248806;

const TRIGGER_AURA: &str = "Enchant Weapon - Revelation";
const TICKS_TRIGGER_AURA: &str = "Enchant Weapon - Revelation (ticks)";
const BUFF_AURA: &str = "Revelation";

/// Go `revelationBuffs`: each class's buff, client 1.60.1.70425, by its spell and the class mask
/// it names. Warriors and rogues have none.
///
/// Client 1.60.1.70630 adds bit 32768 of the second mask word to the Mage buff (SpellEffect
/// 1251976 of 1248808), which names Arcane Barrage and Arcane Wrath; Forever's rules follow it and
/// the Go reference keeps 70425's mask.
fn class_buff(class: &str, rules: Rules) -> Option<(i32, [u32; 4])> {
    Some(match class {
        "ClassMage" => (
            1248808,
            [
                547494642,
                if rules == Rules::Forever { 32768 } else { 0 },
                0,
                8,
            ],
        ),
        "ClassPriest" => (1323377, [161758864, 8486934, 128, 0]),
        "ClassWarlock" => (1323392, [541161, 8650816, 0, 0]),
        "ClassDruid" => (1323400, [4194341, 2, 0, 0]),
        "ClassHunter" => (1323410, [10240, 0, 0, 0]),
        "ClassShaman" => (1323418, [0x801001c3, 266240, 0, 0]),
        "ClassPaladin" => (1323419, [0xc0200400, 2097792, 64, 67108864]),
        _ => return None,
    })
}

/// The trigger every spell hit, heal and tick passes before its own rule.
fn listener() -> ProcTrigger {
    ProcTrigger {
        proc_mask: ProcMask::SPELL_DAMAGE | ProcMask::SPELL_DAMAGE_PROC | ProcMask::SPELL_HEALING,
        can_proc_from_procs: true,
        ..ProcTrigger::default()
    }
}

/// Go `applyRevelation`: the buff, then the hit and tick triggers, each a proc of the enchant.
pub(crate) fn apply_revelation(env: &mut Environment) {
    let unit = env.player;
    let class = env.sim.character(unit).class.clone();
    let Some((buff_spell, _)) = class_buff(&class, env.sim.rules) else {
        return;
    };
    let noop: AuraCallback = Rc::new(|_: &mut Sim, _| {});
    env.sim.register_aura(
        unit,
        AuraConfig {
            label: BUFF_AURA.to_string(),
            action_id: Some(ActionId::spell(buff_spell)),
            duration: 15 * SECOND,
            on_gain: Some(noop.clone()),
            on_expire: Some(noop),
            events: EventCallbacks {
                on_cast_complete: true,
                ..EventCallbacks::default()
            },
            ..AuraConfig::default()
        },
    );
    for (name, callback) in [
        (
            TRIGGER_AURA,
            CallbackMask::ON_SPELL_HIT_DEALT | CallbackMask::ON_HEAL_DEALT,
        ),
        (TICKS_TRIGGER_AURA, CallbackMask::ON_PERIODIC_DAMAGE_DEALT),
    ] {
        let trigger = env.sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: name.to_string(),
                action_id: ActionId::spell(TRIGGER_SPELL_ID),
                callback,
                outcome: HitOutcome::LANDED,
                proc_chance: 1.0,
                ..listener()
            },
        );
        env.sim
            .register_enchant_proc(unit, REVELATION_ENCHANT, trigger);
    }
}

/// Go `hasPeriodicPart` and `ticksByCastingASpell`: whether the row carries one of the auras.
fn row_has_aura(id: i32, auras: &[i32]) -> bool {
    store::find(id)
        .effects
        .iter()
        .any(|effect| auras.contains(&effect.aura))
}

/// Go `revelationCanTriggerFrom`: whether a hit, or a tick, of the spell can trigger it.
fn can_trigger_from(spell: &Spell, periodic: bool) -> bool {
    let id = spell.action_id.spell_id;
    if periodic {
        return spell.aoe_dot.is_some()
            && row_has_aura(
                id,
                &[
                    dbcenums::A_PERIODIC_TRIGGER_SPELL,
                    dbcenums::A_PERIODIC_DUMMY,
                ],
            );
    }
    let periodic_part = row_has_aura(
        id,
        &[
            dbcenums::A_PERIODIC_DAMAGE,
            dbcenums::A_PERIODIC_LEECH,
            dbcenums::A_PERIODIC_HEAL,
        ],
    );
    if spell.proc_mask.matches(ProcMask::SPELL_HEALING) {
        return !periodic_part;
    }
    spell.defense_type == DefenseType::Magic && !periodic_part
}

/// tools/oracle-v2 `revelationEffect`: the spells each trigger hears, the spells the buff names,
/// which of them spend it and which channels end it. `None` without the enchant's auras.
pub(crate) fn revelation_effect(
    env: &Environment,
    _unrepresented: &mut Vec<String>,
) -> Option<Value> {
    let unit = env.player;
    let trigger = env.sim.get_aura(unit, TRIGGER_AURA)?;
    let ticks = env.sim.get_aura(unit, TICKS_TRIGGER_AURA)?;
    let buff = env.sim.get_aura(unit, BUFF_AURA)?;
    let class = &env.sim.character(unit).class;
    let (_, mask) = class_buff(class, env.sim.rules)?;
    let mask = ClassFlags {
        family: super::resolve_proc::class_spell_family(class).unwrap_or(0),
        mask,
    };
    let spellbook = &env.sim.unit(unit).spellbook;
    let (mut affected, mut consuming, mut channels) = (Vec::new(), Vec::new(), Vec::new());
    for (position, &id) in spellbook.iter().enumerate() {
        let spell = env.sim.spell(id);
        let mut flags = spell.class_flags;
        if flags.is_zero() {
            flags = store::find(spell.action_id.spell_id).class_flags;
        }
        if spell.action_id.spell_id == 0 || !mask.matches(&flags) {
            continue;
        }
        affected.push(position);
        if spell.flags.matches(SpellFlag::CHANNELED) {
            channels.push(position);
        }
        if !spell.flags.matches(SpellFlag::CHANNELED | SpellFlag::PROC) {
            consuming.push(position);
        }
    }
    let heard = proc_trigger_spells(env, &listener());
    let eligible = |periodic: bool| -> Vec<usize> {
        heard
            .iter()
            .copied()
            .filter(|&position| can_trigger_from(env.sim.spell(spellbook[position]), periodic))
            .collect()
    };
    let mut effect = json!({
        "kind": "revelation", "trigger_aura": env.sim.aura(trigger).label,
        "ticks_trigger_aura": env.sim.aura(ticks).label, "aura": env.sim.aura(buff).label,
        "hit_spells": eligible(false), "tick_spells": eligible(true), "affected_spells": affected,
        "consuming_spells": consuming, "channel_spells": channels, "bonus_crit_percent": 100.0,
        "chance_at_no_crit": CHANCE_AT_NO_CRIT,
        "delay_ns": super::common_effects::SPELL_BATCH_WINDOW,
    });
    // Blizzard's hotfixes of 25 November 2026: "The Revelation weapon enchant no longer rolls its
    // proc chance for every target struck by an area of effect spell. It now rolls once per
    // wave." and "The Revelation enchant no longer affects the entire duration of channeled
    // spells." The Go reference keeps both.
    if env.sim.rules == Rules::Forever {
        effect["per_wave"] = json!(true);
    }
    Some(effect)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Client 1.60.1.70630 adds 32768 of the second mask word to the Mage buff under Forever's
    /// rules; every other class, and the Go reference, keep the 70425 masks.
    #[test]
    fn the_mage_buff_names_the_second_word_bit_only_under_forever() {
        let mask = |class, rules| class_buff(class, rules).unwrap().1;
        assert_eq!(mask("ClassMage", Rules::Forever)[1], 32768);
        assert_eq!(mask("ClassMage", Rules::GoReference)[1], 0);
        assert_eq!(
            mask("ClassMage", Rules::Forever)[0],
            mask("ClassMage", Rules::GoReference)[0]
        );
        for class in ["ClassPriest", "ClassWarlock", "ClassPaladin"] {
            assert_eq!(mask(class, Rules::Forever), mask(class, Rules::GoReference));
        }
    }
}
