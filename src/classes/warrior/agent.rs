//! The Warrior class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use crate::{
    contracts::prepared_v2::{ActionId, Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        battle_shout::{self, BattleShout},
        berserker_rage::{self, BerserkerRage},
        bloodrage::{self, Bloodrage},
        bloodthirst::{self, Bloodthirst},
        death_wish::{self, DeathWish},
        execute::{self, Execute},
        hamstring,
        heroic_strike::{self, Queue, Strike},
        mortal_strike, overpower,
        rend::{self, Rend},
        slam,
        spearing_strike::{self, SpearingStrike},
        stances::{self, Stance, StanceCast, StanceLock},
        whirlwind,
    },
    talents::{
        anger_management::{self, AngerManagement},
        bloodthrill::{self, Bloodthrill},
        deep_wounds::{self, DeepWounds},
        flurry::{self, Flurry},
        unbridled_wrath::{self, UnbridledWrath},
        weaponmaster::{self, WeaponmasterSword},
    },
};

/// What a Warrior spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarriorSpell {
    Bloodthirst,
    Whirlwind,
    WhirlwindOffHand,
    Execute,
    Hamstring,
    Bloodrage,
    BerserkerRage,
    DeathWish,
    Recklessness,
    /// The warrior's own Sunder Armor, castable only while no other aura holds its category.
    SunderArmor,
    /// A stance cast or a spell a stance gates, which the gate keeps from ever casting.
    StanceLocked(StanceLock),
    DeepWounds,
    /// Heroic Strike or Cleave, by strike index.
    Strike(usize),
    /// A strike's queue cast, by strike index.
    QueueStrike(usize),
    /// A stance cast, by its position among the stances.
    Stance(usize),
    BattleShout,
    Rend,
    Overpower,
    MortalStrike,
    SpearingStrike,
    Slam,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WarriorAura {
    DeepWoundsTrigger,
    UnbridledWrath,
    FlurryTrigger,
    Flurry,
    OverpowerTrigger,
    DeathWish,
    BerserkerRage,
    /// A strike's queue aura, by strike index.
    Queue(usize),
    BloodthrillTrigger,
    WeaponmasterSword,
}

/// The class periodic tag of a strike's queue delay, plus the strike index.
const QUEUE_TAG: u32 = 16;

/// Warrior state that Go keeps in the `Warrior` struct and its closures.
#[derive(Default)]
pub(crate) struct WarriorAgent {
    stance: Stance,
    bloodthirst: Option<Bloodthirst>,
    whirlwind_off_hand: Option<SpellId>,
    execute: Option<Execute>,
    hamstring: f64,
    bloodrage: Option<Bloodrage>,
    berserker_rage: Option<BerserkerRage>,
    death_wish: Option<DeathWish>,
    recklessness: Option<AuraRef>,
    sunder_blocked: bool,
    deep_wounds: Option<DeepWounds>,
    unbridled_wrath: Option<UnbridledWrath>,
    flurry: Option<Flurry>,
    anger_management: Option<AngerManagement>,
    overpower_window: Option<AuraRef>,
    queue: Queue,
    /// Whether each spell's proc mask holds `ProcMaskEmpty`.
    empty_mask: Vec<bool>,
    /// Whether each spell's proc mask holds a main hand bit.
    main_hand_mask: Vec<bool>,
    /// Whether each spell's proc mask holds a bit of a hand that wields a sword.
    sword_mask: Vec<bool>,
    /// Go `WarriorInputs.DefaultStance`, which every reset restores.
    default_stance: Stance,
    stances: Vec<StanceCast>,
    max_retained_rage: f64,
    battle_shout: Option<BattleShout>,
    rend: Option<Rend>,
    overpower: f64,
    mortal_strike: f64,
    spearing_strike: Option<SpearingStrike>,
    slam: f64,
    bloodthrill: Option<Bloodthrill>,
    weaponmaster: Option<WeaponmasterSword>,
}

/// Whether a prepared input carries the effect of a kind.
fn has_effect(prepared: &PreparedV2, kind: &str) -> bool {
    prepared.effects.iter().any(|effect| effect.kind() == kind)
}

fn id_of(spell: &ExportedSpell) -> ActionId {
    spell.action_id.clone().unwrap_or_default()
}

impl WarriorAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    fn spell(prepared: &PreparedV2, spell: &ExportedSpell) -> Option<WarriorSpell> {
        let has = |kind| has_effect(prepared, kind);
        let id = id_of(spell);
        for effect in &prepared.effects {
            match effect {
                Effect::Bloodrage { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
                    return Some(WarriorSpell::Bloodrage)
                }
                Effect::HeroicStrikeQueue { strikes, .. } => {
                    if let Some(index) = strikes.iter().position(|s| s.spell_id == id.spell_id) {
                        match id.tag {
                            0 => return Some(WarriorSpell::Strike(index)),
                            1 => return Some(WarriorSpell::QueueStrike(index)),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        let stance = |name: &str| {
            prepared.effects.iter().find_map(|effect| match effect {
                Effect::WarriorStances { stances, .. } => stances
                    .iter()
                    .position(|s| s.spell_id == id.spell_id && s.stance == name)
                    .map(WarriorSpell::Stance),
                _ => None,
            })
        };
        match spell.class_spell.as_deref()? {
            "bloodthirst" if has("bloodthirst") => Some(WarriorSpell::Bloodthirst),
            "whirlwind" if has("whirlwind") => Some(WarriorSpell::Whirlwind),
            "whirlwind_off_hand" if has("whirlwind") => Some(WarriorSpell::WhirlwindOffHand),
            "execute" if has("execute") => Some(WarriorSpell::Execute),
            "hamstring" if has("hamstring") => Some(WarriorSpell::Hamstring),
            "berserker_rage" if has("berserker_rage") => Some(WarriorSpell::BerserkerRage),
            "death_wish" if has("death_wish") => Some(WarriorSpell::DeathWish),
            "recklessness" if has("recklessness") => Some(WarriorSpell::Recklessness),
            "sunder_armor" if has("sunder_armor") => Some(WarriorSpell::SunderArmor),
            "deep_wounds" if spell.dot.is_some() && has("deep_wounds") => {
                Some(WarriorSpell::DeepWounds)
            }
            "battle_stance" => stance("battle"),
            "defensive_stance" => stance("defensive"),
            "berserker_stance" => stance("berserker"),
            "battle_shout" if has("battle_shout") => Some(WarriorSpell::BattleShout),
            "rend" if spell.dot.is_some() && has("rend") => Some(WarriorSpell::Rend),
            "overpower" if has("overpower") => Some(WarriorSpell::Overpower),
            "mortal_strike" if has("mortal_strike") => Some(WarriorSpell::MortalStrike),
            "spearing_strike" if has("spearing_strike") => Some(WarriorSpell::SpearingStrike),
            "slam" if has("slam") => Some(WarriorSpell::Slam),
            "retaliation" if has("warrior_stances") => {
                Some(WarriorSpell::StanceLocked(StanceLock::Battle))
            }
            "shield_wall" if has("warrior_stances") => {
                Some(WarriorSpell::StanceLocked(StanceLock::Defensive))
            }
            _ => None,
        }
    }

    /// Aura labels claimed by implemented class effects, as (unit, label, kind).
    fn auras(prepared: &PreparedV2) -> Vec<(String, WarriorAura)> {
        let mut auras = Vec::new();
        for effect in &prepared.effects {
            match effect {
                Effect::DeepWounds { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), WarriorAura::DeepWoundsTrigger))
                }
                Effect::UnbridledWrath { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), WarriorAura::UnbridledWrath))
                }
                Effect::WarriorFlurry {
                    trigger_aura, aura, ..
                } => {
                    auras.push((trigger_aura.clone(), WarriorAura::FlurryTrigger));
                    auras.push((aura.clone(), WarriorAura::Flurry));
                }
                Effect::Bloodthrill { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), WarriorAura::BloodthrillTrigger))
                }
                Effect::WeaponmasterSword { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), WarriorAura::WeaponmasterSword))
                }
                Effect::OverpowerWindow { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), WarriorAura::OverpowerTrigger))
                }
                Effect::DeathWish { aura, .. } => {
                    auras.push((aura.clone(), WarriorAura::DeathWish))
                }
                Effect::BerserkerRage { aura, .. } => {
                    auras.push((aura.clone(), WarriorAura::BerserkerRage))
                }
                Effect::HeroicStrikeQueue { strikes, .. } => {
                    for (index, strike) in strikes.iter().enumerate() {
                        auras.push((strike.queue_aura.clone(), WarriorAura::Queue(index)));
                    }
                }
                _ => {}
            }
        }
        auras
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<WarriorAgent>, String> {
        let auras = Self::auras(prepared);
        let spell = |exported: &ExportedSpell| WarriorAgent::spell(prepared, exported);
        let mut fight = Fight::new(prepared, WarriorAgent::default(), spell, |unit, label| {
            (unit == "player")
                .then(|| {
                    auras
                        .iter()
                        .find(|(name, _)| name == label)
                        .map(|(_, kind)| *kind)
                })
                .flatten()
        })?;
        fight.agent.main_hand_mask = prepared
            .player
            .spells
            .iter()
            .map(|spell| {
                spell
                    .proc_mask
                    .iter()
                    .any(|mask| mask == "ProcMaskMeleeMHAuto" || mask == "ProcMaskMeleeMHSpecial")
            })
            .collect();
        fight.agent.empty_mask = prepared
            .player
            .spells
            .iter()
            .map(|spell| spell.proc_mask.iter().any(|mask| mask == "ProcMaskEmpty"))
            .collect();
        let find = |fight: &Fight<WarriorAgent>, spell_id: i32, tag: i32| {
            fight
                .spells
                .iter()
                .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == tag)
                .ok_or_else(|| format!("spell {spell_id} tag {tag} is not registered"))
        };
        let rage_metrics = |fight: &mut Fight<WarriorAgent>, spell_id: i32| {
            fight.new_rage_metrics(ActionId {
                spell_id,
                ..ActionId::default()
            })
        };
        for effect in &prepared.effects {
            match effect {
                Effect::WarriorStances {
                    default_stance,
                    stances,
                    max_retained_rage,
                } => {
                    fight.agent.default_stance = Stance::from_name(default_stance);
                    fight.agent.stance = fight.agent.default_stance;
                    fight.agent.max_retained_rage = *max_retained_rage;
                    for entry in stances {
                        let aura = fight.player_aura(&entry.aura)?;
                        let metrics = rage_metrics(&mut fight, entry.spell_id);
                        fight.agent.stances.push(StanceCast {
                            stance: Stance::from_name(&entry.stance),
                            aura,
                            metrics,
                        });
                    }
                }
                Effect::BattleShout {
                    aura,
                    value,
                    refresh_threshold_ns,
                    ..
                } => {
                    let aura = fight.player_aura(aura)?;
                    let category = fight
                        .exclusive
                        .iter()
                        .position(|category| category.members.iter().any(|m| m.aura == aura))
                        .ok_or("Battle Shout has no exclusive category".to_string())?;
                    fight.agent.battle_shout = Some(BattleShout {
                        aura,
                        category,
                        value: *value,
                        refresh_threshold: *refresh_threshold_ns,
                    });
                }
                Effect::Rend {
                    spell_id,
                    tick_base,
                    attack_power_per_tick,
                    tick_can_crit,
                    ..
                } => {
                    let spell = find(&fight, *spell_id, 0)?;
                    let dot = fight.spells[spell]
                        .dot
                        .ok_or("Rend has no dot".to_string())?;
                    fight.agent.rend = Some(Rend {
                        dot,
                        tick_base: *tick_base,
                        attack_power_per_tick: *attack_power_per_tick,
                        tick_can_crit: *tick_can_crit,
                    });
                }
                Effect::Overpower { base_damage, .. } => fight.agent.overpower = *base_damage,
                Effect::MortalStrike { base_damage, .. } => {
                    fight.agent.mortal_strike = *base_damage
                }
                Effect::SpearingStrike {
                    weapon_share,
                    mob_multiplier,
                    ..
                } => {
                    fight.agent.spearing_strike = Some(SpearingStrike {
                        weapon_share: *weapon_share,
                        mob_multiplier: *mob_multiplier,
                    })
                }
                Effect::Slam { base_damage, .. } => fight.agent.slam = *base_damage,
                Effect::WeaponmasterSword {
                    trigger_aura,
                    proc_chance,
                    extra_attack_tag,
                    sword_hands,
                } => {
                    let extra_attack = fight
                        .spells
                        .iter()
                        .position(|spell| {
                            spell.id.other_id == "OtherActionAttack"
                                && spell.id.tag == *extra_attack_tag
                        })
                        .ok_or("Weaponmaster has no extra attack".to_string())?;
                    fight.spells[extra_attack].behavior =
                        crate::core::fight::SpellBehavior::MeleeAuto(
                            crate::core::fight::melee::Hand::Main,
                        );
                    let hand_bits = |mask: &str| {
                        (sword_hands.iter().any(|h| h == "main")
                            && matches!(mask, "ProcMaskMeleeMHAuto" | "ProcMaskMeleeMHSpecial"))
                            || (sword_hands.iter().any(|h| h == "off")
                                && matches!(mask, "ProcMaskMeleeOHAuto" | "ProcMaskMeleeOHSpecial"))
                    };
                    fight.agent.sword_mask = prepared
                        .player
                        .spells
                        .iter()
                        .map(|spell| spell.proc_mask.iter().any(|mask| hand_bits(mask)))
                        .collect();
                    fight.agent.weaponmaster = Some(WeaponmasterSword {
                        trigger: fight.player_aura(trigger_aura)?,
                        proc_chance: *proc_chance,
                        extra_attack,
                    });
                }
                Effect::Bloodthirst {
                    attack_power_share,
                    base_damage,
                    ..
                } => {
                    fight.agent.bloodthirst = Some(Bloodthirst {
                        attack_power_share: *attack_power_share,
                        base_damage: *base_damage,
                    })
                }
                Effect::Whirlwind { spell_id, off_hand } => {
                    if *off_hand && prepared.melee.dual_wielding {
                        fight.agent.whirlwind_off_hand = Some(find(&fight, *spell_id, 2)?);
                    }
                }
                Effect::Execute {
                    base_damage,
                    damage_per_rage,
                    ..
                } => {
                    fight.agent.execute = Some(Execute {
                        base_damage: *base_damage,
                        damage_per_rage: *damage_per_rage,
                    })
                }
                Effect::Hamstring { base_damage, .. } => fight.agent.hamstring = *base_damage,
                Effect::Bloodrage {
                    spell_id,
                    instant_rage,
                    rage_per_tick,
                    ticks,
                    period_ns,
                    health_cost,
                    rage_threshold,
                } => {
                    let metrics = rage_metrics(&mut fight, *spell_id);
                    fight.agent.bloodrage = Some(Bloodrage {
                        instant_rage: *instant_rage,
                        rage_per_tick: *rage_per_tick,
                        ticks: *ticks,
                        period: *period_ns,
                        health_cost: *health_cost,
                        rage_threshold: *rage_threshold,
                        metrics,
                    });
                }
                Effect::BerserkerRage {
                    spell_id,
                    aura,
                    rage_gain,
                } => {
                    let metrics = rage_metrics(&mut fight, *spell_id);
                    fight.agent.berserker_rage = Some(BerserkerRage {
                        aura: fight.player_aura(aura)?,
                        rage_gain: *rage_gain,
                        metrics,
                    });
                }
                Effect::DeathWish {
                    aura,
                    physical_multiplier,
                    wait_ns,
                    ..
                } => {
                    fight.agent.death_wish = Some(DeathWish {
                        aura: fight.player_aura(aura)?,
                        physical_multiplier: *physical_multiplier,
                        wait: *wait_ns,
                    })
                }
                Effect::Recklessness { aura, .. } => {
                    fight.agent.recklessness = Some(fight.player_aura(aura)?)
                }
                Effect::SunderArmor { blocked, .. } => fight.agent.sunder_blocked = *blocked,
                Effect::DeepWounds {
                    spell_id,
                    share,
                    tick_can_crit,
                    ..
                } => {
                    let spell = find(&fight, *spell_id, 0)?;
                    let dot = fight.spells[spell]
                        .dot
                        .ok_or("Deep Wounds has no dot".to_string())?;
                    fight.agent.deep_wounds = Some(DeepWounds {
                        spell,
                        dot,
                        share: *share,
                        tick_can_crit: *tick_can_crit,
                    });
                }
                Effect::UnbridledWrath {
                    trigger_aura,
                    spell_id,
                    proc_chance,
                    rage,
                    two_handed,
                    ..
                } => {
                    let metrics = rage_metrics(&mut fight, *spell_id);
                    fight.agent.unbridled_wrath = Some(UnbridledWrath {
                        trigger: fight.player_aura(trigger_aura)?,
                        proc_chance: *proc_chance,
                        rage: *rage,
                        two_handed: *two_handed,
                        metrics,
                    });
                }
                Effect::WarriorFlurry {
                    aura,
                    melee_speed_multiplier,
                    charges,
                    ..
                } => {
                    fight.agent.flurry = Some(Flurry {
                        aura: fight.player_aura(aura)?,
                        melee_speed_multiplier: *melee_speed_multiplier,
                        charges: *charges,
                    })
                }
                Effect::AngerManagement {
                    spell_id,
                    rage,
                    period_ns,
                } => {
                    let metrics = rage_metrics(&mut fight, *spell_id);
                    fight.agent.anger_management = Some(AngerManagement {
                        rage: *rage,
                        period: *period_ns,
                        metrics,
                    });
                }
                Effect::OverpowerWindow { aura, .. } => {
                    fight.agent.overpower_window = Some(fight.player_aura(aura)?)
                }
                Effect::HeroicStrikeQueue {
                    queue_delay_ns,
                    strikes,
                } => {
                    let mut queue = Queue::default();
                    for strike in strikes {
                        queue.strikes.push(Strike {
                            spell: find(&fight, strike.spell_id, 0)?,
                            queue_aura: fight.player_aura(&strike.queue_aura)?,
                            base_damage: strike.base_damage,
                            cleave: strike.cleave,
                        });
                        queue.queued.push(false);
                    }
                    // Go queuedRealismICD: its own timer.
                    fight.timers.push(crate::core::time::STARTING_CD_TIME);
                    queue.realism = Some((fight.timers.len() - 1, *queue_delay_ns));
                    fight.agent.queue = queue;
                }
                _ => {}
            }
        }
        for effect in &prepared.effects {
            if let Effect::Bloodthrill {
                trigger_aura,
                proc_chance,
                window_ns,
                ..
            } = effect
            {
                let rend = fight
                    .agent
                    .rend
                    .ok_or("Bloodthrill needs Rend".to_string())?;
                let window = fight
                    .agent
                    .overpower_window
                    .ok_or("Bloodthrill needs the Overpower window".to_string())?;
                fight.agent.bloodthrill = Some(Bloodthrill {
                    trigger: fight.player_aura(trigger_aura)?,
                    proc_chance: *proc_chance,
                    window_duration: *window_ns,
                    overpower_window: window,
                    rend_dot: rend.dot,
                });
            }
        }
        Ok(fight)
    }
}

impl Agent for WarriorAgent {
    type Spell = WarriorSpell;
    type Aura = WarriorAura;

    fn apply_effects(
        fight: &mut Fight<Self>,
        spell: SpellId,
        target: Side,
        behavior: WarriorSpell,
    ) {
        match behavior {
            WarriorSpell::Bloodthirst => {
                let params = fight.agent.bloodthirst.expect("Bloodthirst is bound");
                bloodthirst::apply(fight, spell, target, params);
            }
            WarriorSpell::Whirlwind => {
                let off_hand = fight.agent.whirlwind_off_hand;
                whirlwind::apply(fight, spell, target, off_hand);
            }
            WarriorSpell::WhirlwindOffHand => whirlwind::apply_off_hand(fight, spell, target),
            WarriorSpell::Execute => {
                let params = fight.agent.execute.expect("Execute is bound");
                execute::apply(fight, spell, target, params);
            }
            WarriorSpell::Hamstring => {
                let base = fight.agent.hamstring;
                hamstring::apply(fight, spell, target, base);
            }
            WarriorSpell::Bloodrage => {
                let params = fight.agent.bloodrage.expect("Bloodrage is bound");
                bloodrage::apply(fight, params);
            }
            WarriorSpell::BerserkerRage => {
                let params = fight.agent.berserker_rage.expect("Berserker Rage is bound");
                berserker_rage::apply(fight, params);
            }
            WarriorSpell::DeathWish => {
                let params = fight.agent.death_wish.expect("Death Wish is bound");
                death_wish::apply(fight, params);
            }
            WarriorSpell::Recklessness => {
                let aura = fight.agent.recklessness.expect("Recklessness is bound");
                fight.activate_aura(aura);
            }
            WarriorSpell::DeepWounds => {
                let params = fight.agent.deep_wounds.expect("Deep Wounds is bound");
                deep_wounds::apply(fight, spell, target, params);
            }
            WarriorSpell::Strike(index) => {
                let params = fight.agent.queue.strikes[index];
                heroic_strike::strike(fight, spell, target, params);
                if let Some(current) = fight.agent.queue.current {
                    let aura = fight.agent.queue.strikes[current].queue_aura;
                    fight.deactivate_aura(aura);
                }
            }
            WarriorSpell::QueueStrike(index) => {
                let mut queue = std::mem::take(&mut fight.agent.queue);
                let armed = heroic_strike::queue(fight, &mut queue, index);
                let delay = queue.realism.map_or(0, |(_, delay)| delay);
                fight.agent.queue = queue;
                if armed.is_some() {
                    let tag = QUEUE_TAG + index as u32;
                    fight.start_class_periodic(tag, delay, 1, crate::core::fight::PRIORITY_GCD);
                }
            }
            WarriorSpell::Stance(index) => {
                let cast = fight.agent.stances[index];
                let retained = fight.agent.max_retained_rage;
                fight.agent.stance = stances::change(fight, cast, retained);
            }
            WarriorSpell::BattleShout => {
                let params = fight.agent.battle_shout.expect("Battle Shout is bound");
                battle_shout::apply(fight, spell, target, params);
            }
            WarriorSpell::Rend => {
                let params = fight.agent.rend.expect("Rend is bound");
                rend::apply(fight, spell, target, params);
            }
            WarriorSpell::Overpower => {
                let base = fight.agent.overpower;
                let window = fight.agent.overpower_window.expect("the window is bound");
                overpower::apply(fight, spell, target, base, window);
            }
            WarriorSpell::MortalStrike => {
                let base = fight.agent.mortal_strike;
                mortal_strike::apply(fight, spell, target, base);
            }
            WarriorSpell::SpearingStrike => {
                let params = fight
                    .agent
                    .spearing_strike
                    .expect("Spearing Strike is bound");
                spearing_strike::apply(fight, spell, target, params);
            }
            WarriorSpell::Slam => {
                let base = fight.agent.slam;
                slam::apply(fight, spell, target, base);
            }
            WarriorSpell::SunderArmor | WarriorSpell::StanceLocked(_) => {
                panic!("the gate keeps {behavior:?} from being cast")
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: WarriorSpell) -> bool {
        let stance = fight.agent.stance;
        match behavior {
            // whirlwind.go, recklessness.go and berserker_rage.go: Berserker Stance.
            WarriorSpell::Whirlwind | WarriorSpell::Recklessness | WarriorSpell::BerserkerRage => {
                stance == Stance::Berserker
            }
            // execute.go: Berserker or Battle Stance, in the execute phase.
            WarriorSpell::Execute => {
                matches!(stance, Stance::Berserker | Stance::Battle) && fight.is_execute_phase_20()
            }
            // hamstring.go: Battle or Berserker Stance.
            WarriorSpell::Hamstring => matches!(stance, Stance::Berserker | Stance::Battle),
            // sunder_armor.go CanApplySunderAura: the warrior's own stacks, or an empty category.
            WarriorSpell::SunderArmor => !fight.agent.sunder_blocked,
            WarriorSpell::StanceLocked(lock) => lock.allows(stance),
            // stances.go: a stance cast needs another stance.
            WarriorSpell::Stance(index) => fight.agent.stances[index].stance != stance,
            WarriorSpell::BattleShout => {
                battle_shout::condition(fight, fight.agent.battle_shout.expect("bound"))
            }
            // rend.go: Battle or Defensive Stance.
            WarriorSpell::Rend => matches!(stance, Stance::Battle | Stance::Defensive),
            // overpower.go: Battle Stance with the window open.
            WarriorSpell::Overpower => {
                stance == Stance::Battle
                    && fight
                        .agent
                        .overpower_window
                        .is_some_and(|window| fight.aura(window).active)
            }
            // talents_arms.go: Spearing Strike requires Battle Stance.
            WarriorSpell::SpearingStrike => stance == Stance::Battle,
            WarriorSpell::QueueStrike(index) => {
                heroic_strike::queue_condition(fight, &fight.agent.queue, index)
            }
            _ => true,
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: WarriorSpell) -> bool {
        match behavior {
            WarriorSpell::Bloodrage => {
                bloodrage::should_activate(fight, fight.agent.bloodrage.expect("bound"))
            }
            WarriorSpell::BerserkerRage => {
                berserker_rage::should_activate(fight, fight.agent.berserker_rage.expect("bound"))
            }
            // retaliation.go and shield_wall.go: manual use only for a DPS warrior.
            WarriorSpell::StanceLocked(StanceLock::Battle | StanceLock::Defensive) => false,
            _ => true,
        }
    }

    fn replace_mh_swing(fight: &mut Fight<Self>, swing: SpellId) -> SpellId {
        // heroic_strike_cleave.go TryHSOrCleave.
        let Some(current) = fight.agent.queue.current else {
            return swing;
        };
        let strike = fight.agent.queue.strikes[current];
        if !fight.aura(strike.queue_aura).active {
            return swing;
        }
        if !fight.can_cast(strike.spell) {
            fight.deactivate_aura(strike.queue_aura);
            return swing;
        }
        strike.spell
    }

    fn on_periodic(fight: &mut Fight<Self>, tag: u32) {
        match tag {
            anger_management::PERIODIC_TAG => {
                let params = fight
                    .agent
                    .anger_management
                    .expect("Anger Management is bound");
                anger_management::tick(fight, params);
            }
            bloodrage::PERIODIC_TAG => {
                let params = fight.agent.bloodrage.expect("Bloodrage is bound");
                bloodrage::tick(fight, params);
            }
            tag if tag >= QUEUE_TAG => {
                let index = (tag - QUEUE_TAG) as usize;
                let aura = fight.agent.queue.strikes[index].queue_aura;
                fight.activate_aura(aura);
                fight.agent.queue.queued[index] = false;
            }
            _ => {}
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: WarriorSpell) {
        match behavior {
            WarriorSpell::DeepWounds => {
                let params = fight.agent.deep_wounds.expect("Deep Wounds is bound");
                deep_wounds::tick(fight, dot, params.tick_can_crit);
            }
            WarriorSpell::Rend => {
                let params = fight.agent.rend.expect("Rend is bound");
                rend::tick(fight, dot, params);
            }
            _ => {}
        }
    }

    fn reset(fight: &mut Fight<Self>) {
        // Go Warrior.Reset: the default stance.
        fight.agent.stance = fight.agent.default_stance;
        if let Some(params) = fight.agent.anger_management {
            anger_management::reset(fight, params);
        }
        let queue = &mut fight.agent.queue;
        queue.current = None;
        queue.queued.iter_mut().for_each(|queued| *queued = false);
    }

    fn on_gain(fight: &mut Fight<Self>, aura: AuraRef, kind: WarriorAura) {
        match kind {
            WarriorAura::Flurry => flurry::on_gain(fight, fight.agent.flurry.expect("bound")),
            WarriorAura::DeathWish => {
                death_wish::on_gain(fight, fight.agent.death_wish.expect("bound"))
            }
            WarriorAura::BerserkerRage => berserker_rage::on_gain(fight),
            WarriorAura::Queue(index) => {
                if let Some(current) = fight.agent.queue.current {
                    let current = fight.agent.queue.strikes[current].queue_aura;
                    fight.deactivate_aura(current);
                }
                let _ = aura;
                fight.agent.queue.current = Some(index);
            }
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: WarriorAura) {
        match kind {
            WarriorAura::Flurry => flurry::on_expire(fight, fight.agent.flurry.expect("bound")),
            WarriorAura::DeathWish => {
                death_wish::on_expire(fight, fight.agent.death_wish.expect("bound"))
            }
            WarriorAura::BerserkerRage => berserker_rage::on_expire(fight),
            WarriorAura::Queue(_) => fight.agent.queue.current = None,
            _ => {}
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: WarriorAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            WarriorAura::DeepWoundsTrigger => {
                let params = fight.agent.deep_wounds.expect("Deep Wounds is bound");
                let empty = fight.agent.empty_mask[spell];
                deep_wounds::on_hit(fight, params, spell, empty, result);
            }
            WarriorAura::UnbridledWrath => {
                let params = fight.agent.unbridled_wrath.expect("bound");
                unbridled_wrath::on_hit(fight, params, spell, result);
            }
            WarriorAura::FlurryTrigger => {
                let params = fight.agent.flurry.expect("Flurry is bound");
                flurry::on_hit(fight, params, spell, result);
            }
            WarriorAura::BloodthrillTrigger => {
                let params = fight.agent.bloodthrill.expect("Bloodthrill is bound");
                let main_hand = fight.agent.main_hand_mask[spell];
                bloodthrill::on_hit(fight, params, spell, main_hand, result);
            }
            WarriorAura::WeaponmasterSword => {
                let params = fight.agent.weaponmaster.expect("Weaponmaster is bound");
                let sword = fight.agent.sword_mask[spell];
                weaponmaster::on_hit(fight, params, spell, sword, result);
            }
            WarriorAura::OverpowerTrigger => {
                let window = fight.agent.overpower_window.expect("bound");
                overpower::on_hit(fight, window, spell, result);
            }
            _ => {}
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: WarriorAura,
        _spell: SpellId,
        _result: SpellResult,
    ) {
        match kind {
            WarriorAura::UnbridledWrath => {
                let params = fight.agent.unbridled_wrath.expect("bound");
                unbridled_wrath::grant(fight, params);
            }
            WarriorAura::BloodthrillTrigger => {
                let params = fight.agent.bloodthrill.expect("bound");
                bloodthrill::open(fight, params);
            }
            _ => {}
        }
    }
}
