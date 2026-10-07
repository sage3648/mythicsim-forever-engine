//! The Shaman's spell registrations: Go `sim/shaman` `chain_lightning.go`, `lightning_bolt.go`,
//! `electric_spell.go`, `lava_burst.go`, `shocks.go`, `fire_totems.go` and `stormstrike.go`. A
//! closure Go gives a spell config (`ApplyEffects`, `OnTick` and the like) only runs in a fight,
//! so what preparation keeps of it is the field that says it is there:
//! `has_extra_cast_condition`.

use std::sync::OnceLock;

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::sim::{AuraConfig, Cooldown, Duration, EventCallbacks, Sim, UnitId, UnitType};
use crate::prepare::spell::{
    school, Cast, CastConfig, CostOptions, DefenseType, DotConfig, ProcMask, SpellConfig,
    SpellFlag, GCD_DEFAULT,
};
use crate::prepare::spelldata::{Ladder, Spell as Row};

use super::spell_data::spell_data;
use super::{flags, masks, Shaman, CAST_TAG_LIGHTNING_OVERLOAD};

pub(super) fn spell_action(id: i32) -> ActionId {
    ActionId {
        spell_id: id,
        ..ActionId::default()
    }
}

pub(super) fn tagged_action(id: i32, tag: i32) -> ActionId {
    ActionId {
        spell_id: id,
        tag,
        ..ActionId::default()
    }
}

/// `ManaCost: core.ManaCostOptions{FlatCost: flat}`.
pub(super) fn flat_cost(flat: i32) -> CostOptions {
    CostOptions {
        mana_flat_cost: flat,
        ..CostOptions::default()
    }
}

/// `Cast: core.CastConfig{DefaultCast: core.Cast{GCD: gcd, CastTime: cast_time}}`.
pub(super) fn default_cast(gcd: Duration, cast_time: Duration) -> CastConfig {
    CastConfig {
        default_cast: Cast {
            gcd,
            cast_time,
            ..Cast::default()
        },
        ..CastConfig::default()
    }
}

/// `max(rank.Cooldown(), rank.CategoryCooldown())`.
pub(super) fn longest_cooldown(row: &Row) -> Duration {
    row.cooldown().max(row.category_cooldown())
}

/// Go `Environment.TotalTargetCount`: every enemy the encounter holds.
pub(super) fn total_target_count(sim: &Sim) -> usize {
    sim.env_units
        .iter()
        .filter(|unit| sim.unit(**unit).unit_type == UnitType::Enemy)
        .count()
}

/// Lightning Overload's second bolt, one client row per Lightning Bolt rank (lightning_bolt.go).
pub(super) fn lightning_bolt_overload_ranks() -> &'static Ladder {
    static LADDER: OnceLock<Ladder> = OnceLock::new();
    LADDER.get_or_init(|| {
        Ladder::ranked(&[
            408439, 408440, 408441, 408442, 408443, 408472, 408473, 408474, 408475, 408477,
        ])
    })
}

/// Lightning Overload's Chain Lightning, one client row per rank (chain_lightning.go).
pub(super) fn chain_lightning_overload_ranks() -> &'static Ladder {
    static LADDER: OnceLock<Ladder> = OnceLock::new();
    LADDER.get_or_init(|| Ladder::ranked(&[408479, 408481, 408482, 408484]))
}

/// What Go's `ShamSpellConfig` carries into `newElectricSpellConfig`.
struct ElectricConfig {
    action_id: ActionId,
    rank: i32,
    base_flat_cost: i32,
    base_cast_time: Duration,
    is_elemental_overload: bool,
    bonus_coefficient: f64,
    class_spell_mask: i64,
}

/// Go `newElectricSpellConfig`: the shared precomputation of Lightning Bolt and Chain Lightning.
fn electric_spell_config(config: &ElectricConfig) -> SpellConfig {
    let mut spell_flags = flags::SHAMAN_SPELL | flags::FOCUSABLE;
    if config.is_elemental_overload {
        spell_flags = spell_flags | SpellFlag::PASSIVE_SPELL | SpellFlag::PROC;
    } else {
        spell_flags |= SpellFlag::APL;
    }
    let mut spell = SpellConfig {
        action_id: config.action_id.clone(),
        spell_school: school::NATURE,
        defense_type: DefenseType::Magic,
        proc_mask: ProcMask::SPELL_DAMAGE,
        flags: spell_flags,
        class_spell_mask: config.class_spell_mask,
        rank: config.rank,
        cost: flat_cost(if config.is_elemental_overload {
            0
        } else {
            config.base_flat_cost
        }),
        // ModifyCast holds the melee swing for a hard cast, which only a fight runs.
        cast: default_cast(GCD_DEFAULT, config.base_cast_time),
        damage_multiplier: 1.0,
        bonus_coefficient: config.bonus_coefficient,
        threat_multiplier: 1.0,
        ..SpellConfig::default()
    };
    if config.is_elemental_overload {
        spell.action_id.tag = CAST_TAG_LIGHTNING_OVERLOAD;
        spell.cost = flat_cost(0);
        spell.cast = CastConfig::default();
        spell.threat_multiplier = 0.0;
    }
    spell
}

impl Shaman {
    /// Go `registerChainLightningSpell`.
    pub(super) fn register_chain_lightning_spell(&self, sim: &mut Sim, unit: UnitId) {
        let max_hits = total_target_count(sim).min(3);
        let shared_cd_timer = sim.new_timer(unit);
        spell_data().chain_lightning.each(|rank, config| {
            self.new_chain_lightning_spell(sim, unit, config, rank, false, Some(shared_cd_timer));
            for _ in 0..max_hits {
                self.new_chain_lightning_spell(sim, unit, config, rank, true, None);
            }
        });
    }

    fn new_chain_lightning_spell(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        config: &'static Row,
        rank: i32,
        is_elemental_overload: bool,
        shared_cd_timer: Option<crate::prepare::sim::TimerId>,
    ) {
        let damage = if is_elemental_overload {
            chain_lightning_overload_ranks().rank(rank).damage_effect()
        } else {
            config.damage_effect()
        };
        let mut spell_config = electric_spell_config(&ElectricConfig {
            action_id: spell_action(config.id),
            rank,
            base_flat_cost: config.cost() as i32,
            base_cast_time: config.cast_time(),
            is_elemental_overload,
            bonus_coefficient: damage.coeff(),
            class_spell_mask: if is_elemental_overload {
                masks::CHAIN_LIGHTNING_OVERLOAD
            } else {
                masks::CHAIN_LIGHTNING
            },
        });
        if !is_elemental_overload {
            spell_config.cast.cd = Cooldown {
                timer: shared_cd_timer,
                duration: longest_cooldown(config),
            };
        }
        sim.register_spell(unit, spell_config);
    }

    /// Go `registerLightningBoltSpell`.
    pub(super) fn register_lightning_bolt_spell(&self, sim: &mut Sim, unit: UnitId) {
        spell_data().lightning_bolt.each(|rank, config| {
            for is_elemental_overload in [false, true] {
                let damage = if is_elemental_overload {
                    lightning_bolt_overload_ranks().rank(rank).damage_effect()
                } else {
                    config.damage_effect()
                };
                let mut spell_config = electric_spell_config(&ElectricConfig {
                    action_id: spell_action(config.id),
                    rank,
                    base_flat_cost: config.cost() as i32,
                    base_cast_time: config.cast_time(),
                    is_elemental_overload,
                    bonus_coefficient: damage.coeff(),
                    class_spell_mask: if is_elemental_overload {
                        masks::LIGHTNING_BOLT_OVERLOAD
                    } else {
                        masks::LIGHTNING_BOLT
                    },
                });
                spell_config.missile_speed = 20.0;
                sim.register_spell(unit, spell_config);
            }
        });
    }

    /// Go `registerLavaBurstSpell`: Lava Burst is new in Forever.
    pub(super) fn register_lava_burst_spell(&self, sim: &mut Sim, unit: UnitId) {
        if !self.has_talent("lava_burst") {
            return;
        }
        let rank = spell_data().lava_burst.highest();
        let timer = sim.new_timer(unit);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_DAMAGE,
                flags: SpellFlag::APL | flags::SHAMAN_SPELL | flags::FOCUSABLE,
                class_spell_mask: masks::LAVA_BURST,
                missile_speed: f64::from(rank.speed),
                cost: flat_cost(rank.cost() as i32),
                cast: CastConfig {
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: longest_cooldown(rank),
                    },
                    ..default_cast(rank.gcd(), rank.cast_time())
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: rank.damage_effect().coeff(),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `newShockSpellConfig`: shared logic for all shocks.
    fn new_shock_spell_config(
        &self,
        rank: &'static Row,
        spell_school: u8,
        shock_timer: crate::prepare::sim::TimerId,
    ) -> SpellConfig {
        SpellConfig {
            action_id: spell_action(rank.id),
            spell_school,
            defense_type: DefenseType::Magic,
            proc_mask: ProcMask::SPELL_DAMAGE,
            flags: flags::SHAMAN_SPELL | flags::SHOCK | SpellFlag::APL | flags::INSTANT,
            max_range: f64::from(rank.max_range),
            cost: flat_cost(rank.cost() as i32),
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(shock_timer),
                    duration: longest_cooldown(rank),
                },
                ..default_cast(rank.gcd(), 0)
            },
            damage_multiplier: 1.0,
            bonus_coefficient: rank.damage_effect().coeff(),
            threat_multiplier: 1.0,
            ..SpellConfig::default()
        }
    }

    /// Go `registerShocks`.
    pub(super) fn register_shocks(&mut self, sim: &mut Sim, unit: UnitId) {
        let shock_timer = sim.new_timer(unit);
        self.register_earth_shock_spell(sim, unit, shock_timer);
        self.register_flame_shock_spell(sim, unit, shock_timer);
        self.register_frost_shock_spell(sim, unit, shock_timer);
    }

    fn register_earth_shock_spell(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        shock_timer: crate::prepare::sim::TimerId,
    ) {
        let rank = spell_data().earth_shock.highest();
        let mut config = self.new_shock_spell_config(rank, school::NATURE, shock_timer);
        config.class_spell_mask = masks::EARTH_SHOCK;
        config.flags |= SpellFlag::BINARY;
        sim.register_spell(unit, config);
    }

    fn register_flame_shock_spell(
        &mut self,
        sim: &mut Sim,
        unit: UnitId,
        shock_timer: crate::prepare::sim::TimerId,
    ) {
        let rank = spell_data().flame_shock.highest();
        let tick = rank.periodic_effect();

        let mut config = self.new_shock_spell_config(rank, school::FIRE, shock_timer);
        config.class_spell_mask = masks::FLAME_SHOCK_DIRECT;
        let dot_flags = SpellFlag(config.flags.0 & !SpellFlag::APL.0) | SpellFlag::PASSIVE_SPELL;
        let flame_shock = sim.register_spell(unit, config);
        self.flame_shock = Some(flame_shock);

        let dot_spell = sim.register_spell(
            unit,
            SpellConfig {
                action_id: tagged_action(rank.id, 1),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_DAMAGE,
                flags: dot_flags,
                class_spell_mask: masks::FLAME_SHOCK_DOT,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                dot: DotConfig {
                    aura: AuraConfig {
                        label: "Flame Shock".to_string(),
                        ..AuraConfig::default()
                    },
                    number_of_ticks: (rank.duration() / tick.period()) as i32,
                    tick_length: tick.period(),
                    bonus_coefficient: tick.coeff(),
                    ..DotConfig::default()
                },
                ..SpellConfig::default()
            },
        );
        sim.spell_mut(flame_shock).related_dot_spell = Some(dot_spell);
    }

    fn register_frost_shock_spell(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        shock_timer: crate::prepare::sim::TimerId,
    ) {
        let rank = spell_data().frost_shock.highest();
        let mut config = self.new_shock_spell_config(rank, school::FROST, shock_timer);
        config.class_spell_mask = masks::FROST_SHOCK;
        config.flags |= SpellFlag::BINARY;
        config.threat_multiplier *= 2.0;
        sim.register_spell(unit, config);
    }

    /// Go `registerSearingTotemSpell`: the totem lays down a pulse spell of its own.
    pub(super) fn register_searing_totem_spell(&self, sim: &mut Sim, unit: UnitId) {
        let data = spell_data();
        let rank = data.searing_totem.highest();
        let attack = data.searing_totem_triggered.highest();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(attack.id),
                spell_school: attack.spell_school(),
                defense_type: attack.defense_type_core(),
                proc_mask: ProcMask::EMPTY,
                flags: flags::SHAMAN_SPELL | SpellFlag::PASSIVE_SPELL,
                class_spell_mask: masks::SEARING_TOTEM,
                missile_speed: f64::from(attack.speed),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: attack.damage_effect().coeff(),
                ..SpellConfig::default()
            },
        );

        // The pulse's own cast time is the interval between pulses.
        let tick_length = attack.cast_time();
        let duration = rank.duration();

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL | flags::SHAMAN_SPELL | flags::INSTANT,
                class_spell_mask: masks::SEARING_TOTEM,
                cost: flat_cost(rank.cost() as i32),
                cast: CastConfig {
                    ignore_haste: true,
                    ..default_cast(rank.gcd(), 0)
                },
                damage_multiplier: 1.0,
                dot: DotConfig {
                    aura: AuraConfig {
                        label: "Searing Totem".to_string(),
                        ..AuraConfig::default()
                    },
                    number_of_ticks: (duration / tick_length) as i32,
                    tick_length,
                    ..DotConfig::default()
                },
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerMagmaTotemSpell`.
    pub(super) fn register_magma_totem_spell(&mut self, sim: &mut Sim, unit: UnitId) {
        let data = spell_data();
        let rank = data.magma_totem.highest();
        let pulse = data.magma_totem_triggered.by_id(10581);
        let duration = rank.duration();
        let tick_length = crate::prepare::sim::seconds(2.0);

        let magma_totem = sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL | flags::SHAMAN_SPELL | flags::INSTANT,
                class_spell_mask: masks::MAGMA_TOTEM,
                cost: flat_cost(rank.cost() as i32),
                cast: CastConfig {
                    ignore_haste: true,
                    ..default_cast(rank.gcd(), 0)
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                dot: DotConfig {
                    is_aoe: true,
                    aura: AuraConfig {
                        label: "Magma Totem".to_string(),
                        ..AuraConfig::default()
                    },
                    number_of_ticks: (duration / tick_length) as i32,
                    tick_length,
                    bonus_coefficient: pulse.damage_effect().coeff(),
                    ..DotConfig::default()
                },
                ..SpellConfig::default()
            },
        );
        self.magma_totem = Some(magma_totem);
    }

    /// Go `registerFireNovaSpell`: the caster-centred nova on a 10 sec cooldown. Its damage is
    /// 408428, which the scripted dummy casts.
    pub(super) fn register_fire_nova_spell(&self, sim: &mut Sim, unit: UnitId) {
        let data = spell_data();
        let rank = data.fire_nova.highest();
        let damage = data.fire_nova_triggered.by_id(408428);
        let timer = sim.new_timer(unit);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_DAMAGE,
                flags: SpellFlag::APL | flags::SHAMAN_SPELL | flags::INSTANT,
                class_spell_mask: masks::FIRE_NOVA,
                cost: flat_cost(rank.cost() as i32),
                cast: CastConfig {
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: longest_cooldown(rank),
                    },
                    ..default_cast(rank.gcd(), 0)
                },
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: damage.damage_effect().coeff(),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerStormstrikeSpell`: the two hits, the debuff on every enemy, then the cast.
    pub(super) fn register_stormstrike_spell(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().stormstrike.highest();
        for is_mh in [true, false] {
            sim.register_spell(
                unit,
                SpellConfig {
                    action_id: tagged_action(rank.id, if is_mh { 1 } else { 2 }),
                    spell_school: school::PHYSICAL,
                    defense_type: DefenseType::Melee,
                    proc_mask: if is_mh {
                        ProcMask::MELEE_MH_SPECIAL
                    } else {
                        ProcMask::MELEE_OH_SPECIAL
                    },
                    flags: SpellFlag::MELEE_METRICS,
                    class_spell_mask: masks::STORMSTRIKE_DAMAGE,
                    threat_multiplier: 1.0,
                    damage_multiplier: 1.0,
                    ..SpellConfig::default()
                },
            );
        }

        let label = format!("Stormstrike-{}", sim.unit(unit).label);
        let duration = rank.duration();
        let max_stacks = i32::from(rank.proc_charges);
        let action_id = spell_action(rank.id);
        let enemies: Vec<UnitId> = sim
            .env_units
            .iter()
            .copied()
            .filter(|target| sim.unit(*target).unit_type == UnitType::Enemy)
            .collect();
        for target in enemies {
            let aura = sim.get_or_register_aura(
                target,
                AuraConfig {
                    label: label.clone(),
                    action_id: Some(action_id.clone()),
                    duration,
                    max_stacks,
                    events: EventCallbacks {
                        on_spell_hit_taken: true,
                        ..EventCallbacks::default()
                    },
                    ..AuraConfig::default()
                },
            );
            // Client 17364 (aura 271): only this shaman's Lightning Bolt, Chain Lightning and
            // Earth Shock take the bonus.
            sim.attach_ddbc(aura, 0, 1, unit);
        }

        let timer = sim.new_timer(unit);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::MELEE_METRICS | SpellFlag::APL,
                class_spell_mask: masks::STORMSTRIKE_CAST,
                cost: flat_cost(rank.cost() as i32),
                cast: CastConfig {
                    ignore_haste: true,
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: longest_cooldown(rank),
                    },
                    ..default_cast(GCD_DEFAULT, 0)
                },
                has_extra_cast_condition: true,
                ..SpellConfig::default()
            },
        );
    }
}
