//! The Shaman class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    masks::{is_class, HOLDS_MELEE},
    spells::{
        chain_lightning::{self, ChainLightning},
        earth_shock, fire_nova, flame_shock, lava_burst,
        lightning_bolt::{self, Overload},
        searing_totem::{self, SearingTotem},
        stormstrike::{self, Stormstrike},
        totems::{self, Expirations, StrengthOfEarth},
        weapon_imbues,
    },
    talents::{
        elemental_devastation::{self, ElementalDevastation},
        elemental_focus::{self, ElementalFocus},
        flurry::{self, Flurry},
        improved_stormstrike::{self, ImprovedStormstrike},
        maelstrom_weapon::{self, MaelstromWeapon},
        rage_of_the_farseer::{self, RageOfTheFarseer},
    },
};
use crate::rotation::Totem;

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
    EarthShock,
    StrengthOfEarthTotem,
    StormstrikeCast,
    FrostShock,
    MagmaTotem,
    LightningShield,
    GraceOfAirTotem,
    ManaSpringTotem,
    FlametongueTotem,
    FlametongueTotemAttack,
    /// The Flametongue Weapon hit of one hand, by its position in the effect.
    FlametongueHit(usize),
    FrostbrandHit,
    StormstrikeMainHand,
    StormstrikeOffHand,
    RageOfTheFarseer,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShamanAura {
    ElementalFocus,
    Clearcasting,
    ElementalDevastationTrigger,
    ElementalDevastation,
    ImprovedStormstrikeTrigger,
    ImprovedStormstrike,
    MaelstromWeaponTrigger,
    MaelstromWeapon,
    /// Stormstrike's debuff on the target.
    Stormstrike,
    FlurryTrigger,
    FlametongueTotem,
    FlametongueTotemTrigger,
    /// One hand's Flametongue Weapon trigger, by its position in the effect.
    FlametongueTrigger(usize),
    FrostbrandTrigger,
    Flurry,
    RageOfTheFarseer,
    RockbiterWeapon,
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
    totems: Expirations,
    searing_duration: i64,
    strength_of_earth: Option<StrengthOfEarth>,
    stormstrike: Option<Stormstrike>,
    devastation: Option<ElementalDevastation>,
    improved_stormstrike: Option<ImprovedStormstrike>,
    maelstrom: Option<MaelstromWeapon>,
    maelstrom_chances: maelstrom_weapon::Chances,
    flurry: Option<Flurry>,
    /// Flurry's charge cooldown, a Go timer: when it is ready.
    flurry_icd: i64,
    /// Melee auto attacks, Go `ProcMaskMeleeWhiteHit`, by spellbook position.
    white: Vec<bool>,
    farseer: Option<RageOfTheFarseer>,
    /// Stormstrike's main hand strike, when the character has a main hand weapon.
    stormstrike_main_hand: Option<SpellId>,
    /// Go `HasMHWeapon() || HasOHWeapon()`, Stormstrike's cast condition.
    stormstrike_weapon: bool,
    /// Each Flametongue Weapon hand: its hit spell, whether it deals damage, its base, and the
    /// spells that trigger it by spellbook position.
    flametongue: Vec<(SpellId, bool, f64, Vec<bool>)>,
    /// Frostbrand Weapon's hit spell and base, and its trigger's chance by spellbook position.
    frostbrand: Option<(SpellId, f64)>,
    frostbrand_chances: Vec<Option<f64>>,
    /// Magma Totem's pulse base and lifetime.
    magma: Option<(f64, i64)>,
    /// Lightning Shield's aura and charges.
    lightning_shield: Option<(AuraRef, i32)>,
    grace_of_air: Option<StrengthOfEarth>,
    mana_spring: Option<StrengthOfEarth>,
    flametongue_totem: Option<totems::FlametongueTotem>,
    /// The last air totem aura cast, which a new one replaces.
    air_totem: Option<AuraRef>,
    /// Rockbiter Weapon's gain and loss lines.
    rockbiter_logs: Option<(String, String)>,
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
        // Class spells without a class mask, which Go casts by action ID.
        let farseer = prepared.effects.iter().any(|effect| {
            matches!(effect, Effect::RageOfTheFarseer { spell_id, .. }
                if *spell_id == id.spell_id && id.tag == 0)
        });
        if spell.class_spell.is_none() && farseer {
            return Some(ShamanSpell::RageOfTheFarseer);
        }
        let position = prepared
            .player
            .spells
            .iter()
            .position(|exported| std::ptr::eq(exported, spell));
        for effect in &prepared.effects {
            match effect {
                Effect::FlametongueTotem { attack_spell, .. }
                    if Some(*attack_spell) == position =>
                {
                    return Some(ShamanSpell::FlametongueTotemAttack);
                }
                // Go registers Lightning Shield's cast without a class mask.
                Effect::LightningShield { spell_id, .. }
                    if spell.class_spell.is_none() && *spell_id == id.spell_id && id.tag == 0 =>
                {
                    return Some(ShamanSpell::LightningShield);
                }
                Effect::FlametongueWeapon { hands } => {
                    if let Some(hand) = hands.iter().position(|hand| Some(hand.spell) == position) {
                        return Some(ShamanSpell::FlametongueHit(hand));
                    }
                }
                Effect::FrostbrandWeapon { spell_id, .. }
                    if *spell_id == id.spell_id && id.tag == 0 =>
                {
                    return Some(ShamanSpell::FrostbrandHit);
                }
                _ => {}
            }
        }
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
            "earth_shock" if damage && has("earth_shock") => Some(ShamanSpell::EarthShock),
            "frost_shock" if damage && has("frost_shock") => Some(ShamanSpell::FrostShock),
            "magma_totem" if spell.dot.is_some() && has("magma_totem") => {
                Some(ShamanSpell::MagmaTotem)
            }
            "flametongue_totem"
                if prepared.effects.iter().any(|effect| {
                    matches!(effect, Effect::FlametongueTotem { spell_id, .. }
                        if *spell_id == id.spell_id && id.tag == 0)
                }) =>
            {
                Some(ShamanSpell::FlametongueTotem)
            }
            "stormstrike_cast" if has("stormstrike") => Some(ShamanSpell::StormstrikeCast),
            "stormstrike_damage" if has("stormstrike") && id.tag == 1 => {
                Some(ShamanSpell::StormstrikeMainHand)
            }
            "stormstrike_damage" if has("stormstrike") && id.tag == 2 => {
                Some(ShamanSpell::StormstrikeOffHand)
            }
            "basic_totem"
                if prepared.effects.iter().any(|effect| {
                    matches!(effect, Effect::StrengthOfEarthTotem { spell_id, .. }
                        if *spell_id == id.spell_id && id.tag == 0)
                }) =>
            {
                Some(ShamanSpell::StrengthOfEarthTotem)
            }
            "basic_totem"
                if prepared.effects.iter().any(|effect| {
                    matches!(effect, Effect::GraceOfAirTotem { spell_id, .. }
                        if *spell_id == id.spell_id && id.tag == 0)
                }) =>
            {
                Some(ShamanSpell::GraceOfAirTotem)
            }
            "basic_totem"
                if prepared.effects.iter().any(|effect| {
                    matches!(effect, Effect::ManaSpringTotem { spell_id, .. }
                        if *spell_id == id.spell_id && id.tag == 0)
                }) =>
            {
                Some(ShamanSpell::ManaSpringTotem)
            }
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
                Effect::ElementalDevastation {
                    trigger_aura, aura, ..
                } => vec![
                    (
                        trigger_aura.clone(),
                        ShamanAura::ElementalDevastationTrigger,
                    ),
                    (aura.clone(), ShamanAura::ElementalDevastation),
                ],
                Effect::ImprovedStormstrike {
                    trigger_aura, aura, ..
                } => vec![
                    (trigger_aura.clone(), ShamanAura::ImprovedStormstrikeTrigger),
                    (aura.clone(), ShamanAura::ImprovedStormstrike),
                ],
                Effect::MaelstromWeapon {
                    trigger_aura, aura, ..
                } => vec![
                    (trigger_aura.clone(), ShamanAura::MaelstromWeaponTrigger),
                    (aura.clone(), ShamanAura::MaelstromWeapon),
                ],
                Effect::Flurry {
                    trigger_aura, aura, ..
                } => vec![
                    (trigger_aura.clone(), ShamanAura::FlurryTrigger),
                    (aura.clone(), ShamanAura::Flurry),
                ],
                Effect::RageOfTheFarseer { aura, .. } => {
                    vec![(aura.clone(), ShamanAura::RageOfTheFarseer)]
                }
                Effect::RockbiterWeapon { aura, .. } => {
                    vec![(aura.clone(), ShamanAura::RockbiterWeapon)]
                }
                Effect::FlametongueWeapon { hands } => hands
                    .iter()
                    .enumerate()
                    .map(|(hand, state)| {
                        (
                            state.trigger_aura.clone(),
                            ShamanAura::FlametongueTrigger(hand),
                        )
                    })
                    .collect(),
                Effect::FrostbrandWeapon { trigger_aura, .. } => {
                    vec![(trigger_aura.clone(), ShamanAura::FrostbrandTrigger)]
                }
                Effect::FlametongueTotem {
                    aura, trigger_aura, ..
                } => vec![
                    (aura.clone(), ShamanAura::FlametongueTotem),
                    (trigger_aura.clone(), ShamanAura::FlametongueTotemTrigger),
                ],
                _ => Vec::new(),
            })
            .collect();
        let target_auras: Vec<(String, ShamanAura)> = prepared
            .effects
            .iter()
            .filter_map(|effect| match effect {
                Effect::Stormstrike { aura, .. } => Some((aura.clone(), ShamanAura::Stormstrike)),
                _ => None,
            })
            .collect();
        let spell = |exported: &ExportedSpell| ShamanAgent::spell(prepared, exported);
        let mut fight = Fight::new(prepared, ShamanAgent::default(), spell, |unit, label| {
            let auras = match unit {
                "player" => &auras,
                _ => &target_auras,
            };
            auras
                .iter()
                .find(|(name, _)| name == label)
                .map(|(_, kind)| *kind)
        })?;
        let find = |fight: &Fight<ShamanAgent>, spell_id: i32, tag: i32| {
            fight
                .spells
                .iter()
                .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == tag)
        };
        fight.agent.overloads = vec![None; fight.spells.len()];
        fight.agent.maelstrom_chances = vec![None; fight.spells.len()];
        fight.agent.frostbrand_chances = vec![None; fight.spells.len()];
        fight.agent.white = prepared
            .player
            .spells
            .iter()
            .map(|spell| {
                spell
                    .proc_mask
                    .iter()
                    .any(|mask| mask == "ProcMaskMeleeMHAuto" || mask == "ProcMaskMeleeOHAuto")
            })
            .collect();
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
                    duration_ns,
                    ..
                } => {
                    fight.agent.searing_duration = *duration_ns;
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
                Effect::StrengthOfEarthTotem {
                    aura, duration_ns, ..
                } => {
                    fight.agent.strength_of_earth = Some(StrengthOfEarth {
                        aura: fight.player_aura(aura)?,
                        duration: *duration_ns,
                    });
                }
                Effect::Stormstrike {
                    spell_id,
                    aura,
                    damage_multiplier,
                    has_main_hand,
                    has_off_hand,
                } => {
                    fight.agent.stormstrike_weapon = *has_main_hand || *has_off_hand;
                    fight.agent.stormstrike =
                        Some(stormstrike::bind(&fight, aura, *damage_multiplier)?);
                    fight.agent.stormstrike_main_hand =
                        has_main_hand.then(|| find(&fight, *spell_id, 1)).flatten();
                }
                Effect::Flurry {
                    trigger_aura,
                    aura,
                    melee_speed_multiplier,
                    charge_icd_ns,
                    ..
                } => {
                    fight.agent.flurry = Some(flurry::bind(
                        &fight,
                        trigger_aura,
                        aura,
                        *melee_speed_multiplier,
                        *charge_icd_ns,
                    )?);
                }
                Effect::RageOfTheFarseer {
                    aura,
                    melee_speed_multiplier,
                    ..
                } => {
                    fight.agent.farseer = Some(rage_of_the_farseer::bind(
                        &fight,
                        aura,
                        *melee_speed_multiplier,
                    )?);
                }
                Effect::RockbiterWeapon {
                    gain_log,
                    expire_log,
                    ..
                } => fight.agent.rockbiter_logs = Some((gain_log.clone(), expire_log.clone())),
                Effect::FlametongueWeapon { hands } => {
                    for hand in hands {
                        let mut triggers = vec![false; fight.spells.len()];
                        for &spell in &hand.trigger_spells {
                            *triggers.get_mut(spell).ok_or_else(|| {
                                format!("Flametongue Weapon names spell {spell}")
                            })? = true;
                        }
                        if hand.spell >= fight.spells.len() {
                            return Err(format!("Flametongue Weapon names spell {}", hand.spell));
                        }
                        fight.agent.flametongue.push((
                            hand.spell,
                            hand.deals_damage,
                            hand.base_damage,
                            triggers,
                        ));
                    }
                }
                Effect::MagmaTotem {
                    pulse_damage,
                    duration_ns,
                    ..
                } => fight.agent.magma = Some((*pulse_damage, *duration_ns)),
                Effect::LightningShield { aura, charges, .. } => {
                    fight.agent.lightning_shield = Some((fight.player_aura(aura)?, *charges));
                }
                Effect::GraceOfAirTotem {
                    aura, duration_ns, ..
                } => {
                    fight.agent.grace_of_air = Some(StrengthOfEarth {
                        aura: fight.player_aura(aura)?,
                        duration: *duration_ns,
                    });
                }
                Effect::ManaSpringTotem {
                    aura, duration_ns, ..
                } => {
                    fight.agent.mana_spring = Some(StrengthOfEarth {
                        aura: fight.player_aura(aura)?,
                        duration: *duration_ns,
                    });
                }
                Effect::FlametongueTotem {
                    aura,
                    trigger_aura,
                    attack_spell,
                    attack_deals_damage,
                    attack_damage,
                    trigger_spells,
                    disabled_by_weapon,
                    duration_ns,
                    ..
                } => {
                    let mut triggers = vec![false; fight.spells.len()];
                    for &spell in trigger_spells {
                        *triggers
                            .get_mut(spell)
                            .ok_or_else(|| format!("Flametongue Totem names spell {spell}"))? =
                            true;
                    }
                    fight.agent.flametongue_totem = Some(totems::FlametongueTotem {
                        aura: fight.player_aura(aura)?,
                        trigger: fight.player_aura(trigger_aura)?,
                        attack: *attack_spell,
                        attack_deals_damage: *attack_deals_damage,
                        attack_damage: *attack_damage,
                        triggers,
                        enabled: !*disabled_by_weapon,
                        duration: *duration_ns,
                    });
                }
                Effect::FrostbrandWeapon {
                    spell_id,
                    base_damage,
                    chances,
                    ..
                } => {
                    let spell = find(&fight, *spell_id, 0).ok_or_else(|| {
                        format!("Frostbrand Weapon spell {spell_id} is not registered")
                    })?;
                    fight.agent.frostbrand = Some((spell, *base_damage));
                    for chance in chances {
                        *fight
                            .agent
                            .frostbrand_chances
                            .get_mut(chance.spell)
                            .ok_or_else(|| {
                                format!("Frostbrand Weapon names spell {}", chance.spell)
                            })? = Some(chance.chance);
                    }
                }
                Effect::ElementalDevastation {
                    trigger_aura,
                    aura,
                    melee_crit,
                } => {
                    let melee = (0..fight.spells.len())
                        .filter(|&spell| {
                            let state = &fight.spells[spell];
                            state.melee_proc && !state.flags.no_spell_mods
                        })
                        .collect();
                    fight.agent.devastation = Some(elemental_devastation::bind(
                        &mut fight,
                        trigger_aura,
                        aura,
                        *melee_crit,
                        melee,
                    )?);
                }
                Effect::ImprovedStormstrike {
                    trigger_aura,
                    aura,
                    proc_chance,
                    spirit_regen_rate_casting,
                    ..
                } => {
                    fight.agent.improved_stormstrike = Some(improved_stormstrike::bind(
                        &fight,
                        trigger_aura,
                        aura,
                        *proc_chance,
                        *spirit_regen_rate_casting,
                    )?);
                }
                Effect::MaelstromWeapon {
                    trigger_aura,
                    aura,
                    per_stack,
                    chances,
                    ..
                } => {
                    for chance in chances {
                        let slot = fight
                            .agent
                            .maelstrom_chances
                            .get_mut(chance.spell)
                            .ok_or_else(|| {
                                format!("Maelstrom Weapon names spell {}", chance.spell)
                            })?;
                        *slot = Some(chance.chance);
                    }
                    let bolts = fight.spells_with_class(&["lightning_bolt"]);
                    fight.agent.maelstrom = Some(maelstrom_weapon::bind(
                        &mut fight,
                        trigger_aura,
                        aura,
                        *per_stack,
                        bolts,
                    )?);
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

    /// Go `cancelFireTotems`: Magma Totem's dot, Searing Totem's dot, then Flametongue Totem's
    /// aura. Totem of Wrath is never set.
    fn cancel_fire_totems(fight: &mut Fight<Self>) {
        let magma = fight.spells.iter().position(|spell| {
            matches!(
                spell.behavior,
                crate::core::fight::SpellBehavior::Class(ShamanSpell::MagmaTotem)
            )
        });
        if let Some(dot) = magma.and_then(|spell| fight.spells[spell].dot) {
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
        }
        let searing = fight.spells.iter().position(|spell| {
            matches!(
                spell.behavior,
                crate::core::fight::SpellBehavior::Class(ShamanSpell::SearingTotem)
            )
        });
        if let Some(dot) = searing.and_then(|spell| fight.spells[spell].dot) {
            let aura = fight.dots[dot].aura;
            fight.deactivate_aura(aura);
        }
        if let Some(aura) = fight
            .agent
            .flametongue_totem
            .as_ref()
            .map(|state| state.aura)
        {
            fight.deactivate_aura(aura);
        }
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
                let expires = fight.now + fight.agent.searing_duration;
                fight.agent.totems.set(Totem::Fire, expires);
            }
            ShamanSpell::EarthShock | ShamanSpell::FrostShock => {
                earth_shock::apply(fight, spell, target)
            }
            ShamanSpell::MagmaTotem => {
                Self::cancel_fire_totems(fight);
                let dot = fight.spells[spell].dot.expect("Magma Totem has a dot");
                fight.apply_dot(dot);
                let (_, duration) = fight.agent.magma.expect("Magma Totem is bound");
                let expires = fight.now + duration;
                fight.agent.totems.set(Totem::Fire, expires);
            }
            ShamanSpell::FlametongueTotem => {
                Self::cancel_fire_totems(fight);
                let state = fight
                    .agent
                    .flametongue_totem
                    .clone()
                    .expect("Flametongue Totem is bound");
                let expires = fight.now + state.duration;
                fight.agent.totems.set(Totem::Fire, expires);
                fight.activate_aura(state.aura);
            }
            ShamanSpell::FlametongueTotemAttack => {
                let state = fight
                    .agent
                    .flametongue_totem
                    .clone()
                    .expect("Flametongue Totem is bound");
                if state.attack_deals_damage {
                    weapon_imbues::hit(fight, spell, target, state.attack_damage);
                }
            }
            ShamanSpell::LightningShield => {
                let (aura, charges) = fight
                    .agent
                    .lightning_shield
                    .expect("Lightning Shield is bound");
                // Go deactivateShields; Water Shield is a talent no supported build casts.
                fight.deactivate_aura(aura);
                fight.activate_aura(aura);
                fight.set_stacks(aura, charges);
            }
            ShamanSpell::GraceOfAirTotem => {
                let totem = fight
                    .agent
                    .grace_of_air
                    .expect("Grace of Air Totem is bound");
                if let Some(previous) = fight.agent.air_totem {
                    fight.deactivate_aura(previous);
                }
                fight.agent.air_totem = Some(totem.aura);
                let expires = totems::strength_of_earth(fight, totem);
                fight.agent.totems.set(Totem::Air, expires);
            }
            ShamanSpell::ManaSpringTotem => {
                // The only water totem in scope is this one, so the previous aura is its own.
                let totem = fight.agent.mana_spring.expect("Mana Spring Totem is bound");
                let expires = totems::strength_of_earth(fight, totem);
                fight.agent.totems.set(Totem::Water, expires);
            }
            ShamanSpell::FlametongueHit(hand) => {
                let (_, deals_damage, base, _) = fight.agent.flametongue[hand];
                if deals_damage {
                    weapon_imbues::hit(fight, spell, target, base);
                }
            }
            ShamanSpell::FrostbrandHit => {
                let (_, base) = fight.agent.frostbrand.expect("Frostbrand Weapon is bound");
                weapon_imbues::hit(fight, spell, target, base);
            }
            ShamanSpell::StormstrikeCast => {
                let state = fight.agent.stormstrike.expect("Stormstrike is bound");
                let main_hand = fight.agent.stormstrike_main_hand;
                state.cast(fight, spell, target, main_hand);
            }
            ShamanSpell::StormstrikeMainHand => stormstrike::strike(fight, spell, target, true),
            ShamanSpell::StormstrikeOffHand => stormstrike::strike(fight, spell, target, false),
            ShamanSpell::RageOfTheFarseer => {
                let aura = fight
                    .agent
                    .farseer
                    .expect("Rage of the Farseer is bound")
                    .aura;
                fight.activate_aura(aura);
            }
            ShamanSpell::StrengthOfEarthTotem => {
                let totem = fight
                    .agent
                    .strength_of_earth
                    .expect("Strength of Earth Totem is bound");
                let expires = totems::strength_of_earth(fight, totem);
                fight.agent.totems.set(Totem::Earth, expires);
            }
            ShamanSpell::SearingTotemAttack => {
                let base = Self::searing_totem(fight).attack_damage;
                searing_totem::attack(fight, spell, target, base);
            }
        }
    }

    fn reset(fight: &mut Fight<Self>) {
        fight.agent.totems = Expirations::default();
        fight.agent.flurry_icd = crate::core::time::STARTING_CD_TIME;
    }

    /// Stormstrike needs a weapon.
    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: ShamanSpell) -> bool {
        match behavior {
            ShamanSpell::StormstrikeCast => fight.agent.stormstrike_weapon,
            _ => true,
        }
    }

    fn totem_expiration(fight: &Fight<Self>, totem: Totem) -> i64 {
        fight.agent.totems.get(totem)
    }

    /// Go `holdMeleeForCast`: a hardcast holds the melee swing until it completes.
    fn modify_cast(fight: &mut Fight<Self>, spell: SpellId, _behavior: ShamanSpell) {
        if !is_class(fight.spells[spell].class_spell.as_deref(), HOLDS_MELEE) {
            return;
        }
        let cast_time = fight.spells[spell].cur_cast.cast_time;
        let cast_time = fight.apply_cast_speed_for_spell(cast_time, spell);
        if cast_time > 0 {
            fight.hold_melee_for_cast(fight.now + cast_time);
        }
    }

    fn caster_damage_multiplier(fight: &Fight<Self>, spell: SpellId) -> Option<f64> {
        fight
            .agent
            .stormstrike
            .and_then(|state| state.caster_multiplier(fight, spell))
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        aura: AuraRef,
        kind: ShamanAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            ShamanAura::ElementalDevastationTrigger => {
                let state = fight
                    .agent
                    .devastation
                    .expect("Elemental Devastation is bound");
                state.on_spell_hit_dealt(fight, spell, result);
            }
            ShamanAura::FlurryTrigger => {
                let state = fight.agent.flurry.expect("Flurry is bound");
                state.on_spell_hit_dealt(fight, spell, result);
            }
            ShamanAura::FlametongueTotemTrigger => {
                let state = fight
                    .agent
                    .flametongue_totem
                    .clone()
                    .expect("Flametongue Totem is bound");
                if state.triggers[spell] && result.landed() {
                    fight.cast(state.attack, result.target);
                }
            }
            ShamanAura::FlametongueTrigger(hand) => {
                let (hit, _, _, ref triggers) = fight.agent.flametongue[hand];
                if triggers[spell] && result.landed() {
                    fight.cast(hit, result.target);
                }
            }
            ShamanAura::FrostbrandTrigger => {
                let (hit, _) = fight.agent.frostbrand.expect("Frostbrand Weapon is bound");
                let chance = fight.agent.frostbrand_chances[spell];
                weapon_imbues::frostbrand_trigger(fight, aura, hit, result, chance);
            }
            ShamanAura::MaelstromWeaponTrigger => {
                let state = fight.agent.maelstrom.expect("Maelstrom Weapon is bound");
                let chance = fight.agent.maelstrom_chances[spell];
                state.on_spell_hit_dealt(fight, spell, result, chance);
            }
            _ => {}
        }
    }

    fn on_spell_hit_taken(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: ShamanAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if kind == ShamanAura::Stormstrike {
            let state = fight.agent.stormstrike.expect("Stormstrike is bound");
            state.on_spell_hit_taken(fight, spell, result);
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: ShamanAura,
        spell: SpellId,
        result: SpellResult,
    ) {
        match kind {
            ShamanAura::FlurryTrigger => {
                let state = fight.agent.flurry.expect("Flurry is bound");
                let white = fight.agent.white[spell];
                let mut icd = fight.agent.flurry_icd;
                state.handle(fight, &result, white, &mut icd);
                fight.agent.flurry_icd = icd;
            }
            ShamanAura::ElementalDevastationTrigger => fight
                .agent
                .devastation
                .expect("Elemental Devastation is bound")
                .grant(fight),
            ShamanAura::ImprovedStormstrikeTrigger => fight
                .agent
                .improved_stormstrike
                .expect("Improved Stormstrike is bound")
                .grant(fight),
            ShamanAura::MaelstromWeaponTrigger => fight
                .agent
                .maelstrom
                .expect("Maelstrom Weapon is bound")
                .grant(fight),
            _ => {}
        }
    }

    fn on_stacks_change(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: ShamanAura,
        _old: i32,
        new: i32,
    ) {
        if kind == ShamanAura::MaelstromWeapon {
            let state = fight.agent.maelstrom.expect("Maelstrom Weapon is bound");
            state.on_stacks_change(fight, new);
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
            // Go CalcPeriodicAoeDamage and DealBatchedPeriodicDamage on the one target.
            ShamanSpell::MagmaTotem => {
                let (base, _) = fight.agent.magma.expect("Magma Totem is bound");
                fight.periodic_damage_tick_on(
                    dot,
                    Side::Target,
                    base,
                    crate::core::fight::Outcome::TickMagicHitAndCrit,
                );
            }
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
        match kind {
            ShamanAura::Clearcasting => Self::focus(fight).on_gain(fight),
            ShamanAura::ElementalDevastation => fight
                .agent
                .devastation
                .expect("Elemental Devastation is bound")
                .on_gain(fight),
            ShamanAura::ImprovedStormstrike => fight
                .agent
                .improved_stormstrike
                .expect("Improved Stormstrike is bound")
                .on_gain(fight),
            ShamanAura::Flurry => fight.agent.flurry.expect("Flurry is bound").on_gain(fight),
            ShamanAura::FlametongueTotem => {
                let state = fight
                    .agent
                    .flametongue_totem
                    .clone()
                    .expect("Flametongue Totem is bound");
                if state.enabled {
                    fight.activate_aura(state.trigger);
                }
            }
            ShamanAura::RageOfTheFarseer => fight
                .agent
                .farseer
                .expect("Rage of the Farseer is bound")
                .on_gain(fight),
            ShamanAura::RockbiterWeapon => {
                if let (Some((line, _)), true) = (&fight.agent.rockbiter_logs, fight.log.is_some())
                {
                    let line = line.clone();
                    fight.player_log(&line);
                }
            }
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: ShamanAura) {
        match kind {
            ShamanAura::Clearcasting => Self::focus(fight).on_expire(fight),
            ShamanAura::ElementalDevastation => fight
                .agent
                .devastation
                .expect("Elemental Devastation is bound")
                .on_expire(fight),
            ShamanAura::ImprovedStormstrike => fight
                .agent
                .improved_stormstrike
                .expect("Improved Stormstrike is bound")
                .on_expire(fight),
            ShamanAura::MaelstromWeapon => fight
                .agent
                .maelstrom
                .expect("Maelstrom Weapon is bound")
                .on_expire(fight),
            ShamanAura::Flurry => fight
                .agent
                .flurry
                .expect("Flurry is bound")
                .on_expire(fight),
            ShamanAura::FlametongueTotem => {
                let trigger = fight
                    .agent
                    .flametongue_totem
                    .as_ref()
                    .expect("Flametongue Totem is bound")
                    .trigger;
                fight.deactivate_aura(trigger);
            }
            ShamanAura::RageOfTheFarseer => fight
                .agent
                .farseer
                .expect("Rage of the Farseer is bound")
                .on_expire(fight),
            ShamanAura::RockbiterWeapon => {
                if let (Some((_, line)), true) = (&fight.agent.rockbiter_logs, fight.log.is_some())
                {
                    let line = line.clone();
                    fight.player_log(&line);
                }
            }
            _ => {}
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
            ShamanAura::ImprovedStormstrikeTrigger => {
                if fight.spells[spell].class_spell.as_deref() == Some("stormstrike_cast") {
                    fight
                        .agent
                        .improved_stormstrike
                        .expect("Improved Stormstrike is bound")
                        .on_cast_complete(fight, spell);
                }
            }
            ShamanAura::MaelstromWeapon => {
                let bolt = fight.spells[spell].class_spell.as_deref() == Some("lightning_bolt");
                fight
                    .agent
                    .maelstrom
                    .expect("Maelstrom Weapon is bound")
                    .on_cast_complete(fight, bolt);
            }
            _ => {}
        }
    }
}
