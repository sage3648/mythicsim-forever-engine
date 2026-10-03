//! The Shaman class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        chain_lightning::{self, ChainLightning},
        fire_nova, flame_shock, lava_burst,
        lightning_bolt::{self, Overload},
        searing_totem::{self, SearingTotem},
    },
    talents::elemental_focus::{self, ElementalFocus},
};

/// What a Shaman spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShamanSpell {
    LightningBolt,
    LightningBoltOverload,
    ChainLightning,
    ChainLightningOverload,
    FlameShock,
    FlameShockDot,
    LavaBurst,
    FireNova,
    SearingTotem,
    SearingTotemAttack,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShamanAura {
    ElementalFocus,
    Clearcasting,
}

/// Shaman state that Go keeps in the `Shaman` struct and its closures.
#[derive(Default)]
pub(crate) struct ShamanAgent {
    /// The overload of each Lightning Bolt and Chain Lightning rank, by spellbook position.
    overloads: Vec<Option<SpellId>>,
    lightning_bolt: Option<Overload>,
    chain_lightning: Option<ChainLightning>,
    /// Flame Shock's periodic half.
    flame_shock_dot: Option<SpellId>,
    lava_burst_bonus: f64,
    fire_nova_damage: f64,
    searing_totem: Option<SearingTotem>,
    elemental_focus: Option<ElementalFocus>,
    focus_trigger: elemental_focus::Trigger,
}

/// Whether a prepared input carries the effect of a kind.
fn has_effect(prepared: &PreparedV2, kind: &str) -> bool {
    prepared.effects.iter().any(|effect| effect.kind() == kind)
}

impl ShamanAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    fn spell(prepared: &PreparedV2, spell: &ExportedSpell) -> Option<ShamanSpell> {
        let has = |kind| has_effect(prepared, kind);
        let damage = spell.damage_effect.is_some();
        let id = spell.action_id.clone().unwrap_or_default();
        let searing_attack = prepared.effects.iter().any(|effect| {
            matches!(effect, Effect::SearingTotem { attack_spell_id, .. }
                if *attack_spell_id == id.spell_id && id.tag == 0)
        });
        match spell.class_spell.as_deref()? {
            "lightning_bolt" if damage && has("lightning_bolt") => Some(ShamanSpell::LightningBolt),
            "lightning_bolt_overload" if has("lightning_bolt") => {
                Some(ShamanSpell::LightningBoltOverload)
            }
            "chain_lightning" if damage && has("chain_lightning") => {
                Some(ShamanSpell::ChainLightning)
            }
            "chain_lightning_overload" if has("chain_lightning") => {
                Some(ShamanSpell::ChainLightningOverload)
            }
            "flame_shock_direct" if damage && has("flame_shock") => Some(ShamanSpell::FlameShock),
            "flame_shock_dot" if spell.dot.is_some() && has("flame_shock") => {
                Some(ShamanSpell::FlameShockDot)
            }
            "lava_burst" if damage && has("lava_burst") => Some(ShamanSpell::LavaBurst),
            "fire_nova" if has("fire_nova") => Some(ShamanSpell::FireNova),
            "searing_totem" if spell.dot.is_some() && has("searing_totem") => {
                Some(ShamanSpell::SearingTotem)
            }
            "searing_totem" if searing_attack => Some(ShamanSpell::SearingTotemAttack),
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<ShamanAgent>, String> {
        let auras: Vec<(String, ShamanAura)> = prepared
            .effects
            .iter()
            .flat_map(|effect| match effect {
                Effect::ElementalFocus {
                    trigger_aura, aura, ..
                } => vec![
                    (trigger_aura.clone(), ShamanAura::ElementalFocus),
                    (aura.clone(), ShamanAura::Clearcasting),
                ],
                _ => Vec::new(),
            })
            .collect();
        let spell = |exported: &ExportedSpell| ShamanAgent::spell(prepared, exported);
        let mut fight = Fight::new(prepared, ShamanAgent::default(), spell, |unit, label| {
            (unit == "player")
                .then(|| {
                    auras
                        .iter()
                        .find(|(name, _)| name == label)
                        .map(|(_, kind)| *kind)
                })
                .flatten()
        })?;
        let find = |fight: &Fight<ShamanAgent>, spell_id: i32, tag: i32| {
            fight
                .spells
                .iter()
                .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == tag)
        };
        fight.agent.overloads = vec![None; fight.spells.len()];
        for effect in &prepared.effects {
            match effect {
                Effect::LightningBolt {
                    overload_chance,
                    overload_tag,
                    rng_label,
                }
                | Effect::ChainLightning {
                    overload_chance,
                    overload_tag,
                    rng_label,
                    ..
                } => {
                    let (cast, overload) = match effect {
                        Effect::LightningBolt { .. } => (
                            ShamanSpell::LightningBolt,
                            ShamanSpell::LightningBoltOverload,
                        ),
                        _ => (
                            ShamanSpell::ChainLightning,
                            ShamanSpell::ChainLightningOverload,
                        ),
                    };
                    Self::bind_overloads(&mut fight, cast, overload, *overload_tag, find)?;
                    match effect {
                        Effect::ChainLightning {
                            bounce_reduction,
                            bounce_bonus,
                            ..
                        } => {
                            fight.agent.chain_lightning = Some(ChainLightning {
                                overload_chance: *overload_chance,
                                label: rng_label.clone(),
                                bounce_reduction: *bounce_reduction,
                                bounce_bonus: *bounce_bonus,
                            });
                        }
                        _ => {
                            fight.agent.lightning_bolt = Some(Overload {
                                chance: *overload_chance,
                                label: rng_label.clone(),
                            });
                        }
                    }
                }
                Effect::FlameShock {
                    spell_id,
                    tick_base,
                    tick_can_crit,
                } => {
                    let dot_spell = fight.spells.iter().position(|spell| {
                        spell.id.spell_id == *spell_id
                            && matches!(
                                spell.behavior,
                                crate::core::fight::SpellBehavior::Class(
                                    ShamanSpell::FlameShockDot
                                )
                            )
                    });
                    if let Some(dot_spell) = dot_spell {
                        let dot = fight.spells[dot_spell]
                            .dot
                            .expect("the periodic half has a dot");
                        fight.dots[dot].tick_base = Some(*tick_base);
                        fight.dots[dot].tick_can_crit = *tick_can_crit;
                        fight.agent.flame_shock_dot = Some(dot_spell);
                    }
                }
                Effect::LavaBurst {
                    flame_shock_bonus, ..
                } => fight.agent.lava_burst_bonus = *flame_shock_bonus,
                Effect::FireNova { base_damage, .. } => fight.agent.fire_nova_damage = *base_damage,
                Effect::SearingTotem {
                    attack_spell_id,
                    attack_damage,
                    magma_totem_aura,
                    flametongue_totem_aura,
                    ..
                } => {
                    if let Some(attack) = find(&fight, *attack_spell_id, 0) {
                        let magma_totem = fight.player_aura(magma_totem_aura).ok();
                        let flametongue_totem = fight.player_aura(flametongue_totem_aura).ok();
                        fight.agent.searing_totem = Some(SearingTotem {
                            attack,
                            attack_damage: *attack_damage,
                            magma_totem,
                            flametongue_totem,
                        });
                    }
                }
                Effect::ElementalFocus {
                    trigger_aura,
                    aura,
                    proc_chance,
                    cost_percent_add,
                    max_stacks,
                } => {
                    let bound = elemental_focus::bind(
                        &mut fight,
                        trigger_aura,
                        aura,
                        *proc_chance,
                        *cost_percent_add,
                        *max_stacks,
                    )?;
                    fight.agent.elemental_focus = Some(bound);
                }
                _ => {}
            }
        }
        Ok(fight)
    }

    /// Pair each rank with its overload, which rolls its parent's client damage row.
    fn bind_overloads(
        fight: &mut Fight<ShamanAgent>,
        cast: ShamanSpell,
        overload: ShamanSpell,
        tag: i32,
        find: impl Fn(&Fight<ShamanAgent>, i32, i32) -> Option<SpellId>,
    ) -> Result<(), String> {
        let behavior =
            |fight: &Fight<ShamanAgent>, spell: SpellId| match fight.spells[spell].behavior {
                crate::core::fight::SpellBehavior::Class(kind) => Some(kind),
                _ => None,
            };
        for spell in 0..fight.spells.len() {
            if behavior(fight, spell) != Some(cast) {
                continue;
            }
            let id = fight.spells[spell].id.spell_id;
            let Some(copy) = find(fight, id, tag).filter(|&s| behavior(fight, s) == Some(overload))
            else {
                return Err(format!("spell {id} has no overload"));
            };
            fight.spells[copy].damage_effect = fight.spells[spell].damage_effect;
            fight.agent.overloads[spell] = Some(copy);
        }
        Ok(())
    }

    fn focus(fight: &Fight<Self>) -> ElementalFocus {
        fight
            .agent
            .elemental_focus
            .expect("Elemental Focus is bound")
    }

    fn searing_totem(fight: &Fight<Self>) -> SearingTotem {
        fight
            .agent
            .searing_totem
            .clone()
            .expect("Searing Totem is bound")
    }
}

impl Agent for ShamanAgent {
    type Spell = ShamanSpell;
    type Aura = ShamanAura;

    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: ShamanSpell) {
        match behavior {
            ShamanSpell::LightningBolt | ShamanSpell::LightningBoltOverload => {
                lightning_bolt::apply(fight, spell, target)
            }
            ShamanSpell::ChainLightning | ShamanSpell::ChainLightningOverload => {
                let state = fight
                    .agent
                    .chain_lightning
                    .clone()
                    .expect("Chain Lightning is bound");
                let overload = (behavior == ShamanSpell::ChainLightning)
                    .then(|| fight.agent.overloads[spell].expect("every rank has an overload"));
                chain_lightning::apply(fight, spell, target, &state, overload);
            }
            ShamanSpell::FlameShock => {
                let dot_spell = fight.agent.flame_shock_dot.expect("Flame Shock is bound");
                flame_shock::apply(fight, spell, target, dot_spell);
            }
            ShamanSpell::FlameShockDot => flame_shock::apply_dot(fight, spell),
            ShamanSpell::LavaBurst => {
                let dot = fight
                    .agent
                    .flame_shock_dot
                    .and_then(|dot_spell| fight.spells[dot_spell].dot);
                let bonus = fight.agent.lava_burst_bonus;
                lava_burst::apply(fight, spell, target, dot, bonus);
            }
            ShamanSpell::FireNova => {
                let base = fight.agent.fire_nova_damage;
                fire_nova::apply(fight, spell, base);
            }
            ShamanSpell::SearingTotem => {
                let state = Self::searing_totem(fight);
                searing_totem::apply(fight, spell, &state);
            }
            ShamanSpell::SearingTotemAttack => {
                let base = Self::searing_totem(fight).attack_damage;
                searing_totem::attack(fight, spell, target, base);
            }
        }
    }

    fn on_travel(
        fight: &mut Fight<Self>,
        spell: SpellId,
        result: SpellResult,
        behavior: ShamanSpell,
    ) {
        let overload = match behavior {
            ShamanSpell::LightningBolt => {
                let roll = fight
                    .agent
                    .lightning_bolt
                    .clone()
                    .expect("Lightning Bolt is bound");
                let copy = fight.agent.overloads[spell].expect("every rank has an overload");
                Some((copy, roll))
            }
            _ => None,
        };
        lightning_bolt::arrive(
            fight,
            spell,
            result,
            overload.as_ref().map(|(copy, roll)| (*copy, roll)),
        );
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: ShamanSpell) {
        match behavior {
            ShamanSpell::FlameShockDot => fight.snapshot_dot_tick(dot),
            ShamanSpell::SearingTotem => {
                let state = Self::searing_totem(fight);
                let side = fight.dots[dot].side;
                searing_totem::tick(fight, side, &state);
            }
            _ => {}
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: ShamanAura) {
        if kind == ShamanAura::Clearcasting {
            Self::focus(fight).on_gain(fight);
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: ShamanAura) {
        if kind == ShamanAura::Clearcasting {
            Self::focus(fight).on_expire(fight);
        }
    }

    fn on_cast_complete(fight: &mut Fight<Self>, _aura: AuraRef, kind: ShamanAura, spell: SpellId) {
        match kind {
            ShamanAura::ElementalFocus => {
                let focus = Self::focus(fight);
                if let Some(trigger) = focus.roll(fight, spell) {
                    fight.agent.focus_trigger = Some(trigger);
                    focus.grant(fight);
                }
            }
            ShamanAura::Clearcasting => {
                let last = fight.agent.focus_trigger;
                Self::focus(fight).consume(fight, spell, last);
            }
        }
    }
}
