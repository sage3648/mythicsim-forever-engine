//! Go sim/core/aura_helpers.go: the helpers that build and extend auras.
//!
//! Go's helpers take `*Aura` and return it for chaining. Here an aura is an `AuraId` in the
//! `Sim` arena, so a helper is a `Sim` method taking the id and returning it. A helper that
//! Go calls on an `Aura` literal before registering it takes the `AuraConfig` instead.
//!
//! Closures that only run in a fight (a proc handler, an extra condition, the damage absorbed
//! callbacks) are not carried: preparation records which callbacks the aura has, as the exported
//! `callbacks` list shows them, and runs every lifecycle callback a reset can reach.
//!
//! Not ported here: `AttachSpellMod` lives in spell_mod.rs, and `MakePermanent` in sim.rs.

use std::cell::Cell;
use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::data::spells::ClassFlags;

use super::character::cooldown_type;
use super::periodic_action::PeriodicActionOptions;
use super::procs::DynamicProcManager;
use super::sim::{
    school_array_index, AuraConfig, AuraId, Cooldown, Duration, EventCallbacks, Sim, UnitId,
    UnitType, NEVER_EXPIRES,
};
use super::spell::{ProcMask, Spell, SpellFlag};
use super::spell_mod::{class_flags_is_zero, class_flags_matches};
use super::stats::{DepId, PseudoStats, SchoolIndex, Stat, Stats};

/// Go `OnTemporaryStatsChange`: told the stats a temporary stats aura added or removed.
pub(crate) type TemporaryStatsListener = Rc<dyn Fn(&mut Sim, AuraId, &Stats)>;

/// Go `AuraCallback`: which event a proc trigger listens to. The bit values are Go's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CallbackMask(pub u16);

#[allow(dead_code)]
impl CallbackMask {
    pub const EMPTY: CallbackMask = CallbackMask(0);
    pub const ON_SPELL_HIT_DEALT: CallbackMask = CallbackMask(1 << 1);
    pub const ON_SPELL_HIT_TAKEN: CallbackMask = CallbackMask(1 << 2);
    pub const ON_PERIODIC_DAMAGE_DEALT: CallbackMask = CallbackMask(1 << 3);
    pub const ON_HEAL_DEALT: CallbackMask = CallbackMask(1 << 4);
    pub const ON_PERIODIC_HEAL_DEALT: CallbackMask = CallbackMask(1 << 5);
    pub const ON_CAST_COMPLETE: CallbackMask = CallbackMask(1 << 6);
    pub const ON_APPLY_EFFECTS: CallbackMask = CallbackMask(1 << 7);
    pub const ON_PERIODIC_DAMAGE_TAKEN: CallbackMask = CallbackMask(1 << 8);

    pub fn matches(self, other: CallbackMask) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for CallbackMask {
    type Output = CallbackMask;
    fn bitor(self, other: CallbackMask) -> CallbackMask {
        CallbackMask(self.0 | other.0)
    }
}

/// Go `HitOutcome`. The bit values are Go's: `OutcomeEmpty` is 0 and the single bits start at 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HitOutcome(pub u16);

#[allow(dead_code)]
impl HitOutcome {
    pub const EMPTY: HitOutcome = HitOutcome(0);
    pub const MISS: HitOutcome = HitOutcome(1 << 1);
    pub const HIT: HitOutcome = HitOutcome(1 << 2);
    pub const DODGE: HitOutcome = HitOutcome(1 << 3);
    pub const GLANCE: HitOutcome = HitOutcome(1 << 4);
    pub const PARRY: HitOutcome = HitOutcome(1 << 5);
    pub const BLOCK: HitOutcome = HitOutcome(1 << 6);
    pub const CRIT: HitOutcome = HitOutcome(1 << 7);
    pub const CRUSH: HitOutcome = HitOutcome(1 << 8);
    pub const PARTIAL_1_4: HitOutcome = HitOutcome(1 << 9);
    pub const PARTIAL_2_4: HitOutcome = HitOutcome(1 << 10);
    pub const PARTIAL_3_4: HitOutcome = HitOutcome(1 << 11);
    pub const PARTIAL: HitOutcome =
        HitOutcome(Self::PARTIAL_1_4.0 | Self::PARTIAL_2_4.0 | Self::PARTIAL_3_4.0);
    pub const LANDED: HitOutcome =
        HitOutcome(Self::HIT.0 | Self::CRIT.0 | Self::CRUSH.0 | Self::GLANCE.0 | Self::BLOCK.0);

    pub fn matches(self, other: HitOutcome) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for HitOutcome {
    type Output = HitOutcome;
    fn bitor(self, other: HitOutcome) -> HitOutcome {
        HitOutcome(self.0 | other.0)
    }
}

/// Go `IsEmptyAction`: neither a spell, an item nor an other ID (the tag is ignored).
pub(crate) fn is_empty_action(id: &ActionId) -> bool {
    id.spell_id == 0 && id.item_id == 0 && id.other_id.is_empty()
}

/// Go `ProcTrigger`, without its handler and extra condition, which only run in a fight.
#[derive(Clone, Default)]
pub(crate) struct ProcTrigger {
    pub name: String,
    pub action_id: ActionId,
    pub metrics_action_id: ActionId,
    pub duration: Duration,
    pub callback: CallbackMask,
    pub proc_mask: ProcMask,
    pub proc_mask_exclude: ProcMask,
    pub spell_flags: SpellFlag,
    pub spell_flags_exclude: SpellFlag,
    pub outcome: HitOutcome,
    pub require_damage_dealt: bool,
    pub proc_chance: f64,
    pub dpm: Option<Rc<DynamicProcManager>>,
    pub icd: Duration,
    /// If false (the default), Go calls the handler one spell batch window later.
    pub trigger_immediately: bool,
    pub class_spell_mask: i64,
    /// The client's EffectSpellClassMask: the spells this listener fires on. A trigger that
    /// sets both this and `class_spell_mask` fires only on the spells both name.
    pub class_flags: ClassFlags,
    /// The Only Proc From Class Abilities flag.
    pub class_spells_only: bool,
    /// The Can Proc From Procs attribute: the listener also fires on hits from spells flagged
    /// `SpellFlag::PROC`.
    pub can_proc_from_procs: bool,
    /// A weapon proc: the game casts these off every melee or ranged hit that lacks
    /// `SUPPRESS_WEAPON_PROCS`, whether or not that hit was itself a proc.
    pub is_weapon_proc: bool,
}

impl ProcTrigger {
    /// Go `ProcTrigger.canProcFrom`: the game's rule for whether a hit may reach a listener at
    /// all, before the proc flags are matched.
    pub(crate) fn can_proc_from(&self, spell: &Spell) -> bool {
        if self.is_weapon_proc {
            return !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS);
        }
        self.can_proc_from_procs || !spell.flags.matches(SpellFlag::PROC)
    }

    /// Go `ProcTrigger.matchesSpell`: the spell-side checks every callback path applies.
    pub(crate) fn matches_spell(&self, spell: &Spell) -> bool {
        if !self.can_proc_from(spell) {
            return false;
        }
        if self.spell_flags != SpellFlag::NONE && !spell.flags.matches(self.spell_flags) {
            return false;
        }
        if self.spell_flags_exclude != SpellFlag::NONE
            && spell.flags.matches(self.spell_flags_exclude)
        {
            return false;
        }
        if self.class_spell_mask > 0 && self.class_spell_mask & spell.class_spell_mask == 0 {
            return false;
        }
        if !class_flags_is_zero(&self.class_flags)
            && !class_flags_matches(&self.class_flags, &spell.class_flags)
        {
            return false;
        }
        if self.class_spells_only
            && spell.class_spell_mask == 0
            && class_flags_is_zero(&spell.class_flags)
        {
            return false;
        }
        if self.proc_mask_exclude != ProcMask::UNKNOWN
            && spell.proc_mask.matches(self.proc_mask_exclude)
        {
            return false;
        }
        if self.proc_mask != ProcMask::UNKNOWN && !spell.proc_mask.matches(self.proc_mask) {
            return false;
        }
        true
    }
}

/// What `AttachProcTriggerCallback` registers on an aura: the ICD cooldown with its new timer,
/// the dynamic proc manager and the callbacks the trigger listens to.
struct ProcTriggerRegistration {
    icd: Option<Cooldown>,
    dpm: Option<Rc<DynamicProcManager>>,
    events: EventCallbacks,
}

impl Sim {
    /// The part of Go's `AttachProcTriggerCallback` that is state: it creates the ICD's timer
    /// first, as Go does.
    fn register_proc_trigger(
        &mut self,
        unit: UnitId,
        config: &ProcTrigger,
    ) -> ProcTriggerRegistration {
        let icd = (config.icd != 0).then(|| Cooldown {
            timer: Some(self.new_timer(unit)),
            duration: config.icd,
        });
        let callback = config.callback;
        let events = EventCallbacks {
            on_spell_hit_dealt: callback.matches(CallbackMask::ON_SPELL_HIT_DEALT),
            on_spell_hit_taken: callback.matches(CallbackMask::ON_SPELL_HIT_TAKEN),
            on_periodic_damage_dealt: callback.matches(CallbackMask::ON_PERIODIC_DAMAGE_DEALT),
            on_heal_dealt: callback.matches(CallbackMask::ON_HEAL_DEALT),
            on_periodic_heal_dealt: callback.matches(CallbackMask::ON_PERIODIC_HEAL_DEALT),
            on_cast_complete: callback.matches(CallbackMask::ON_CAST_COMPLETE),
            on_apply_effects: callback.matches(CallbackMask::ON_APPLY_EFFECTS),
            on_periodic_damage_taken: callback.matches(CallbackMask::ON_PERIODIC_DAMAGE_TAKEN),
            ..EventCallbacks::default()
        };
        ProcTriggerRegistration {
            icd,
            dpm: config.dpm.clone(),
            events,
        }
    }

    /// Go `AttachProcTriggerCallback` on an `Aura` literal that is not registered yet.
    pub(crate) fn attach_proc_trigger_callback_to_config(
        &mut self,
        aura: &mut AuraConfig,
        unit: UnitId,
        config: &ProcTrigger,
    ) {
        let registration = self.register_proc_trigger(unit, config);
        if registration.icd.is_some() {
            aura.icd = registration.icd;
        }
        if registration.dpm.is_some() {
            aura.dpm = registration.dpm;
        }
        merge_events(&mut aura.events, &registration.events);
    }

    /// Go `procAura.AttachProcTriggerCallback(unit, config)` on a registered aura.
    pub(crate) fn attach_proc_trigger_callback(
        &mut self,
        aura: AuraId,
        unit: UnitId,
        config: &ProcTrigger,
    ) {
        let registration = self.register_proc_trigger(unit, config);
        let aura = self.aura_mut(aura);
        if registration.icd.is_some() {
            aura.icd = registration.icd;
        }
        if registration.dpm.is_some() {
            aura.dpm = registration.dpm;
        }
        merge_events(&mut aura.events, &registration.events);
    }

    /// Go `unit.MakeProcTriggerAura`.
    pub(crate) fn make_proc_trigger_aura(&mut self, unit: UnitId, config: &ProcTrigger) -> AuraId {
        let mut aura = AuraConfig {
            label: config.name.clone(),
            action_id_for_proc: Some(config.action_id.clone()),
            duration: config.duration,
            action_id: Some(config.metrics_action_id.clone()),
            ..AuraConfig::default()
        };
        if config.duration == 0 {
            aura.duration = NEVER_EXPIRES;
            aura.on_reset = Some(Rc::new(|sim: &mut Sim, aura: AuraId| sim.activate(aura)));
        }
        self.attach_proc_trigger_callback_to_config(&mut aura, unit, config);
        self.get_or_register_aura(unit, aura)
    }

    /// Go `parentAura.MakeDependentProcTriggerAura`: the trigger fires only while the parent
    /// is active, a condition that only runs in a fight.
    pub(crate) fn make_dependent_proc_trigger_aura(
        &mut self,
        _parent: AuraId,
        unit: UnitId,
        config: &ProcTrigger,
    ) -> AuraId {
        self.make_proc_trigger_aura(unit, config)
    }

    /// Go `parentAura.AttachProcTrigger`: returns the parent aura.
    pub(crate) fn attach_proc_trigger(&mut self, parent: AuraId, config: &ProcTrigger) -> AuraId {
        let unit = self.aura(parent).unit;
        self.attach_proc_trigger_callback(parent, unit, config);
        parent
    }
}

/// Go sets the callbacks the trigger names on the aura, replacing any earlier one; presence is
/// all the export lists, so setting a flag is the same.
fn merge_events(into: &mut EventCallbacks, from: &EventCallbacks) {
    into.on_apply_effects |= from.on_apply_effects;
    into.on_cast_complete |= from.on_cast_complete;
    into.on_spell_hit_dealt |= from.on_spell_hit_dealt;
    into.on_spell_hit_taken |= from.on_spell_hit_taken;
    into.on_periodic_damage_dealt |= from.on_periodic_damage_dealt;
    into.on_periodic_damage_taken |= from.on_periodic_damage_taken;
    into.on_heal_dealt |= from.on_heal_dealt;
    into.on_periodic_heal_dealt |= from.on_periodic_heal_dealt;
}

/// Go `CustomStatBuffProcCondition`.
pub(crate) type CustomStatBuffProcCondition = Rc<dyn Fn(&Sim, AuraId) -> bool>;

/// Go `StatBuffAura`: an aura that additionally links to the stats it buffs.
#[derive(Clone)]
pub(crate) struct StatBuffAura {
    pub aura: AuraId,
    /// Every stat buffed (before dependencies) when the aura is activated.
    pub buffed_stat_types: Vec<Stat>,
    /// Any special condition beyond the standard ICD checks that must hold to activate it.
    pub custom_proc_condition: Option<CustomStatBuffProcCondition>,
    /// Whether the aura is currently swapped out in another item set.
    pub is_swapped: bool,
}

impl StatBuffAura {
    pub(crate) fn new(aura: AuraId, buffed_stat_types: Vec<Stat>) -> StatBuffAura {
        StatBuffAura {
            aura,
            buffed_stat_types,
            custom_proc_condition: None,
            is_swapped: false,
        }
    }

    /// Go `BuffsMatchingStat`.
    pub(crate) fn buffs_matching_stat(&self, stats_to_match: &[Stat]) -> bool {
        self.buffed_stat_types
            .iter()
            .any(|stat| stats_to_match.contains(stat))
    }

    /// Go `CanProc`.
    pub(crate) fn can_proc(&self, sim: &Sim) -> bool {
        !self.is_swapped
            && self
                .custom_proc_condition
                .as_ref()
                .is_none_or(|condition| condition(sim, self.aura))
    }

    /// Go `InferCDType`: the `CooldownType` bits.
    pub(crate) fn infer_cd_type(&self) -> u32 {
        if self.buffs_matching_stat(&[
            Stat::Armor,
            Stat::BlockPercent,
            Stat::DodgeRating,
            Stat::ParryRating,
            Stat::DodgePercent,
            Stat::ParryPercent,
            Stat::Health,
        ]) {
            cooldown_type::UNKNOWN | cooldown_type::SURVIVAL
        } else {
            cooldown_type::UNKNOWN | cooldown_type::DPS
        }
    }
}

/// Go `stats.GetBuffedStatTypes`: every stat with a positive value, in stat order.
pub(crate) fn buffed_stat_types(stats: &Stats) -> Vec<Stat> {
    Stat::ALL
        .into_iter()
        .filter(|stat| stats[*stat] > 0.0)
        .collect()
}

/// Go `StackingStatAura`.
#[derive(Clone, Default)]
pub(crate) struct StackingStatAura {
    pub aura: AuraConfig,
    pub bonus_per_stack: Stats,
}

/// Go `TemporaryStatBuffWithStacksConfig`.
#[derive(Clone, Default)]
pub(crate) struct TemporaryStatBuffWithStacksConfig {
    pub stacking_aura_label: String,
    pub stacking_aura_action_id: ActionId,
    pub aura_label: String,
    pub action_id: ActionId,
    pub bonus_per_stack: Stats,
    pub max_stacks: i32,
    pub time_per_stack: Duration,
    pub duration: Duration,
    pub tick_immediately: bool,
    /// Set when stacks come from an event rather than a timer. The window aura is still built
    /// and bounds the stacking aura, which is given no duration of its own and is dropped when
    /// the window ends.
    pub stacks_from_event: bool,
}

/// A stat an aura multiplies and the factor it multiplies it by: Go `StatMultiplier`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct StatMultiplier {
    pub stat: Stat,
    pub multiplier: f64,
}

impl Sim {
    /// Go `MakeStackingAura`.
    pub(crate) fn make_stacking_aura(
        &mut self,
        unit: UnitId,
        mut config: StackingStatAura,
    ) -> StatBuffAura {
        let bonus_per_stack = config.bonus_per_stack;
        config.aura.on_stacks_change = Some(Rc::new(
            move |sim: &mut Sim, _aura: AuraId, old_stacks: i32, new_stacks: i32| {
                sim.add_stats_dynamic(
                    unit,
                    &bonus_per_stack.multiply(f64::from(new_stacks - old_stacks)),
                );
            },
        ));
        StatBuffAura::new(
            self.get_or_register_aura(unit, config.aura),
            buffed_stat_types(&bonus_per_stack),
        )
    }

    /// Go `BlockPrepull`: the aura is deactivated at the encounter start.
    pub(crate) fn block_prepull(&mut self, aura: AuraId) -> AuraId {
        self.apply_on_encounter_start(aura);
        aura
    }

    /// Go `Character.NewTemporaryStatBuffWithStacks`. The periodic stack timer only runs in a
    /// fight.
    pub(crate) fn new_temporary_stat_buff_with_stacks(
        &mut self,
        unit: UnitId,
        config: &TemporaryStatBuffWithStacksConfig,
    ) -> (StatBuffAura, Option<AuraId>) {
        let stacking = self.make_stacking_aura(
            unit,
            StackingStatAura {
                aura: AuraConfig {
                    label: if config.stacking_aura_label.is_empty() {
                        config.aura_label.clone()
                    } else {
                        config.stacking_aura_label.clone()
                    },
                    action_id: Some(if is_empty_action(&config.stacking_aura_action_id) {
                        config.action_id.clone()
                    } else {
                        config.stacking_aura_action_id.clone()
                    }),
                    duration: if config.stacks_from_event {
                        NEVER_EXPIRES
                    } else {
                        config.duration
                    },
                    max_stacks: config.max_stacks,
                    ..AuraConfig::default()
                },
                bonus_per_stack: config.bonus_per_stack,
            },
        );
        if config.time_per_stack > 0 || config.stacks_from_event {
            let stacking_aura = stacking.aura;
            let stacks_from_event = config.stacks_from_event;
            let aura = self.register_aura(
                unit,
                AuraConfig {
                    label: config.aura_label.clone(),
                    action_id: Some(config.action_id.clone()),
                    duration: config.duration,
                    on_gain: Some(Rc::new(move |sim: &mut Sim, _| {
                        sim.activate(stacking_aura);
                    })),
                    on_expire: Some(Rc::new(move |sim: &mut Sim, _| {
                        // The window owns the stacking aura's lifetime in the event case,
                        // because there the child has no duration of its own to expire on.
                        if stacks_from_event {
                            sim.deactivate(stacking_aura);
                        }
                    })),
                    ..AuraConfig::default()
                },
            );
            return (stacking, Some(aura));
        }
        (stacking, None)
    }

    /// Go `Character.NewTemporaryStatsAura`.
    pub(crate) fn new_temporary_stats_aura(
        &mut self,
        unit: UnitId,
        label: &str,
        action_id: &ActionId,
        stats: Stats,
        duration: Duration,
    ) -> StatBuffAura {
        self.new_temporary_stats_aura_wrapped(unit, label, action_id, stats, duration, None)
    }

    /// Go `Character.NewTemporaryStatsAuraWrapped`: `mod_config` may edit the aura's config.
    pub(crate) fn new_temporary_stats_aura_wrapped(
        &mut self,
        unit: UnitId,
        label: &str,
        action_id: &ActionId,
        mut buffs: Stats,
        duration: Duration,
        mod_config: Option<&dyn Fn(&mut AuraConfig)>,
    ) -> StatBuffAura {
        // A health bonus also heals, through the max health update; the health metrics Go
        // creates for it are not part of the prepared state.
        let amount_healed = buffs[Stat::Health];
        let includes_health_buff = amount_healed > 0.0;
        let buffed = buffed_stat_types(&buffs);
        if includes_health_buff {
            buffs[Stat::Health] = 0.0;
        }
        let mut config = AuraConfig {
            label: label.to_string(),
            action_id: Some(action_id.clone()),
            duration,
            on_gain: Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
                sim.add_stats_dynamic(unit, &buffs);
                if includes_health_buff {
                    sim.update_max_health(unit, amount_healed);
                }
                for listener in sim.unit(unit).on_temporary_stats_changes.clone() {
                    listener(sim, aura, &buffs);
                }
            })),
            on_expire: Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
                let inverted = buffs.invert();
                sim.add_stats_dynamic(unit, &inverted);
                if includes_health_buff {
                    sim.update_max_health(unit, -amount_healed);
                }
                for listener in sim.unit(unit).on_temporary_stats_changes.clone() {
                    listener(sim, aura, &inverted);
                }
            })),
            ..AuraConfig::default()
        };
        if let Some(mod_config) = mod_config {
            mod_config(&mut config);
        }
        StatBuffAura::new(self.get_or_register_aura(unit, config), buffed)
    }

    /// Go `Character.NewTemporaryStatMultiplierAura`: a buff that multiplies stats through
    /// dynamic stat dependencies, turned on and off with the aura. The temporary stats listeners
    /// hear the stats the dependencies add on gain and remove on expire, measured at that moment.
    pub(crate) fn new_temporary_stat_multiplier_aura(
        &mut self,
        unit: UnitId,
        mut config: AuraConfig,
        multipliers: &[StatMultiplier],
    ) -> StatBuffAura {
        let deps: Vec<DepId> = multipliers
            .iter()
            .map(|m| self.new_dynamic_multiply_stat(unit, m.stat, m.multiplier))
            .collect();
        let buffed: Vec<Stat> = multipliers.iter().map(|m| m.stat).collect();

        fn toggle(sim: &mut Sim, unit: UnitId, aura: AuraId, deps: &[DepId], enable: bool) {
            let set = |sim: &mut Sim, dep: DepId| {
                if enable {
                    sim.enable_build_phase_stat_dep(unit, dep);
                } else {
                    sim.disable_build_phase_stat_dep(unit, dep);
                }
            };
            if sim.unit(unit).on_temporary_stats_changes.is_empty() {
                for dep in deps {
                    set(sim, *dep);
                }
                return;
            }
            let before = sim.stats(unit);
            for dep in deps {
                set(sim, *dep);
            }
            let change = sim.stats(unit).subtract(&before);
            for listener in sim.unit(unit).on_temporary_stats_changes.clone() {
                listener(sim, aura, &change);
            }
        }

        let gain_deps = deps.clone();
        config.on_gain = Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
            toggle(sim, unit, aura, &gain_deps, true);
        }));
        config.on_expire = Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
            toggle(sim, unit, aura, &deps, false);
        }));
        StatBuffAura::new(self.get_or_register_aura(unit, config), buffed)
    }

    /// Go `parentAura.AttachStatDependency`.
    pub(crate) fn attach_stat_dependency(&mut self, aura: AuraId, dep: DepId) -> AuraId {
        let unit = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| sim.enable_build_phase_stat_dep(unit, dep)),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| sim.disable_build_phase_stat_dep(unit, dep)),
        );
        if self.aura(aura).active {
            self.unit_mut(unit).sdm.enable_dynamic_stat_dep(dep);
        }
        aura
    }

    /// Go `parentAura.AttachStatsBuff`.
    pub(crate) fn attach_stats_buff(&mut self, aura: AuraId, stats: Stats) -> AuraId {
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, aura: AuraId| {
                let unit = sim.aura(aura).unit;
                sim.add_stats_dynamic(unit, &stats);
            }),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, aura: AuraId| {
                let unit = sim.aura(aura).unit;
                sim.add_stats_dynamic(unit, &stats.invert());
            }),
        );
        if self.aura(aura).active {
            let unit = self.aura(aura).unit;
            self.add_stats(unit, &stats);
        }
        aura
    }

    /// Go `parentAura.AttachStatBuff`.
    pub(crate) fn attach_stat_buff(&mut self, aura: AuraId, stat: Stat, value: f64) -> AuraId {
        let mut stats = Stats::default();
        stats[stat] = value;
        self.attach_stats_buff(aura, stats)
    }

    /// Go `parentAura.AttachMultiplicativePseudoStatBuff`.
    pub(crate) fn attach_multiplicative_pseudo_stat_buff(
        &mut self,
        aura: AuraId,
        field: PseudoStatField,
        multiplier: f64,
    ) -> AuraId {
        let unit = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                *field.get_mut(&mut sim.unit_mut(unit).pseudo_stats) *= multiplier;
            }),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                *field.get_mut(&mut sim.unit_mut(unit).pseudo_stats) /= multiplier;
            }),
        );
        if self.aura(aura).active {
            *field.get_mut(&mut self.unit_mut(unit).pseudo_stats) *= multiplier;
        }
        aura
    }

    /// Go `parentAura.AttachAdditivePseudoStatBuff`.
    pub(crate) fn attach_additive_pseudo_stat_buff(
        &mut self,
        aura: AuraId,
        field: PseudoStatField,
        bonus: f64,
    ) -> AuraId {
        let unit = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                *field.get_mut(&mut sim.unit_mut(unit).pseudo_stats) += bonus;
            }),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                *field.get_mut(&mut sim.unit_mut(unit).pseudo_stats) -= bonus;
            }),
        );
        if self.aura(aura).active {
            *field.get_mut(&mut self.unit_mut(unit).pseudo_stats) += bonus;
        }
        aura
    }

    /// Go `parentAura.AttachReducedCritTakenPercentBuff`: updates the base and recalculates.
    pub(crate) fn attach_reduced_crit_taken_percent_buff(
        &mut self,
        aura: AuraId,
        bonus: f64,
    ) -> AuraId {
        let unit = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| sim.add_reduced_crit_taken_percent(unit, bonus)),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| sim.add_reduced_crit_taken_percent(unit, -bonus)),
        );
        if self.aura(aura).active {
            self.add_reduced_crit_taken_percent(unit, bonus);
        }
        aura
    }

    /// Go `parentAura.AttachMultiplyCastSpeed`.
    pub(crate) fn attach_multiply_cast_speed(&mut self, aura: AuraId, multiplier: f64) -> AuraId {
        self.attach_speed_multiplier(aura, multiplier, Sim::multiply_cast_speed)
    }

    /// Go `parentAura.AttachMultiplyMeleeSpeed`.
    pub(crate) fn attach_multiply_melee_speed(&mut self, aura: AuraId, multiplier: f64) -> AuraId {
        self.attach_speed_multiplier(aura, multiplier, Sim::multiply_melee_speed)
    }

    /// Go `parentAura.AttachMultiplyAttackSpeed`.
    pub(crate) fn attach_multiply_attack_speed(&mut self, aura: AuraId, multiplier: f64) -> AuraId {
        self.attach_speed_multiplier(aura, multiplier, Sim::multiply_attack_speed)
    }

    /// Go `parentAura.AttachMultiplyRangedSpeed`.
    pub(crate) fn attach_multiply_ranged_speed(&mut self, aura: AuraId, multiplier: f64) -> AuraId {
        self.attach_speed_multiplier(aura, multiplier, Sim::multiply_ranged_speed)
    }

    /// Go `parentAura.AttachMultiplyRangedHaste`.
    pub(crate) fn attach_multiply_ranged_haste(&mut self, aura: AuraId, multiplier: f64) -> AuraId {
        self.attach_speed_multiplier(aura, multiplier, Sim::multiply_ranged_haste)
    }

    /// The shared shape of the speed helpers: multiply on gain, by the reciprocal on expire.
    fn attach_speed_multiplier(
        &mut self,
        aura: AuraId,
        multiplier: f64,
        multiply: fn(&mut Sim, UnitId, f64),
    ) -> AuraId {
        let unit = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| multiply(sim, unit, multiplier)),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| multiply(sim, unit, 1.0 / multiplier)),
        );
        aura
    }

    /// Go `parentAura.AttachDDBC`: a damage done by caster handler in slot `index` of the
    /// table `attacker` has against the aura's unit. Go reads the attacker's tables, which only
    /// exist once finalized; the handler itself only runs in a fight, so the slot is recorded in
    /// `damage_done_by_caster`.
    pub(crate) fn attach_ddbc(
        &mut self,
        aura: AuraId,
        index: usize,
        max_index: usize,
        attacker: UnitId,
    ) -> AuraId {
        let defender = self.aura(aura).unit;
        self.apply_on_gain(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                sim.enable_damage_done_by_caster(index, max_index, attacker, defender)
            }),
        );
        self.apply_on_expire(
            aura,
            Rc::new(move |sim: &mut Sim, _| {
                sim.disable_damage_done_by_caster(index, attacker, defender)
            }),
        );
        if self.aura(aura).active {
            self.enable_damage_done_by_caster(index, max_index, attacker, defender);
        }
        aura
    }

    /// Go `EnableDamageDoneByCaster`.
    fn enable_damage_done_by_caster(
        &mut self,
        index: usize,
        max_index: usize,
        attacker: UnitId,
        defender: UnitId,
    ) {
        assert!(
            self.is_finalized(),
            "Attack tables exist only once the environment is finalized"
        );
        let slots = self
            .damage_done_by_caster
            .entry((attacker, defender))
            .or_insert_with(|| vec![false; max_index]);
        slots[index] = true;
    }

    /// Go `DisableDamageDoneByCaster`.
    fn disable_damage_done_by_caster(&mut self, index: usize, attacker: UnitId, defender: UnitId) {
        assert!(
            self.is_finalized(),
            "Attack tables exist only once the environment is finalized"
        );
        let slots = self
            .damage_done_by_caster
            .get_mut(&(attacker, defender))
            .expect("a damage done by caster handler enabled before it is disabled");
        slots[index] = false;
    }

    /// How many damage done by caster handlers the attacker's table against the defender has:
    /// Go's count of non-nil `DamageDoneByCasterExtraMultiplier` entries.
    pub(crate) fn damage_done_by_caster_handlers(
        &self,
        attacker: UnitId,
        defender: UnitId,
    ) -> usize {
        self.damage_done_by_caster
            .get(&(attacker, defender))
            .map_or(0, |slots| slots.iter().filter(|set| **set).count())
    }

    /// Go `parentAura.AttachPeriodicAction`. The action itself only runs in a fight.
    pub(crate) fn attach_periodic_action(
        &mut self,
        aura: AuraId,
        _options: PeriodicActionOptions,
    ) -> AuraId {
        self.apply_on_gain(aura, Rc::new(|_: &mut Sim, _| {}));
        self.apply_on_expire(aura, Rc::new(|_: &mut Sim, _| {}));
        assert!(
            !self.aura(aura).active,
            "Can't attach a periodic action to an active aura."
        );
        aura
    }

    /// Go `ApplyFixedUptimeAura`: a reset effect that starts the periodic actions rolling the
    /// aura's uptime. They only run in a fight, so the effect does nothing in preparation; Go
    /// still registers it.
    pub(crate) fn apply_fixed_uptime_aura(
        &mut self,
        aura: AuraId,
        _uptime: f64,
        _tick_length: Duration,
        _start_time: Duration,
    ) {
        let unit = self.aura(aura).unit;
        self.register_reset_effect(unit, Rc::new(|_: &mut Sim| {}));
    }
}

/// Go's `*float64` into a unit's `PseudoStats`: which field an aura helper scales or adds to.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub(crate) enum PseudoStatField {
    CastSpeedMultiplier,
    MeleeSpeedMultiplier,
    RangedSpeedMultiplier,
    RangedHasteMultiplier,
    AttackSpeedMultiplier,
    SpiritRegenMultiplier,
    ThreatMultiplier,
    DamageDealtMultiplier,
    SchoolDamageDealtMultiplier(SchoolIndex),
    DotDamageMultiplierAdditive,
    HealingDealtMultiplier,
    PeriodicHealingDealtMultiplier,
    CritDamageMultiplier,
    BlockValueMultiplier,
    DamageTakenMultiplier,
    SchoolDamageTakenMultiplier(SchoolIndex),
    SchoolBonusSpellDamage(SchoolIndex),
    SchoolBonusHitChance(SchoolIndex),
    DiseaseDamageTakenMultiplier,
    PeriodicPhysicalDamageTakenMultiplier,
    ArmorMultiplier,
    HealingTakenMultiplier,
    ExternalHealingTakenMultiplier,
    MovementSpeedMultiplier,
    SelfHealingMultiplier,
    PushbackChance,
    BaseDodgeChance,
    BaseParryChance,
    BaseBlockChance,
    BaseReducedCritTakenPercent,
    BonusHealingTaken,
    BonusSpellCritPercentTaken,
    BonusPhysicalDamageTaken,
    BonusSpellDamageTaken,
    BonusMhDps,
    BonusOhDps,
    BonusRangedDps,
    BonusAttackPower,
    BonusRangedAttackPower,
    IncreasedMissChance,
    DodgeReduction,
    FearDurationMultiplier,
    StunDurationMultiplier,
    ReducedPhysicalHitTakenChance,
    ReducedArcaneHitTakenChance,
    ReducedFireHitTakenChance,
    ReducedFrostHitTakenChance,
    ReducedNatureHitTakenChance,
    ReducedShadowHitTakenChance,
    /// Any other field, named by a function: `|p| &mut p.some_field`.
    Custom(for<'a> fn(&'a mut PseudoStats) -> &'a mut f64),
}

impl PseudoStatField {
    /// The field of `stats` this names.
    pub(crate) fn get_mut(self, stats: &mut PseudoStats) -> &mut f64 {
        use PseudoStatField::*;
        match self {
            CastSpeedMultiplier => &mut stats.cast_speed_multiplier,
            MeleeSpeedMultiplier => &mut stats.melee_speed_multiplier,
            RangedSpeedMultiplier => &mut stats.ranged_speed_multiplier,
            RangedHasteMultiplier => &mut stats.ranged_haste_multiplier,
            AttackSpeedMultiplier => &mut stats.attack_speed_multiplier,
            SpiritRegenMultiplier => &mut stats.spirit_regen_multiplier,
            ThreatMultiplier => &mut stats.threat_multiplier,
            DamageDealtMultiplier => &mut stats.damage_dealt_multiplier,
            SchoolDamageDealtMultiplier(school) => {
                &mut stats.school_damage_dealt_multiplier[school_array_index(school)]
            }
            DotDamageMultiplierAdditive => &mut stats.dot_damage_multiplier_additive,
            HealingDealtMultiplier => &mut stats.healing_dealt_multiplier,
            PeriodicHealingDealtMultiplier => &mut stats.periodic_healing_dealt_multiplier,
            CritDamageMultiplier => &mut stats.crit_damage_multiplier,
            BlockValueMultiplier => &mut stats.block_value_multiplier,
            DamageTakenMultiplier => &mut stats.damage_taken_multiplier,
            SchoolDamageTakenMultiplier(school) => {
                &mut stats.school_damage_taken_multiplier[school_array_index(school)]
            }
            SchoolBonusSpellDamage(school) => {
                &mut stats.school_bonus_spell_damage[school_array_index(school)]
            }
            SchoolBonusHitChance(school) => {
                &mut stats.school_bonus_hit_chance[school_array_index(school)]
            }
            DiseaseDamageTakenMultiplier => &mut stats.disease_damage_taken_multiplier,
            PeriodicPhysicalDamageTakenMultiplier => {
                &mut stats.periodic_physical_damage_taken_multiplier
            }
            ArmorMultiplier => &mut stats.armor_multiplier,
            HealingTakenMultiplier => &mut stats.healing_taken_multiplier,
            ExternalHealingTakenMultiplier => &mut stats.external_healing_taken_multiplier,
            MovementSpeedMultiplier => &mut stats.movement_speed_multiplier,
            SelfHealingMultiplier => &mut stats.self_healing_multiplier,
            PushbackChance => &mut stats.pushback_chance,
            BaseDodgeChance => &mut stats.base_dodge_chance,
            BaseParryChance => &mut stats.base_parry_chance,
            BaseBlockChance => &mut stats.base_block_chance,
            BaseReducedCritTakenPercent => &mut stats.base_reduced_crit_taken_percent,
            BonusHealingTaken => &mut stats.bonus_healing_taken,
            BonusSpellCritPercentTaken => &mut stats.bonus_spell_crit_percent_taken,
            BonusPhysicalDamageTaken => &mut stats.bonus_physical_damage_taken,
            BonusSpellDamageTaken => &mut stats.bonus_spell_damage_taken,
            BonusMhDps => &mut stats.bonus_mh_dps,
            BonusOhDps => &mut stats.bonus_oh_dps,
            BonusRangedDps => &mut stats.bonus_ranged_dps,
            BonusAttackPower => &mut stats.bonus_attack_power,
            BonusRangedAttackPower => &mut stats.bonus_ranged_attack_power,
            IncreasedMissChance => &mut stats.increased_miss_chance,
            DodgeReduction => &mut stats.dodge_reduction,
            FearDurationMultiplier => &mut stats.fear_duration_multiplier,
            StunDurationMultiplier => &mut stats.stun_duration_multiplier,
            ReducedPhysicalHitTakenChance => &mut stats.reduced_physical_hit_taken_chance,
            ReducedArcaneHitTakenChance => &mut stats.reduced_arcane_hit_taken_chance,
            ReducedFireHitTakenChance => &mut stats.reduced_fire_hit_taken_chance,
            ReducedFrostHitTakenChance => &mut stats.reduced_frost_hit_taken_chance,
            ReducedNatureHitTakenChance => &mut stats.reduced_nature_hit_taken_chance,
            ReducedShadowHitTakenChance => &mut stats.reduced_shadow_hit_taken_chance,
            Custom(field) => field(stats),
        }
    }
}

/// Go `ShieldStrengthCalculator`.
pub(crate) type ShieldStrengthCalculator = Rc<dyn Fn(&Sim, UnitId) -> f64>;

/// Go `AbsorptionAuraConfig`, without the callbacks that only run in a fight.
#[derive(Clone, Default)]
pub(crate) struct AbsorptionAuraConfig {
    pub aura: AuraConfig,
    pub damage_multiplier: f64,
    pub max_absorb_per_hit: f64,
    pub shield_strength_calculator: Option<ShieldStrengthCalculator>,
}

/// Go `DamageAbsorptionAura`.
#[derive(Clone)]
pub(crate) struct DamageAbsorptionAura {
    pub aura: AuraId,
    fresh_shield_strength_calculator: Option<ShieldStrengthCalculator>,
    shield_strength: Rc<Cell<f64>>,
}

impl DamageAbsorptionAura {
    /// Go `DamageAbsorptionAura.ShieldStrength`.
    pub(crate) fn shield_strength(&self) -> f64 {
        self.shield_strength.get()
    }

    /// Go `DamageAbsorptionAura.Activate`: activates the aura, then sizes the shield and the
    /// stacks that show it.
    pub(crate) fn activate(&self, sim: &mut Sim) {
        sim.activate(self.aura);
        let calculator = self
            .fresh_shield_strength_calculator
            .as_ref()
            .expect("a shield strength calculator");
        let unit = sim.aura(self.aura).unit;
        self.shield_strength.set(calculator(sim, unit));
        let stacks = std::cmp::max(1, self.shield_strength.get() as i32);
        sim.aura_mut(self.aura).max_stacks = stacks;
        sim.set_stacks(self.aura, stacks);
    }
}

impl Sim {
    /// Go `unit.NewDamageAbsorptionAura`: the aura and the dynamic damage taken modifier that
    /// takes damage off the shield, which only runs in a fight.
    pub(crate) fn new_damage_absorption_aura(
        &mut self,
        unit: UnitId,
        mut config: AbsorptionAuraConfig,
    ) -> DamageAbsorptionAura {
        if config.aura.duration == 0 {
            config.aura.duration = NEVER_EXPIRES;
        }
        let aura = self.register_aura(unit, config.aura);
        let absorption = DamageAbsorptionAura {
            aura,
            fresh_shield_strength_calculator: config.shield_strength_calculator,
            shield_strength: Rc::new(Cell::new(0.0)),
        };
        let shield_strength = Rc::clone(&absorption.shield_strength);
        self.apply_on_expire(
            aura,
            Rc::new(move |_: &mut Sim, _| shield_strength.set(0.0)),
        );
        self.add_dynamic_damage_taken_modifier(unit);
        self.unit_mut(unit).absorption_auras.push(aura);
        absorption
    }

    /// Go `unit.AddDynamicDamageTakenModifier`: the modifier runs in a fight.
    pub(crate) fn add_dynamic_damage_taken_modifier(&mut self, unit: UnitId) {
        assert!(
            !self.is_finalized(),
            "Already finalized, cannot add dynamic damage taken modifier!"
        );
        self.unit_mut(unit).dynamic_damage_taken_modifiers += 1;
    }
}

/// Go `DamageAbsorptionAuraArray`: the shield each unit gets, by unit index.
#[derive(Clone, Default)]
pub(crate) struct DamageAbsorptionAuraArray(pub Vec<Option<DamageAbsorptionAura>>);

impl DamageAbsorptionAuraArray {
    /// Go `Get`.
    pub(crate) fn get(&self, sim: &Sim, target: UnitId) -> Option<&DamageAbsorptionAura> {
        self.0
            .get(sim.unit(target).unit_index as usize)
            .and_then(Option::as_ref)
    }

    /// Go `IsEmpty`.
    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    /// Go `FindLabel`.
    pub(crate) fn find_label(&self, sim: &Sim) -> String {
        match self.0.iter().flatten().next() {
            Some(aura) => sim.aura(aura.aura).label.clone(),
            None => panic!("No valid damage absorption auras in array!"),
        }
    }
}

impl Sim {
    /// Go `caster.NewAllyDamageAbsorptionAuraArray`: an aura on every unit that is not an enemy.
    pub(crate) fn new_ally_damage_absorption_aura_array(
        &mut self,
        mut make_aura: impl FnMut(&mut Sim, UnitId) -> DamageAbsorptionAura,
    ) -> DamageAbsorptionAuraArray {
        let units = self.all_units();
        let mut auras = vec![None; units.len()];
        for target in units {
            if self.unit(target).unit_type != UnitType::Enemy {
                let index = self.unit(target).unit_index as usize;
                auras[index] = Some(make_aura(self, target));
            }
        }
        DamageAbsorptionAuraArray(auras)
    }
}

// Unit methods the helpers above call.
impl Sim {
    /// Go `unit.NewDynamicMultiplyStat`.
    pub(crate) fn new_dynamic_multiply_stat(
        &mut self,
        unit: UnitId,
        stat: Stat,
        amount: f64,
    ) -> DepId {
        self.unit_mut(unit)
            .sdm
            .new_dynamic_multiply_stat(stat, amount)
    }

    /// Go `unit.NewDynamicStatDependency`.
    pub(crate) fn new_dynamic_stat_dependency(
        &mut self,
        unit: UnitId,
        src: Stat,
        dst: Stat,
        amount: f64,
    ) -> DepId {
        self.unit_mut(unit)
            .sdm
            .new_dynamic_stat_dependency(src, dst, amount)
    }

    /// Go `unit.AddReducedCritTakenPercent`: adds to the base and recalculates the total.
    pub(crate) fn add_reduced_crit_taken_percent(&mut self, unit: UnitId, amount: f64) {
        self.unit_mut(unit)
            .pseudo_stats
            .base_reduced_crit_taken_percent += amount;
        self.update_reduced_crit_taken_percent(unit);
    }

    /// Go `unit.MultiplyRangedHaste`: ranged haste that also speeds resource regeneration.
    pub(crate) fn multiply_ranged_haste(&mut self, unit: UnitId, amount: f64) {
        self.unit_mut(unit).pseudo_stats.ranged_haste_multiplier *= amount;
        self.multiply_ranged_speed(unit, amount);
        self.multiply_resource_regen_speed(unit, amount);
    }

    /// Go `unit.MultiplyResourceRegenSpeed`. Only the energy bar regenerates faster here; Go's
    /// pet inheritance is not prepared yet, since a unit with pets is refused.
    pub(crate) fn multiply_resource_regen_speed(&mut self, unit: UnitId, amount: f64) {
        if self.unit(unit).energy_bar.enabled {
            self.multiply_energy_regen_speed(unit, amount);
        }
    }

    /// Go `energyBar.MultiplyEnergyRegenSpeed`. Go first gives a partial tick if the bar is
    /// ticking; the bar is not ticking while a reset runs the auras, which is the only time
    /// preparation gains one.
    pub(crate) fn multiply_energy_regen_speed(&mut self, unit: UnitId, multiplier: f64) {
        if multiplier == 1.0 {
            return;
        }
        self.unit_mut(unit).energy_bar.energy_regen_multiplier *= multiplier;
    }

    /// Go `unit.MultiplyManaRegenSpeed`.
    pub(crate) fn multiply_mana_regen_speed(&mut self, unit: UnitId, multiplier: f64) {
        self.unit_mut(unit).mana_bar.mana_regen_multiplier *= multiplier;
    }

    /// Go `unit.MultiplyMovementSpeed`. No movement is in progress while preparing.
    pub(crate) fn multiply_movement_speed(&mut self, unit: UnitId, amount: f64) {
        self.unit_mut(unit).pseudo_stats.movement_speed_multiplier *= amount;
    }

    /// Go `unit.HasManaBar`.
    pub(crate) fn has_mana_bar(&self, unit: UnitId) -> bool {
        self.unit(unit).mana_bar.enabled
    }

    /// Go `healthBar.UpdateMaxHealth`: the max health changes through the dynamic stats, and the
    /// current health gains the bonus or loses what the new maximum cuts, leaving at least 1.
    pub(crate) fn update_max_health(&mut self, unit: UnitId, bonus_health: f64) {
        let mut bonus = Stats::default();
        bonus[Stat::Health] = bonus_health;
        self.add_stats_dynamic(unit, &bonus);
        let max = self.unit(unit).stats[Stat::Health];
        let current = self.unit(unit).current_health;
        self.unit_mut(unit).current_health = if bonus_health >= 0.0 {
            f64::min(current + bonus_health, max)
        } else {
            current - f64::max(0.0, f64::min(-bonus_health, current - 1.0))
        };
    }
}

#[cfg(test)]
mod tests;
