//! The Druid class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{innervate, insect_swarm, moonfire, starfire, wrath},
    talents::{eclipse, natures_grace, omen_of_clarity},
};

/// What a Druid spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DruidSpell {
    MoonkinForm,
    Starfire,
    Wrath,
    Moonfire,
    MoonfireDot,
    InsectSwarm,
    Innervate,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DruidAura {
    Clearcasting,
    OmenOfClarity,
    NaturesGrace,
    NaturesGraceTrigger,
    Eclipse,
    EclipseTrigger,
    Innervate,
}

/// Druid state that Go keeps in the `Druid` struct and its closures.
#[derive(Default)]
pub(crate) struct DruidAgent {
    moonkin_form: Option<AuraRef>,
    moonfire_dot: Option<SpellId>,
    insect_swarm_debuff: Option<AuraRef>,
    innervate: Option<Rc<innervate::Innervate>>,
    omen: Option<Rc<omen_of_clarity::OmenOfClarity>>,
    natures_grace: Option<Rc<natures_grace::NaturesGrace>>,
    eclipse: Option<Rc<eclipse::Eclipse>>,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, DruidAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::OmenOfClarity {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), DruidAura::Clearcasting));
                auras.push((trigger_aura.clone(), DruidAura::OmenOfClarity));
            }
            Effect::NaturesGrace {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), DruidAura::NaturesGrace));
                auras.push((trigger_aura.clone(), DruidAura::NaturesGraceTrigger));
            }
            Effect::Eclipse {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), DruidAura::Eclipse));
                auras.push((trigger_aura.clone(), DruidAura::EclipseTrigger));
            }
            Effect::Innervate { aura, .. } => auras.push((aura.clone(), DruidAura::Innervate)),
            _ => {}
        }
    }
    auras
}

impl DruidAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell) -> Option<DruidSpell> {
        match spell.class_spell.as_deref()? {
            "moonkin_form" => Some(DruidSpell::MoonkinForm),
            "starfire" if spell.damage_effect.is_some() => Some(DruidSpell::Starfire),
            "wrath" if spell.damage_effect.is_some() => Some(DruidSpell::Wrath),
            "moonfire" if spell.damage_effect.is_some() => Some(DruidSpell::Moonfire),
            "moonfire_dot" if spell.dot.is_some() => Some(DruidSpell::MoonfireDot),
            "insect_swarm" if spell.dot.is_some() => Some(DruidSpell::InsectSwarm),
            "innervate" => Some(DruidSpell::Innervate),
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<DruidAgent>, String> {
        let auras = class_auras(prepared);
        let mut fight = Fight::new(
            prepared,
            DruidAgent::default(),
            DruidAgent::spell,
            |unit, label| {
                (unit == "player")
                    .then(|| {
                        auras
                            .iter()
                            .find(|(name, _)| name == label)
                            .map(|(_, kind)| *kind)
                    })
                    .flatten()
            },
        )?;
        let find_spell = |fight: &Fight<DruidAgent>, id: i32, tag: i32| {
            fight.spells.iter().position(|spell| {
                spell.id.spell_id == id && spell.id.tag == tag && spell.id.item_id == 0
            })
        };
        for effect in &prepared.effects {
            match effect {
                Effect::MoonkinForm { aura, .. } => {
                    fight.agent.moonkin_form = Some(fight.player_aura(aura)?);
                }
                Effect::Moonfire { rank } => {
                    let dot_spell = find_spell(&fight, rank.spell_id, 1)
                        .ok_or("Moonfire's dot spell is not registered")?;
                    let dot = fight.spells[dot_spell]
                        .dot
                        .ok_or("Moonfire's dot spell has no dot")?;
                    fight.dots[dot].tick_base = Some(rank.tick_base);
                    fight.dots[dot].tick_can_crit = rank.tick_can_crit;
                    fight.agent.moonfire_dot = Some(dot_spell);
                }
                Effect::InsectSwarm { rank, debuff_aura } => {
                    let spell = find_spell(&fight, rank.spell_id, 0)
                        .ok_or("Insect Swarm is not registered")?;
                    let dot = fight.spells[spell].dot.ok_or("Insect Swarm has no dot")?;
                    fight.dots[dot].tick_base = Some(rank.tick_base);
                    fight.dots[dot].tick_can_crit = rank.tick_can_crit;
                    fight.agent.insect_swarm_debuff = fight.trackers[Side::Target.index()]
                        .find(debuff_aura)
                        .map(|index| AuraRef {
                            side: Side::Target,
                            index,
                        });
                }
                Effect::Innervate {
                    aura,
                    spirit_regen_multiplier,
                    regen_metrics_action_id,
                    ..
                } => {
                    let bound = innervate::bind(
                        &mut fight,
                        aura,
                        *spirit_regen_multiplier,
                        regen_metrics_action_id,
                    )?;
                    fight.agent.innervate = Some(Rc::new(bound));
                }
                Effect::OmenOfClarity {
                    trigger_aura,
                    aura,
                    trigger_immediately,
                    proc_chance,
                    trigger_spells,
                    icd_ns,
                    ppm,
                    gcd_ns,
                    moonkin_chance_multiplier,
                    moonkin_cooldown_multiplier,
                    cost_spells,
                    cost_percent_add,
                    ..
                } => {
                    let moonkin = prepared.effects.iter().find_map(|effect| match effect {
                        Effect::MoonkinForm { aura, .. } => Some(aura.as_str()),
                        _ => None,
                    });
                    let bound = omen_of_clarity::bind(
                        &mut fight,
                        omen_of_clarity::Params {
                            aura,
                            trigger: trigger_aura,
                            trigger_spells,
                            cost_spells,
                            icd: *icd_ns,
                            ppm: *ppm,
                            gcd: *gcd_ns,
                            moonkin,
                            moonkin_chance: *moonkin_chance_multiplier,
                            moonkin_cooldown: *moonkin_cooldown_multiplier,
                            trigger_immediately: *trigger_immediately,
                            proc_chance: *proc_chance,
                            cost_percent_add: *cost_percent_add,
                        },
                    )?;
                    fight.agent.omen = Some(Rc::new(bound));
                }
                Effect::NaturesGrace {
                    aura,
                    haste_multiplier,
                    gcd_reduction_ns,
                    gcd_spells,
                    trigger_spells,
                    ..
                } => {
                    let bound = natures_grace::bind(
                        &mut fight,
                        aura,
                        *haste_multiplier,
                        *gcd_reduction_ns,
                        gcd_spells,
                        trigger_spells,
                    )?;
                    fight.agent.natures_grace = Some(Rc::new(bound));
                }
                Effect::Eclipse {
                    aura,
                    cast_time_reduction_ns,
                    charges_per_wrath,
                    ..
                } => {
                    let bound = eclipse::bind(
                        &mut fight,
                        aura,
                        *cast_time_reduction_ns,
                        *charges_per_wrath,
                    )?;
                    fight.agent.eclipse = Some(Rc::new(bound));
                }
                _ => {}
            }
        }
        Ok(fight)
    }

    fn innervate(fight: &Fight<Self>) -> Rc<innervate::Innervate> {
        fight.agent.innervate.clone().expect("Innervate is bound")
    }

    fn omen(fight: &Fight<Self>) -> Rc<omen_of_clarity::OmenOfClarity> {
        fight.agent.omen.clone().expect("Omen of Clarity is bound")
    }

    fn natures_grace(fight: &Fight<Self>) -> Rc<natures_grace::NaturesGrace> {
        fight
            .agent
            .natures_grace
            .clone()
            .expect("Nature's Grace is bound")
    }

    fn eclipse(fight: &Fight<Self>) -> Rc<eclipse::Eclipse> {
        fight.agent.eclipse.clone().expect("Eclipse is bound")
    }
}

impl Agent for DruidAgent {
    type Spell = DruidSpell;
    type Aura = DruidAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: DruidSpell) {
        match behavior {
            DruidSpell::MoonkinForm => {
                let aura = fight.agent.moonkin_form.expect("Moonkin Form is bound");
                fight.activate_aura(aura);
            }
            DruidSpell::Starfire => starfire::apply(fight, spell, target),
            DruidSpell::Wrath => wrath::apply(fight, spell, target),
            DruidSpell::Moonfire => {
                let dot_spell = fight.agent.moonfire_dot.expect("Moonfire is bound");
                moonfire::apply(fight, spell, target, dot_spell);
            }
            DruidSpell::MoonfireDot => moonfire::apply_dot(fight, spell, target),
            DruidSpell::InsectSwarm => insect_swarm::apply(fight, spell, target),
            DruidSpell::Innervate => {
                let aura = Self::innervate(fight).aura;
                fight.activate_aura(aura);
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: DruidSpell) -> bool {
        // The form check always passes: the gate admits only Moonkin druids casting spells
        // Moonkin Form allows.
        match behavior {
            DruidSpell::Innervate => Self::innervate(fight).can_cast(fight),
            _ => true,
        }
    }

    fn should_activate(_fight: &Fight<Self>, _spell: SpellId, behavior: DruidSpell) -> bool {
        // Go leaves Innervate to the rotation.
        behavior != DruidSpell::Innervate
    }

    fn on_dot_gain(fight: &mut Fight<Self>, _dot: DotId, behavior: DruidSpell) {
        if behavior == DruidSpell::InsectSwarm {
            insect_swarm::on_dot_gain(fight, fight.agent.insect_swarm_debuff);
        }
    }

    fn on_dot_expire(fight: &mut Fight<Self>, _dot: DotId, behavior: DruidSpell) {
        if behavior == DruidSpell::InsectSwarm {
            insect_swarm::on_dot_expire(fight, fight.agent.insect_swarm_debuff);
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: DruidSpell) {
        if matches!(behavior, DruidSpell::MoonfireDot | DruidSpell::InsectSwarm) {
            fight.snapshot_dot_tick(dot);
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura) {
        match kind {
            DruidAura::Clearcasting => Self::omen(fight).on_gain(fight),
            DruidAura::NaturesGrace => Self::natures_grace(fight).on_gain(fight),
            DruidAura::Eclipse => Self::eclipse(fight).on_gain(fight),
            DruidAura::Innervate => Self::innervate(fight).on_gain(fight),
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura) {
        match kind {
            DruidAura::Clearcasting => Self::omen(fight).on_expire(fight),
            DruidAura::NaturesGrace => Self::natures_grace(fight).on_expire(fight),
            DruidAura::Eclipse => Self::eclipse(fight).on_expire(fight),
            DruidAura::Innervate => Self::innervate(fight).on_expire(fight),
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: DruidAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            DruidAura::OmenOfClarity => Self::omen(fight).on_spell_hit_dealt(fight, spell, result),
            DruidAura::NaturesGraceTrigger => {
                Self::natures_grace(fight).on_spell_hit_dealt(fight, spell, result)
            }
            _ => {}
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: DruidAura,
        _spell: SpellId,
        _result: SpellResult,
    ) {
        if kind == DruidAura::OmenOfClarity {
            Self::omen(fight).on_delayed_proc(fight);
        }
    }

    fn on_cast_complete(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura, spell: SpellId) {
        match kind {
            DruidAura::Clearcasting => Self::omen(fight).on_cast_complete(fight, spell),
            DruidAura::EclipseTrigger => Self::eclipse(fight).on_cast_complete(fight, spell),
            _ => {}
        }
    }
}
