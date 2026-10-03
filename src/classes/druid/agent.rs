//! The Druid class agent for the fight runtime: class spells and auras by name, and the
//! hooks that dispatch to each spell's and talent's module.

use std::rc::Rc;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell as ExportedSpell},
    core::fight::{Agent, AuraRef, DotId, Fight, Side, SpellId, SpellResult},
};

use super::{
    forms::{self, Forms},
    spells::{
        cat_builders::{Builder, CatBuilders},
        cat_form::{self, CatForm},
        faerie_fire::FaerieFire,
        ferocious_bite::FerociousBite,
        innervate, insect_swarm, moonfire,
        prowl::{self, Prowl},
        rip::{self, Rip},
        shifting_power::{self, ShiftingPower},
        starfire, wrath,
    },
    talents::{
        berserk::{self, Berserk},
        blood_frenzy::{self, BloodFrenzy},
        eclipse, natures_grace, omen_of_clarity, rend_and_tear,
    },
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
    CatForm,
    Prowl,
    Builder(Builder),
    Rip,
    FerociousBite,
    ShiftingPower,
    FaerieFire,
    Berserk,
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
    CatForm,
    Prowl,
    Berserk,
    BloodFrenzy,
    /// Blood Frenzy's bear half, which acts only in Bear Form.
    BloodFrenzyBear,
}

/// Druid state that Go keeps in the `Druid` struct and its closures.
pub(crate) struct DruidAgent {
    moonkin_form: Option<AuraRef>,
    moonfire_dot: Option<SpellId>,
    insect_swarm_debuff: Option<AuraRef>,
    innervate: Option<Rc<innervate::Innervate>>,
    omen: Option<Rc<omen_of_clarity::OmenOfClarity>>,
    natures_grace: Option<Rc<natures_grace::NaturesGrace>>,
    eclipse: Option<Rc<eclipse::Eclipse>>,
    forms: Option<Rc<Forms>>,
    /// Go `Druid.form`.
    pub(crate) form: u8,
    /// Go `PseudoStats.MovementSpeedMultiplier`, which only logs in scope.
    pub(crate) movement_speed: f64,
    /// Go `lastCatFormEnergy` and `lastCatFormExitAt`, which no reset clears.
    pub(crate) last_cat_form_energy: f64,
    pub(crate) last_cat_form_exit: i64,
    pub(crate) cat_form: Option<Rc<CatForm>>,
    pub(crate) prowl: Option<Rc<Prowl>>,
    builders: Option<Rc<CatBuilders>>,
    rip: Option<Rc<Rip>>,
    pub(crate) rip_snapshot: rip::Snapshot,
    ferocious_bite: Option<FerociousBite>,
    shifting_power: Option<ShiftingPower>,
    faerie_fire: Option<FaerieFire>,
    berserk: Option<Berserk>,
    blood_frenzy: Option<Rc<BloodFrenzy>>,
}

impl Default for DruidAgent {
    fn default() -> Self {
        DruidAgent {
            moonkin_form: None,
            moonfire_dot: None,
            insect_swarm_debuff: None,
            innervate: None,
            omen: None,
            natures_grace: None,
            eclipse: None,
            forms: None,
            form: forms::HUMANOID,
            movement_speed: 1.0,
            last_cat_form_energy: 0.0,
            last_cat_form_exit: 0,
            cat_form: None,
            prowl: None,
            builders: None,
            rip: None,
            rip_snapshot: rip::Snapshot::default(),
            ferocious_bite: None,
            shifting_power: None,
            faerie_fire: None,
            berserk: None,
            blood_frenzy: None,
        }
    }
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
            Effect::CatForm { aura, .. } => auras.push((aura.clone(), DruidAura::CatForm)),
            Effect::Prowl { aura, .. } => auras.push((aura.clone(), DruidAura::Prowl)),
            Effect::Berserk { aura, .. } => auras.push((aura.clone(), DruidAura::Berserk)),
            Effect::BloodFrenzy {
                trigger_aura,
                bear_trigger_aura,
                ..
            } => {
                auras.push((trigger_aura.clone(), DruidAura::BloodFrenzy));
                auras.push((bear_trigger_aura.clone(), DruidAura::BloodFrenzyBear));
            }
            _ => {}
        }
    }
    auras
}

/// The spell position an effect names, by kind.
fn effect_spell(prepared: &PreparedV2, position: usize) -> Option<DruidSpell> {
    prepared.effects.iter().find_map(|effect| match effect {
        Effect::Rip { spell, .. } if *spell == position => Some(DruidSpell::Rip),
        Effect::FerociousBite { spell, .. } if *spell == position => {
            Some(DruidSpell::FerociousBite)
        }
        Effect::ShiftingPower { spell, .. } if *spell == position => {
            Some(DruidSpell::ShiftingPower)
        }
        Effect::FaerieFire { spell, .. } if *spell == position => Some(DruidSpell::FaerieFire),
        Effect::CatBuilders { builders, .. } => builders
            .iter()
            .find(|builder| builder.spell == position)
            .and_then(|builder| Builder::parse(&builder.kind))
            .map(DruidSpell::Builder),
        _ => None,
    })
}

impl DruidAgent {
    /// The class behavior of an exported spell, if Rust implements it.
    pub(crate) fn spell(
        prepared: &PreparedV2,
        position: usize,
        spell: &ExportedSpell,
    ) -> Option<DruidSpell> {
        let id = spell.action_id.clone().unwrap_or_default();
        // Spells Go registers without a class mask, named by their effects.
        for effect in &prepared.effects {
            match effect {
                Effect::Prowl { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
                    return Some(DruidSpell::Prowl)
                }
                Effect::Berserk { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
                    return Some(DruidSpell::Berserk)
                }
                _ => {}
            }
        }
        match spell.class_spell.as_deref()? {
            "moonkin_form" => Some(DruidSpell::MoonkinForm),
            "starfire" if spell.damage_effect.is_some() => Some(DruidSpell::Starfire),
            "wrath" if spell.damage_effect.is_some() => Some(DruidSpell::Wrath),
            "moonfire" if spell.damage_effect.is_some() => Some(DruidSpell::Moonfire),
            "moonfire_dot" if spell.dot.is_some() => Some(DruidSpell::MoonfireDot),
            "insect_swarm" if spell.dot.is_some() => Some(DruidSpell::InsectSwarm),
            "innervate" => Some(DruidSpell::Innervate),
            "cat_form" => prepared
                .effects
                .iter()
                .any(|effect| matches!(effect, Effect::CatForm { .. }))
                .then_some(DruidSpell::CatForm),
            "rip" | "ferocious_bite" | "shifting_power" | "faerie_fire" | "shred" | "claw"
            | "ravage" => effect_spell(prepared, position),
            _ => None,
        }
    }

    /// Build a fight for a prepared input that passed the coverage gate.
    pub(crate) fn fight(prepared: &PreparedV2) -> Result<Fight<DruidAgent>, String> {
        let auras = class_auras(prepared);
        let positions: Vec<*const ExportedSpell> = prepared
            .player
            .spells
            .iter()
            .map(|spell| spell as *const ExportedSpell)
            .collect();
        let mut fight = Fight::new(
            prepared,
            DruidAgent::default(),
            |exported| {
                let position = positions
                    .iter()
                    .position(|&spell| std::ptr::eq(spell, exported))?;
                DruidAgent::spell(prepared, position, exported)
            },
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
        let spell_count = fight.spells.len();
        // Go AddStatsDynamic: a stat aura's bit in the stat combinations.
        let stat_bit = |label: &str| {
            prepared.effects.iter().find_map(|effect| match effect {
                Effect::StatAuras { auras, .. } => auras
                    .iter()
                    .position(|aura| aura == label)
                    .map(|bit| 1u32 << bit),
                _ => None,
            })
        };
        for effect in &prepared.effects {
            match effect {
                Effect::DruidForms {
                    starting_form,
                    spells,
                } => {
                    let bound = Forms::new(spell_count, starting_form, spells);
                    fight.agent.form = bound.starting;
                    fight.agent.forms = Some(Rc::new(bound));
                }
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
                    let spell_masked = prepared
                        .player
                        .spells
                        .iter()
                        .map(|spell| {
                            spell.proc_mask.iter().any(|mask| {
                                mask == "ProcMaskSpellDamage" || mask == "ProcMaskSpellHealing"
                            })
                        })
                        .collect();
                    let bound = omen_of_clarity::bind(
                        &mut fight,
                        omen_of_clarity::Params {
                            aura,
                            trigger: trigger_aura,
                            trigger_spells,
                            spell_masked,
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
                Effect::CatForm {
                    spell_id,
                    aura,
                    initial_threat_multiplier,
                    threat_multiplier,
                    initial_spirit_regen_multiplier,
                    spirit_regen_multiplier,
                    initial_movement_speed_multiplier,
                    movement_speed_bonus,
                    furor_max,
                    cost_spells,
                    gcd_spells,
                    gcd_delta_ns,
                    form_breaking_spells,
                    main_hand,
                    cat_weapon,
                } => {
                    let spell = find_spell(&fight, *spell_id, 0)
                        .ok_or("Cat Form's spell is not registered")?;
                    let stat_bit = stat_bit(aura).ok_or("Cat Form is not a stat aura")?;
                    let bound = cat_form::bind(
                        &mut fight,
                        cat_form::Params {
                            spell,
                            spell_id: *spell_id,
                            aura,
                            stat_bit,
                            initial_threat_multiplier: *initial_threat_multiplier,
                            threat_multiplier: *threat_multiplier,
                            initial_spirit_regen_multiplier: *initial_spirit_regen_multiplier,
                            spirit_regen_multiplier: *spirit_regen_multiplier,
                            initial_movement_speed_multiplier: *initial_movement_speed_multiplier,
                            movement_speed_bonus: *movement_speed_bonus,
                            furor_max: *furor_max,
                            cost_spells,
                            gcd_spells,
                            gcd_delta: *gcd_delta_ns,
                            form_breaking_spells,
                            main_hand,
                            cat_weapon,
                        },
                    )?;
                    fight.agent.cat_form = Some(Rc::new(bound));
                }
                Effect::Prowl {
                    spell_id,
                    aura,
                    movement_speed_multiplier,
                } => {
                    let spell =
                        find_spell(&fight, *spell_id, 0).ok_or("Prowl is not registered")?;
                    let bound = prowl::bind(&mut fight, spell, aura, *movement_speed_multiplier)?;
                    fight.agent.prowl = Some(Rc::new(bound));
                }
                Effect::CatBuilders {
                    builders,
                    cannot_shred,
                } => {
                    let mut flat_damage = vec![None; spell_count];
                    for builder in builders {
                        if let Some(slot) = flat_damage.get_mut(builder.spell) {
                            *slot = Some(builder.flat_damage);
                        }
                    }
                    fight.agent.builders = Some(Rc::new(CatBuilders {
                        flat_damage,
                        cannot_shred: *cannot_shred,
                    }));
                }
                Effect::Rip {
                    tick_base,
                    tick_per_combo_point,
                    attack_power_share_per_combo_point,
                    attack_power_share_max_points,
                    tick_can_crit,
                    expected_combo_points,
                    short_name,
                    ..
                } => {
                    fight.agent.rip = Some(Rc::new(Rip {
                        tick_base: *tick_base,
                        tick_per_combo_point: *tick_per_combo_point,
                        share_per_combo_point: *attack_power_share_per_combo_point,
                        share_max_points: *attack_power_share_max_points,
                        tick_can_crit: *tick_can_crit,
                        expected_combo_points: *expected_combo_points,
                        short_name: short_name.clone(),
                    }));
                }
                Effect::FerociousBite {
                    damage_per_energy,
                    damage_per_combo_point,
                    attack_power_per_combo_point,
                    ..
                } => {
                    fight.agent.ferocious_bite = Some(FerociousBite {
                        damage_per_energy: *damage_per_energy,
                        damage_per_combo_point: *damage_per_combo_point,
                        attack_power_per_combo_point: *attack_power_per_combo_point,
                    });
                }
                Effect::ShiftingPower { spell, energy } => {
                    let bound = shifting_power::bind(&mut fight, *spell, *energy);
                    fight.agent.shifting_power = Some(bound);
                }
                Effect::FaerieFire { aura, .. } => {
                    let index = fight.trackers[Side::Target.index()]
                        .find(aura)
                        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
                    fight.agent.faerie_fire = Some(FaerieFire {
                        aura: AuraRef {
                            side: Side::Target,
                            index,
                        },
                    });
                }
                Effect::Berserk {
                    aura,
                    crit_percent,
                    crit_spells,
                    ..
                } => {
                    let bound = berserk::bind(&mut fight, aura, *crit_percent, crit_spells)?;
                    fight.agent.berserk = Some(bound);
                }
                Effect::BloodFrenzy {
                    trigger_aura,
                    proc_chance,
                    trigger_spells,
                    trigger_immediately,
                    metrics_action_id,
                    ..
                } => {
                    let bound = blood_frenzy::bind(
                        &mut fight,
                        trigger_aura,
                        *proc_chance,
                        trigger_spells,
                        *trigger_immediately,
                        metrics_action_id,
                    )?;
                    fight.agent.blood_frenzy = Some(Rc::new(bound));
                }
                Effect::RendAndTear {
                    multiplier,
                    spells,
                    bleed_spells,
                } => rend_and_tear::bind(&mut fight, *multiplier, spells, bleed_spells)?,
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

    fn cat_form(fight: &Fight<Self>) -> Rc<CatForm> {
        fight.agent.cat_form.clone().expect("Cat Form is bound")
    }

    fn prowl(fight: &Fight<Self>) -> Rc<Prowl> {
        fight.agent.prowl.clone().expect("Prowl is bound")
    }

    fn builders(fight: &Fight<Self>) -> Rc<CatBuilders> {
        fight
            .agent
            .builders
            .clone()
            .expect("the cat builders are bound")
    }

    fn rip(fight: &Fight<Self>) -> Rc<Rip> {
        fight.agent.rip.clone().expect("Rip is bound")
    }

    fn blood_frenzy(fight: &Fight<Self>) -> Rc<BloodFrenzy> {
        fight
            .agent
            .blood_frenzy
            .clone()
            .expect("Blood Frenzy is bound")
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
            DruidSpell::CatForm => Self::cat_form(fight).apply(fight),
            DruidSpell::Prowl => Self::prowl(fight).apply(fight),
            DruidSpell::Builder(_) => Self::builders(fight).apply(fight, spell, target),
            DruidSpell::Rip => Self::rip(fight).apply(fight, spell, target),
            DruidSpell::FerociousBite => {
                let bite = fight.agent.ferocious_bite.expect("Ferocious Bite is bound");
                bite.apply(fight, spell, target);
            }
            DruidSpell::ShiftingPower => {
                let power = fight.agent.shifting_power.expect("Shifting Power is bound");
                power.apply(fight);
            }
            DruidSpell::FaerieFire => {
                let faerie_fire = fight.agent.faerie_fire.expect("Faerie Fire is bound");
                faerie_fire.apply(fight, spell, target);
            }
            DruidSpell::Berserk => {
                let aura = fight.agent.berserk.expect("Berserk is bound").aura;
                fight.activate_aura(aura);
            }
        }
    }

    fn extra_cast_condition(fight: &Fight<Self>, _spell: SpellId, behavior: DruidSpell) -> bool {
        match behavior {
            DruidSpell::Innervate => Self::innervate(fight).can_cast(fight),
            DruidSpell::Prowl => Self::prowl(fight).can_cast(fight),
            DruidSpell::Builder(builder) => Self::builders(fight).can_cast(fight, builder),
            DruidSpell::Rip => Self::rip(fight).can_cast(fight),
            DruidSpell::FerociousBite => fight
                .agent
                .ferocious_bite
                .is_some_and(|bite| bite.can_cast(fight)),
            _ => true,
        }
    }

    /// Go `RegisterSpell`'s form check, which logs before the spell's own condition.
    fn extra_cast_condition_logged(
        fight: &mut Fight<Self>,
        spell: SpellId,
        behavior: DruidSpell,
    ) -> bool {
        let form = fight.agent.form;
        if let Some(forms) = fight.agent.forms.clone() {
            if forms.wrong_form(form, spell) {
                if fight.log.is_some() {
                    let id = crate::core::fight::action_string(&fight.spells[spell].id);
                    fight.sim_log(&format!("Failed cast to spell {id}, wrong form"));
                }
                return false;
            }
        }
        Self::extra_cast_condition(fight, spell, behavior)
    }

    /// Go `RegisterSpell`'s `ModifyCast`: a humanoid spell cast outside its forms clears the
    /// form.
    fn modify_cast(fight: &mut Fight<Self>, spell: SpellId, _behavior: DruidSpell) {
        let form = fight.agent.form;
        let clears = fight
            .agent
            .forms
            .as_ref()
            .is_some_and(|forms| forms.clears_form(form, spell));
        if clears {
            match fight.agent.cat_form.clone() {
                Some(cat) => cat.clear_form(fight),
                None => fight.agent.form = forms::HUMANOID,
            }
        }
    }

    fn should_activate(_fight: &Fight<Self>, _spell: SpellId, behavior: DruidSpell) -> bool {
        // Go leaves Innervate to the rotation.
        behavior != DruidSpell::Innervate
    }

    fn cooldown_activation_condition(fight: &Fight<Self>, spell: SpellId) -> bool {
        fight
            .agent
            .cat_form
            .as_ref()
            .is_none_or(|cat| cat.activation_allowed(fight, spell))
    }

    fn after_apply_effects(fight: &mut Fight<Self>, spell: SpellId) {
        if let Some(cat) = fight.agent.cat_form.clone() {
            cat.after_apply_effects(fight, spell);
        }
    }

    /// Go restores the initial pseudo stats, which the exported ones include the form in.
    fn reset(fight: &mut Fight<Self>) {
        if let Some(cat) = fight.agent.cat_form.clone() {
            fight.player.threat_multiplier = cat.initial_threat_multiplier;
            fight.player.spirit_regen_multiplier = cat.initial_spirit_regen_multiplier;
            fight.agent.movement_speed = cat.initial_movement_speed_multiplier;
            fight.player.powers = fight.stat_combos[0];
        }
    }

    /// Go `Druid.Reset`, and the feral cat's: back to the starting form, then out of it and
    /// into Cat Form.
    fn agent_reset(fight: &mut Fight<Self>) {
        if let Some(forms) = fight.agent.forms.clone() {
            fight.agent.form = forms.starting;
        }
        if let Some(cat) = fight.agent.cat_form.clone() {
            cat.clear_form(fight);
            fight.activate_aura(cat.aura);
        }
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
        match behavior {
            DruidSpell::MoonfireDot | DruidSpell::InsectSwarm => fight.snapshot_dot_tick(dot),
            DruidSpell::Rip => Self::rip(fight).tick(fight, dot),
            _ => {}
        }
    }

    fn on_exclusive_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura) {
        if kind == DruidAura::CatForm {
            Self::cat_form(fight).on_exclusive_gain(fight);
        }
    }

    fn on_gain(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura) {
        match kind {
            DruidAura::Clearcasting => Self::omen(fight).on_gain(fight),
            DruidAura::NaturesGrace => Self::natures_grace(fight).on_gain(fight),
            DruidAura::Eclipse => Self::eclipse(fight).on_gain(fight),
            DruidAura::Innervate => Self::innervate(fight).on_gain(fight),
            DruidAura::CatForm => Self::cat_form(fight).on_gain(fight),
            DruidAura::Prowl => Self::prowl(fight).on_gain(fight),
            DruidAura::Berserk => fight.agent.berserk.expect("bound").on_gain(fight),
            _ => {}
        }
    }

    fn on_expire(fight: &mut Fight<Self>, _aura: AuraRef, kind: DruidAura) {
        match kind {
            DruidAura::Clearcasting => Self::omen(fight).on_expire(fight),
            DruidAura::NaturesGrace => Self::natures_grace(fight).on_expire(fight),
            DruidAura::Eclipse => Self::eclipse(fight).on_expire(fight),
            DruidAura::Innervate => Self::innervate(fight).on_expire(fight),
            DruidAura::CatForm => Self::cat_form(fight).on_expire(fight),
            DruidAura::Prowl => Self::prowl(fight).on_expire(fight),
            DruidAura::Berserk => fight.agent.berserk.expect("bound").on_expire(fight),
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
            DruidAura::Prowl => Self::prowl(fight).on_spell_hit_dealt(fight),
            DruidAura::BloodFrenzy => {
                Self::blood_frenzy(fight).on_spell_hit_dealt(fight, spell, result)
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
        match kind {
            DruidAura::OmenOfClarity => Self::omen(fight).on_delayed_proc(fight),
            DruidAura::BloodFrenzy => Self::blood_frenzy(fight).on_delayed_proc(fight),
            _ => {}
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
