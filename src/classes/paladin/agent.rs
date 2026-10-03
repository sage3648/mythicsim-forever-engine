//! The Paladin class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{ConsecrationRank, Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    spells::{
        holy::{self, DivineFavor},
        judgement, judgement_refresh,
        seals::{self, Seals},
        strikes,
    },
    talents::{
        protection::{
            self, HolyShield, IronCreed, Reckoning, Redoubt, RighteousFury, ShieldSpecialization,
            SwiftJudgement,
        },
        retribution::{self, SanctifiedJudgement, Vengeance, Vindication},
        twist_of_light::{self, Echo},
    },
};

/// What a Paladin spell does when its effects apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaladinSpell {
    /// A castable seal, by its position among the bound seals.
    Seal(usize),
    SealOfCommandProc,
    /// A Seal of Righteousness rank's damage spell, by its seal's position.
    SealOfRighteousnessProc(usize),
    JudgementOfCommand,
    JudgementOfRighteousness,
    Judgement,
    /// A Holy Strike rank, by its position in the effect.
    HolyStrike(usize),
    HammerOfWrath,
    /// A Consecration rank, by its position in the effect.
    Consecration(usize),
    /// A Holy Shock rank's damage, by its position in the effect.
    HolyShock(usize),
    DivineFavor,
    RighteousFury,
    SwiftJudgement,
    TemplarsBulwark,
    HolyShield,
    HolyShieldProc,
}

/// Class auras with Rust behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PaladinAura {
    JudgementRefresh,
    /// A seal's aura, by the seal's position.
    Seal(usize),
    VengeanceTrigger,
    Vengeance,
    VindicationTrigger,
    SanctifiedJudgement,
    SacredArbiter,
    TwistOfLight,
    DivineFavor,
    RighteousFury,
    InstrumentOfLaw,
    SwiftJudgement,
    RedoubtTrigger,
    Redoubt,
    ShieldSpecialization,
    ReckoningBlock,
    ReckoningCrit,
    IronCreedTrigger,
    IronCreed,
    HolyShield,
}

/// Paladin state that Go keeps in the `Paladin` struct and its closures.
#[derive(Default)]
pub(crate) struct PaladinAgent {
    judgement_refresh: Option<Rc<judgement_refresh::JudgementRefresh>>,
    pub(crate) seals: Seals,
    judgement_wake_delay: i64,
    holy_strike: Vec<f64>,
    consecration: Vec<ConsecrationRank>,
    vengeance: Option<Vengeance>,
    vindication: Option<Vindication>,
    sanctified_judgement: Option<SanctifiedJudgement>,
    sacred_arbiter: Rc<Vec<AuraRef>>,
    pub(crate) echoes: Vec<Echo>,
    holy_shock: Vec<(f64, f64)>,
    divine_favor: Option<Rc<DivineFavor>>,
    /// Consecrated Ground's mark on the target and its Holy damage multiplier.
    pub(crate) consecrated_ground: Option<(AuraRef, f64)>,
    pub(crate) righteous_fury: Option<RighteousFury>,
    swift_judgement: Option<Rc<SwiftJudgement>>,
    redoubt: Option<Rc<Redoubt>>,
    shield_specialization: Option<Rc<ShieldSpecialization>>,
    reckoning: Option<Rc<Reckoning>>,
    iron_creed: Option<Rc<IronCreed>>,
    holy_shield: Option<Rc<HolyShield>>,
    forbearance: Option<AuraRef>,
}

impl PaladinAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    fn spell(prepared: &PreparedV2, spell: &ExportedSpell) -> Option<PaladinSpell> {
        let effects = &prepared.effects;
        let id = spell.action_id.clone().unwrap_or_default();
        // Holy Shield's damage carries its rank's tag 2.
        if spell.class_spell.as_deref() == Some("holy_shield_proc") {
            return effects.iter().find_map(|effect| match effect {
                Effect::HolyShield { spell_id, .. } if *spell_id == id.spell_id && id.tag == 2 => {
                    Some(PaladinSpell::HolyShieldProc)
                }
                _ => None,
            });
        }
        if id.tag != 0 {
            return None;
        }
        let damage = spell.damage_effect.is_some();
        let has = |kind: &str| effects.iter().any(|effect| effect.kind() == kind);
        match spell.class_spell.as_deref()? {
            "seal_of_command" | "seal_of_righteousness" => {
                seals::seal_index(effects, id.spell_id).map(PaladinSpell::Seal)
            }
            "seal_of_command_proc" if has("seal_of_command") => {
                Some(PaladinSpell::SealOfCommandProc)
            }
            "seal_of_righteousness_proc" => effects.iter().find_map(|effect| match effect {
                Effect::SealOfRighteousness { ranks, .. } => ranks
                    .iter()
                    .find(|rank| rank.proc_spell_id == id.spell_id)
                    .and_then(|rank| seals::seal_index(effects, rank.seal_spell_id))
                    .map(PaladinSpell::SealOfRighteousnessProc),
                _ => None,
            }),
            "judgement_of_command" if damage && has("seal_of_command") => {
                Some(PaladinSpell::JudgementOfCommand)
            }
            "judgement_of_righteousness" if damage && has("seal_of_righteousness") => {
                Some(PaladinSpell::JudgementOfRighteousness)
            }
            "judgement" if has("judgement") => Some(PaladinSpell::Judgement),
            "holy_strike" if damage => effects.iter().find_map(|effect| match effect {
                Effect::HolyStrike { ranks } => ranks
                    .iter()
                    .position(|rank| rank.spell_id == id.spell_id)
                    .map(PaladinSpell::HolyStrike),
                _ => None,
            }),
            "hammer_of_wrath" if damage && has("hammer_of_wrath") => {
                Some(PaladinSpell::HammerOfWrath)
            }
            "consecration" if spell.dot.is_some() => {
                effects.iter().find_map(|effect| match effect {
                    Effect::Consecration { ranks, .. } => ranks
                        .iter()
                        .position(|rank| rank.spell_id == id.spell_id)
                        .map(PaladinSpell::Consecration),
                    _ => None,
                })
            }
            "holy_shock" => effects.iter().find_map(|effect| match effect {
                Effect::HolyShock { ranks } => ranks
                    .iter()
                    .position(|rank| rank.spell_id == id.spell_id)
                    .map(PaladinSpell::HolyShock),
                _ => None,
            }),
            "righteous_fury" if has("righteous_fury") => Some(PaladinSpell::RighteousFury),
            "swift_judgement" if has("swift_judgement") => Some(PaladinSpell::SwiftJudgement),
            "templars_bulwark" if has("templars_bulwark") => Some(PaladinSpell::TemplarsBulwark),
            "holy_shield" => effects.iter().find_map(|effect| match effect {
                Effect::HolyShield { spell_id, .. } if *spell_id == id.spell_id => {
                    Some(PaladinSpell::HolyShield)
                }
                _ => None,
            }),
            "divine_favor" => effects.iter().find_map(|effect| match effect {
                Effect::DivineFavor { spell_id, .. } if *spell_id == id.spell_id => {
                    Some(PaladinSpell::DivineFavor)
                }
                _ => None,
            }),
            _ => None,
        }
    }

    /// The class auras of a prepared input, by unit and label.
    fn auras(prepared: &PreparedV2) -> Vec<(String, PaladinAura)> {
        let mut auras = Vec::new();
        let mut seal = 0;
        for effect in &prepared.effects {
            match effect {
                Effect::JudgementRefresh { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::JudgementRefresh))
                }
                Effect::Vengeance {
                    trigger_aura, aura, ..
                } => {
                    auras.push((trigger_aura.clone(), PaladinAura::VengeanceTrigger));
                    auras.push((aura.clone(), PaladinAura::Vengeance));
                }
                Effect::Vindication { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::VindicationTrigger))
                }
                Effect::SanctifiedJudgement { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::SanctifiedJudgement))
                }
                Effect::SacredArbiter { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::SacredArbiter))
                }
                Effect::TwistOfLight { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::TwistOfLight))
                }
                Effect::DivineFavor { aura, .. } => {
                    auras.push((aura.clone(), PaladinAura::DivineFavor))
                }
                Effect::RighteousFury {
                    aura,
                    instrument_of_law,
                    ..
                } => {
                    auras.push((aura.clone(), PaladinAura::RighteousFury));
                    if let Some(law) = instrument_of_law {
                        auras.push((law.aura.clone(), PaladinAura::InstrumentOfLaw));
                    }
                }
                Effect::SwiftJudgement { aura, .. } => {
                    auras.push((aura.clone(), PaladinAura::SwiftJudgement))
                }
                Effect::Redoubt {
                    trigger_aura, aura, ..
                } => {
                    auras.push((trigger_aura.clone(), PaladinAura::RedoubtTrigger));
                    auras.push((aura.clone(), PaladinAura::Redoubt));
                }
                Effect::ShieldSpecialization { trigger_aura, .. } => {
                    auras.push((trigger_aura.clone(), PaladinAura::ShieldSpecialization))
                }
                Effect::Reckoning {
                    block_aura,
                    crit_aura,
                    ..
                } => {
                    auras.push((block_aura.clone(), PaladinAura::ReckoningBlock));
                    auras.push((crit_aura.clone(), PaladinAura::ReckoningCrit));
                }
                Effect::IronCreed {
                    trigger_aura, aura, ..
                } => {
                    auras.push((trigger_aura.clone(), PaladinAura::IronCreedTrigger));
                    auras.push((aura.clone(), PaladinAura::IronCreed));
                }
                Effect::HolyShield { aura, .. } => {
                    auras.push((aura.clone(), PaladinAura::HolyShield))
                }
                _ => {}
            }
        }
        // Seal auras in the order seals::bind builds the seals.
        for effect in &prepared.effects {
            if let Effect::SealOfCommand { ranks, .. } = effect {
                for rank in ranks {
                    auras.push((rank.aura.clone(), PaladinAura::Seal(seal)));
                    seal += 1;
                }
            }
        }
        for effect in &prepared.effects {
            if let Effect::SealOfRighteousness { ranks, .. } = effect {
                for rank in ranks {
                    auras.push((rank.aura.clone(), PaladinAura::Seal(seal)));
                    seal += 1;
                }
            }
        }
        auras
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<PaladinAgent>, String> {
        let auras = Self::auras(prepared);
        let spell = |exported: &ExportedSpell| PaladinAgent::spell(prepared, exported);
        let mut fight = Fight::new(prepared, PaladinAgent::default(), spell, |unit, label| {
            (unit == "player")
                .then(|| {
                    auras
                        .iter()
                        .find(|(name, _)| name == label)
                        .map(|(_, kind)| *kind)
                })
                .flatten()
        })?;
        let effects = &prepared.effects;
        fight.agent.seals = seals::bind(&mut fight, effects)?;
        for effect in effects {
            match effect {
                Effect::JudgementRefresh {
                    judgement_auras, ..
                } => {
                    let bound = judgement_refresh::bind(&fight, judgement_auras)?;
                    fight.agent.judgement_refresh = Some(Rc::new(bound));
                }
                Effect::Judgement { wake_delay_ns, .. } => {
                    fight.agent.judgement_wake_delay = *wake_delay_ns;
                }
                Effect::HolyStrike { ranks } => {
                    fight.agent.holy_strike =
                        ranks.iter().map(|rank| rank.weapon_percent).collect();
                }
                Effect::Consecration {
                    ranks,
                    consecrated_ground,
                } => {
                    fight.agent.consecration = ranks.clone();
                    if let Some(ground) = consecrated_ground {
                        let aura = fight.trackers[Side::Target.index()]
                            .find(&ground.aura)
                            .map(|index| AuraRef {
                                side: Side::Target,
                                index,
                            })
                            .ok_or_else(|| {
                                format!("target aura {} is not registered", ground.aura)
                            })?;
                        fight.agent.consecrated_ground = Some((aura, ground.multiplier));
                    }
                }
                Effect::HolyShock { ranks } => {
                    fight.agent.holy_shock =
                        ranks.iter().map(|rank| (rank.min, rank.max)).collect();
                }
                Effect::RighteousFury {
                    aura,
                    threat_percent,
                    instrument_of_law,
                    ..
                } => {
                    let law = instrument_of_law
                        .as_ref()
                        .map(|law| (law.aura.as_str(), law.multiplier));
                    let bound =
                        RighteousFury::bind(&mut fight, effects, aura, *threat_percent, law)?;
                    fight.agent.righteous_fury = Some(bound);
                }
                Effect::SwiftJudgement {
                    aura,
                    cost_percent_add,
                    ..
                } => {
                    let bound = SwiftJudgement::bind(&mut fight, aura, *cost_percent_add)?;
                    fight.agent.swift_judgement = Some(Rc::new(bound));
                }
                Effect::TemplarsBulwark { .. } => {
                    fight.agent.forbearance = fight.player_aura("Forbearance").ok();
                }
                Effect::Redoubt {
                    aura, proc_chance, ..
                } => {
                    let bound = Redoubt::bind(&fight, effects, aura, *proc_chance)?;
                    fight.agent.redoubt = Some(Rc::new(bound));
                }
                Effect::ShieldSpecialization {
                    proc_chance,
                    mana_share,
                    metrics_action_id,
                    ..
                } => {
                    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
                    fight.agent.shield_specialization = Some(Rc::new(ShieldSpecialization {
                        chance: *proc_chance,
                        mana_share: *mana_share,
                        metrics,
                    }));
                }
                Effect::Reckoning {
                    block_chance,
                    crit_chance,
                    ..
                } => {
                    fight.agent.reckoning = Some(Rc::new(Reckoning {
                        block_chance: *block_chance,
                        crit_chance: *crit_chance,
                    }));
                }
                Effect::IronCreed { aura, .. } => {
                    let bound = IronCreed::bind(&fight, effects, aura)?;
                    fight.agent.iron_creed = Some(Rc::new(bound));
                }
                Effect::HolyShield {
                    proc_spell,
                    aura,
                    charges,
                    damage,
                    ..
                } => {
                    let bound =
                        HolyShield::bind(&fight, effects, aura, *proc_spell, *charges, *damage)?;
                    fight.agent.holy_shield = Some(Rc::new(bound));
                }
                Effect::DivineFavor {
                    aura, crit, spells, ..
                } => {
                    let bound = DivineFavor::bind(&mut fight, aura, *crit, spells)?;
                    fight.agent.divine_favor = Some(Rc::new(bound));
                }
                Effect::Vengeance {
                    aura,
                    per_stack,
                    spells,
                    ..
                } => {
                    let bound = Vengeance::bind(&mut fight, aura, *per_stack, spells)?;
                    fight.agent.vengeance = Some(bound);
                }
                Effect::Vindication {
                    proc_chance,
                    aura,
                    target_aura,
                    ..
                } => {
                    let target_aura = fight.trackers[Side::Target.index()]
                        .find(target_aura)
                        .map(|index| AuraRef {
                            side: Side::Target,
                            index,
                        })
                        .ok_or_else(|| format!("target aura {target_aura} is not registered"))?;
                    fight.agent.vindication = Some(Vindication {
                        chance: *proc_chance,
                        aura: fight.player_aura(aura)?,
                        target_aura,
                    });
                }
                Effect::SanctifiedJudgement {
                    proc_chance,
                    refund,
                    metrics_action_id,
                    ..
                } => {
                    let metrics = fight.new_mana_metrics(metrics_action_id.clone());
                    fight.agent.sanctified_judgement = Some(SanctifiedJudgement {
                        chance: *proc_chance,
                        refund: *refund,
                        metrics,
                    });
                }
                Effect::SacredArbiter {
                    judgement_auras, ..
                } => {
                    let bound = judgement_refresh::bind(&fight, judgement_auras)?;
                    fight.agent.sacred_arbiter = Rc::new(bound.judgements().to_vec());
                }
                Effect::TwistOfLight { echoes, .. } => {
                    fight.agent.echoes = twist_of_light::bind(&fight, echoes)?;
                }
                _ => {}
            }
        }
        Ok(fight)
    }
}

impl PaladinAgent {
    /// A bound talent or spell state.
    fn bound<T>(state: &Option<Rc<T>>) -> Rc<T> {
        Rc::clone(state.as_ref().expect("the effect is bound"))
    }

    /// The gain and loss of the tanking auras.
    fn protection_toggle(fight: &mut Fight<Self>, kind: PaladinAura, active: bool) {
        match kind {
            PaladinAura::RighteousFury => {
                let fury = fight
                    .agent
                    .righteous_fury
                    .clone()
                    .expect("Righteous Fury is bound");
                if active {
                    fury.on_gain(fight);
                } else {
                    fury.on_expire(fight);
                }
            }
            PaladinAura::InstrumentOfLaw => {
                let fury = fight
                    .agent
                    .righteous_fury
                    .as_ref()
                    .expect("Righteous Fury is bound");
                let multiplier = fury.law.expect("Instrument of Law is bound").1;
                if active {
                    protection::law_gain(fight, multiplier);
                } else {
                    protection::law_expire(fight, multiplier);
                }
            }
            PaladinAura::SwiftJudgement => {
                let swift = Self::bound(&fight.agent.swift_judgement);
                if active {
                    swift.on_gain(fight);
                } else {
                    swift.on_expire(fight);
                }
            }
            PaladinAura::Redoubt => Self::bound(&fight.agent.redoubt).toggle(fight, active),
            PaladinAura::IronCreed => Self::bound(&fight.agent.iron_creed).toggle(fight, active),
            PaladinAura::HolyShield => Self::bound(&fight.agent.holy_shield).toggle(fight, active),
            _ => {}
        }
    }

    fn divine_favor(fight: &Fight<Self>) -> Rc<DivineFavor> {
        Rc::clone(
            fight
                .agent
                .divine_favor
                .as_ref()
                .expect("Divine Favor is bound"),
        )
    }
}

impl Agent for PaladinAgent {
    type Spell = PaladinSpell;
    type Aura = PaladinAura;

    fn apply_effects(
        fight: &mut Fight<Self>,
        spell: SpellId,
        target: Side,
        behavior: PaladinSpell,
    ) {
        match behavior {
            PaladinSpell::Seal(seal) => seals::apply(fight, seal),
            PaladinSpell::SealOfCommandProc => seals::command_proc(fight, spell, target),
            PaladinSpell::SealOfRighteousnessProc(seal) => {
                seals::righteousness_proc(fight, spell, target, seal)
            }
            PaladinSpell::JudgementOfCommand => judgement::command(fight, spell, target),
            PaladinSpell::JudgementOfRighteousness => {
                judgement::righteousness(fight, spell, target)
            }
            PaladinSpell::Judgement => {
                let delay = fight.agent.judgement_wake_delay;
                judgement::apply(fight, spell, target, delay);
            }
            PaladinSpell::HolyStrike(rank) => {
                let weapon_percent = fight.agent.holy_strike[rank];
                strikes::holy_strike(fight, spell, target, weapon_percent);
            }
            PaladinSpell::HammerOfWrath => strikes::hammer_of_wrath(fight, spell, target),
            PaladinSpell::Consecration(_) => strikes::consecration(fight, spell, target),
            PaladinSpell::HolyShock(rank) => {
                let roll = fight.agent.holy_shock[rank];
                holy::holy_shock(fight, spell, target, roll);
            }
            PaladinSpell::DivineFavor => Self::divine_favor(fight).apply(fight),
            PaladinSpell::RighteousFury => {
                let aura = fight
                    .agent
                    .righteous_fury
                    .as_ref()
                    .expect("Righteous Fury is bound")
                    .aura;
                fight.activate_aura(aura);
            }
            PaladinSpell::SwiftJudgement => Self::bound(&fight.agent.swift_judgement).apply(fight),
            PaladinSpell::TemplarsBulwark => {
                unreachable!(
                    "the gate admits Templar's Bulwark only as a survival cooldown Go never fires"
                )
            }
            PaladinSpell::HolyShield => Self::bound(&fight.agent.holy_shield).apply(fight),
            PaladinSpell::HolyShieldProc => {
                Self::bound(&fight.agent.holy_shield).proc_damage(fight, spell, target)
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: PaladinSpell) -> bool {
        match behavior {
            PaladinSpell::Judgement => seals::active_seal(fight).is_some(),
            PaladinSpell::HammerOfWrath => fight.is_execute_phase_20(),
            PaladinSpell::TemplarsBulwark => !fight
                .agent
                .forbearance
                .is_some_and(|aura| fight.aura(aura).active),
            _ => true,
        }
    }

    fn should_activate(fight: &Fight<Self>, _spell: SpellId, behavior: PaladinSpell) -> bool {
        match behavior {
            PaladinSpell::SwiftJudgement => fight
                .agent
                .swift_judgement
                .as_ref()
                .expect("Swift Judgement is bound")
                .should_activate(fight),
            _ => true,
        }
    }

    fn modify_cast(fight: &mut Fight<Self>, spell: SpellId, behavior: PaladinSpell) {
        if behavior == PaladinSpell::HammerOfWrath {
            strikes::hammer_of_wrath_modify_cast(fight, spell);
        }
    }

    fn on_dot_tick(fight: &mut Fight<Self>, dot: DotId, behavior: PaladinSpell) {
        if let PaladinSpell::Consecration(rank) = behavior {
            let rank = fight.agent.consecration[rank].clone();
            strikes::consecration_tick(fight, dot, &rank);
        }
    }

    fn reset(fight: &mut Fight<Self>) {
        fight.agent.seals.current = None;
        if let Some(fury) = fight.agent.righteous_fury.as_mut() {
            fury.law_at_reset = true;
        }
        twist_of_light::reset(fight);
    }

    fn caster_damage_multiplier(fight: &Fight<Self>, spell: SpellId) -> Option<f64> {
        // Consecrated Ground's handler: Holy spells, while the target is marked.
        let (aura, multiplier) = fight.agent.consecrated_ground?;
        fight.aura(aura).active.then(|| {
            if fight.spells[spell].school & 2 != 0 {
                multiplier
            } else {
                1.0
            }
        })
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: PaladinAura) {
        Self::protection_toggle(fight, kind, true);
        if kind == PaladinAura::DivineFavor {
            Self::divine_favor(fight).on_gain(fight);
        }
        if kind == PaladinAura::Vengeance {
            fight
                .agent
                .vengeance
                .expect("Vengeance is bound")
                .on_gain(fight);
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: PaladinAura) {
        Self::protection_toggle(fight, kind, false);
        if kind == PaladinAura::DivineFavor {
            Self::divine_favor(fight).on_expire(fight);
        }
        if kind == PaladinAura::Vengeance {
            fight
                .agent
                .vengeance
                .expect("Vengeance is bound")
                .on_expire(fight);
        }
    }

    fn on_stacks_change(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PaladinAura,
        _old: i32,
        new: i32,
    ) {
        if kind == PaladinAura::Vengeance {
            fight
                .agent
                .vengeance
                .expect("Vengeance is bound")
                .on_stacks_change(fight, new);
        }
    }

    fn on_cast_complete(fight: &mut Fight<Self>, aura: AuraRef, kind: PaladinAura, spell: SpellId) {
        if kind == PaladinAura::SwiftJudgement {
            Self::bound(&fight.agent.swift_judgement).on_cast_complete(fight, spell);
        }
        if kind == PaladinAura::DivineFavor {
            Self::divine_favor(fight).on_cast_complete(fight, spell);
        }
        if kind == PaladinAura::SanctifiedJudgement {
            fight
                .agent
                .sanctified_judgement
                .expect("Sanctified Judgement is bound")
                .on_cast_complete(fight, aura, spell);
        }
    }

    fn on_spell_hit_dealt(
        fight: &mut Fight<Self>,
        aura: AuraRef,
        kind: PaladinAura,
        spell: SpellId,
        result: &SpellResult,
    ) {
        match kind {
            PaladinAura::JudgementRefresh => {
                let refresh = fight
                    .agent
                    .judgement_refresh
                    .clone()
                    .expect("Judgement Refresh is bound");
                refresh.on_spell_hit_dealt(fight, spell, result);
            }
            PaladinAura::Seal(_) => seals::on_spell_hit_dealt(fight, aura, spell, result),
            PaladinAura::VengeanceTrigger => {
                Vengeance::on_spell_hit_dealt(fight, aura, spell, result)
            }
            PaladinAura::VindicationTrigger => fight
                .agent
                .vindication
                .expect("Vindication is bound")
                .on_spell_hit_dealt(fight, aura, spell, result),
            PaladinAura::SacredArbiter => {
                let judgements = Rc::clone(&fight.agent.sacred_arbiter);
                retribution::sacred_arbiter_hit(fight, &judgements, spell, result);
            }
            PaladinAura::TwistOfLight => twist_of_light::on_spell_hit_dealt(fight, spell, result),
            PaladinAura::IronCreedTrigger => {
                Self::bound(&fight.agent.iron_creed).trigger(fight, aura, spell, result)
            }
            _ => {}
        }
    }

    fn on_enemy_hit_taken(
        fight: &mut Fight<Self>,
        aura: AuraRef,
        kind: PaladinAura,
        result: &SpellResult,
    ) {
        match kind {
            PaladinAura::RedoubtTrigger => {
                Self::bound(&fight.agent.redoubt).trigger(fight, aura, result)
            }
            PaladinAura::Redoubt => Self::bound(&fight.agent.redoubt).block(fight, result),
            PaladinAura::ShieldSpecialization => {
                Self::bound(&fight.agent.shield_specialization).trigger(fight, aura, result)
            }
            PaladinAura::ReckoningBlock => {
                Self::bound(&fight.agent.reckoning).trigger(fight, aura, true, result)
            }
            PaladinAura::ReckoningCrit => {
                Self::bound(&fight.agent.reckoning).trigger(fight, aura, false, result)
            }
            PaladinAura::HolyShield => Self::bound(&fight.agent.holy_shield).block(fight, result),
            _ => {}
        }
    }

    fn on_delayed_proc(
        fight: &mut Fight<Self>,
        _aura: AuraRef,
        kind: PaladinAura,
        _spell: SpellId,
        result: SpellResult,
    ) {
        match kind {
            PaladinAura::Seal(seal) => seals::on_delayed_proc(fight, seal, result.target),
            PaladinAura::VengeanceTrigger => fight
                .agent
                .vengeance
                .expect("Vengeance is bound")
                .on_delayed_proc(fight),
            PaladinAura::VindicationTrigger => fight
                .agent
                .vindication
                .expect("Vindication is bound")
                .on_delayed_proc(fight),
            PaladinAura::ReckoningBlock | PaladinAura::ReckoningCrit => fight.extra_mh_attacks(1),
            PaladinAura::IronCreedTrigger => {
                let aura = Self::bound(&fight.agent.iron_creed).aura;
                fight.activate_aura(aura);
            }
            _ => {}
        }
    }
}
