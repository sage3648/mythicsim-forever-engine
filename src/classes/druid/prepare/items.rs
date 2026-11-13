//! Go sim/druid/items.go and item_sets.go: the idols and the Wolfshead Helm the class wires,
//! and the item sets it registers.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger, PseudoStatField};
use crate::prepare::character::constants::SPELL_CRIT_RATING_PER_CRIT_PERCENT;
use crate::prepare::client_set_effects::DEMON;
use crate::prepare::env::Environment;
use crate::prepare::item_sets::{ClientItemSet, ItemSet};
use crate::prepare::sim::{AuraConfig, AuraId, Rules, Sim, UnitId, MILLISECOND, SECOND};
use crate::prepare::spell::ProcMask;
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::stats::{Stat, Stats};

use super::{masks, Druid};

/// The idols the druid package registers whose effect needs nothing of a druid (Go
/// `core.NewItemEffect` over `agent.GetCharacter()`): Idol of the Moon (23197), Idol of Ferocity
/// (22397), Idol of Brutality (23198), Howling Idol (272427) and Idol of the Dream (220606). A character of another class wears them as well, and
/// their spell mods find spells by class mask alone. Under Forever's rules the flat shapeshift
/// cost items join them (`shapeshift_cost_spell`). Answers whether the item is one of them.
pub(crate) fn apply_agentless_idol_effect(sim: &mut Sim, unit: UnitId, item: i32) -> bool {
    match item {
        23197 => {
            // Idol of the Moon.
            let aura = sim.register_aura(
                unit,
                AuraConfig {
                    label: "Improved Moonfire".to_string(),
                    ..AuraConfig::default()
                },
            );
            sim.attach_spell_mod(
                aura,
                SpellModConfig {
                    class_mask: masks::MOONFIRE,
                    kind: SpellModType::BaseDamageFlat,
                    float_value: 33.0,
                    ..SpellModConfig::default()
                },
            );
            sim.make_permanent(aura);
            true
        }
        22397 => {
            // Idol of Ferocity: reduces the energy cost of Claw and Rake by 2.
            sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::CLAW | masks::RAKE,
                    kind: SpellModType::PowerCostFlat,
                    int_value: -2,
                    ..SpellModConfig::default()
                },
            );
            true
        }
        23198 => {
            // Idol of Brutality: reduces the rage cost of Maul, Swipe and Primal Bite by 2.
            sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::MAUL | masks::SWIPE | masks::PRIMAL_BITE,
                    kind: SpellModType::PowerCostFlat,
                    int_value: -2,
                    ..SpellModConfig::default()
                },
            );
            true
        }
        272427 => {
            // Howling Idol: "Reduces the cooldown of your Shifting Power ability by 0.5 sec."
            // (1291059: A_ADD_FLAT_MODIFIER, SPELLMOD_COOLDOWN on Shifting Power's class mask).
            // Stacks with Improved Shifting Power. Go `HowlingIdolFromRow` (patch 179): the
            // reduction is the row's, -500 ms at client 70425, not patch 103's -1000 ms.
            let row = crate::prepare::spelldata::store::must_find(1291059);
            sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::SHIFTING_POWER,
                    kind: SpellModType::CooldownFlat,
                    time_value: (row.effect_n(1).base_value() as i64) * MILLISECOND,
                    ..SpellModConfig::default()
                },
            );
            true
        }
        220606 => {
            // Idol of the Dream: Improved Rip (446212, client 1.60.1.70532: A_ADD_FLAT_MODIFIER,
            // SPELLMOD_DURATION on Rip's class mask, +2000 ms). Rip lasts 2 seconds longer with
            // the same tick period and tick damage, so it ticks once more. Ported from
            // wowsims/forever 8d879a401; Go's pinned engine registers no effect for it.
            let row = crate::prepare::spelldata::store::must_find(446212);
            sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::RIP,
                    kind: SpellModType::DurationFlat,
                    time_value: (row.effect_n(1).base_value() as i64) * MILLISECOND,
                    ..SpellModConfig::default()
                },
            );
            true
        }
        _ => apply_shapeshift_cost_item(sim, unit, item),
    }
}

/// The spell of a Forever druid item that lowers the mana cost of shapeshifting by a flat amount:
/// the Feral PvP grips, Shapeshift Cost Reduction (459594, "Reduces the mana cost of your
/// shapeshifts by 150"), and Idol of Shifting Tides, Clever Shapeshifter (1270470, "Reduces the
/// mana cost of shapeshifting spells by 40"). The item to spell links are the client's
/// ItemXItemEffect rows (wago.tools wowf, 4 November 2026). Go's pinned engine registers neither,
/// so only Forever's rules apply them.
pub(crate) fn shapeshift_cost_spell(item: i32) -> Option<i32> {
    match item {
        // Blood Guard's and Knight-Lieutenant's Dragonhide Grips, General's and Marshal's
        // Dragonhide Grips.
        227180 | 227183 | 231688 | 231694 => Some(459594),
        // Idol of Shifting Tides.
        263411 => Some(1270470),
        _ => None,
    }
}

/// A flat shapeshift cost item under Forever's rules (`shapeshift_cost_spell`). Both spells are an
/// `A_ADD_FLAT_MODIFIER` of `SPELLMOD_COST` on the shapeshift class mask Natural Shapeshifter
/// reads (word 0 bits 29 to 31, the idol also word 1 bit 16), which covers Cat, Bear and Moonkin
/// Form and Shifting Power, whose tooltip says its cost "is reduced by effects that reduce the
/// cost of Shapeshifting". A flat cost modifier comes off the base cost before the percentage
/// ones (`finalize_spell`), as the client applies spell mods: with Natural Shapeshifter 3/3 the
/// grips save 150 x 0.7 = 105 mana a shift.
fn apply_shapeshift_cost_item(sim: &mut Sim, unit: UnitId, item: i32) -> bool {
    if sim.rules != Rules::Forever {
        return false;
    }
    let Some(spell) = shapeshift_cost_spell(item) else {
        return false;
    };
    let row = crate::prepare::spelldata::store::must_find(spell);
    sim.add_static_mod(
        unit,
        SpellModConfig {
            class_mask: masks::CAT_FORM
                | masks::BEAR_FORM
                | masks::MOONKIN_FORM
                | masks::SHIFTING_POWER,
            kind: SpellModType::PowerCostFlat,
            int_value: row.effect_n(1).base_value() as i32,
            ..SpellModConfig::default()
        },
    );
    true
}

impl Druid {
    /// The `core.NewItemEffect` registrations of Go's druid package: the idols above and Wolfshead
    /// Helm (8345), which reads the druid agent. Answers whether the class registers an effect for
    /// the item.
    pub(super) fn apply_idol_effect(&mut self, sim: &mut Sim, unit: UnitId, item: i32) -> bool {
        if apply_agentless_idol_effect(sim, unit, item) {
            return true;
        }
        match item {
            8345 => {
                // Wolfshead Helm: the bonus belongs to Shifting Power and Enrage. The live helm
                // gives 5 Energy from Shifting Power (Wowhead Forever's tooltip of 8345 and its
                // spell 17768: "an additional 5 Rage from activating Enrage and an additional 5
                // Energy from activating Shifting Power"); the Go reference keeps 20.
                let energy_gain = if sim.rules == Rules::Forever {
                    5.0
                } else {
                    20.0
                };
                let gain = Rc::clone(&self.st);
                let expire = Rc::clone(&self.st);
                let aura = sim.register_aura(
                    unit,
                    AuraConfig {
                        label: "Wolfshead Helm".to_string(),
                        action_id: Some(ActionId::spell(17768)),
                        on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                            let energy = gain.wolfshead_shifting_power_energy;
                            sim.set_state(energy, sim.state(energy) + energy_gain);
                            let rage = gain.wolfshead_enrage_rage;
                            sim.set_state(rage, sim.state(rage) + 5.0);
                        })),
                        on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                            let energy = expire.wolfshead_shifting_power_energy;
                            sim.set_state(energy, sim.state(energy) - energy_gain);
                            let rage = expire.wolfshead_enrage_rage;
                            sim.set_state(rage, sim.state(rage) - 5.0);
                        })),
                        ..AuraConfig::default()
                    },
                );
                sim.make_permanent(aura);
                true
            }
            _ => false,
        }
    }
}

fn stats(pairs: &[(Stat, f64)]) -> Stats {
    Stats::from_pairs(pairs)
}

fn feralheart_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[
            (Stat::ArcaneResistance, 8.0),
            (Stat::FireResistance, 8.0),
            (Stat::FrostResistance, 8.0),
            (Stat::NatureResistance, 8.0),
            (Stat::ShadowResistance, 8.0),
        ]),
    );
}

fn feralheart_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::MP5, 8.0);
}

/// Nature's Bounty: 2% chance to restore 200 mana on spell cast, 20 energy on a white hit, or
/// 10 rage when struck.
fn feralheart_4(env: &mut Environment, aura: AuraId) {
    let action = ActionId::spell(450608);
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action.clone(),
            name: "Nature's Bounty (Mana)".to_string(),
            callback: CallbackMask::ON_CAST_COMPLETE,
            proc_mask: ProcMask::SPELL_DAMAGE | ProcMask::SPELL_HEALING,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action.clone(),
            name: "Nature's Bounty (Energy)".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::LANDED,
            proc_mask: ProcMask::MELEE_WHITE_HIT,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: action,
            name: "Nature's Bounty (Rage)".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            proc_mask: ProcMask::MELEE,
            proc_chance: 0.02,
            ..ProcTrigger::default()
        },
    );
}

fn nothing(_: &mut Environment, _: AuraId) {}

fn feralheart_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[
            (Stat::SpellDamage, 23.0),
            (Stat::HealingPower, 23.0),
            (Stat::AttackPower, 40.0),
            (Stat::RangedAttackPower, 40.0),
        ]),
    );
}

fn cenarion_5(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(
        aura,
        Stat::SpellCritRating,
        2.0 * SPELL_CRIT_RATING_PER_CRIT_PERCENT,
    );
}

fn stormrage_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_additive_pseudo_stat_buff(
        aura,
        PseudoStatField::Custom(|p| &mut p.spirit_regen_rate_casting),
        0.15,
    );
}

fn symbols_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            name: "Symbols of Unending Life Finisher Bonus".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            outcome: HitOutcome::MISS | HitOutcome::DODGE | HitOutcome::BLOCK | HitOutcome::PARRY,
            class_spell_mask: masks::FEROCIOUS_BITE | masks::RIP,
            ..ProcTrigger::default()
        },
    );
}

fn refuge_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[(Stat::HealingPower, 44.0), (Stat::SpellDamage, 15.0)]),
    );
}

fn stamina_20(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(aura, Stat::Stamina, 20.0);
}

fn sanctuary_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        stats(&[(Stat::AttackPower, 40.0), (Stat::RangedAttackPower, 40.0)]),
    );
}

/// Go's druid `core.NewItemSet` calls, in Go's registration order.
pub(crate) static ITEM_SETS: &[ItemSet] = &[
    ItemSet {
        id: 0,
        name: "Feralheart Raiment",
        alternative_name: "",
        bonuses: &[
            (2, feralheart_2),
            (3, feralheart_3),
            (4, feralheart_4),
            (5, nothing),
            (6, feralheart_6),
        ],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Cenarion Raiment",
        alternative_name: "",
        bonuses: &[(3, nothing), (5, cenarion_5), (8, nothing)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Stormrage Raiment",
        alternative_name: "",
        bonuses: &[(3, stormrage_3), (5, nothing), (8, nothing)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Symbols of Unending Life",
        alternative_name: "",
        bonuses: &[(3, symbols_3)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Champion's Refuge",
        alternative_name: "",
        bonuses: &[(2, refuge_2), (4, nothing), (6, stamina_20)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Lieutenant Commander's Refuge",
        alternative_name: "",
        bonuses: &[(2, refuge_2), (4, nothing), (6, stamina_20)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Field Marshal's Sanctuary",
        alternative_name: "",
        bonuses: &[(2, stamina_20), (3, nothing), (6, sanctuary_6)],
        required_profession: "",
    },
    ItemSet {
        id: 0,
        name: "Warlord's Sanctuary",
        alternative_name: "",
        bonuses: &[(2, stamina_20), (3, nothing), (6, sanctuary_6)],
        required_profession: "",
    },
];

/// Grovekeeper Rage (Guardian, ItemSet 2113: Leggings 273896, Gauntlets 273897, Spaulders 273898,
/// Treads 273899), Grovekeeper Eclipse (Balance, ItemSet 2114: Trousers 273884, Grips 273885,
/// Shoulderpads 273886, Stompers 273887) and Grovekeeper Ferocity (Feral, ItemSet 2115: Kilt
/// 273888, Mitts 273889, Mantle 273890, Sandals 273891), the Druid's damage and tank Tier 1 of the
/// Forever client, which the reference does not register. The bonuses are the ItemSetSpell
/// spells of `data/spells.jsonl` (client 1.60.1.70425), beside the line the Wowhead Forever
/// tooltips gave them on 13 November 2026.
///
/// Grovekeeper Rage:
///
/// - (2) "Increased Defense +7." Spell 1300942: a skill aura (30) of 7 on Defense (skill 95).
/// - (3) "Your Rebirth spell is now castable in Bear Form and Dire Bear Form and its cast time is
///   reduced by 2.0 sec while you are in Bear Form or Dire Bear Form." Spell 1301005: a cast time
///   mod (107, misc 10) of -2000 ms and an aura (275) that lets Rebirth be cast in the form, on
///   Rebirth (20484 to 20747). Applied with no effect: nothing registers Rebirth (a resurrection,
///   which no rotation casts).
/// - (4) "Reduces the chance for your melee attacks to be Dodged or Parried by 1.2%." Spell
///   1301076: a rating aura (189) of 12 on Expertise (mask 8388608).
/// - (5) "Reduces the cooldown of your Berserk ability by 15.0 sec." Spell 1301249: a cooldown
///   mod (107, misc 11) of -15000 ms on mask [0, 0, 64] of family 7, which is Berserk (417141).
///   The Feral Berserk this engine registers (`apply_berserk`, a 180 sec cooldown) carries no
///   class spell mask, and a new mask would be a new class spell name in the prepared export, so
///   no mod is attached: `apply_berserk` reads the bonus's status aura (`berserk_cooldown`) and
///   registers Berserk with 165 sec. The bonus does nothing without the Berserk talent, which
///   registers no spell. Hotfix push 113531 removed Berserk from Forever's Druid tree, and the
///   client sets exist under Forever's rules alone, so no request registers Berserk today and the
///   bonus is its status aura until the talent returns.
///
/// Grovekeeper Eclipse:
///
/// - (2) "Improves your chance to hit by 1.0%." Spell 1300940: a melee and a spell hit aura of 1.
/// - (3) "Reduces the cooldown of your Nature's Grasp spell by 15 sec." Spell 1300975: a cooldown
///   mod (107, misc 11) of -15000 ms on Nature's Grasp (16689 to 16812). Applied with no effect:
///   nothing registers Nature's Grasp (a root, which no rotation casts).
/// - (4) "Increases damage done by your spells and effects by up to 21 when fighting Demons."
///   Spell 1301073: spell damage versus a creature type (59, mask 4) of 21.
/// - (5) "Increases the duration of your Insect Swarm spell by 3 sec." Spell 1301242: a duration
///   mod (107, misc 1) of +3000 ms on mask 2097152 of family 7, which is Insect Swarm (5570 to
///   24977). Insect Swarm ticks every 2 sec for 12, so the 15 sec is 7.5 ticks, which the dot
///   rounds to even: 8 ticks. Whether the game ticks 7 or 8 times is not stated by the row or
///   the tooltip.
///
/// Grovekeeper Ferocity:
///
/// - (2) "Increases your attack speed and casting speed by 1%." Spell 1300934: a melee and ranged
///   haste aura (342) and a casting speed aura (65) of 1.
/// - (3) "Your Hibernate spell is now castable in Cat Form and its cast time is reduced by 1.5
///   sec while you are in Cat Form, but Hibernate gains a cooldown of 15 sec." Spell 1300999: a
///   cast time mod (107, misc 10) of -1500 ms, a cooldown mod (misc 11) of +15000 ms and an aura
///   (275), on Hibernate (2637, 18657, 18658). Applied with no effect: nothing registers
///   Hibernate (a crowd control spell, which no rotation casts).
/// - (4) "+36 Attack Power against Demons." Spell 1301075: melee attack power versus a creature
///   type (102, mask 4) of 36.
/// - (5) "Reduces the cooldown of your Shifting Power ability by 1.0 sec." Spell 1301247: a
///   cooldown mod (107, misc 11) of -1000 ms on mask [0, 2147483648] of family 7, which is
///   Shifting Power (1322605, 8 sec after hotfix push 113531 under Forever's rules).
pub(crate) static CLIENT_ITEM_SETS: &[ClientItemSet] = &[
    ClientItemSet {
        set: ItemSet {
            id: 2113,
            name: "Grovekeeper Rage",
            alternative_name: "",
            bonuses: &[
                (2, grovekeeper_rage_2),
                (3, grovekeeper_rage_3),
                (4, grovekeeper_rage_4),
                (5, grovekeeper_rage_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
    ClientItemSet {
        set: ItemSet {
            id: 2114,
            name: "Grovekeeper Eclipse",
            alternative_name: "",
            bonuses: &[
                (2, grovekeeper_eclipse_2),
                (3, grovekeeper_eclipse_3),
                (4, grovekeeper_eclipse_4),
                (5, grovekeeper_eclipse_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
    ClientItemSet {
        set: ItemSet {
            id: 2115,
            name: "Grovekeeper Ferocity",
            alternative_name: "",
            bonuses: &[
                (2, grovekeeper_ferocity_2),
                (3, grovekeeper_ferocity_3),
                (4, grovekeeper_ferocity_4),
                (5, grovekeeper_ferocity_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
];

/// Tier 1 Guardian: increased Defense +7.
fn grovekeeper_rage_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_defense(aura, 7.0);
    env.sim.expose_to_apl(aura, 1300942);
}

/// Tier 1 Guardian: your Rebirth spell is now castable in Bear Form and Dire Bear Form and its
/// cast time is reduced by 2.0 sec while you are in Bear Form or Dire Bear Form. Rebirth is not a
/// spell this engine registers, so the bonus is the status aura alone.
fn grovekeeper_rage_3(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1301005);
}

/// Tier 1 Guardian: reduces the chance for your melee attacks to be Dodged or Parried by 1.2%.
fn grovekeeper_rage_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_expertise(aura);
    env.sim.expose_to_apl(aura, 1301076);
}

/// The label of the Rage 5 piece bonus's status aura (`make_set_bonus_status_aura`), which
/// [`berserk_cooldown`] reads.
const GROVEKEEPER_RAGE_5_AURA: &str = "Grovekeeper Rage 5P";

/// Berserk's cooldown: 180 sec, less the 15 sec (the row's -15000 ms) of the Grovekeeper Rage 5
/// piece bonus when the druid wears it. `apply_berserk` registers the spell with it.
pub(super) fn berserk_cooldown(sim: &Sim, unit: UnitId) -> i64 {
    let bonus = sim.get_aura(unit, GROVEKEEPER_RAGE_5_AURA).is_some();
    (180 - if bonus { 15 } else { 0 }) * SECOND
}

/// Tier 1 Guardian: reduces the cooldown of your Berserk ability by 15.0 sec. The status aura
/// marks it; `apply_berserk` shortens Berserk's cooldown when it registers the spell.
fn grovekeeper_rage_5(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1301249);
}

/// Tier 1 Balance: improves your chance to hit by 1.0%.
fn grovekeeper_eclipse_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_hit(aura);
    env.sim.expose_to_apl(aura, 1300940);
}

/// Tier 1 Balance: reduces the cooldown of your Nature's Grasp spell by 15 sec. Nature's Grasp is
/// not a spell this engine registers, so the bonus is the status aura alone.
fn grovekeeper_eclipse_3(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1300975);
}

/// Tier 1 Balance: increases damage done by your spells and effects by up to 21 when fighting
/// Demons.
fn grovekeeper_eclipse_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_versus_spell_damage(aura, DEMON, 21.0);
    env.sim.expose_to_apl(aura, 1301073);
}

/// Tier 1 Balance: increases the duration of your Insect Swarm spell by 3 sec.
fn grovekeeper_eclipse_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            class_mask: masks::INSECT_SWARM,
            kind: SpellModType::DurationFlat,
            time_value: 3 * SECOND,
            ..SpellModConfig::default()
        },
    );
    sim.expose_to_apl(aura, 1301242);
}

/// Tier 1 Feral: increases your attack speed and casting speed by 1%.
fn grovekeeper_ferocity_2(env: &mut Environment, aura: AuraId) {
    env.sim.attach_set_haste(aura);
    env.sim.expose_to_apl(aura, 1300934);
}

/// Tier 1 Feral: your Hibernate spell is now castable in Cat Form and its cast time is reduced by
/// 1.5 sec while you are in Cat Form, but Hibernate gains a cooldown of 15 sec. Hibernate is not
/// a spell this engine registers, so the bonus is the status aura alone.
fn grovekeeper_ferocity_3(env: &mut Environment, aura: AuraId) {
    env.sim.expose_to_apl(aura, 1300999);
}

/// Tier 1 Feral: +36 Attack Power against Demons.
fn grovekeeper_ferocity_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_versus_attack_power(aura, DEMON, 36.0);
    env.sim.expose_to_apl(aura, 1301075);
}

/// Tier 1 Feral: reduces the cooldown of your Shifting Power ability by 1.0 sec.
fn grovekeeper_ferocity_5(env: &mut Environment, aura: AuraId) {
    let sim = &mut env.sim;
    sim.attach_spell_mod(
        aura,
        SpellModConfig {
            class_mask: masks::SHIFTING_POWER,
            kind: SpellModType::CooldownFlat,
            time_value: -SECOND,
            ..SpellModConfig::default()
        },
    );
    sim.expose_to_apl(aura, 1301247);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::request::Request;
    use crate::prepare::item_sets::EXTRA_PIECES;

    const REQUEST: &str =
        include_str!("../../../../fixtures/forever-gaps/feral-bear-druid-thorns.request.json");

    fn environment(pieces: usize) -> Environment {
        EXTRA_PIECES.with(|extra| {
            *extra.borrow_mut() = (0..pieces)
                .map(|_| (2113, "Grovekeeper Rage".to_string(), 0))
                .collect();
        });
        let mut value: serde_json::Value = serde_json::from_str(REQUEST).expect("a request");
        value["raid"]["parties"][0]["players"][0]["talentsString"] =
            serde_json::json!("-5500033212310212551-055");
        let bytes = serde_json::to_vec(&value).expect("json");
        let request = Request::from_json(&bytes).expect("parsed");
        let env = Environment::with_rules(
            request.message(),
            crate::classes::prepare_agent,
            Rules::Forever,
        );
        EXTRA_PIECES.with(|extra| extra.borrow_mut().clear());
        env.expect("prepared")
    }

    /// The 5 piece bonus takes 15 sec off Berserk's 180, and four pieces take none.
    #[test]
    fn the_five_piece_bonus_shortens_berserks_cooldown() {
        let (four, five) = (environment(4), environment(5));
        assert_eq!(berserk_cooldown(&four.sim, four.player), 180 * SECOND);
        assert_eq!(berserk_cooldown(&five.sim, five.player), 165 * SECOND);
    }
}
