//! The spells and auras Go's `RegisterBaselineSpells` and `ApplyTalents` give every druid:
//! Innervate, Thorns, Omen of Clarity, Faerie Fire.

use crate::classes::druid::forms::{ANY, HUMANOID, MOONKIN, TREE};
use crate::contracts::prepared_v2::ActionId;
use crate::prepare::buffs::drivers::attach_innervate_regen;
use crate::prepare::buffs::generated::{FAERIE_FIRE, INNERVATES, THORNS};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::dbcenums;
use crate::prepare::major_cooldown::COOLDOWN_PRIORITY_DEFAULT;
use crate::prepare::resolve_proc::{chance, proc_trigger};
use crate::prepare::sim::{AuraConfig, AuraId, Cooldown, EventCallbacks, Sim, UnitId};
use crate::prepare::spell::{
    Cast, CastConfig, CostOptions, LabeledAuraArrays, ProcMask, SpellConfig, SpellFlag,
};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};
use crate::prepare::spelldata::Ladder;

use super::{masks, Druid};

/// Go `NewEnemyAuraArray`: the aura `make` builds on each enemy, by unit index.
pub(super) fn new_enemy_aura_array(
    sim: &mut Sim,
    make: impl Fn(&mut Sim, UnitId) -> AuraId,
) -> Vec<Option<AuraId>> {
    let units = sim.all_units();
    let mut auras = vec![None; units.len()];
    for target in units {
        if sim.unit(target).unit_type == crate::prepare::sim::UnitType::Enemy {
            let index = sim.unit(target).unit_index as usize;
            auras[index] = Some(make(sim, target));
        }
    }
    auras
}

/// Go `AuraArray.ToMap`: nil for an empty array, otherwise the array under its first label.
pub(super) fn aura_array_to_map(sim: &Sim, auras: &[Option<AuraId>]) -> LabeledAuraArrays {
    let mut map = LabeledAuraArrays::new();
    if let Some(first) = auras.iter().flatten().next() {
        map.insert(sim.aura(*first).label.clone(), auras.to_vec());
    }
    map
}

impl Druid {
    /// Go `applyOmenOfClarity`.
    pub(super) fn apply_omen_of_clarity(&mut self, sim: &mut Sim) {
        let unit = self.unit;
        let clearcasting = Ladder::ranked(&[16870]).highest();

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Clearcasting".to_string(),
                action_id: Some(ActionId::spell(clearcasting.id)),
                duration: clearcasting.duration(),
                events: EventCallbacks {
                    on_cast_complete: true,
                    ..EventCallbacks::default()
                },
                ..AuraConfig::default()
            },
        );
        sim.attach_spell_mod(
            aura,
            SpellModConfig {
                kind: SpellModType::PowerCostPctAdd,
                class_mask: masks::CLEARCASTING_SPELLS,
                float_value: clearcasting
                    .effect(dbcenums::A_ADD_PCT_MODIFIER, dbcenums::SPELLMOD_COST)
                    .percent(),
                ..SpellModConfig::default()
            },
        );
        self.clearcasting_aura = Some(aura);

        let omen = Ladder::ranked(&[16864]).highest();
        let trigger = proc_trigger(sim, Some(unit), omen, &[chance(1.0)]);
        sim.make_proc_trigger_aura(unit, &trigger);
    }

    /// Go `registerInnervateCD`.
    pub(super) fn register_innervate_cd(&mut self, sim: &mut Sim) {
        let unit = self.unit;
        let rank = Ladder::ranked(&[29166]).highest();
        let index = sim.unit(unit).index;
        let action = ActionId {
            spell_id: rank.id,
            tag: index,
            ..ActionId::default()
        };

        // druid.innervateAura: the aura an earlier Innervate cast already registered, or the
        // generated one with the regen attached.
        let aura = match sim.get_aura(unit, "Innervates (Player)") {
            Some(aura) => aura,
            None => {
                let aura = INNERVATES.aura(sim, unit, true, 0, 0.0);
                attach_innervate_regen(sim, unit, aura)
            }
        };
        let _ = aura;

        let mana = rank.mana_cost();
        let timer = sim.new_timer(unit);
        let spell = self.register_spell(
            sim,
            HUMANOID | MOONKIN | TREE,
            SpellConfig {
                action_id: action,
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                class_spell_mask: masks::INNERVATE,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                max_range: f64::from(rank.max_range),
                cost: CostOptions {
                    mana_base_cost_percent: mana.base_cost_percent,
                    mana_flat_cost: mana.flat_cost,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: rank.gcd(),
                        ..Cast::default()
                    },
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: rank.cooldown().max(rank.category_cooldown()),
                    },
                    ..CastConfig::default()
                },
                ..SpellConfig::default()
            },
        );
        sim.add_major_cooldown(
            unit,
            MajorCooldown {
                spell,
                priority: COOLDOWN_PRIORITY_DEFAULT,
                cooldown_type: cooldown_type::MANA,
                allow_spell_queueing: false,
                timings: Vec::new(),
            },
        );
    }

    /// Go `registerThornsSpell`.
    pub(super) fn register_thorns_spell(&mut self, sim: &mut Sim) {
        let unit = self.unit;
        let rank = Ladder::ranked(&[467, 782, 1075, 8914, 9756, 9910]).highest();
        let thorns_aura = THORNS.aura(sim, unit, true, 0, 0.0);
        self.register_spell(
            sim,
            HUMANOID | MOONKIN | TREE,
            SpellConfig {
                action_id: ActionId::spell(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                class_spell_mask: masks::THORNS,
                proc_mask: ProcMask::EMPTY,
                max_range: f64::from(rank.max_range),
                cost: CostOptions {
                    mana_flat_cost: rank.cost() as i32,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: rank.gcd(),
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                related_self_buff: Some(thorns_aura),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerFaerieFireSpell`.
    pub(super) fn register_faerie_fire_spell(&mut self, sim: &mut Sim) {
        let rank = Ladder::ranked(&[770, 778, 9749, 9907]).highest();
        // Forever has no Improved Faerie Fire node, so there are no talent points to pass.
        let auras = new_enemy_aura_array(sim, |sim, target| {
            FAERIE_FIRE.aura(sim, target, true, 0, 0.0)
        });
        self.faerie_fire_auras = auras.clone();
        let spell = self.register_spell(
            sim,
            ANY,
            SpellConfig {
                class_spell_mask: masks::FAERIE_FIRE,
                action_id: ActionId::spell(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_DAMAGE,
                flags: SpellFlag::APL,
                rank: rank.rank_number(),
                cost: CostOptions {
                    mana_flat_cost: rank.cost() as i32,
                    ..CostOptions::default()
                },
                cast: CastConfig {
                    default_cast: Cast {
                        gcd: rank.gcd(),
                        ..Cast::default()
                    },
                    ..CastConfig::default()
                },
                threat_multiplier: 1.0,
                // Two threat a level, the sim's long-standing value; the client states none.
                flat_threat_bonus: 2.0
                    * f64::from(crate::prepare::character::constants::CHARACTER_LEVEL),
                max_range: f64::from(rank.max_range),
                related_aura_arrays: aura_array_to_map(sim, &auras),
                ..SpellConfig::default()
            },
        );
        self.faerie_fire = Some(spell);
    }
}
