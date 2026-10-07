//! Druid preparation: Go sim/druid's construction and initialization (the class package and its
//! Balance, Feral Cat and Feral Bear specs) and the exporter's Druid description
//! (tools/oracle-v2/druid.go, druid_feral.go, druid_bear.go and druid_items.go).
//!
//! Go keeps the druid's state in struct fields its callbacks close over. Here the state a
//! callback reads is [`forms::State`], shared by reference counting; the rest stays on
//! [`Druid`], which is the class agent.

mod balance;
mod baseline;
mod bear;
mod export;
mod feral;
pub(crate) mod forms;
pub(crate) mod items;
pub(crate) mod masks;
mod restoration;
mod talents;

use std::rc::Rc;

use crate::classes::druid::forms::{BEAR, CAT, HUMANOID, MOONKIN};
use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;
use crate::prepare::agent::{fill_talents, ClassSpellName, PrepAgent};
use crate::prepare::attack::AutoAttackOptions;
use crate::prepare::energy::EnergyBarOptions;
use crate::prepare::rage::RageBarOptions;
use crate::prepare::sim::{AuraId, PowerBar, Sim, SpellId, UnitId};
use crate::prepare::spell::SpellConfig;
use crate::prepare::stats::Stat;
use crate::prepare::Refusal;

use talents::Talents;

/// Go druid.TalentTreeSizes.
const TALENT_TREE_SIZES: [usize; 3] = [16, 20, 16];

/// Which of Go's three registered Druid specs the player is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Spec {
    Balance,
    FeralCat,
    FeralBear,
}

/// tools/oracle-v2/druid.go `druidClassSpells`.
static CLASS_SPELLS: &[ClassSpellName] = &[
    ClassSpellName {
        mask: masks::ENTANGLING_ROOTS,
        name: "entangling_roots",
    },
    ClassSpellName {
        mask: masks::CLAW,
        name: "claw",
    },
    ClassSpellName {
        mask: masks::DEMORALIZING_ROAR,
        name: "demoralizing_roar",
    },
    ClassSpellName {
        mask: masks::FAERIE_FIRE,
        name: "faerie_fire",
    },
    ClassSpellName {
        mask: masks::FAERIE_FIRE_FERAL,
        name: "faerie_fire_feral",
    },
    ClassSpellName {
        mask: masks::HURRICANE,
        name: "hurricane",
    },
    ClassSpellName {
        mask: masks::FEROCIOUS_BITE,
        name: "ferocious_bite",
    },
    ClassSpellName {
        mask: masks::FRENZIED_REGENERATION,
        name: "frenzied_regeneration",
    },
    ClassSpellName {
        mask: masks::INNERVATE,
        name: "innervate",
    },
    ClassSpellName {
        mask: masks::INSECT_SWARM,
        name: "insect_swarm",
    },
    ClassSpellName {
        mask: masks::LACERATE,
        name: "lacerate",
    },
    ClassSpellName {
        mask: masks::PRIMAL_BITE,
        name: "primal_bite",
    },
    ClassSpellName {
        mask: masks::MAUL,
        name: "maul",
    },
    ClassSpellName {
        mask: masks::MOONFIRE_INITIAL,
        name: "moonfire",
    },
    ClassSpellName {
        mask: masks::MOONFIRE_DOT,
        name: "moonfire_dot",
    },
    ClassSpellName {
        mask: masks::RAKE,
        name: "rake",
    },
    ClassSpellName {
        mask: masks::RAVAGE,
        name: "ravage",
    },
    ClassSpellName {
        mask: masks::RIP,
        name: "rip",
    },
    ClassSpellName {
        mask: masks::SHRED,
        name: "shred",
    },
    ClassSpellName {
        mask: masks::STARFIRE,
        name: "starfire",
    },
    ClassSpellName {
        mask: masks::SWIPE,
        name: "swipe",
    },
    ClassSpellName {
        mask: masks::THORNS,
        name: "thorns",
    },
    ClassSpellName {
        mask: masks::WRATH,
        name: "wrath",
    },
    ClassSpellName {
        mask: masks::ENRAGE,
        name: "enrage",
    },
    ClassSpellName {
        mask: masks::SHIFTING_POWER,
        name: "shifting_power",
    },
    ClassSpellName {
        mask: masks::CAT_FORM,
        name: "cat_form",
    },
    ClassSpellName {
        mask: masks::BEAR_FORM,
        name: "bear_form",
    },
    ClassSpellName {
        mask: masks::MOONKIN_FORM,
        name: "moonkin_form",
    },
    ClassSpellName {
        mask: masks::HEALING_TOUCH,
        name: "healing_touch",
    },
    ClassSpellName {
        mask: masks::REGROWTH,
        name: "regrowth",
    },
    ClassSpellName {
        mask: masks::LIFEBLOOM,
        name: "lifebloom",
    },
    ClassSpellName {
        mask: masks::REJUVENATION,
        name: "rejuvenation",
    },
    ClassSpellName {
        mask: masks::TRANQUILITY,
        name: "tranquility",
    },
    ClassSpellName {
        mask: masks::MARK_OF_THE_WILD,
        name: "mark_of_the_wild",
    },
    ClassSpellName {
        mask: masks::SWIFTMEND,
        name: "swiftmend",
    },
    ClassSpellName {
        mask: masks::CENARION_WARD,
        name: "cenarion_ward",
    },
    ClassSpellName {
        mask: masks::REVIVE,
        name: "revive",
    },
];

/// Go `Druid`, with the `FeralDruid`, `GuardianDruid` and `BalanceDruid` embedding it.
pub(crate) struct Druid {
    pub(super) unit: UnitId,
    pub(super) spec: Spec,
    pub(super) talents: Message,
    pub(super) tal: Talents,
    pub(super) st: Rc<forms::State>,
    /// The forms each spell registered through `druid.RegisterSpell` may be cast in, in
    /// registration order.
    pub(super) form_masks: Vec<(SpellId, u8)>,
    /// Go `CannotShredTarget`.
    pub(super) cannot_shred_target: bool,
    /// Go `FurorProcChance`.
    pub(super) furor_proc_chance: f64,
    /// Go `IntensityEnrageRageBonus`.
    pub(super) intensity_enrage_rage_bonus: f64,
    pub(super) cat_form_aura: Option<AuraId>,
    pub(super) bear_form_aura: Option<AuraId>,
    pub(super) moonkin_form_aura: Option<AuraId>,
    pub(super) clearcasting_aura: Option<AuraId>,
    pub(super) berserk_aura: Option<AuraId>,
    pub(super) eclipse_aura: Option<AuraId>,
    pub(super) prowl_aura: Option<AuraId>,
    pub(super) cat_form: Option<SpellId>,
    pub(super) bear_form: Option<SpellId>,
    pub(super) moonkin_form: Option<SpellId>,
    pub(super) starfire: Vec<SpellId>,
    pub(super) wrath: Option<SpellId>,
    pub(super) moonfire: Option<SpellId>,
    pub(super) insect_swarm: Option<SpellId>,
    pub(super) hurricane: Option<SpellId>,
    pub(super) faerie_fire: Option<SpellId>,
    /// Go `FaerieFireAuras`, by unit index.
    pub(super) faerie_fire_auras: Vec<Option<AuraId>>,
    pub(super) claw: Option<SpellId>,
    pub(super) shred: Option<SpellId>,
    pub(super) ravage: Option<SpellId>,
    pub(super) rake: Option<SpellId>,
    pub(super) rip: Option<SpellId>,
    pub(super) ferocious_bite: Option<SpellId>,
    pub(super) shifting_power: Option<SpellId>,
    pub(super) prowl: Option<SpellId>,
    pub(super) berserk: Option<SpellId>,
    pub(super) maul: Option<SpellId>,
    pub(super) maul_strike: Option<SpellId>,
    pub(super) lacerate: Option<SpellId>,
    pub(super) primal_bite: Option<SpellId>,
    pub(super) swipe: Option<SpellId>,
    pub(super) enrage: Option<SpellId>,
    pub(super) frenzied_regeneration: Option<SpellId>,
    pub(super) demoralizing_roar: Option<SpellId>,
    /// Go `DemoralizingRoarAuras`, by unit index.
    pub(super) demoralizing_roar_auras: Vec<Option<AuraId>>,
    pub(super) barkskin: Option<SpellId>,
    pub(super) options: SpecOptions,
}

/// The spec options the construction reads.
#[derive(Clone, Debug, Default)]
pub(super) struct SpecOptions {
    pub starting_rage: f64,
}

/// Go `druid.New`, `NewBalanceDruid`, `NewFeralCatDruid` and `NewFeralBearDruid`.
pub(crate) fn new_druid(
    sim: &mut Sim,
    unit: UnitId,
    player: &Message,
) -> Result<Box<dyn PrepAgent>, Refusal> {
    let Some((field, spec_value)) = player.oneof("spec") else {
        return Err(Refusal::new(
            "spec",
            "a Druid without a spec is not prepared".to_string(),
        ));
    };
    let spec = match field {
        "balance_druid" => Spec::Balance,
        "feral_cat_druid" => Spec::FeralCat,
        "feral_bear_druid" => Spec::FeralBear,
        other => {
            return Err(Refusal::new(
                "spec",
                format!("the {other} spec is not prepared in Rust yet"),
            ))
        }
    };
    let spec_message = match spec_value {
        crate::contracts::request::Value::Message(message) => message,
        _ => {
            return Err(Refusal::new(
                "spec",
                "a Druid spec without options".to_string(),
            ))
        }
    };
    let options = spec_message.message("options");
    if let Some(target) = options
        .and_then(|options| options.message("class_options"))
        .and_then(|class| class.message("innervate_target"))
    {
        // Only an empty reference and the player itself resolve to the druid's own unit.
        let kind = target.enum_name("type");
        let own =
            kind == "Unknown" || kind == "Self" || (kind == "Player" && target.i32("index") == 0);
        if !own {
            return Err(Refusal::new(
                "innervate_target",
                "an Innervate target other than the druid is not prepared yet".to_string(),
            ));
        }
    }

    let talents = fill_talents(
        "proto.DruidTalents",
        player.str("talents_string"),
        TALENT_TREE_SIZES,
    )
    .map_err(|err| Refusal::new("talents", err))?;
    let tal = Talents::from_message(&talents);

    // druid.New.
    sim.enable_mana_bar(unit);
    let crit_per_agi = sim.crit_per_agi_max_level(unit);
    {
        let sdm = &mut sim.unit_mut(unit).sdm;
        // Two attack power a point of Strength in every form, as on master.
        sdm.add_stat_dependency(Stat::Strength, Stat::AttackPower, 2.0);
        sdm.add_stat_dependency(Stat::BonusArmor, Stat::Armor, 1.0);
        sdm.add_stat_dependency(Stat::Agility, Stat::PhysicalCritPercent, crit_per_agi);
        // Dodge is Classic's at level 60: 0.9% base and 20 Agility a percent.
        sdm.add_stat_dependency(
            Stat::Agility,
            Stat::DodgeRating,
            crit_per_agi * crate::prepare::character::constants::DODGE_RATING_PER_DODGE_PERCENT,
        );
    }
    sim.unit_mut(unit).pseudo_stats.base_dodge_chance += 0.009;

    let starting_form = match spec {
        Spec::Balance => MOONKIN,
        Spec::FeralCat => CAT,
        Spec::FeralBear => BEAR,
    };
    let mut druid = Druid {
        unit,
        spec,
        talents,
        tal,
        st: Rc::new(forms::State::new(starting_form)),
        form_masks: Vec::new(),
        cannot_shred_target: false,
        furor_proc_chance: 0.0,
        intensity_enrage_rage_bonus: 0.0,
        cat_form_aura: None,
        bear_form_aura: None,
        moonkin_form_aura: None,
        clearcasting_aura: None,
        berserk_aura: None,
        eclipse_aura: None,
        prowl_aura: None,
        cat_form: None,
        bear_form: None,
        moonkin_form: None,
        starfire: Vec::new(),
        wrath: None,
        moonfire: None,
        insect_swarm: None,
        hurricane: None,
        faerie_fire: None,
        faerie_fire_auras: Vec::new(),
        claw: None,
        shred: None,
        ravage: None,
        rake: None,
        rip: None,
        ferocious_bite: None,
        shifting_power: None,
        prowl: None,
        berserk: None,
        maul: None,
        maul_strike: None,
        lacerate: None,
        primal_bite: None,
        swipe: None,
        enrage: None,
        frenzied_regeneration: None,
        demoralizing_roar: None,
        demoralizing_roar_auras: Vec::new(),
        barkskin: None,
        options: SpecOptions::default(),
    };

    match spec {
        Spec::Balance => {
            // moonkin.RegisterMoonkinFormSpell() then RegisterMoonkinFormAura().
            druid.register_moonkin_form_spell(sim);
            druid.register_moonkin_form_aura(sim);
        }
        Spec::FeralCat => {
            druid.cannot_shred_target = spec_message
                .message("options")
                .is_some_and(|options| options.bool("cannot_shred_target"));
            sim.enable_energy_bar(
                unit,
                EnergyBarOptions {
                    max_combo_points: 5,
                    max_energy: 100.0,
                    has_no_regen: false,
                },
            );
            // The Cat never swings in Bear Form unless a custom rotation shifts. Its bar pays
            // the 75% crit Rage bonus like the Bear's.
            sim.enable_rage_bar(
                unit,
                RageBarOptions {
                    base_rage_multiplier: 1.0,
                    ..RageBarOptions::default()
                },
            );
            let paw = forms::cat_weapon(sim, unit);
            sim.enable_auto_attacks(
                unit,
                AutoAttackOptions {
                    main_hand: paw,
                    auto_swing_melee: true,
                    ..AutoAttackOptions::default()
                },
            );
            druid.register_cat_form_aura(sim);
            druid.register_bear_form_aura(sim);
        }
        Spec::FeralBear => {
            druid.options.starting_rage = spec_message
                .message("options")
                .map_or(0.0, |options| options.f64("starting_rage"));
            if player.message("healing_model").is_some() {
                return Err(Refusal::new(
                    "healing_model",
                    "healing models are unsupported".to_string(),
                ));
            }
            sim.enable_energy_bar(
                unit,
                EnergyBarOptions {
                    max_combo_points: 5,
                    max_energy: 100.0,
                    has_no_regen: false,
                },
            );
            sim.enable_rage_bar(
                unit,
                RageBarOptions {
                    max_rage: 0.0,
                    starting_rage: druid.options.starting_rage,
                    base_rage_multiplier: 1.0,
                },
            );
            let paw = forms::bear_weapon(sim, unit);
            sim.enable_auto_attacks(
                unit,
                AutoAttackOptions {
                    main_hand: paw,
                    auto_swing_melee: true,
                    replace_mh_swing: true,
                    ..AutoAttackOptions::default()
                },
            );
            druid.register_bear_form_aura(sim);
            druid.register_cat_form_aura(sim);
        }
    }
    Ok(Box::new(druid))
}

impl Druid {
    /// Go `druid.RegisterSpell`: a spell with the forms it may be cast in. The wrapper gives
    /// every one of them an extra cast condition.
    pub(super) fn register_spell(
        &mut self,
        sim: &mut Sim,
        form_mask: u8,
        mut config: SpellConfig,
    ) -> SpellId {
        config.has_extra_cast_condition = true;
        let id = sim.register_spell(self.unit, config);
        self.form_masks.push((id, form_mask));
        id
    }

    /// Go `Druid.Initialize`.
    fn initialize_druid(&mut self, sim: &mut Sim) {
        self.st.form.set(self.st.starting_form);
        // RegisterBaselineSpells.
        self.register_innervate_cd(sim);
        self.register_thorns_spell(sim);
    }
}

impl PrepAgent for Druid {
    fn add_party_buffs(&self, party_buffs: &mut Message) {
        match self.spec {
            // BalanceDruid does not override AddPartyBuffs: Druid's does.
            Spec::Balance => {
                if self.st.in_form(BEAR | CAT) && self.tal.leader_of_the_pack {
                    party_buffs.set_bool("leader_of_the_pack", true);
                } else if self.st.in_form(MOONKIN) && self.tal.moonkin_form {
                    party_buffs.set_bool("moonkin_aura", true);
                }
            }
            Spec::FeralCat | Spec::FeralBear => {
                if self.tal.leader_of_the_pack {
                    party_buffs.set_bool("leader_of_the_pack", true);
                }
            }
        }
    }

    fn apply_talents(&mut self, sim: &mut Sim, _unit: UnitId) {
        // Omen of Clarity is a baseline passive in Forever, wired in here with the procs.
        self.apply_omen_of_clarity(sim);
        self.register_balance_talents(sim);
        self.register_feral_combat_talents(sim);
        self.register_restoration_talents(sim);
    }

    fn initialize(&mut self, sim: &mut Sim, _unit: UnitId) {
        self.initialize_druid(sim);
        match self.spec {
            Spec::Balance => self.register_balance_spells(sim),
            Spec::FeralCat => self.register_feral_cat_spells(sim),
            Spec::FeralBear => self.register_feral_tank_spells(sim),
        }
    }

    fn reset(&mut self, sim: &mut Sim, unit: UnitId) {
        self.st.form.set(self.st.starting_form);
        match self.spec {
            Spec::Balance => {}
            Spec::FeralCat => {
                self.st.clear_form(sim, unit);
                if let Some(aura) = self.cat_form_aura {
                    sim.activate(aura);
                }
            }
            Spec::FeralBear => {
                self.st.clear_form(sim, unit);
                if let Some(aura) = self.bear_form_aura {
                    sim.activate(aura);
                }
            }
        }
        let _ = (HUMANOID, PowerBar::Mana);
    }

    fn apply_item_effect(&mut self, sim: &mut Sim, unit: UnitId, item: i32) -> bool {
        self.apply_idol_effect(sim, unit, item)
    }

    fn talents(&self) -> &Message {
        &self.talents
    }

    fn class_spells(&self) -> &'static [ClassSpellName] {
        CLASS_SPELLS
    }

    fn effects_in(
        &self,
        env: &crate::prepare::env::Environment,
        notes: &mut Vec<String>,
    ) -> Vec<serde_json::Value> {
        self.druid_effects(env, notes)
    }

    fn damage_effect(&self, sim: &Sim, spell: SpellId) -> Option<serde_json::Value> {
        self.druid_damage_effect(sim, spell)
    }

    fn custom_apl_action(&self, _sim: &Sim, _unit: UnitId, action: &Message) -> Option<bool> {
        // Go: the cat's and the bear's NewAPLAction build their optimal rotation action; the
        // other specs build nothing for it.
        match action.oneof("action") {
            Some(("cat_optimal_rotation_action", _)) => Some(self.spec == Spec::FeralCat),
            Some(("bear_optimal_rotation_action", _)) => Some(self.spec == Spec::FeralBear),
            _ => None,
        }
    }

    fn swing_replacement_keeps_swing(&self) -> bool {
        false
    }

    /// tools/oracle-v2/druid_feral.go `druidStatAuras`: Cat Form changes stats through
    /// AddStatsDynamic and its stat dependencies, and a bear's Enrage cuts its armor. Bear Form
    /// stays up for the whole fight of a bear, so its stats are the base ones.
    fn stat_auras(&self, sim: &Sim, unit: UnitId) -> Vec<String> {
        let mut labels = Vec::new();
        if self.cat_form.is_some() && sim.get_aura(unit, "Cat Form").is_some() {
            labels.push("Cat Form".to_string());
        }
        if let Some(aura) = self.st.enrage_aura.get() {
            labels.push(sim.aura(aura).label.clone());
        }
        // barkskin.go: the physical damage taken cut changes the target's swings.
        if self.barkskin.is_some() && sim.get_aura(unit, "Barkskin").is_some() {
            labels.push("Barkskin".to_string());
        }
        // forms.go: Bear Form's stats, armor and health, which the target's swings read.
        if let (Some(_), Some(aura)) = (self.bear_form, self.bear_form_aura) {
            labels.push(sim.aura(aura).label.clone());
        }
        labels
    }

    /// Rend and Tear registers one dynamic damage taken modifier on every target.
    fn damage_taken_modifiers(&self) -> usize {
        usize::from(self.tal.rend_and_tear > 0)
    }
}

/// The action ID Go writes for a spell with a tag.
pub(super) fn with_tag(id: i32, tag: i32) -> ActionId {
    ActionId {
        spell_id: id,
        tag,
        ..ActionId::default()
    }
}
