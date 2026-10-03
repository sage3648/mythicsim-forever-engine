//! The Warlock class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult, PRIORITY_REGEN},
};

use super::{
    pets::{self, DemonAi},
    spells::{
        bane_of_agony::{self, BaneOfAgony},
        bane_of_doom, bind_snapshot_dot,
        conflagrate::Conflagrate,
        corruption,
        curse_of_the_elements::{self, CurseOfTheElements},
        drain_life::DrainLife,
        find_spell, immolate, incinerate,
        life_tap::{self, LifeTap},
        searing_pain, shadow_bolt, shadowburn,
        siphon_life::SiphonLife,
        soul_fire,
    },
    talents::{
        decimation::{self, Decimation},
        demonic_brand::{self, DemonicBrand},
        improved_shadow_bolt::{self, ImprovedShadowBolt},
        nightfall::{self, Nightfall},
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
    AmplifyCurse,
    SiphonLife,
    BaneOfDoom,
    DrainLife,
    Incinerate,
    /// The Succubus's Lash of Pain.
    LashOfPain,
    /// The demon's Demonic Brand hit.
    DemonicBrand,
    /// The Imp's Firebolt.
    Firebolt,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarlockAura {
    CurseOfTheElements,
    ImprovedShadowBoltTrigger,
    ShadowAndFlameTrigger,
    ShadowAndFlame,
    NightfallTrigger,
    ShadowTrance,
    DecimationTrigger,
    Decimation,
    DemonicBrandTrigger,
    /// The demon's aura that spends the brand's charges.
    DemonicBrandConsumer,
    /// The Voidwalker's sacrifice.
    FelEnergy,
}

/// [`Agent::on_periodic`] tags of Warlock periodic actions.
const FEL_ENERGY_TICK: u32 = 1;

/// Warlock state that Go keeps in the `Warlock` struct and its closures.
#[derive(Default)]
pub(crate) struct WarlockAgent {
    /// Immolate's dot, which Immolate applies and Conflagrate consumes.
    immolate_dot: Option<DotId>,
    corruption_dot: Option<DotId>,
    pub(crate) bane_of_agony: Option<BaneOfAgony>,
    siphon_life: Option<SiphonLife>,
    bane_of_doom_dot: Option<DotId>,
    drain_life: Option<DrainLife>,
    /// Incinerate's bonus on a target burning with Immolate.
    incinerate_bonus: f64,
    /// Go `currentActiveBane` on the one target.
    pub(crate) bane_slot: Option<AuraRef>,
    curse_of_the_elements: Option<Rc<CurseOfTheElements>>,
    life_tap: Option<LifeTap>,
    conflagrate: Option<Rc<Conflagrate>>,
    improved_shadow_bolt: Option<Rc<ImprovedShadowBolt>>,
    shadow_and_flame: Option<Rc<ShadowAndFlame>>,
    amplify_curse: Option<AuraRef>,
    nightfall: Option<Rc<Nightfall>>,
    demon: Option<Rc<DemonAi>>,
    lash_of_pain_base: f64,
    /// Firebolt's damage roll bounds.
    firebolt: (f64, f64),
    decimation: Option<Rc<Decimation>>,
    demonic_brand: Option<Rc<DemonicBrand>>,
    /// Fel Energy's period, share of maximum mana and mana metrics.
    fel_energy: Option<(i64, f64, usize)>,
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
            Effect::Nightfall {
                trigger_aura, aura, ..
            } => {
                auras.push((
                    "player",
                    trigger_aura.clone(),
                    WarlockAura::NightfallTrigger,
                ));
                auras.push(("player", aura.clone(), WarlockAura::ShadowTrance));
            }
            Effect::FelEnergy { aura, .. } => {
                auras.push(("player", aura.clone(), WarlockAura::FelEnergy))
            }
            Effect::Decimation {
                trigger_aura, aura, ..
            } => {
                auras.push((
                    "player",
                    trigger_aura.clone(),
                    WarlockAura::DecimationTrigger,
                ));
                auras.push(("player", aura.clone(), WarlockAura::Decimation));
            }
            Effect::DemonicBrand {
                trigger_aura,
                consumer_aura,
                ..
            } => {
                auras.push((
                    "player",
                    trigger_aura.clone(),
                    WarlockAura::DemonicBrandTrigger,
                ));
                if let Some(consumer) = consumer_aura {
                    auras.push(("pet", consumer.clone(), WarlockAura::DemonicBrandConsumer));
                }
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
            "amplify_curse" => Some(WarlockSpell::AmplifyCurse),
            "siphon_life" if dot => Some(WarlockSpell::SiphonLife),
            "bane_of_doom" if dot => Some(WarlockSpell::BaneOfDoom),
            "drain_life" if dot => Some(WarlockSpell::DrainLife),
            "incinerate" if damage => Some(WarlockSpell::Incinerate),
            "succubus_lash_of_pain" => Some(WarlockSpell::LashOfPain),
            "demonic_brand" => Some(WarlockSpell::DemonicBrand),
            "imp_firebolt" => Some(WarlockSpell::Firebolt),
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
                Effect::SiphonLife {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                    self_healing_multiplier,
                } => {
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    let metrics = fight.spells[find_spell(&fight, *spell_id)?]
                        .health_metrics
                        .ok_or("Siphon Life has no health metrics")?;
                    fight.agent.siphon_life =
                        Some(SiphonLife::new(dot, *self_healing_multiplier, metrics));
                }
                Effect::DrainLife {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                    soul_siphon,
                    self_healing_multiplier,
                } => {
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    let metrics = fight.spells[find_spell(&fight, *spell_id)?]
                        .health_metrics
                        .ok_or("Drain Life has no health metrics")?;
                    fight.agent.drain_life = Some(DrainLife::new(
                        dot,
                        *soul_siphon,
                        *self_healing_multiplier,
                        metrics,
                    ));
                }
                Effect::Incinerate { immolate_bonus } => {
                    fight.agent.incinerate_bonus = *immolate_bonus;
                }
                Effect::BaneOfDoom {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                } => {
                    let dot = bind_snapshot_dot(&mut fight, *spell_id, *tick_base, *tick_can_crit)?;
                    fight.agent.bane_of_doom_dot = Some(dot);
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
                    pet_mana_share,
                } => {
                    let bound = life_tap::bind(
                        &mut fight,
                        *spell_id,
                        *base_amount,
                        *mana_multiplier,
                        spirit,
                        *pet_mana_share,
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
                Effect::AmplifyCurse { aura, .. } => {
                    fight.agent.amplify_curse = Some(fight.player_aura(aura)?);
                }
                Effect::Nightfall {
                    aura,
                    proc_chance,
                    rng_label,
                    trigger_spells,
                    consume_spells,
                    modded_spells,
                    cast_time_percent,
                    ..
                } => {
                    let bound = nightfall::bind(
                        &mut fight,
                        aura,
                        *proc_chance,
                        rng_label,
                        trigger_spells,
                        consume_spells,
                        modded_spells,
                        *cast_time_percent,
                    )?;
                    fight.agent.nightfall = Some(Rc::new(bound));
                }
                Effect::WarlockPet {
                    min_mana,
                    autocast_spells,
                    wait_ns,
                    ..
                } => {
                    let bound = pets::bind(&fight, autocast_spells, *min_mana, *wait_ns)?;
                    fight.agent.demon = Some(Rc::new(bound));
                }
                Effect::LashOfPain { base_damage } => fight.agent.lash_of_pain_base = *base_damage,
                Effect::Firebolt {
                    min_damage,
                    max_damage,
                } => fight.agent.firebolt = (*min_damage, *max_damage),
                Effect::FelEnergy {
                    spell_id,
                    mana_fraction,
                    period_ns,
                    ..
                } => {
                    let metrics = fight.new_mana_metrics(crate::contracts::prepared_v2::ActionId {
                        spell_id: *spell_id,
                        ..Default::default()
                    });
                    fight.agent.fel_energy = Some((*period_ns, *mana_fraction, metrics));
                }
                Effect::Decimation {
                    aura,
                    execute_phase,
                    trigger_spells,
                    damage_spells,
                    damage_done_flat,
                    cast_spells,
                    cast_time_percent,
                    ..
                } => {
                    let bound = decimation::bind(
                        &mut fight,
                        aura,
                        *execute_phase,
                        trigger_spells,
                        damage_spells,
                        *damage_done_flat,
                        cast_spells,
                        *cast_time_percent,
                    )?;
                    fight.agent.decimation = Some(Rc::new(bound));
                }
                Effect::DemonicBrand {
                    target_aura,
                    charges,
                    trigger_spells,
                    pet,
                    marker_aura,
                    brand_spell,
                    min_damage,
                    max_damage,
                    spell_power_coefficient,
                    school_power_stat,
                    ..
                } => {
                    let demon = match (pet, marker_aura, brand_spell, school_power_stat) {
                        (None, ..) => None,
                        (Some(_), Some(marker_aura), Some(brand_spell), Some(stat)) => {
                            Some(demonic_brand::DemonConfig {
                                marker_aura,
                                brand_spell: *brand_spell,
                                min_damage: *min_damage,
                                max_damage: *max_damage,
                                coefficient: *spell_power_coefficient,
                                school_power: prepared
                                    .player
                                    .stats
                                    .get(stat)
                                    .copied()
                                    .ok_or_else(|| format!("prepared stats lack {stat}"))?,
                            })
                        }
                        _ => return Err("Demonic Brand's demon half is incomplete".into()),
                    };
                    let bound =
                        demonic_brand::bind(&fight, target_aura, *charges, trigger_spells, demon)?;
                    fight.agent.demonic_brand = Some(Rc::new(bound));
                }
                _ => {}
            }
        }
        // Bane of Agony spends Amplify Curse, which is bound above.
        for effect in &prepared.effects {
            if let Effect::BaneOfAgony {
                amplify: Some(factor),
                ..
            } = effect
            {
                if let (Some(agony), Some(aura)) = (
                    fight.agent.bane_of_agony.as_mut(),
                    fight.agent.amplify_curse,
                ) {
                    agony.amplify = Some((aura, *factor));
                }
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
            WarlockSpell::SiphonLife => {
                let dot = fight.agent.siphon_life.expect("Siphon Life is bound").dot;
                corruption::apply(fight, spell, target, dot);
            }
            WarlockSpell::BaneOfDoom => {
                let dot = fight.agent.bane_of_doom_dot.expect("Bane of Doom is bound");
                bane_of_doom::apply(fight, spell, target, dot);
            }
            WarlockSpell::DrainLife => {
                let dot = fight.agent.drain_life.expect("Drain Life is bound").dot;
                corruption::apply(fight, spell, target, dot);
            }
            WarlockSpell::Incinerate => {
                let (immolate, bonus) = (fight.agent.immolate_dot, fight.agent.incinerate_bonus);
                incinerate::apply(fight, spell, target, immolate, bonus);
            }
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
            WarlockSpell::AmplifyCurse => {
                let aura = fight.agent.amplify_curse.expect("Amplify Curse is bound");
                fight.activate_aura(aura);
            }
            WarlockSpell::LashOfPain => {
                let base = fight.agent.lash_of_pain_base;
                pets::lash_of_pain(fight, spell, target, base);
            }
            WarlockSpell::Firebolt => {
                let bounds = fight.agent.firebolt;
                pets::firebolt(fight, spell, target, bounds);
            }
            WarlockSpell::DemonicBrand => {
                let brand = fight
                    .agent
                    .demonic_brand
                    .clone()
                    .expect("Demonic Brand is bound");
                brand.brand_hit(fight, spell, target);
            }
            WarlockSpell::ImmolateDot => panic!("Immolate's dot spell is never cast"),
        }
    }

    fn on_periodic(fight: &mut Fight<Self>, tag: u32) {
        if tag == FEL_ENERGY_TICK {
            let (_, fraction, metrics) = fight.agent.fel_energy.expect("bound");
            let max_mana = fight.unit_config(Side::Player).max_mana;
            fight.add_mana(max_mana * fraction, metrics);
        }
    }

    fn health_metrics_before_cost(behavior: WarlockSpell) -> bool {
        // siphon_life.go and drain_life.go register their health metrics before the spell.
        matches!(behavior, WarlockSpell::SiphonLife | WarlockSpell::DrainLife)
    }

    fn pet_rotation(fight: &mut Fight<Self>) {
        let demon = fight.agent.demon.clone().expect("the demon's AI is bound");
        demon.rotation(fight);
    }

    fn on_periodic_damage_dealt(
        fight: &mut Fight<Self>,
        aura: AuraRef,
        kind: WarlockAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if kind == WarlockAura::NightfallTrigger {
            let talent = fight.agent.nightfall.clone().expect("bound");
            talent.on_periodic_damage_dealt(fight, aura, spell, result);
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: WarlockAura,
        spell: SpellId,
        _result: SpellResult,
    ) {
        match kind {
            WarlockAura::NightfallTrigger => {
                let talent = fight.agent.nightfall.clone().expect("bound");
                talent.on_trigger(fight);
            }
            WarlockAura::ShadowTrance => {
                let talent = fight.agent.nightfall.clone().expect("bound");
                talent.on_consume(fight, spell);
            }
            _ => {}
        }
    }

    fn on_cast_complete(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: WarlockAura,
        spell: SpellId,
    ) {
        if kind == WarlockAura::ShadowTrance {
            let talent = fight.agent.nightfall.clone().expect("bound");
            talent.on_cast_complete(fight, spell);
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
            WarlockSpell::ImmolateDot | WarlockSpell::Corruption | WarlockSpell::BaneOfDoom => {
                fight.snapshot_dot_tick(dot)
            }
            WarlockSpell::SiphonLife => {
                let siphon = fight.agent.siphon_life.expect("Siphon Life is bound");
                siphon.tick(fight);
            }
            WarlockSpell::DrainLife => {
                let drain = fight.agent.drain_life.expect("Drain Life is bound");
                drain.tick(fight);
            }
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
            WarlockAura::ShadowTrance => {
                let talent = fight.agent.nightfall.clone().expect("bound");
                talent.on_gain(fight);
            }
            WarlockAura::Decimation => {
                let talent = fight.agent.decimation.clone().expect("bound");
                talent.on_gain(fight);
            }
            WarlockAura::FelEnergy => {
                // Go StartPeriodicAction at the regeneration priority, first tick a period on.
                let (period, _, _) = fight.agent.fel_energy.expect("bound");
                fight.start_class_periodic(FEL_ENERGY_TICK, period, 0, PRIORITY_REGEN);
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
            WarlockAura::ShadowTrance => {
                let talent = fight.agent.nightfall.clone().expect("bound");
                talent.on_expire(fight);
            }
            WarlockAura::Decimation => {
                let talent = fight.agent.decimation.clone().expect("bound");
                talent.on_expire(fight);
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
            WarlockAura::DecimationTrigger => {
                let talent = fight.agent.decimation.clone().expect("bound");
                talent.on_spell_hit_dealt(fight, spell, result);
            }
            WarlockAura::DemonicBrandTrigger => {
                let talent = fight.agent.demonic_brand.clone().expect("bound");
                talent.on_spell_hit_dealt(fight, spell, result);
            }
            WarlockAura::DemonicBrandConsumer => {
                let talent = fight.agent.demonic_brand.clone().expect("bound");
                talent.on_demon_hit(fight, spell, result);
            }
            _ => {}
        }
    }
}
