//! The Priest class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{dark_sacrifice, devouring_plague, direct, holy, periodic, shadowform},
    talents::{inner_focus, power_in_light, searing_light, shadow_weaving},
};

/// What a Priest spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PriestSpell {
    MindBlast,
    ShadowWordDeath,
    ShadowWordPain,
    DevouringPlague,
    MindFlay,
    Shadowform,
    InnerFocus,
    DarkSacrifice,
    Smite,
    HolyFire,
    Penance,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PriestAura {
    Shadowform,
    InnerFocus,
    ShadowWeaving,
    ShadowWeavingTrigger,
    SearingLight,
    SearingLightTrigger,
}

/// Priest state that Go keeps in the `Priest` struct and its closures.
#[derive(Default)]
pub(crate) struct PriestAgent {
    early_demise_crit: f64,
    /// Devouring Plague ranks by spell, with their health metrics.
    devouring_plague: Vec<(SpellId, usize)>,
    shadowform: Option<Rc<shadowform::Shadowform>>,
    inner_focus: Option<Rc<inner_focus::InnerFocus>>,
    shadow_weaving: Option<Rc<shadow_weaving::ShadowWeaving>>,
    dark_sacrifice: Option<Rc<dark_sacrifice::DarkSacrifice>>,
    searing_light: Option<Rc<searing_light::SearingLight>>,
}

/// Player aura labels claimed by implemented class effects.
fn class_auras(prepared: &PreparedV2) -> Vec<(String, PriestAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::Shadowform { aura, .. } => auras.push((aura.clone(), PriestAura::Shadowform)),
            Effect::InnerFocus { aura, .. } => auras.push((aura.clone(), PriestAura::InnerFocus)),
            Effect::ShadowWeaving {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), PriestAura::ShadowWeaving));
                auras.push((trigger_aura.clone(), PriestAura::ShadowWeavingTrigger));
            }
            Effect::SearingLight {
                aura, trigger_aura, ..
            } => {
                auras.push((aura.clone(), PriestAura::SearingLight));
                auras.push((trigger_aura.clone(), PriestAura::SearingLightTrigger));
            }
            _ => {}
        }
    }
    auras
}

/// The spell IDs of class spells Go registers without a class mask.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct UnmaskedSpells {
    pub(crate) inner_focus: Option<i32>,
    pub(crate) dark_sacrifice: Option<i32>,
}

impl UnmaskedSpells {
    pub(crate) fn of(effects: &[Effect]) -> Self {
        let mut ids = UnmaskedSpells::default();
        for effect in effects {
            match effect {
                Effect::InnerFocus { spell_id, .. } => ids.inner_focus = Some(*spell_id),
                Effect::DarkSacrifice { spell_id, .. } => ids.dark_sacrifice = Some(*spell_id),
                _ => {}
            }
        }
        ids
    }
}

impl PriestAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell, unmasked: UnmaskedSpells) -> Option<PriestSpell> {
        let id = spell.action_id.clone().unwrap_or_default();
        let Some(class) = spell.class_spell.as_deref() else {
            let own = |known: Option<i32>| known == Some(id.spell_id) && id.tag == 0;
            return if own(unmasked.inner_focus) {
                Some(PriestSpell::InnerFocus)
            } else if own(unmasked.dark_sacrifice) && spell.dot.is_some() {
                Some(PriestSpell::DarkSacrifice)
            } else {
                None
            };
        };
        match class {
            "mind_blast" if spell.damage_effect.is_some() => Some(PriestSpell::MindBlast),
            "shadow_word_death" if spell.damage_effect.is_some() => {
                Some(PriestSpell::ShadowWordDeath)
            }
            "shadow_word_pain" if spell.dot.is_some() => Some(PriestSpell::ShadowWordPain),
            "devouring_plague" if spell.dot.is_some() => Some(PriestSpell::DevouringPlague),
            "mind_flay" if spell.dot.as_ref().is_some_and(|dot| dot.channeled) => {
                Some(PriestSpell::MindFlay)
            }
            "shadowform" => Some(PriestSpell::Shadowform),
            "smite" if spell.damage_effect.is_some() => Some(PriestSpell::Smite),
            "holy_fire" if spell.damage_effect.is_some() && spell.dot.is_some() => {
                Some(PriestSpell::HolyFire)
            }
            "penance" if spell.dot.as_ref().is_some_and(|dot| dot.channeled) => {
                Some(PriestSpell::Penance)
            }
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<PriestAgent>, String> {
        let auras = class_auras(prepared);
        let unmasked = UnmaskedSpells::of(&prepared.effects);
        let mut fight = Fight::new(
            prepared,
            PriestAgent::default(),
            |spell| PriestAgent::spell(spell, unmasked),
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
        let find_spell = |fight: &Fight<PriestAgent>, id: i32| {
            fight.spells.iter().position(|spell| {
                spell.id.spell_id == id && spell.id.tag == 0 && spell.id.item_id == 0
            })
        };
        let set_ticks =
            |fight: &mut Fight<PriestAgent>,
             ranks: &[crate::contracts::prepared_v2::FireballRank]| {
                for rank in ranks {
                    let dot = fight
                        .spells
                        .iter()
                        .find(|spell| spell.id.spell_id == rank.spell_id && spell.id.tag == 0)
                        .and_then(|spell| spell.dot);
                    if let Some(dot) = dot {
                        fight.dots[dot].tick_base = Some(rank.tick_base);
                        fight.dots[dot].tick_can_crit = rank.tick_can_crit;
                    }
                }
            };
        for effect in &prepared.effects {
            match effect {
                Effect::ShadowWordDeath { early_demise_crit } => {
                    fight.agent.early_demise_crit = *early_demise_crit;
                }
                Effect::ShadowWordPain { ranks }
                | Effect::MindFlay { ranks }
                | Effect::HolyFire { ranks } => {
                    set_ticks(&mut fight, ranks);
                }
                Effect::Penance {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                } => {
                    let rank = crate::contracts::prepared_v2::FireballRank {
                        spell_id: *spell_id,
                        tick_base: *tick_base,
                        tick_can_crit: *tick_can_crit,
                    };
                    set_ticks(&mut fight, &[rank]);
                }
                Effect::PowerInLight {
                    multiplier,
                    spells,
                    holy_fire_spells,
                } => power_in_light::bind(&mut fight, *multiplier, spells, holy_fire_spells)?,
                Effect::SearingLight {
                    trigger_aura,
                    aura,
                    trigger_immediately,
                    proc_chance,
                    trigger_spells,
                    cost_percent_add,
                    cost_spells,
                    cancel_spells,
                    ..
                } => {
                    let bound = searing_light::bind(
                        &mut fight,
                        aura,
                        trigger_aura,
                        *proc_chance,
                        *trigger_immediately,
                        trigger_spells,
                        *cost_percent_add,
                        cost_spells,
                        cancel_spells,
                    )?;
                    fight.agent.searing_light = Some(Rc::new(bound));
                }
                Effect::DevouringPlague {
                    ranks,
                    heal_metrics_tag,
                } => {
                    set_ticks(&mut fight, ranks);
                    // Go registers each rank's health metrics with the spell.
                    for rank in ranks {
                        if let Some(spell) = find_spell(&fight, rank.spell_id) {
                            let mut id = fight.spells[spell].id.clone();
                            id.tag = *heal_metrics_tag;
                            let metrics = fight.new_health_metrics(id);
                            fight.agent.devouring_plague.push((spell, metrics));
                        }
                    }
                }
                Effect::Shadowform {
                    aura,
                    damage_percent,
                    cost_percent,
                    crit_multiplier,
                    school_spells,
                    crit_spells,
                    cancel_spells,
                    ..
                } => {
                    let bound = shadowform::bind(
                        &mut fight,
                        aura,
                        *damage_percent,
                        *cost_percent,
                        *crit_multiplier,
                        school_spells,
                        crit_spells,
                        cancel_spells,
                    )?;
                    fight.agent.shadowform = Some(Rc::new(bound));
                }
                Effect::InnerFocus {
                    spell_id,
                    aura,
                    cost_percent,
                    crit_percent,
                    crit_spells,
                    spender_spells,
                } => {
                    let spell = find_spell(&fight, *spell_id)
                        .ok_or_else(|| format!("Inner Focus {spell_id} is not registered"))?;
                    let bound = inner_focus::bind(
                        &mut fight,
                        spell,
                        aura,
                        *cost_percent,
                        *crit_percent,
                        crit_spells,
                        spender_spells,
                    )?;
                    fight.agent.inner_focus = Some(Rc::new(bound));
                }
                Effect::ShadowWeaving {
                    trigger_aura,
                    aura,
                    trigger_immediately,
                    proc_chance,
                    trigger_spells,
                    damage_per_stack,
                    damage_spells,
                    ..
                } => {
                    let bound = shadow_weaving::bind(
                        &mut fight,
                        aura,
                        trigger_aura,
                        *proc_chance,
                        *trigger_immediately,
                        trigger_spells,
                        *damage_per_stack,
                        damage_spells,
                    )?;
                    fight.agent.shadow_weaving = Some(Rc::new(bound));
                }
                Effect::DarkSacrifice {
                    spell_id,
                    tick_base,
                    spirit_divisor,
                    metrics_action_id,
                    ..
                } => {
                    let spell = find_spell(&fight, *spell_id)
                        .ok_or_else(|| format!("Dark Sacrifice {spell_id} is not registered"))?;
                    let spirit = prepared
                        .player
                        .stats
                        .get("Spirit")
                        .copied()
                        .ok_or("prepared stats lack Spirit")?;
                    let bound = dark_sacrifice::bind(
                        &mut fight,
                        spell,
                        *tick_base,
                        spirit,
                        *spirit_divisor,
                        metrics_action_id,
                    )?;
                    fight.agent.dark_sacrifice = Some(Rc::new(bound));
                }
                _ => {}
            }
        }
        Ok(fight)
    }

    fn shadowform(fight: &Fight<Self>) -> Rc<shadowform::Shadowform> {
        fight.agent.shadowform.clone().expect("Shadowform is bound")
    }

    fn inner_focus(fight: &Fight<Self>) -> Rc<inner_focus::InnerFocus> {
        fight
            .agent
            .inner_focus
            .clone()
            .expect("Inner Focus is bound")
    }

    fn shadow_weaving(fight: &Fight<Self>) -> Rc<shadow_weaving::ShadowWeaving> {
        fight
            .agent
            .shadow_weaving
            .clone()
            .expect("Shadow Weaving is bound")
    }

    fn searing_light(fight: &Fight<Self>) -> Rc<searing_light::SearingLight> {
        fight
            .agent
            .searing_light
            .clone()
            .expect("Searing Light is bound")
    }

    fn dark_sacrifice(fight: &Fight<Self>) -> Rc<dark_sacrifice::DarkSacrifice> {
        fight
            .agent
            .dark_sacrifice
            .clone()
            .expect("Dark Sacrifice is bound")
    }
}

impl Agent for PriestAgent {
    type Spell = PriestSpell;
    type Aura = PriestAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: PriestSpell) {
        match behavior {
            PriestSpell::MindBlast | PriestSpell::Smite => direct::apply(fight, spell, target),
            PriestSpell::HolyFire => holy::apply_holy_fire(fight, spell, target),
            PriestSpell::Penance => holy::apply_penance(fight, spell, target),
            PriestSpell::ShadowWordDeath => {
                let crit = fight.agent.early_demise_crit;
                direct::apply_shadow_word_death(fight, spell, target, crit);
            }
            PriestSpell::ShadowWordPain | PriestSpell::DevouringPlague | PriestSpell::MindFlay => {
                periodic::apply(fight, spell, target)
            }
            PriestSpell::Shadowform => {
                let aura = Self::shadowform(fight).aura;
                fight.activate_aura(aura);
            }
            PriestSpell::InnerFocus => {
                let aura = Self::inner_focus(fight).aura;
                fight.activate_aura(aura);
            }
            PriestSpell::DarkSacrifice => Self::dark_sacrifice(fight).apply(fight),
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: PriestSpell) -> bool {
        match behavior {
            PriestSpell::DarkSacrifice => Self::dark_sacrifice(fight).should_activate(fight),
            _ => true,
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: PriestSpell) {
        match behavior {
            PriestSpell::ShadowWordPain
            | PriestSpell::MindFlay
            | PriestSpell::HolyFire
            | PriestSpell::Penance => {
                fight.snapshot_dot_tick(dot);
            }
            PriestSpell::DevouringPlague => {
                let spell = fight.dots[dot].spell;
                let metrics = fight
                    .agent
                    .devouring_plague
                    .iter()
                    .find(|(rank, _)| *rank == spell)
                    .map(|(_, metrics)| *metrics)
                    .expect("every Devouring Plague rank has health metrics");
                devouring_plague::tick(fight, dot, metrics);
            }
            PriestSpell::DarkSacrifice => Self::dark_sacrifice(fight).tick(fight),
            _ => {}
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: PriestAura) {
        match kind {
            PriestAura::Shadowform => Self::shadowform(fight).on_gain(fight),
            PriestAura::InnerFocus => Self::inner_focus(fight).on_gain(fight),
            PriestAura::ShadowWeaving => Self::shadow_weaving(fight).on_gain(fight),
            PriestAura::SearingLight => Self::searing_light(fight).on_gain(fight),
            PriestAura::ShadowWeavingTrigger | PriestAura::SearingLightTrigger => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: PriestAura) {
        match kind {
            PriestAura::Shadowform => Self::shadowform(fight).on_expire(fight),
            PriestAura::InnerFocus => Self::inner_focus(fight).on_expire(fight),
            PriestAura::ShadowWeaving => Self::shadow_weaving(fight).on_expire(fight),
            PriestAura::SearingLight => Self::searing_light(fight).on_expire(fight),
            PriestAura::ShadowWeavingTrigger | PriestAura::SearingLightTrigger => {}
        }
    }

    fn on_stacks_change(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PriestAura,
        _old: i32,
        new: i32,
    ) {
        if kind == PriestAura::ShadowWeaving {
            Self::shadow_weaving(fight).on_stacks_change(fight, new);
        }
    }

    fn on_cast_complete(fight: &mut Fight<Self>, _aura: AuraRef, kind: PriestAura, spell: SpellId) {
        match kind {
            PriestAura::Shadowform => Self::shadowform(fight).on_cast_complete(fight, spell),
            PriestAura::InnerFocus => Self::inner_focus(fight).on_cast_complete(fight, spell),
            PriestAura::SearingLight => Self::searing_light(fight).on_cast_complete(fight, spell),
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PriestAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if kind == PriestAura::ShadowWeavingTrigger {
            Self::shadow_weaving(fight).on_spell_hit_dealt(fight, spell, result);
        }
    }

    fn on_periodic_damage_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PriestAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if kind == PriestAura::SearingLightTrigger {
            Self::searing_light(fight).on_periodic_damage_dealt(fight, spell, result);
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PriestAura,
        _spell: SpellId,
        _result: SpellResult,
    ) {
        match kind {
            PriestAura::ShadowWeavingTrigger => Self::shadow_weaving(fight).handler(fight),
            PriestAura::SearingLightTrigger => {
                let aura = Self::searing_light(fight).aura;
                fight.activate_aura(aura);
            }
            _ => {}
        }
    }
}
