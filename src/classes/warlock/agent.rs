//! The Warlock class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        bane_of_agony::{self, BaneOfAgony},
        bind_snapshot_dot,
        conflagrate::Conflagrate,
        corruption,
        curse_of_the_elements::{self, CurseOfTheElements},
        find_spell, immolate,
        life_tap::{self, LifeTap},
        searing_pain, shadow_bolt, shadowburn, soul_fire,
    },
    talents::{
        improved_shadow_bolt::{self, ImprovedShadowBolt},
        shadow_and_flame::{self, ShadowAndFlame},
    },
};

/// What a Warlock spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarlockSpell {
    ShadowBolt,
    Immolate,
    /// Immolate's related dot spell, which only ticks.
    ImmolateDot,
    Corruption,
    BaneOfAgony,
    CurseOfTheElements,
    LifeTap,
    Conflagrate,
    Shadowburn,
    SearingPain,
    SoulFire,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarlockAura {
    CurseOfTheElements,
    ImprovedShadowBoltTrigger,
    ShadowAndFlameTrigger,
    ShadowAndFlame,
}

/// Warlock state that Go keeps in the `Warlock` struct and its closures.
#[derive(Default)]
pub(crate) struct WarlockAgent {
    /// Immolate's dot, which Immolate applies and Conflagrate consumes.
    immolate_dot: Option<DotId>,
    corruption_dot: Option<DotId>,
    pub(crate) bane_of_agony: Option<BaneOfAgony>,
    curse_of_the_elements: Option<Rc<CurseOfTheElements>>,
    life_tap: Option<LifeTap>,
    conflagrate: Option<Rc<Conflagrate>>,
    improved_shadow_bolt: Option<Rc<ImprovedShadowBolt>>,
    shadow_and_flame: Option<Rc<ShadowAndFlame>>,
}

/// Aura labels claimed by implemented class effects, as (unit, label, kind).
fn class_auras(prepared: &PreparedV2) -> Vec<(&'static str, String, WarlockAura)> {
    let mut auras = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::CurseOfTheElements { aura, .. } => {
                auras.push(("target", aura.clone(), WarlockAura::CurseOfTheElements))
            }
            Effect::ImprovedShadowBolt { trigger_aura, .. } => auras.push((
                "player",
                trigger_aura.clone(),
                WarlockAura::ImprovedShadowBoltTrigger,
            )),
            Effect::ShadowAndFlame {
                trigger_aura,
                shadow_aura,
                fire_aura,
                ..
            } => {
                auras.push((
                    "player",
                    trigger_aura.clone(),
                    WarlockAura::ShadowAndFlameTrigger,
                ));
                auras.push(("player", shadow_aura.clone(), WarlockAura::ShadowAndFlame));
                auras.push(("player", fire_aura.clone(), WarlockAura::ShadowAndFlame));
            }
            _ => {}
        }
    }
    auras
}

impl WarlockAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(spell: &ExportedSpell) -> Option<WarlockSpell> {
        let damage = spell.damage_effect.is_some();
        let dot = spell.dot.is_some();
        match spell.class_spell.as_deref()? {
            "shadow_bolt" if damage => Some(WarlockSpell::ShadowBolt),
            "immolate" if damage && spell.related_dot_spell.is_some() => {
                Some(WarlockSpell::Immolate)
            }
            "immolate_dot" if dot => Some(WarlockSpell::ImmolateDot),
            "corruption" if dot => Some(WarlockSpell::Corruption),
            "bane_of_agony" if dot => Some(WarlockSpell::BaneOfAgony),
            "curse_of_the_elements" => Some(WarlockSpell::CurseOfTheElements),
            "life_tap" => Some(WarlockSpell::LifeTap),
            "conflagrate" if damage => Some(WarlockSpell::Conflagrate),
            "shadowburn" if damage => Some(WarlockSpell::Shadowburn),
            "searing_pain" if damage => Some(WarlockSpell::SearingPain),
            "soul_fire" if damage => Some(WarlockSpell::SoulFire),
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<WarlockAgent>, String> {
        let auras = class_auras(prepared);
        let mut fight = Fight::new(
            prepared,
            WarlockAgent::default(),
            WarlockAgent::spell,
            |unit, label| {
                auras
                    .iter()
                    .find(|(side, name, _)| *side == unit && name == label)
                    .map(|(_, _, kind)| *kind)
            },
        )?;
        let spirit = prepared
            .player
            .stats
            .get("Spirit")
            .copied()
            .ok_or("prepared stats lack Spirit")?;
        for effect in &prepared.effects {
            match effect {
                Effect::Immolate {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                } => {
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    fight.agent.immolate_dot = Some(dot);
                }
                Effect::Corruption {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                } => {
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    fight.agent.corruption_dot = Some(dot);
                }
                Effect::BaneOfAgony {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                    ramp_share,
                    ramp_every_ticks,
                    ..
                } => {
                    if *ramp_every_ticks <= 0 {
                        return Err("Bane of Agony ramps every nonpositive tick count".into());
                    }
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    fight.agent.bane_of_agony = Some(BaneOfAgony::new(
                        dot,
                        *tick_base,
                        *ramp_share,
                        *ramp_every_ticks,
                    ));
                }
                Effect::CurseOfTheElements {
                    aura,
                    resistance_delta,
                    school_damage_taken_multiplier,
                    ..
                } => {
                    let bound = curse_of_the_elements::bind(
                        &fight,
                        aura,
                        resistance_delta,
                        school_damage_taken_multiplier,
                    )?;
                    fight.agent.curse_of_the_elements = Some(Rc::new(bound));
                }
                Effect::LifeTap {
                    spell_id,
                    base_amount,
                    mana_multiplier,
                } => {
                    let bound = life_tap::bind(
                        &mut fight,
                        *spell_id,
                        *base_amount,
                        *mana_multiplier,
                        spirit,
                    );
                    fight.agent.life_tap = Some(bound);
                }
                Effect::ImprovedShadowBolt {
                    aura,
                    multiplier,
                    trigger_spells,
                    ..
                } => {
                    let bound =
                        improved_shadow_bolt::bind(&mut fight, aura, *multiplier, trigger_spells)?;
                    fight.agent.improved_shadow_bolt = Some(Rc::new(bound));
                }
                Effect::ShadowAndFlame {
                    shadow_aura,
                    fire_aura,
                    multiplier,
                    trigger_spells,
                    shadow_spells,
                    ..
                } => {
                    let bound = shadow_and_flame::bind(
                        &fight,
                        shadow_aura,
                        fire_aura,
                        *multiplier,
                        trigger_spells,
                        shadow_spells,
                    )?;
                    fight.agent.shadow_and_flame = Some(Rc::new(bound));
                }
                _ => {}
            }
        }
        // Conflagrate reads Immolate's dot, which is bound above.
        for effect in &prepared.effects {
            if let Effect::Conflagrate {
                spell_id,
                keep_immolate_chance,
                rng_label,
            } = effect
            {
                find_spell(&fight, *spell_id)?;
                let immolate = fight
                    .agent
                    .immolate_dot
                    .ok_or("Conflagrate needs Immolate's dot")?;
                fight.agent.conflagrate = Some(Rc::new(Conflagrate::new(
                    immolate,
                    *keep_immolate_chance,
                    rng_label,
                )));
            }
        }
        Ok(fight)
    }
}

impl Agent for WarlockAgent {
    type Spell = WarlockSpell;
    type Aura = WarlockAura;

    fn apply_effects(
        fight: &mut Fight<Self>,
        spell: SpellId,
        target: Side,
        behavior: WarlockSpell,
    ) {
        match behavior {
            WarlockSpell::ShadowBolt => shadow_bolt::apply(fight, spell, target),
            WarlockSpell::SoulFire => soul_fire::apply(fight, spell, target),
            WarlockSpell::Shadowburn => shadowburn::apply(fight, spell, target),
            WarlockSpell::SearingPain => searing_pain::apply(fight, spell, target),
            WarlockSpell::Immolate => {
                let dot = fight.agent.immolate_dot.expect("Immolate is bound");
                immolate::apply(fight, spell, target, dot);
            }
            WarlockSpell::Corruption => {
                let dot = fight.agent.corruption_dot.expect("Corruption is bound");
                corruption::apply(fight, spell, target, dot);
            }
            WarlockSpell::BaneOfAgony => bane_of_agony::apply(fight, spell, target),
            WarlockSpell::CurseOfTheElements => {
                let curse = fight
                    .agent
                    .curse_of_the_elements
                    .clone()
                    .expect("Curse of the Elements is bound");
                curse.apply(fight, spell, target);
            }
            WarlockSpell::LifeTap => {
                let tap = fight.agent.life_tap.expect("Life Tap is bound");
                tap.apply(fight);
            }
            WarlockSpell::Conflagrate => {
                let conflagrate = fight
                    .agent
                    .conflagrate
                    .clone()
                    .expect("Conflagrate is bound");
                conflagrate.apply(fight, spell, target);
            }
            WarlockSpell::ImmolateDot => panic!("Immolate's dot spell is never cast"),
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: WarlockSpell) -> bool {
        match behavior {
            WarlockSpell::Conflagrate => fight
                .agent
                .conflagrate
                .as_ref()
                .expect("Conflagrate is bound")
                .can_cast(fight),
            _ => true,
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: WarlockSpell) {
        match behavior {
            WarlockSpell::BaneOfAgony => bane_of_agony::tick(fight),
            WarlockSpell::ImmolateDot | WarlockSpell::Corruption => fight.snapshot_dot_tick(dot),
            _ => {}
        }
    }

    fn on_gain(fight: &mut Fight<Self>, aura: AuraRef, kind: WarlockAura) {
        match kind {
            WarlockAura::CurseOfTheElements => {
                let curse = fight.agent.curse_of_the_elements.clone().expect("bound");
                curse.on_gain(fight);
            }
            WarlockAura::ShadowAndFlame => {
                let talent = fight.agent.shadow_and_flame.clone().expect("bound");
                talent.on_gain(fight, aura);
            }
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, aura: AuraRef, kind: WarlockAura) {
        match kind {
            WarlockAura::CurseOfTheElements => {
                let curse = fight.agent.curse_of_the_elements.clone().expect("bound");
                curse.on_expire(fight);
            }
            WarlockAura::ShadowAndFlame => {
                let talent = fight.agent.shadow_and_flame.clone().expect("bound");
                talent.on_expire(fight, aura);
            }
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: WarlockAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            WarlockAura::ImprovedShadowBoltTrigger => {
                let talent = fight.agent.improved_shadow_bolt.clone().expect("bound");
                talent.on_spell_hit_dealt(fight, spell, result);
            }
            WarlockAura::ShadowAndFlameTrigger => {
                let talent = fight.agent.shadow_and_flame.clone().expect("bound");
                talent.on_spell_hit_dealt(fight, spell, result);
            }
            _ => {}
        }
    }
}
