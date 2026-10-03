//! A class-independent fight runtime that mirrors the pinned Go `sim/core`.
//!
//! One player fights one target. The runtime owns time, the pending-action queue, random
//! streams, units, auras, spells, casting, damage, channels, cooldowns, the rotation and
//! metrics. Class behavior plugs in through [`Agent`], so the runtime never names a
//! class. Event order, random draw order and floating-point operation order follow Go,
//! because the shared random stream makes every later draw depend on them.
//!
//! The runtime is built from a validated prepared v2 input. Static modifiers are already
//! applied there and are never applied again here.

mod aura;
mod cast;
mod damage;
mod dot;
pub(crate) mod energy;
mod log;
pub(crate) mod melee;
pub(crate) mod metrics;
mod racial;
mod rotation;
mod spell_mod;

use std::collections::BTreeMap;

pub(crate) use aura::{AuraBehavior, AuraRef, Tracker};
pub(crate) use damage::{Outcome, SpellResult, OUTCOME_CRIT, OUTCOME_LANDED};
pub(crate) use dot::Dot;
pub(crate) use metrics::{ActionReport, ActionTotals, FightReport};
pub(crate) use spell_mod::{ModId, ModKind};

use crate::{
    contracts::prepared_v2::{ActionId, Effect, PreparedV2, Schools},
    core::{
        queue::{Handle, PendingQueue},
        rng::SimRng,
        time::{NEVER_EXPIRES, STARTING_CD_TIME},
    },
    mechanics::mana::RegenInputs,
};

pub(crate) type SpellId = usize;
pub(crate) type DotId = usize;
pub(crate) type TimerId = usize;

/// The two units of a supported fight, by Go unit index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Side {
    Target = 0,
    Player = 1,
}

impl Side {
    pub(crate) fn index(self) -> usize {
        self as usize
    }
}

/// Class behavior hooks. Each hook receives the whole fight; the class reads and writes
/// its own state through `fight.agent`.
pub(crate) trait Agent: Sized {
    /// What a class spell does when its effects apply.
    type Spell: Copy + std::fmt::Debug;
    /// Which class aura a callback belongs to.
    type Aura: Copy + std::fmt::Debug;

    /// Go `Spell.ApplyEffects`.
    fn apply_effects(fight: &mut Fight<Self>, spell: SpellId, target: Side, behavior: Self::Spell);
    /// Go `ExtraCastCondition`.
    fn extra_cast_condition(_fight: &Fight<Self>, _spell: SpellId, _behavior: Self::Spell) -> bool {
        true
    }
    /// Go `MajorCooldown.ShouldActivate` for class cooldowns.
    fn should_activate(_fight: &Fight<Self>, _spell: SpellId, _behavior: Self::Spell) -> bool {
        true
    }
    /// The `WaitTravelTime` callback of a class spell scheduled with
    /// [`Fight::class_after_travel`]: by default the result is dealt on arrival.
    fn on_travel(
        fight: &mut Fight<Self>,
        spell: SpellId,
        result: SpellResult,
        _behavior: Self::Spell,
    ) {
        fight.deal_damage(spell, result, false);
    }
    /// A dot or channel tick of a class spell.
    fn on_dot_tick(_fight: &mut Fight<Self>, _dot: DotId, _behavior: Self::Spell) {}
    /// The class part of a dot aura's OnGain, which Go runs before the dot's own.
    fn on_dot_gain(_fight: &mut Fight<Self>, _dot: DotId, _behavior: Self::Spell) {}
    /// The class part of a dot aura's OnExpire, which Go runs before the dot's own.
    fn on_dot_expire(_fight: &mut Fight<Self>, _dot: DotId, _behavior: Self::Spell) {}
    /// Go reset effects registered by the class, run before auras reset.
    fn reset(_fight: &mut Fight<Self>) {}
    fn on_gain(_fight: &mut Fight<Self>, _aura: AuraRef, _kind: Self::Aura) {}
    fn on_expire(_fight: &mut Fight<Self>, _aura: AuraRef, _kind: Self::Aura) {}
    fn on_stacks_change(
        _fight: &mut Fight<Self>,
        _aura: AuraRef,
        _kind: Self::Aura,
        _old: i32,
        _new: i32,
    ) {
    }
    fn on_cast_complete(
        _fight: &mut Fight<Self>,
        _aura: AuraRef,
        _kind: Self::Aura,
        _spell: SpellId,
    ) {
    }
    fn on_spell_hit_dealt(
        _fight: &mut Fight<Self>,
        _aura: AuraRef,
        _kind: Self::Aura,
        _spell: SpellId,
        _result: &SpellResult,
    ) {
    }
    /// A proc handler that Go delays by the spell batch window.
    fn on_delayed_proc(
        _fight: &mut Fight<Self>,
        _aura: AuraRef,
        _kind: Self::Aura,
        _spell: SpellId,
        _result: SpellResult,
    ) {
    }
}

/// Go `ProcMaskDirect`, by the bit names the exporter writes.
pub(crate) const DIRECT_PROC_MASKS: &[&str] = &[
    "ProcMaskMeleeMHAuto",
    "ProcMaskMeleeOHAuto",
    "ProcMaskMeleeMHSpecial",
    "ProcMaskMeleeOHSpecial",
    "ProcMaskRangedAuto",
    "ProcMaskRangedSpecial",
    "ProcMaskSpellDamage",
];

/// What a non-class spell does. Class spells use [`SpellBehavior::Class`].
#[derive(Clone, Debug)]
pub(crate) enum SpellBehavior<S> {
    /// Registered but not castable by this runtime; reaching it is a coverage failure.
    None,
    Class(S),
    /// Go consumes.go potion: one mana gain with a random spread, always rolled.
    PotionMana {
        label: String,
        min: f64,
        spread: f64,
        stone_multiplier: f64,
        regen_window: f64,
    },
    /// Go consumes.go conjured item: rolled only when the spread exceeds one.
    ConjuredMana {
        label: String,
        min: f64,
        spread: f64,
        selected: bool,
        regen_window: f64,
    },
    /// Go spell_data_energize.go: an item use that rolls a client energize effect.
    EnergizeOnUse {
        average: f64,
        variance: f64,
        whole: f64,
    },
    /// Go racials.go Touch of the Grave's drain: shadow damage from the caster's maximum
    /// health, healing the caster for the damage dealt.
    TouchOfTheGraveDrain {
        health_fraction: f64,
        metrics: usize,
    },
    /// Go racials.go Eureka!'s cast, which activates its aura.
    Eureka,
    /// A racial whose `ApplyEffects` only activates its player aura.
    ActivateAura(usize),
    /// Go attack.go's main or off hand auto attack.
    MeleeAuto(melee::Hand),
}

/// Go spell flags used by the runtime, parsed from exported names.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Flags {
    pub(crate) binary: bool,
    pub(crate) channeled: bool,
    pub(crate) helpful: bool,
    pub(crate) no_on_cast_complete: bool,
    pub(crate) no_metrics: bool,
    pub(crate) no_logs: bool,
    pub(crate) apl: bool,
    pub(crate) mcd: bool,
    pub(crate) reactive: bool,
    pub(crate) no_on_damage_dealt: bool,
    pub(crate) ignore_resists: bool,
    pub(crate) ignore_target_modifiers: bool,
    pub(crate) ignore_attacker_modifiers: bool,
    pub(crate) passive: bool,
    pub(crate) swapped: bool,
    pub(crate) can_cast_while_moving: bool,
    pub(crate) proc: bool,
    pub(crate) melee_metrics: bool,
    pub(crate) no_spell_mods: bool,
    pub(crate) cannot_be_dodged: bool,
}

impl Flags {
    fn parse(names: &[String]) -> Self {
        let mut flags = Flags::default();
        for name in names {
            match name.as_str() {
                "SpellFlagBinary" => flags.binary = true,
                "SpellFlagChanneled" => flags.channeled = true,
                "SpellFlagHelpful" => flags.helpful = true,
                "SpellFlagNoOnCastComplete" => flags.no_on_cast_complete = true,
                "SpellFlagNoMetrics" => flags.no_metrics = true,
                "SpellFlagNoLogs" => flags.no_logs = true,
                "SpellFlagAPL" => flags.apl = true,
                "SpellFlagMCD" => flags.mcd = true,
                "SpellFlagReactive" => flags.reactive = true,
                "SpellFlagNoOnDamageDealt" => flags.no_on_damage_dealt = true,
                "SpellFlagIgnoreResists" => flags.ignore_resists = true,
                "SpellFlagIgnoreTargetModifiers" => flags.ignore_target_modifiers = true,
                "SpellFlagIgnoreAttackerModifiers" => flags.ignore_attacker_modifiers = true,
                "SpellFlagPassiveSpell" => flags.passive = true,
                "SpellFlagSwapped" => flags.swapped = true,
                "SpellFlagCanCastWhileMoving" => flags.can_cast_while_moving = true,
                "SpellFlagProc" => flags.proc = true,
                "SpellFlagMeleeMetrics" => flags.melee_metrics = true,
                "SpellFlagNoSpellMods" => flags.no_spell_mods = true,
                "SpellFlagCannotBeDodged" => flags.cannot_be_dodged = true,
                _ => {}
            }
        }
        flags
    }
}

/// Go `Cast`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Cast {
    pub(crate) cost: f64,
    pub(crate) gcd: i64,
    pub(crate) gcd_min: i64,
    pub(crate) cast_time: i64,
    pub(crate) non_empty: bool,
}

impl Cast {
    /// Go `Cast.GCDTime`.
    pub(crate) fn gcd_time(&self) -> i64 {
        let mut gcd = self.gcd.max(0);
        if self.gcd > 0 {
            gcd = if self.gcd_min != 0 {
                self.gcd_min.max(gcd)
            } else {
                crate::core::time::NS_PER_SECOND.max(gcd)
            };
        }
        gcd
    }

    /// Go `Cast.EffectiveTime`.
    pub(crate) fn effective_time(&self) -> i64 {
        self.gcd_time().max(self.cast_time)
    }
}

/// Go `SpellCost`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cost {
    /// The resource the cost spends.
    pub(crate) kind: ResourceKind,
    /// Go `EnergyCost.Refund`.
    pub(crate) refund: f64,
    pub(crate) base: i32,
    pub(crate) flat_modifier: i32,
    pub(crate) percent_modifier: f64,
    pub(crate) additive_percent_modifier: f64,
}

/// Go `SpellMetrics` for one target during one iteration.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SpellMetrics {
    pub(crate) casts: i32,
    pub(crate) misses: i32,
    pub(crate) dodges: i32,
    pub(crate) parries: i32,
    pub(crate) glances: i32,
    pub(crate) blocks: i32,
    pub(crate) blocked_crits: i32,
    pub(crate) hits: i32,
    pub(crate) resisted_hits: i32,
    pub(crate) crits: i32,
    pub(crate) resisted_crits: i32,
    pub(crate) ticks: i32,
    pub(crate) resisted_ticks: i32,
    pub(crate) crit_ticks: i32,
    pub(crate) resisted_crit_ticks: i32,
    pub(crate) total_damage: f64,
    pub(crate) total_resisted_damage: f64,
    pub(crate) total_crit_damage: f64,
    pub(crate) total_resisted_crit_damage: f64,
    pub(crate) total_tick_damage: f64,
    pub(crate) total_resisted_tick_damage: f64,
    pub(crate) total_crit_tick_damage: f64,
    pub(crate) total_resisted_crit_tick_damage: f64,
    pub(crate) total_glance_damage: f64,
    pub(crate) total_block_damage: f64,
    pub(crate) total_blocked_crit_damage: f64,
    pub(crate) total_threat: f64,
    pub(crate) total_cast_time: i64,
}

pub(crate) struct Spell<S> {
    pub(crate) id: ActionId,
    pub(crate) flags: Flags,
    pub(crate) behavior: SpellBehavior<S>,
    pub(crate) school: u8,
    pub(crate) school_index: usize,
    /// Go `SpellSchoolFrostfire`, which reads the better of its two schools' bonuses.
    pub(crate) frostfire: bool,
    pub(crate) magic_defense: bool,
    pub(crate) direct_proc: bool,
    /// Go `ProcMaskSpellDamage`.
    pub(crate) proc_spell_damage: bool,
    /// Go `ProcMaskMelee`.
    pub(crate) melee_proc: bool,
    /// Go `ProcMaskMeleeOrRanged`.
    pub(crate) melee_or_ranged_proc: bool,
    pub(crate) class_spell: Option<String>,
    pub(crate) class_spell_mask: bool,
    pub(crate) missile_speed: f64,
    pub(crate) cast_kind: crate::contracts::prepared_v2::CastKind,
    pub(crate) has_extra_cast_condition: bool,
    /// Go's range condition, part of `ExtraCastCondition` when either bound is set.
    pub(crate) min_range: f64,
    pub(crate) max_range: f64,
    pub(crate) ignore_haste: bool,
    pub(crate) cost: Option<Cost>,
    pub(crate) default_cast: Cast,
    pub(crate) cur_cast: Cast,
    pub(crate) cd: Option<(TimerId, i64)>,
    pub(crate) shared_cd: Option<(TimerId, i64)>,
    pub(crate) cast_time_multiplier: f64,
    pub(crate) cd_multiplier: f64,
    pub(crate) bonus_hit_percent: f64,
    pub(crate) bonus_crit_percent: f64,
    pub(crate) bonus_spell_damage: f64,
    pub(crate) bonus_expertise_percent: f64,
    pub(crate) damage_multiplier: f64,
    pub(crate) damage_multiplier_additive: f64,
    pub(crate) direct_damage_multiplier_additive: f64,
    pub(crate) crit_multiplier_pct: f64,
    pub(crate) crit_multiplier_additive: f64,
    pub(crate) bonus_base_damage: f64,
    pub(crate) bonus_coefficient: f64,
    pub(crate) threat_multiplier: f64,
    pub(crate) flat_threat_bonus: f64,
    pub(crate) damage_effect: Option<(f64, f64)>,
    pub(crate) dot: Option<DotId>,
    /// Go `RelatedDotSpell`, whose dot `Spell.Dot` resolves to when this spell has none.
    pub(crate) related_dot_spell: Option<SpellId>,
    /// Index into the resource metrics for this spell's mana cost.
    pub(crate) mana_metrics: Option<usize>,
    /// Indexes into the resource metrics for this spell's energy cost and combo points.
    pub(crate) energy_metrics: Option<(usize, usize)>,
    pub(crate) metrics: [SpellMetrics; 2],
    /// Index into the fight's action metrics, absent for `SpellFlagNoMetrics`.
    pub(crate) action: Option<usize>,
}

/// Go `Hardcast`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Hardcast {
    pub(crate) expires: i64,
    pub(crate) spell: Option<SpellId>,
    pub(crate) target: Side,
}

/// Go `QueuedSpell`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct QueuedSpell {
    pub(crate) spell: SpellId,
    pub(crate) target: Side,
    pub(crate) action: Option<Handle>,
    pub(crate) initiated_at: i64,
}

/// The stats a temporary stat change can set, as Go `Unit.stats` entries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Powers {
    pub(crate) spell_damage: f64,
    pub(crate) attack_power: f64,
    pub(crate) ranged_attack_power: f64,
    pub(crate) spell_crit_percent: f64,
    /// Nothing in scope reads physical crit yet; it follows the stat for completeness.
    pub(crate) physical_crit_percent: f64,
}

/// Mutable player state, reset to the prepared values each iteration.
pub(crate) struct Player {
    /// Go `Unit.stats` for the stats auras can change during a fight.
    pub(crate) powers: Powers,
    pub(crate) mana: f64,
    /// Go `healthBar.currentHealth`; the player takes no damage in scope.
    pub(crate) health: f64,
    pub(crate) spell_cost_percent_modifier: i32,
    /// Go `PseudoStats.CastSpeedMultiplier`.
    pub(crate) cast_speed_multiplier: f64,
    pub(crate) spirit_regen_rate_casting: f64,
    pub(crate) spirit_regen_multiplier: f64,
    pub(crate) force_full_spirit_regen: bool,
    pub(crate) mana_regen_multiplier: f64,
    pub(crate) five_second_rule_refresh: i64,
    pub(crate) mana_tick_casting: f64,
    pub(crate) mana_tick_not_casting: f64,
    pub(crate) waiting_for_mana: f64,
    pub(crate) waiting_for_mana_start: i64,
    /// Go `spiritRegenAttribution`: the regeneration source whose bonus mana ticks credit to
    /// its own metrics.
    pub(crate) spirit_attribution: Option<SpiritAttribution>,
    pub(crate) gcd: i64,
    pub(crate) rotation_timer: i64,
    pub(crate) hardcast: Hardcast,
    pub(crate) hardcast_action: Option<Handle>,
    pub(crate) rotation_action: Option<Handle>,
    pub(crate) queued: Option<QueuedSpell>,
    pub(crate) channeled_dot: Option<DotId>,
    pub(crate) mana_spent: f64,
    pub(crate) mana_gained: f64,
    pub(crate) oom_time: i64,
    pub(crate) went_oom: bool,
    pub(crate) first_oom: i64,
}

/// Go buffs/drivers.go `driveSunderArmor`.
#[derive(Clone, Debug)]
struct SunderRamp {
    aura: AuraRef,
    period: i64,
    ticks: i32,
    armor_by_stacks: Vec<f64>,
}

/// Go `spiritRegenAttribution`: the spirit regeneration state before the source applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SpiritAttribution {
    pub(crate) metrics: usize,
    pub(crate) multiplier: f64,
    pub(crate) force_full: bool,
}

/// Static configuration copied from the prepared input.
pub(crate) struct Config {
    pub(crate) iterations: u32,
    pub(crate) seed: i64,
    pub(crate) debug_first_iteration: bool,
    pub(crate) debug: bool,
    pub(crate) base_duration: i64,
    pub(crate) duration_variation: i64,
    pub(crate) player_label: String,
    pub(crate) player_name: String,
    pub(crate) target_label: String,
    pub(crate) player_level: i32,
    pub(crate) target_level: i32,
    pub(crate) reaction: i64,
    pub(crate) channel_clip_delay: i64,
    pub(crate) distance: f64,
    pub(crate) cast_speed: f64,
    pub(crate) spell_haste_rating: f64,
    pub(crate) max_mana: f64,
    pub(crate) max_health: f64,
    pub(crate) teardown_max_mana: f64,
    pub(crate) mp5: f64,
    pub(crate) spirit_regen_per_second: f64,
    pub(crate) spell_hit_percent: f64,
    /// The prepared stats; auras change the player's copy.
    pub(crate) powers: Powers,
    pub(crate) school_damage: [f64; 8],
    pub(crate) spell_piercing: f64,
    pub(crate) initial: InitialPseudo,
    pub(crate) school_bonus_hit_chance: [f64; 8],
    pub(crate) damage_dealt_multiplier: f64,
    pub(crate) school_damage_dealt_multiplier: [f64; 8],
    pub(crate) crit_damage_multiplier: f64,
    pub(crate) threat_multiplier: f64,
    pub(crate) table: crate::contracts::prepared_v2::AttackTable,
    pub(crate) target_resistance: [f64; 8],
    pub(crate) target_damage_taken_multiplier: f64,
    pub(crate) target_school_damage_taken_multiplier: [f64; 8],
    pub(crate) target_school_bonus_spell_damage: [f64; 8],
    pub(crate) target_bonus_spell_damage_taken: f64,
    pub(crate) target_reduced_crit_taken_percent: f64,
    pub(crate) dot_damage_multiplier_additive: f64,
    pub(crate) target_auto_swing_melee: bool,
    pub(crate) melee: crate::contracts::prepared_v2::Melee,
    pub(crate) melee_haste_rating: f64,
    pub(crate) physical_hit_percent: f64,
    pub(crate) expertise_percent: f64,
    pub(crate) armor_penetration: f64,
    pub(crate) physical_damage: f64,
}

/// The pseudo stats Go restores at each reset, after permanent auras applied.
#[derive(Clone, Copy, Debug)]
pub(crate) struct InitialPseudo {
    pub(crate) spell_cost_percent_modifier: i32,
    pub(crate) cast_speed_multiplier: f64,
    pub(crate) spirit_regen_rate_casting: f64,
    pub(crate) spirit_regen_multiplier: f64,
    pub(crate) force_full_spirit_regen: bool,
}

/// One major cooldown, in Go's initial order.
#[derive(Clone, Debug)]
pub(crate) struct MajorCooldown {
    pub(crate) spell: SpellId,
    pub(crate) priority: i32,
    pub(crate) explosive: bool,
    /// Go `CooldownTypeSurvival`.
    pub(crate) survival: bool,
    pub(crate) timings: Vec<i64>,
    pub(crate) uses: usize,
}

/// Go `proto.ResourceType` of a resource metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResourceKind {
    Mana,
    Health,
    Rage,
    Energy,
    ComboPoints,
}

impl ResourceKind {
    /// The Go proto enum name, as Go's result JSON writes it.
    pub(crate) fn proto_name(self) -> &'static str {
        match self {
            ResourceKind::Mana => "ResourceTypeMana",
            ResourceKind::Health => "ResourceTypeHealth",
            ResourceKind::Rage => "ResourceTypeRage",
            ResourceKind::Energy => "ResourceTypeEnergy",
            ResourceKind::ComboPoints => "ResourceTypeComboPoints",
        }
    }
}

/// Go `ResourceMetrics`.
#[derive(Clone, Debug)]
pub(crate) struct ResourceMetrics {
    pub(crate) id: ActionId,
    pub(crate) kind: ResourceKind,
    pub(crate) events: i32,
    pub(crate) gain: f64,
    pub(crate) actual_gain: f64,
    pub(crate) previous_events: i32,
    pub(crate) previous_actual_gain: f64,
    pub(crate) is_mana_regen: bool,
}

/// A scheduled action. Each variant mirrors one Go pending action.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Action {
    EncounterStart,
    ManaTick,
    Rotation,
    Hardcast,
    QueuedCast,
    Travel {
        spell: SpellId,
        result: SpellResult,
        /// A dot the spell applies when it lands.
        dot: Option<DotId>,
    },
    DotTick(DotId),
    /// A class spell's travel callback: [`Agent::on_travel`].
    ClassTravel {
        spell: SpellId,
        result: SpellResult,
    },
    DelayedProc {
        aura: AuraRef,
        spell: SpellId,
        result: SpellResult,
    },
    /// A rotation prepull action: Go `APLActionCastSpell.Execute`.
    Prepull(SpellId),
    /// A tick of the raid's Sunder Armor ramp, with the ticks done so far.
    SunderTick(i32),
}

/// Go `ActionPriority`.
pub(crate) const PRIORITY_GCD: i32 = 0;
pub(crate) const PRIORITY_REGEN: i32 = 1;
pub(crate) const PRIORITY_DOT: i32 = 3;
pub(crate) const PRIORITY_PREPULL: i32 = 10;
/// Go `SpellBatchWindow`.
pub(crate) const SPELL_BATCH_WINDOW: i64 = 10 * crate::core::time::NS_PER_MILLISECOND;

pub(crate) struct Fight<A: Agent> {
    pub(crate) agent: A,
    pub(crate) config: Config,
    pub(crate) now: i64,
    pub(crate) duration: i64,
    end_of_combat: i64,
    pub(crate) rng: SimRng,
    pub(crate) queue: PendingQueue<Action>,
    min_tracker_time: i64,
    /// Go `sim.minTaskTime`.
    min_task_time: i64,
    /// Go `energyBar`, for a player that has one.
    pub(crate) energy: Option<energy::EnergyBar>,
    pub(crate) player: Player,
    /// Go `Unit.CastSpeed`. Go's unit reset restores the pseudo stats but not this value,
    /// so a speed change undone at the end of a fight carries into the next one.
    pub(crate) cast_speed: f64,
    pub(crate) trackers: [Tracker<A::Aura>; 2],
    pub(crate) spells: Vec<Spell<A::Spell>>,
    pub(crate) dots: Vec<Dot>,
    pub(crate) mods: Vec<spell_mod::SpellMod>,
    pub(crate) timers: Vec<i64>,
    pub(crate) major_cooldowns: Vec<MajorCooldown>,
    pub(crate) cooldown_order: Vec<usize>,
    cooldown_min_ready: i64,
    pub(crate) rotation: Vec<rotation::Item>,
    /// Prepull casts by time, in Go's stable time order.
    prepull: Vec<(i64, SpellId)>,
    in_rotation: bool,
    pub(crate) resources: Vec<ResourceMetrics>,
    pub(crate) actions: Vec<ActionTotals>,
    /// The target's registered actions; it never acts, so their metrics stay zero.
    pub(crate) target_actions: Vec<ActionTotals>,
    /// The player's weapon attacks.
    pub(crate) autos: melee::AutoAttacks,
    /// Go `Unit.Armor()` for the target, which the Sunder Armor ramp lowers.
    pub(crate) target_armor: f64,
    sunder: Option<SunderRamp>,
    /// Gnome's Eureka!, when the character has it.
    pub(crate) eureka: Option<racial::Eureka>,
    mana_regen_casting: usize,
    mana_regen_not_casting: usize,
    mana_gain_spell: Option<SpellId>,
    pub(crate) log: Option<Vec<String>>,
    /// Lines auras log when gained, by index.
    pub(crate) aura_logs: Vec<String>,
    pub(crate) totals: metrics::Totals,
    pub(crate) encounter_damage_taken: f64,
}

/// Errors that make a prepared input impossible to run despite passing coverage.
pub(crate) type BuildError = String;

fn schools(values: &Schools) -> [f64; 8] {
    [
        values.none,
        values.physical,
        values.arcane,
        values.fire,
        values.frost,
        values.holy,
        values.nature,
        values.shadow,
    ]
}

/// Go `SpellSchoolFrostfire`: Fire and Frost.
pub(crate) const SCHOOL_FROSTFIRE: u8 = 4 | 16;
/// Go `stats.SchoolIndexFire` and `SchoolIndexFrost`.
const SCHOOL_INDEX_FIRE: usize = 3;
const SCHOOL_INDEX_FROST: usize = 4;

/// Go `SpellSchool` to `SchoolIndex`, in Go's switch order.
pub(crate) fn school_index(school: u8) -> usize {
    match school {
        s if s & 1 != 0 => 1,
        s if s & 64 != 0 => 2,
        s if s & 4 != 0 => 3,
        s if s & 16 != 0 => 4,
        s if s & 2 != 0 => 5,
        s if s & 8 != 0 => 6,
        s if s & 32 != 0 => 7,
        _ => 0,
    }
}

fn stat(stats: &BTreeMap<String, f64>, name: &str) -> Result<f64, BuildError> {
    stats
        .get(name)
        .copied()
        .ok_or_else(|| format!("prepared stats lack {name}"))
}

impl<A: Agent> Fight<A> {
    /// Build a fight from a prepared input that passed [`crate::check_prepared`].
    /// `class_spell` and `class_aura` map exported entries to class behavior.
    pub(crate) fn new(
        prepared: &PreparedV2,
        agent: A,
        class_spell: impl Fn(&crate::contracts::prepared_v2::Spell) -> Option<A::Spell>,
        class_aura: impl Fn(&str, &str) -> Option<A::Aura>,
    ) -> Result<Self, BuildError> {
        let player = &prepared.player;
        let target = &prepared.target;
        let pseudo = &player.pseudo_stats;
        let target_pseudo = &target.pseudo_stats;
        let resistance = |name: &str| stat(&target.stats, name);
        let config = Config {
            iterations: prepared.sim.iterations,
            seed: prepared.sim.seed,
            debug_first_iteration: prepared.sim.debug_first_iteration,
            debug: prepared.sim.debug,
            base_duration: prepared.encounter.duration_ns,
            duration_variation: prepared.encounter.duration_variation_ns,
            player_label: player.label.clone(),
            player_name: player.name.clone(),
            target_label: target.label.clone(),
            player_level: player.level,
            target_level: target.level,
            reaction: player.reaction_ns,
            channel_clip_delay: player.channel_clip_delay_ns,
            distance: player.distance_yards,
            cast_speed: player.cast_speed,
            spell_haste_rating: stat(&player.stats, "SpellHasteRating")?,
            max_mana: player.mana.max,
            max_health: stat(&player.stats, "Health")?,
            teardown_max_mana: player.mana.teardown_max,
            mp5: stat(&player.stats, "MP5")?,
            spirit_regen_per_second: player.mana.spirit_regen_per_second,
            spell_hit_percent: stat(&player.stats, "SpellHitPercent")?,
            powers: Powers {
                spell_crit_percent: stat(&player.stats, "SpellCritPercent")?,
                physical_crit_percent: stat(&player.stats, "PhysicalCritPercent")?,
                spell_damage: stat(&player.stats, "SpellDamage")?,
                attack_power: stat(&player.stats, "AttackPower")?,
                ranged_attack_power: stat(&player.stats, "RangedAttackPower")?,
            },
            school_damage: [
                0.0,
                0.0,
                stat(&player.stats, "ArcaneDamage")?,
                stat(&player.stats, "FireDamage")?,
                stat(&player.stats, "FrostDamage")?,
                stat(&player.stats, "HolyDamage")?,
                stat(&player.stats, "NatureDamage")?,
                stat(&player.stats, "ShadowDamage")?,
            ],
            spell_piercing: stat(&player.stats, "SpellPiercing")?,
            initial: InitialPseudo {
                spell_cost_percent_modifier: pseudo.spell_cost_percent_modifier,
                cast_speed_multiplier: pseudo.cast_speed_multiplier,
                spirit_regen_rate_casting: pseudo.spirit_regen_rate_casting,
                spirit_regen_multiplier: pseudo.spirit_regen_multiplier,
                force_full_spirit_regen: pseudo.force_full_spirit_regen,
            },
            school_bonus_hit_chance: schools(&pseudo.school_bonus_hit_chance),
            damage_dealt_multiplier: pseudo.damage_dealt_multiplier,
            school_damage_dealt_multiplier: schools(&pseudo.school_damage_dealt_multiplier),
            crit_damage_multiplier: pseudo.crit_damage_multiplier,
            threat_multiplier: pseudo.threat_multiplier,
            table: player.attack_table.clone(),
            target_resistance: [
                0.0,
                stat(&target.stats, "Armor")?,
                resistance("ArcaneResistance")?,
                resistance("FireResistance")?,
                resistance("FrostResistance")?,
                0.0,
                resistance("NatureResistance")?,
                resistance("ShadowResistance")?,
            ],
            target_damage_taken_multiplier: target_pseudo.damage_taken_multiplier,
            target_school_damage_taken_multiplier: schools(
                &target_pseudo.school_damage_taken_multiplier,
            ),
            target_school_bonus_spell_damage: schools(&target_pseudo.school_bonus_spell_damage),
            target_bonus_spell_damage_taken: target_pseudo.bonus_spell_damage_taken,
            target_reduced_crit_taken_percent: target_pseudo.reduced_crit_taken_percent,
            dot_damage_multiplier_additive: pseudo.dot_damage_multiplier_additive,
            target_auto_swing_melee: target.auto_swing_melee,
            melee: prepared.melee.clone(),
            melee_haste_rating: stat(&player.stats, "MeleeHasteRating")?,
            physical_hit_percent: stat(&player.stats, "PhysicalHitPercent")?,
            expertise_percent: stat(&player.stats, "ExpertisePercent")?,
            armor_penetration: stat(&player.stats, "ArmorPenetration")?,
            physical_damage: stat(&player.stats, "PhysicalDamage")?,
        };

        let mut timer_names: Vec<String> = Vec::new();
        let mut timer = |name: &str| -> TimerId {
            match timer_names.iter().position(|existing| existing == name) {
                Some(index) => index,
                None => {
                    timer_names.push(name.to_string());
                    timer_names.len() - 1
                }
            }
        };

        let mut resources = Vec::new();
        let mut resource = |id: ActionId, regen: bool, kind: ResourceKind| {
            resources.push(ResourceMetrics {
                id,
                kind,
                events: 0,
                gain: 0.0,
                actual_gain: 0.0,
                previous_events: 0,
                previous_actual_gain: 0.0,
                is_mana_regen: regen,
            });
            resources.len() - 1
        };
        let regen_id = |tag| ActionId {
            other_id: "OtherActionManaRegen".into(),
            tag,
            ..ActionId::default()
        };
        let mana_regen_casting = resource(regen_id(1), false, ResourceKind::Mana);
        let mana_regen_not_casting = resource(regen_id(2), false, ResourceKind::Mana);

        let effects = &prepared.effects;
        let mut spells = Vec::new();
        let mut dots = Vec::new();
        let mut mana_gain_spell = None;
        // Spells that activate a player aura, resolved once the auras are registered.
        let mut activations: Vec<(SpellId, &str)> = Vec::new();
        for exported in &player.spells {
            let id = exported.action_id.clone().unwrap_or_default();
            if id.other_id == "OtherActionManaGain" {
                mana_gain_spell = Some(spells.len());
            }
            let item = id.item_id;
            let behavior = if let Some(class) = class_spell(exported) {
                SpellBehavior::Class(class)
            } else if id.other_id == "OtherActionAttack" && (id.tag == 1 || id.tag == 2) {
                SpellBehavior::MeleeAuto(if id.tag == 1 {
                    melee::Hand::Main
                } else {
                    melee::Hand::Off
                })
            } else {
                effects
                    .iter()
                    .find_map(|effect| match effect {
                        Effect::PotionMana {
                            item_id,
                            rng_label,
                            gains,
                            stone_multiplier,
                            regen_window_seconds,
                        } if *item_id == item && gains.len() == 1 => {
                            Some(SpellBehavior::PotionMana {
                                label: rng_label.clone(),
                                min: gains[0].min,
                                spread: gains[0].spread,
                                stone_multiplier: *stone_multiplier,
                                regen_window: *regen_window_seconds,
                            })
                        }
                        Effect::ConjuredMana {
                            item_id,
                            rng_label,
                            gains,
                            selected,
                            regen_window_seconds,
                        } if *item_id == item && gains.len() == 1 => {
                            Some(SpellBehavior::ConjuredMana {
                                label: rng_label.clone(),
                                min: gains[0].min,
                                spread: gains[0].spread,
                                selected: *selected,
                                regen_window: *regen_window_seconds,
                            })
                        }
                        Effect::EnergizeOnUse {
                            item_id,
                            average,
                            variance,
                            whole,
                            ..
                        } if *item_id == item => Some(SpellBehavior::EnergizeOnUse {
                            average: *average,
                            variance: *variance,
                            whole: *whole,
                        }),
                        Effect::Eureka { spell_id, .. }
                            if id.spell_id == *spell_id && id.tag == 0 =>
                        {
                            Some(SpellBehavior::Eureka)
                        }
                        Effect::Berserking { spell_id, aura, .. }
                        | Effect::BloodFury { spell_id, aura, .. }
                        | Effect::ShatterCurse { spell_id, aura }
                        | Effect::Stoneform { spell_id, aura }
                        | Effect::ReadLeyLine { spell_id, aura, .. }
                        | Effect::TemporaryStats { spell_id, aura, .. }
                            if id.spell_id == *spell_id && id.tag == 0 =>
                        {
                            activations.push((spells.len(), aura));
                            Some(SpellBehavior::None)
                        }
                        Effect::TouchOfTheGrave {
                            drain_spell_id,
                            health_fraction,
                            ..
                        } if id.spell_id == *drain_spell_id && id.tag == 0 => {
                            Some(SpellBehavior::TouchOfTheGraveDrain {
                                health_fraction: *health_fraction,
                                metrics: resource(id.clone(), false, ResourceKind::Health),
                            })
                        }
                        _ => None,
                    })
                    .unwrap_or(SpellBehavior::None)
            };
            let cost = exported.cost.as_ref().map(|cost| Cost {
                kind: if cost.resource == "energy" {
                    ResourceKind::Energy
                } else {
                    ResourceKind::Mana
                },
                refund: cost.refund,
                base: cost.base_cost,
                flat_modifier: cost.flat_modifier,
                percent_modifier: cost.percent_modifier,
                additive_percent_modifier: cost.additive_percent_modifier,
            });
            // Go newEnergyCost registers the energy metrics, then the combo point metrics.
            let (mana_metrics, energy_metrics) = match cost.map(|cost| cost.kind) {
                Some(ResourceKind::Energy) => {
                    let energy = resource(id.clone(), false, ResourceKind::Energy);
                    let combo = resource(id.clone(), false, ResourceKind::ComboPoints);
                    (None, Some((energy, combo)))
                }
                Some(kind) => (Some(resource(id.clone(), false, kind)), None),
                None => (None, None),
            };
            let spell_id = spells.len();
            let dot = exported.dot.as_ref().map(|exported_dot| {
                dots.push(Dot::new(spell_id, exported_dot));
                dots.len() - 1
            });
            let cast = &exported.default_cast;
            spells.push(Spell {
                flags: Flags::parse(&exported.flags),
                behavior,
                school: exported.school,
                school_index: school_index(exported.school),
                frostfire: exported.school == SCHOOL_FROSTFIRE,
                magic_defense: exported.defense_type == "DefenseTypeMagic",
                direct_proc: exported
                    .proc_mask
                    .iter()
                    .any(|mask| DIRECT_PROC_MASKS.contains(&mask.as_str())),
                proc_spell_damage: exported
                    .proc_mask
                    .iter()
                    .any(|mask| mask == "ProcMaskSpellDamage"),
                melee_or_ranged_proc: exported.proc_mask.iter().any(|mask| {
                    matches!(
                        mask.as_str(),
                        "ProcMaskMeleeMHAuto"
                            | "ProcMaskMeleeOHAuto"
                            | "ProcMaskMeleeMHSpecial"
                            | "ProcMaskMeleeOHSpecial"
                            | "ProcMaskRangedAuto"
                            | "ProcMaskRangedSpecial"
                    )
                }),
                melee_proc: exported.proc_mask.iter().any(|mask| {
                    matches!(
                        mask.as_str(),
                        "ProcMaskMeleeMHAuto"
                            | "ProcMaskMeleeOHAuto"
                            | "ProcMaskMeleeMHSpecial"
                            | "ProcMaskMeleeOHSpecial"
                    )
                }),
                class_spell: exported.class_spell.clone(),
                class_spell_mask: exported.class_spell.is_some(),
                missile_speed: exported.missile_speed,
                cast_kind: exported.cast_kind,
                has_extra_cast_condition: exported.has_extra_cast_condition,
                min_range: exported.min_range,
                max_range: exported.max_range,
                ignore_haste: exported.ignore_haste,
                cost,
                default_cast: Cast {
                    cost: cast.cost,
                    gcd: cast.gcd_ns,
                    gcd_min: cast.gcd_min_ns,
                    cast_time: cast.cast_time_ns,
                    non_empty: cast.non_empty,
                },
                cur_cast: Cast::default(),
                cd: exported
                    .cd
                    .as_ref()
                    .map(|cd| (timer(&cd.timer), cd.duration_ns)),
                shared_cd: exported
                    .shared_cd
                    .as_ref()
                    .map(|cd| (timer(&cd.timer), cd.duration_ns)),
                cast_time_multiplier: exported.cast_time_multiplier,
                cd_multiplier: exported.cd_multiplier,
                bonus_hit_percent: exported.bonus_hit_percent,
                bonus_crit_percent: exported.bonus_crit_percent,
                bonus_spell_damage: exported.bonus_spell_damage,
                bonus_expertise_percent: exported.bonus_expertise_percent,
                damage_multiplier: exported.damage_multiplier,
                damage_multiplier_additive: exported.damage_multiplier_additive,
                direct_damage_multiplier_additive: exported.direct_damage_multiplier_additive,
                crit_multiplier_pct: exported.crit_multiplier_pct,
                crit_multiplier_additive: exported.crit_multiplier_additive,
                bonus_base_damage: exported.bonus_base_damage,
                bonus_coefficient: exported.bonus_coefficient,
                threat_multiplier: exported.threat_multiplier,
                flat_threat_bonus: exported.flat_threat_bonus,
                damage_effect: exported.damage_effect.map(|e| (e.average, e.variance)),
                dot,
                related_dot_spell: exported.related_dot_spell,
                mana_metrics,
                energy_metrics,
                metrics: [SpellMetrics::default(); 2],
                action: None,
                id,
            });
        }

        // Go keys action metrics by action ID in spellbook order.
        let mut actions: Vec<ActionTotals> = Vec::new();
        for spell in &mut spells {
            if spell.flags.no_metrics {
                continue;
            }
            let index = match actions.iter().position(|action| action.id == spell.id) {
                Some(index) => index,
                None => {
                    actions.push(ActionTotals {
                        id: spell.id.clone(),
                        melee: spell.flags.melee_metrics,
                        passive: spell.flags.passive,
                        school: spell.school,
                        targets: [ActionReport::new(0), ActionReport::new(1)],
                    });
                    actions.len() - 1
                }
            };
            spell.action = Some(index);
        }

        let mut aura_logs: Vec<String> = Vec::new();
        let mut trackers = [Tracker::default(), Tracker::default()];
        for (side, auras) in [(Side::Target, &target.auras), (Side::Player, &player.auras)] {
            let unit = if side == Side::Player {
                "player"
            } else {
                "target"
            };
            for exported in auras {
                let behavior = if let Some(dot) = dots
                    .iter()
                    .position(|dot: &Dot| dot.aura_label == exported.label && dot.side == side)
                {
                    AuraBehavior::Dot(dot)
                } else if let Some(kind) = class_aura(unit, &exported.label) {
                    AuraBehavior::Class(kind)
                } else if let Some(jow) = effects.iter().find_map(|effect| match effect {
                    Effect::JudgementOfWisdom {
                        aura,
                        proc_chance,
                        mana,
                        metrics_action_id,
                        delay_ns,
                        ..
                    } if side == Side::Target && *aura == exported.label => Some((
                        *proc_chance,
                        *mana,
                        metrics_action_id.clone(),
                        *delay_ns,
                    )),
                    _ => None,
                }) {
                    let metrics = resource(jow.2.clone(), false, ResourceKind::Mana);
                    AuraBehavior::JudgementOfWisdom {
                        chance: jow.0,
                        mana: jow.1,
                        metrics,
                        delay: jow.3,
                    }
                } else if let Some((chance, delay, drain)) =
                    effects.iter().find_map(|effect| match effect {
                        Effect::TouchOfTheGrave {
                            trigger_aura,
                            drain_spell_id,
                            proc_chance,
                            delay_ns,
                            ..
                        } if side == Side::Player && *trigger_aura == exported.label => Some((
                            *proc_chance,
                            *delay_ns,
                            *drain_spell_id,
                        )),
                        _ => None,
                    })
                {
                    let drain = spells
                        .iter()
                        .position(|spell: &Spell<A::Spell>| {
                            spell.id.spell_id == drain && spell.id.tag == 0
                        })
                        .ok_or_else(|| format!("Touch of the Grave drain {drain} is not registered"))?;
                    AuraBehavior::TouchOfTheGrave {
                        chance,
                        delay,
                        drain,
                    }
                } else if side == Side::Player
                    && effects
                        .iter()
                        .any(|effect| matches!(effect, Effect::Eureka { aura, .. } if *aura == exported.label))
                {
                    AuraBehavior::Eureka
                } else if let Some(multiplier) = effects.iter().find_map(|effect| match effect {
                    Effect::Berserking {
                        aura,
                        cast_speed_multiplier,
                        ..
                    } if side == Side::Player && *aura == exported.label => {
                        Some(*cast_speed_multiplier)
                    }
                    _ => None,
                }) {
                    AuraBehavior::MultiplyCastSpeed(multiplier)
                } else if let Some(multiplier) = effects.iter().find_map(|effect| match effect {
                    Effect::ReadLeyLine {
                        aura,
                        regen_multiplier,
                        ..
                    } if side == Side::Player && *aura == exported.label => Some(*regen_multiplier),
                    _ => None,
                }) {
                    AuraBehavior::MultiplyManaRegenSpeed(multiplier)
                } else if let Some((stats, gain_log, expire_log)) =
                    effects.iter().find_map(|effect| match effect {
                        Effect::BloodFury {
                            aura, active_stats, ..
                        } if side == Side::Player && *aura == exported.label => {
                            Some((active_stats, None, None))
                        }
                        Effect::TemporaryStats {
                            aura,
                            active_stats,
                            gain_log,
                            expire_log,
                            ..
                        } if side == Side::Player && *aura == exported.label => {
                            Some((active_stats, Some(gain_log), Some(expire_log)))
                        }
                        _ => None,
                    })
                {
                    let active = |name: &str| stats.get(name).copied();
                    let base = config.powers;
                    let mut logged = |line: Option<&String>| {
                        line.map(|line| {
                            aura_logs.push(line.clone());
                            aura_logs.len() - 1
                        })
                    };
                    let gain_log = logged(gain_log);
                    let expire_log = logged(expire_log);
                    AuraBehavior::TemporaryStats {
                        powers: Powers {
                            spell_damage: active("SpellDamage").unwrap_or(base.spell_damage),
                            attack_power: active("AttackPower").unwrap_or(base.attack_power),
                            ranged_attack_power: active("RangedAttackPower")
                                .unwrap_or(base.ranged_attack_power),
                            spell_crit_percent: active("SpellCritPercent")
                                .unwrap_or(base.spell_crit_percent),
                            physical_crit_percent: active("PhysicalCritPercent")
                                .unwrap_or(base.physical_crit_percent),
                        },
                        gain_log,
                        expire_log,
                    }
                } else if effects.iter().any(|effect| {
                    matches!(effect, Effect::InertListener { unit: u, aura, .. } if u == unit && *aura == exported.label)
                }) {
                    AuraBehavior::Inert
                } else {
                    AuraBehavior::Static
                };
                let icd = exported
                    .icd
                    .as_ref()
                    .map(|icd| (timer(&icd.timer), icd.duration_ns));
                trackers[side.index()].register(exported, behavior, icd);
            }
        }
        for (spell, label) in activations {
            let aura = trackers[Side::Player.index()]
                .find(label)
                .ok_or_else(|| format!("aura {label} is not registered"))?;
            spells[spell].behavior = SpellBehavior::ActivateAura(aura);
        }
        for dot in &mut dots {
            dot.aura = trackers[dot.side.index()]
                .find(&dot.aura_label)
                .map(|index| AuraRef {
                    side: dot.side,
                    index,
                })
                .ok_or_else(|| format!("dot aura {} is not registered", dot.aura_label))?;
        }

        let mut major_cooldowns = Vec::new();
        for cooldown in &player.major_cooldowns {
            let spell = spells
                .iter()
                .position(|spell| spell.id == cooldown.action_id)
                .ok_or_else(|| format!("major cooldown {} has no spell", cooldown.action_id))?;
            major_cooldowns.push(MajorCooldown {
                spell,
                priority: cooldown.priority,
                explosive: cooldown.kind.iter().any(|kind| kind == "explosive"),
                survival: cooldown.kind.iter().any(|kind| kind == "survival"),
                timings: cooldown.timings_ns.clone(),
                uses: 0,
            });
        }

        let parsed =
            crate::rotation::parse(&player.rotation).map_err(|reasons| reasons.join("; "))?;
        let mut fight = Fight {
            agent,
            now: 0,
            duration: config.base_duration,
            end_of_combat: config.base_duration,
            rng: SimRng::new(prepared.sim.labeled_rng, prepared.sim.seed as u64),
            queue: PendingQueue::default(),
            min_tracker_time: NEVER_EXPIRES,
            min_task_time: NEVER_EXPIRES,
            energy: None,
            player: Player {
                powers: config.powers,
                mana: config.max_mana,
                health: config.max_health,
                spell_cost_percent_modifier: config.initial.spell_cost_percent_modifier,
                cast_speed_multiplier: config.initial.cast_speed_multiplier,
                spirit_regen_rate_casting: config.initial.spirit_regen_rate_casting,
                spirit_regen_multiplier: config.initial.spirit_regen_multiplier,
                force_full_spirit_regen: config.initial.force_full_spirit_regen,
                mana_regen_multiplier: 1.0,
                five_second_rule_refresh: 0,
                mana_tick_casting: 0.0,
                mana_tick_not_casting: 0.0,
                waiting_for_mana: 0.0,
                waiting_for_mana_start: 0,
                spirit_attribution: None,
                gcd: STARTING_CD_TIME,
                rotation_timer: STARTING_CD_TIME,
                hardcast: Hardcast {
                    expires: STARTING_CD_TIME,
                    spell: None,
                    target: Side::Target,
                },
                hardcast_action: None,
                rotation_action: None,
                queued: None,
                channeled_dot: None,
                mana_spent: 0.0,
                mana_gained: 0.0,
                oom_time: 0,
                went_oom: false,
                first_oom: 0,
            },
            cast_speed: config.cast_speed,
            config,
            trackers,
            spells,
            dots,
            mods: Vec::new(),
            timers: vec![STARTING_CD_TIME; timer_names.len()],
            cooldown_order: (0..major_cooldowns.len()).collect(),
            major_cooldowns,
            cooldown_min_ready: NEVER_EXPIRES,
            rotation: Vec::new(),
            prepull: Vec::new(),
            in_rotation: false,
            resources,
            actions,
            target_actions: target
                .metrics_actions
                .iter()
                .map(|action| ActionTotals {
                    id: action.action_id.clone(),
                    melee: action.melee_metrics,
                    passive: false,
                    school: action.school,
                    targets: [ActionReport::new(0), ActionReport::new(1)],
                })
                .collect(),
            mana_regen_casting,
            mana_regen_not_casting,
            mana_gain_spell,
            log: None,
            autos: melee::AutoAttacks::default(),
            target_armor: prepared.melee.defender_armor,
            sunder: None,
            aura_logs,
            eureka: None,
            totals: metrics::Totals::default(),
            encounter_damage_taken: 0.0,
        };
        if let Some(energy) = &player.energy {
            fight.enable_energy_bar(energy);
        }
        fight.rotation = fight.compile_rotation(&parsed);
        fight.prepull = fight.compile_prepull(&parsed);
        for effect in effects {
            if let Effect::SunderArmorRamp {
                aura,
                period_ns,
                ticks,
                armor_by_stacks,
            } = effect
            {
                let index = fight.trackers[Side::Target.index()]
                    .find(aura)
                    .ok_or_else(|| format!("target aura {aura} is not registered"))?;
                fight.sunder = Some(SunderRamp {
                    aura: AuraRef {
                        side: Side::Target,
                        index,
                    },
                    period: *period_ns,
                    ticks: *ticks,
                    armor_by_stacks: armor_by_stacks.clone(),
                });
            }
        }
        fight.autos.melee = prepared.melee.auto_swing_melee;
        fight.autos.dual_wielding = prepared.melee.dual_wielding;
        fight.autos.mh.weapon = prepared.melee.main_hand.clone();
        fight.autos.oh.weapon = prepared.melee.off_hand.clone();
        let auto_spell = |tag: i32| {
            fight
                .spells
                .iter()
                .position(|spell| spell.id.other_id == "OtherActionAttack" && spell.id.tag == tag)
        };
        fight.autos.mh.spell = auto_spell(1);
        fight.autos.oh.spell = auto_spell(2);
        for effect in effects {
            if let Effect::Eureka {
                aura,
                cost_percent,
                damage_percent,
                tick_cancel_percent,
                cost_spells,
                damage_spells,
                tick_cancel_spells,
                spending_spells,
                ..
            } = effect
            {
                fight.eureka = Some(fight.bind_eureka(
                    aura,
                    [*cost_percent, *damage_percent, *tick_cancel_percent],
                    [cost_spells, damage_spells, tick_cancel_spells],
                    spending_spells,
                )?);
            }
        }
        Ok(fight)
    }

    /// Go `HasManaBar`: a class without mana, such as a Rogue, exports no maximum mana.
    pub(crate) fn has_mana_bar(&self) -> bool {
        self.config.max_mana > 0.0
    }

    /// The mana regeneration inputs for Go `ManaRegenPerSecondWhileCasting`.
    pub(crate) fn regen_inputs(&self) -> RegenInputs {
        RegenInputs {
            mp5: self.config.mp5,
            spirit_regen_per_second: self.config.spirit_regen_per_second,
            spirit_regen_rate_casting: self.player.spirit_regen_rate_casting,
            force_full_spirit_regen: self.player.force_full_spirit_regen,
            spirit_regen_multiplier: self.player.spirit_regen_multiplier,
            mana_regen_multiplier: self.player.mana_regen_multiplier,
        }
    }

    /// Go `MultiplyManaRegenSpeed`.
    pub(crate) fn multiply_mana_regen_speed(&mut self, multiplier: f64) {
        self.player.mana_regen_multiplier *= multiplier;
        self.update_mana_regen_rates();
    }

    /// Go `UpdateManaRegenRates`.
    pub(crate) fn update_mana_regen_rates(&mut self) {
        let inputs = self.regen_inputs();
        self.player.mana_tick_casting =
            crate::mechanics::mana::regen_per_second_casting(inputs) * 2.0;
        self.player.mana_tick_not_casting =
            crate::mechanics::mana::regen_per_second_not_casting(inputs) * 2.0;
    }

    /// Go `RandomFloat(label)`.
    pub(crate) fn random(&mut self, label: &str) -> f64 {
        self.rng.next_f64(label)
    }

    /// `random` labeled with an aura's label, as Go proc triggers do.
    pub(crate) fn random_for_aura(&mut self, aura: AuraRef) -> f64 {
        let label = &self.trackers[aura.side.index()].auras[aura.index].label;
        self.rng.next_f64(label)
    }

    /// Go `Proc(p, label)`: no draw at the extremes.
    pub(crate) fn proc(&mut self, chance: f64, label: &str) -> bool {
        if chance >= 1.0 {
            true
        } else if chance <= 0.0 {
            false
        } else {
            self.random(label) < chance
        }
    }

    pub(crate) fn schedule(&mut self, time: i64, priority: i32, action: Action) -> Handle {
        self.queue.push(time, priority, action)
    }

    /// Go `rescheduleTracker`.
    pub(crate) fn reschedule_tracker(&mut self, time: i64) {
        self.min_tracker_time = self.min_tracker_time.min(time);
    }

    /// Run every iteration and return the aggregate report.
    pub(crate) fn run(&mut self) -> FightReport {
        let started = std::time::Instant::now();
        let iterations = self.config.iterations;
        let mut total_duration = 0i64;
        let mut first_duration = 0i64;
        let mut logs = String::new();
        for iteration in 0..iterations {
            let seed = self.config.seed + i64::from(iteration);
            self.rng.reseed(seed as u64);
            // Go keeps one buffer: every iteration with debug, otherwise only the first.
            let logged = self.config.debug || (iteration == 0 && self.config.debug_first_iteration);
            self.log = logged.then(Vec::new);
            self.run_once();
            if let Some(lines) = self.log.take() {
                logs.extend(lines);
            }
            if iteration == 0 {
                first_duration = self.duration;
            }
            total_duration += self.duration;
        }
        let elapsed_ns = started.elapsed().as_nanos() as u64;
        self.report(first_duration, total_duration, logs, elapsed_ns)
    }

    /// Go `runOnce`: reset, prepull, the event loop and cleanup.
    fn run_once(&mut self) {
        self.reset();
        // Go PrePull: time starts at the first prepull action; later ones run first at a tie.
        let count = self.prepull.len() as i32;
        if let Some(&(first, _)) = self.prepull.first() {
            self.now = first;
        }
        for index in 0..self.prepull.len() {
            let (at, spell) = self.prepull[index];
            self.schedule(
                at,
                PRIORITY_PREPULL + count - index as i32,
                Action::Prepull(spell),
            );
        }
        self.schedule(0, PRIORITY_PREPULL + count + 1, Action::EncounterStart);
        while self.step() {}
        self.cleanup();
    }

    /// Go `Simulation.reset` and `Environment.reset`.
    fn reset(&mut self) {
        self.duration = self.config.base_duration;
        if self.config.duration_variation != 0 {
            let variation = self.config.duration_variation * 2;
            let roll = self.random("sim duration");
            self.duration += (roll * variation as f64) as i64 - self.config.duration_variation;
        }
        self.queue.clear();
        self.end_of_combat = self.duration;
        self.now = 0;
        self.min_tracker_time = NEVER_EXPIRES;
        self.min_task_time = NEVER_EXPIRES;
        // Go's environment always rolls the pet heartbeat offset, even without pets.
        self.random("Pet Stat Inheritance");
        self.encounter_damage_taken = 0.0;
        self.reset_unit(Side::Target);
        self.reset_unit(Side::Player);
        self.reset_cooldown_manager();
        // Go initManaTickAction, after the environment reset: two seconds after the prepull
        // starts, when a unit has a mana bar.
        if self.has_mana_bar() {
            let prepull_start = self.prepull.first().map_or(0, |&(at, _)| at);
            self.schedule(
                prepull_start + 2 * crate::core::time::NS_PER_SECOND,
                PRIORITY_REGEN,
                Action::ManaTick,
            );
        }
    }

    /// Go `Unit.reset` followed by `Character.reset` for the player.
    fn reset_unit(&mut self, side: Side) {
        if side == Side::Player {
            self.timers.fill(STARTING_CD_TIME);
            let player = &mut self.player;
            player.gcd = STARTING_CD_TIME;
            player.rotation_timer = STARTING_CD_TIME;
            player.hardcast = Hardcast {
                expires: STARTING_CD_TIME,
                spell: None,
                target: Side::Target,
            };
            player.hardcast_action = None;
            player.rotation_action = None;
            player.channeled_dot = None;
            player.queued = None;
            player.mana_spent = 0.0;
            player.mana_gained = 0.0;
            player.oom_time = 0;
            player.went_oom = false;
            player.first_oom = 0;
            for resource in &mut self.resources {
                resource.previous_events = resource.events;
                resource.previous_actual_gain = resource.actual_gain;
            }
            let initial = self.config.initial;
            player.spell_cost_percent_modifier = initial.spell_cost_percent_modifier;
            player.cast_speed_multiplier = initial.cast_speed_multiplier;
            player.powers = self.config.powers;
            player.spirit_regen_rate_casting = initial.spirit_regen_rate_casting;
            player.spirit_regen_multiplier = initial.spirit_regen_multiplier;
            player.force_full_spirit_regen = initial.force_full_spirit_regen;
            player.five_second_rule_refresh = 0;
            player.spirit_attribution = None;
            self.reset_auto_attacks();
            // Go runs reset effects first in the aura tracker's reset.
            self.reset_mods();
            A::reset(self);
        }
        self.reset_auras(side);
        // Go ScheduledAura's OnReset: the ramp's first tick at the pull, at dot priority.
        if side == Side::Target {
            self.target_armor = self.config.melee.defender_armor;
            if self.sunder.is_some() {
                self.schedule(0, PRIORITY_DOT, Action::SunderTick(0));
            }
        }
        if side == Side::Player {
            for spell in &mut self.spells {
                spell.metrics = [SpellMetrics::default(); 2];
            }
            self.player.mana = self.config.max_mana;
            self.player.health = self.config.max_health;
            self.player.mana_regen_multiplier = 1.0;
            self.player.waiting_for_mana = 0.0;
            self.player.waiting_for_mana_start = 0;
            self.update_mana_regen_rates();
            // Go energyBar.reset, after the mana and health bars.
            let prepull_start = self.prepull.first().map_or(0, |&(at, _)| at);
            self.reset_energy(prepull_start);
        }
        // Go AutoAttacks.reset: an enemy with a melee swing rolls its opening offset.
        if side == Side::Target && self.config.target_auto_swing_melee {
            self.random("Enemy Swing Offset");
        }
        self.rotation_reset(side);
        // Go addTracker: the target's tracker first, then the player's.
        let tracker_min = self.trackers[side.index()].min_expires;
        self.reschedule_tracker(tracker_min);
    }

    /// Go `Simulation.Step`. Returns false when the fight is over.
    fn step(&mut self) -> bool {
        // Go runs due weapon swings before the next pending action, ties included.
        let next = self.queue.peek_time().unwrap_or(NEVER_EXPIRES);
        if self.due_weapon_attack(next) && self.autos.min_time <= self.min_task_time {
            if self.autos.min_time > self.end_of_combat {
                return false;
            }
            self.advance_weapon_attacks();
            return true;
        }
        // Then due tasks, the energy ticks.
        if self.due_task(next) {
            if self.min_task_time > self.end_of_combat {
                return false;
            }
            self.advance_tasks();
            return true;
        }
        let Some((time, handle, action)) = self.queue.pop() else {
            return false;
        };
        if time > self.end_of_combat {
            return false;
        }
        if time > self.now {
            self.advance_to(time);
        }
        // An expiring aura can cancel the action that is about to run.
        if !self.handle_still_valid(handle, &action) {
            return true;
        }
        self.run_action(handle, action);
        true
    }

    /// Go `Simulation.advance`: expire auras whose time has come.
    pub(crate) fn advance_to(&mut self, time: i64) {
        self.now = time;
        if self.now >= self.min_tracker_time {
            self.min_tracker_time = NEVER_EXPIRES;
            for side in [Side::Target, Side::Player] {
                let next = self.try_advance_tracker(side);
                self.min_tracker_time = self.min_tracker_time.min(next);
            }
        }
    }

    fn run_action(&mut self, handle: Handle, action: Action) {
        match action {
            Action::EncounterStart => self.encounter_start(),
            Action::ManaTick => {
                self.mana_tick();
                let next = self.now + 2 * crate::core::time::NS_PER_SECOND;
                self.schedule(next, PRIORITY_REGEN, Action::ManaTick);
            }
            Action::Rotation => {
                if self.player.rotation_action == Some(handle) {
                    self.player.rotation_action = None;
                }
                self.complete_due_hardcast();
                self.do_next_action();
            }
            Action::Hardcast => {
                if self.player.hardcast_action == Some(handle) {
                    self.player.hardcast_action = None;
                }
                self.complete_due_hardcast();
            }
            Action::QueuedCast => {
                if let Some(queued) = self.player.queued.as_mut() {
                    if queued.action == Some(handle) {
                        queued.action = None;
                    }
                    let (spell, target) = (queued.spell, queued.target);
                    self.cast(spell, target);
                }
            }
            Action::Travel { spell, result, dot } => {
                self.deal_damage(spell, result, false);
                if let (Some(dot), true) = (dot, result.landed()) {
                    self.apply_dot(dot);
                }
            }
            Action::DotTick(dot) => self.periodic_tick(dot, handle),
            Action::ClassTravel { spell, result } => {
                if let SpellBehavior::Class(behavior) = self.spells[spell].behavior {
                    A::on_travel(self, spell, result, behavior);
                }
            }
            Action::DelayedProc {
                aura,
                spell,
                result,
            } => self.delayed_proc(aura, spell, result),
            Action::Prepull(spell) => self.cast_or_queue(spell, Side::Target),
            Action::SunderTick(done) => self.sunder_tick(done),
        }
    }

    /// Go driveSunderArmor's periodic action: activate, add a stack, and come back a period
    /// later until every tick has run.
    fn sunder_tick(&mut self, done: i32) {
        let ramp = self.sunder.clone().expect("the ramp is bound");
        self.activate_aura(ramp.aura);
        if self.aura(ramp.aura).active {
            self.add_stack(ramp.aura);
        }
        let stacks = self.aura(ramp.aura).stacks.max(0) as usize;
        self.target_armor = ramp.armor_by_stacks[stacks.min(ramp.armor_by_stacks.len() - 1)];
        if done + 1 < ramp.ticks {
            self.schedule(
                self.now + ramp.period,
                PRIORITY_DOT,
                Action::SunderTick(done + 1),
            );
        }
    }

    /// Handles of reusable actions are cleared when consumed so Go's `consumed` checks hold.
    fn handle_still_valid(&mut self, handle: Handle, action: &Action) -> bool {
        match action {
            Action::DotTick(dot) => self.dots[*dot].tick_action == Some(handle),
            _ => true,
        }
    }

    /// Go encounter start: agents and auras, then each unit starts the pull.
    fn encounter_start(&mut self) {
        // No supported unit has encounter start callbacks. The player starts its swings, then
        // its rotation at max(0, GCD ready).
        self.randomize_melee_timing();
        self.start_auto_attacks();
        let ready = self.player.gcd.max(0);
        self.set_gcd_timer(ready);
    }

    /// Go `Simulation.Cleanup` and `doneIteration`.
    fn cleanup(&mut self) {
        self.now = self.duration;
        self.queue.clear();
        self.player.hardcast = Hardcast {
            expires: 0,
            spell: None,
            target: Side::Target,
        };
        self.player_done_iteration();
        self.aura_done_iteration(Side::Player);
        for spell in 0..self.spells.len() {
            self.spell_done_iteration(spell);
        }
        let damage = self.totals.iteration_damage;
        self.aura_done_iteration(Side::Target);
        self.unit_done_iteration(damage);
    }
}
