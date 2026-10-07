//! Go sim/core/aura_helpers_test.go and proc_filter_test.go, and the stat math of the helpers.

use super::*;
use crate::prepare::sim::{school_array_index, EnvState, SpellId, Unit, SECOND};
use crate::prepare::spell::{school, SpellConfig};

fn sim_with_unit() -> (Sim, UnitId) {
    let mut sim = Sim::new();
    let mut unit = Unit::new(UnitType::Player, "player".to_string());
    unit.reaction_time = 1;
    let id = sim.add_unit(unit);
    sim.unit_mut(id)
        .sdm
        .add_stat_dependency(Stat::Stamina, Stat::Health, 10.0);
    (sim, id)
}

/// Finalizes the way `Unit.finalize` leaves the stats: dependencies sorted and applied.
fn finalize(sim: &mut Sim, unit: UnitId) {
    let u = sim.unit_mut(unit);
    u.sdm.finalize_stat_deps();
    u.stats_without_deps = u.stats;
    u.stats = u
        .sdm
        .apply_stat_dependencies(u.stats_without_deps)
        .floor_game_stats();
    sim.update_cast_speed(unit);
    sim.update_attack_speed(unit);
    sim.state = EnvState::Finalized;
}

fn aura_config(label: &str, duration: Duration) -> AuraConfig {
    AuraConfig {
        label: label.to_string(),
        duration,
        ..AuraConfig::default()
    }
}

fn spell_with(sim: &mut Sim, unit: UnitId, flags: SpellFlag, mask: ProcMask) -> SpellId {
    let id = sim.spells.len() as i32 + 1;
    sim.register_spell(
        unit,
        SpellConfig {
            action_id: ActionId::spell(id),
            flags,
            proc_mask: mask,
            spell_school: school::PHYSICAL,
            ..SpellConfig::default()
        },
    )
}

fn flags(family: i32, first: u32) -> ClassFlags {
    ClassFlags {
        family,
        mask: [first, 0, 0, 0],
    }
}

#[test]
fn a_trigger_naming_class_flags_fires_on_exactly_those_spells() {
    let (mut sim, unit) = sim_with_unit();
    let config = ProcTrigger {
        class_flags: flags(8, 0b0011),
        ..ProcTrigger::default()
    };
    for (name, spell_flags, want) in [
        ("overlapping mask", flags(8, 0b0010), true),
        ("disjoint mask in the same family", flags(8, 0b0100), false),
        ("same mask in another family", flags(3, 0b0011), false),
        ("no class flags at all", ClassFlags::default(), false),
    ] {
        let spell = spell_with(&mut sim, unit, SpellFlag::NONE, ProcMask::UNKNOWN);
        sim.spell_mut(spell).class_flags = spell_flags;
        assert_eq!(config.matches_spell(sim.spell(spell)), want, "{name}");
    }
}

#[test]
fn a_trigger_with_both_masks_requires_both() {
    let (mut sim, unit) = sim_with_unit();
    let config = ProcTrigger {
        class_spell_mask: 1 << 2,
        class_flags: flags(8, 0b0001),
        ..ProcTrigger::default()
    };
    for (name, mask, spell_flags, want) in [
        ("both", 1 << 2, flags(8, 0b0001), true),
        ("only the sim tag", 1 << 2, ClassFlags::default(), false),
        ("only the class flags", 0, flags(8, 0b0001), false),
    ] {
        let spell = spell_with(&mut sim, unit, SpellFlag::NONE, ProcMask::UNKNOWN);
        sim.spell_mut(spell).class_spell_mask = mask;
        sim.spell_mut(spell).class_flags = spell_flags;
        assert_eq!(config.matches_spell(sim.spell(spell)), want, "{name}");
    }
}

#[test]
fn only_proc_from_class_abilities_asks_whether_either_mask_names_the_spell() {
    let (mut sim, unit) = sim_with_unit();
    let config = ProcTrigger {
        class_spells_only: true,
        ..ProcTrigger::default()
    };
    for (name, mask, spell_flags, want) in [
        ("tagged by the sim", 1 << 5, ClassFlags::default(), true),
        ("named by the client's class flags", 0, flags(8, 1), true),
        ("neither", 0, ClassFlags::default(), false),
    ] {
        let spell = spell_with(&mut sim, unit, SpellFlag::NONE, ProcMask::UNKNOWN);
        sim.spell_mut(spell).class_spell_mask = mask;
        sim.spell_mut(spell).class_flags = spell_flags;
        assert_eq!(config.matches_spell(sim.spell(spell)), want, "{name}");
    }
}

/// The game's proc eligibility rules, as `AttachProcTriggerCallback` applies them before any
/// mask or outcome check. Rows are the hitting spell, columns the listener.
#[test]
fn proc_trigger_eligibility() {
    let triggers = [
        ("auto attack", SpellFlag::NONE),
        ("ability", SpellFlag::NONE),
        ("proc", SpellFlag::PROC),
        (
            "proc suppressing weapon procs",
            SpellFlag::PROC | SpellFlag::SUPPRESS_WEAPON_PROCS,
        ),
        (
            "ability suppressing weapon procs",
            SpellFlag::SUPPRESS_WEAPON_PROCS,
        ),
    ];
    let listeners: [(&str, ProcTrigger, [bool; 5]); 4] = [
        (
            "aura",
            ProcTrigger::default(),
            [true, true, false, false, true],
        ),
        (
            "aura that can proc from procs",
            ProcTrigger {
                can_proc_from_procs: true,
                ..ProcTrigger::default()
            },
            [true; 5],
        ),
        (
            "weapon proc",
            ProcTrigger {
                is_weapon_proc: true,
                ..ProcTrigger::default()
            },
            [true, true, true, false, false],
        ),
        (
            "aura marked as weapon proc",
            ProcTrigger {
                spell_flags_exclude: SpellFlag::SUPPRESS_WEAPON_PROCS,
                ..ProcTrigger::default()
            },
            [true, true, false, false, false],
        ),
    ];
    let (mut sim, unit) = sim_with_unit();
    for (listener, config, want) in &listeners {
        for (i, (trigger, trigger_flags)) in triggers.iter().enumerate() {
            let spell = spell_with(&mut sim, unit, *trigger_flags, ProcMask::MELEE_MH_SPECIAL);
            let config = ProcTrigger {
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                outcome: HitOutcome::LANDED,
                trigger_immediately: true,
                ..config.clone()
            };
            assert_eq!(
                config.matches_spell(sim.spell(spell)),
                want[i],
                "{listener} hit by {trigger}"
            );
        }
    }
}

#[test]
fn proc_masks_and_exclusions_filter_the_spell() {
    let (mut sim, unit) = sim_with_unit();
    let melee = spell_with(&mut sim, unit, SpellFlag::NONE, ProcMask::MELEE_MH_AUTO);
    let ranged = spell_with(&mut sim, unit, SpellFlag::NONE, ProcMask::RANGED_AUTO);
    let config = ProcTrigger {
        proc_mask: ProcMask::MELEE,
        ..ProcTrigger::default()
    };
    assert!(config.matches_spell(sim.spell(melee)));
    assert!(!config.matches_spell(sim.spell(ranged)));
    let config = ProcTrigger {
        proc_mask_exclude: ProcMask::MELEE_MH_AUTO,
        ..ProcTrigger::default()
    };
    assert!(!config.matches_spell(sim.spell(melee)));
    assert!(config.matches_spell(sim.spell(ranged)));
    let config = ProcTrigger {
        spell_flags: SpellFlag::APL,
        ..ProcTrigger::default()
    };
    assert!(!config.matches_spell(sim.spell(melee)));
}

#[test]
fn registering_a_proc_trigger_sets_the_callbacks_icd_and_dpm() {
    let (mut sim, unit) = sim_with_unit();
    let timers_before = sim.unit(unit).timers.len();
    let dpm = Rc::new(DynamicProcManager::default());
    let id = sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: "Trigger".to_string(),
            action_id: ActionId::spell(7),
            metrics_action_id: ActionId::spell(8),
            callback: CallbackMask::ON_SPELL_HIT_DEALT
                | CallbackMask::ON_CAST_COMPLETE
                | CallbackMask::ON_PERIODIC_DAMAGE_TAKEN,
            icd: 3 * SECOND,
            dpm: Some(Rc::clone(&dpm)),
            ..ProcTrigger::default()
        },
    );
    let aura = sim.aura(id);
    assert_eq!(aura.label, "Trigger");
    assert_eq!(aura.action_id_for_proc, Some(ActionId::spell(7)));
    assert_eq!(aura.action_id, Some(ActionId::spell(8)));
    // No duration: permanent, and a reset activates it.
    assert_eq!(aura.duration, NEVER_EXPIRES);
    assert!(aura.on_reset.is_some());
    assert!(aura.events.on_spell_hit_dealt);
    assert!(aura.events.on_cast_complete);
    assert!(aura.events.on_periodic_damage_taken);
    assert!(!aura.events.on_spell_hit_taken);
    assert!(!aura.events.on_apply_effects);
    assert!(aura
        .dpm
        .as_ref()
        .is_some_and(|aura_dpm| Rc::ptr_eq(aura_dpm, &dpm)));
    let icd = aura.icd.expect("an ICD");
    assert_eq!(icd.duration, 3 * SECOND);
    assert_eq!(sim.unit(unit).timers.len(), timers_before + 1);
    assert_eq!(icd.timer, sim.unit(unit).timers.last().copied());
    assert_eq!(
        aura.callback_names(),
        vec![
            "on_reset",
            "on_cast_complete",
            "on_spell_hit_dealt",
            "on_periodic_damage_taken"
        ]
    );

    // Without an ICD no timer is created, and a duration keeps the aura temporary.
    let timers_before = sim.unit(unit).timers.len();
    let id = sim.make_proc_trigger_aura(
        unit,
        &ProcTrigger {
            name: "Temporary".to_string(),
            duration: 10 * SECOND,
            callback: CallbackMask::ON_APPLY_EFFECTS | CallbackMask::ON_HEAL_DEALT,
            ..ProcTrigger::default()
        },
    );
    let aura = sim.aura(id);
    assert_eq!(aura.duration, 10 * SECOND);
    assert!(aura.on_reset.is_none());
    assert!(aura.icd.is_none());
    assert!(aura.dpm.is_none());
    assert_eq!(sim.unit(unit).timers.len(), timers_before);
    assert!(aura.events.on_apply_effects && aura.events.on_heal_dealt);
}

#[test]
fn attaching_a_proc_trigger_to_a_parent_aura_keeps_the_parent() {
    let (mut sim, unit) = sim_with_unit();
    let parent = sim.register_aura(unit, aura_config("Parent", 10 * SECOND));
    let returned = sim.attach_proc_trigger(
        parent,
        &ProcTrigger {
            callback: CallbackMask::ON_SPELL_HIT_TAKEN,
            icd: SECOND,
            ..ProcTrigger::default()
        },
    );
    assert_eq!(returned, parent);
    assert!(sim.aura(parent).events.on_spell_hit_taken);
    assert!(sim.aura(parent).icd.is_some());
    assert!(sim.aura(parent).on_reset.is_none());

    let dependent = sim.make_dependent_proc_trigger_aura(
        parent,
        unit,
        &ProcTrigger {
            name: "Dependent".to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            ..ProcTrigger::default()
        },
    );
    assert!(sim.aura(dependent).events.on_spell_hit_dealt);
}

#[test]
fn the_callback_and_outcome_bits_are_gos() {
    assert_eq!(CallbackMask::ON_SPELL_HIT_DEALT.0, 2);
    assert_eq!(CallbackMask::ON_PERIODIC_DAMAGE_TAKEN.0, 256);
    assert_eq!(HitOutcome::MISS.0, 2);
    assert_eq!(HitOutcome::LANDED.0, 4 | 16 | 64 | 128 | 256);
    assert!(HitOutcome::LANDED.matches(HitOutcome::CRIT));
    assert!(!HitOutcome::LANDED.matches(HitOutcome::DODGE));
}

#[test]
fn stat_buffs_use_the_build_phase_math_before_finalization_and_the_dependencies_after() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Buff", 10 * SECOND));
    let mut buff = Stats::default();
    buff[Stat::Stamina] = 10.5;
    buff[Stat::SpellCritPercent] = 3.0;
    sim.attach_stats_buff(aura, buff);
    sim.unit_mut(unit).stats[Stat::Stamina] = 100.5;
    // During a build phase the buff adds as it is, with no dependencies and no flooring.
    sim.measuring_stats = true;
    sim.activate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 111.0);
    assert_eq!(sim.stat(unit, Stat::Health), 0.0);
    sim.deactivate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 100.5);
    sim.measuring_stats = false;

    finalize(&mut sim, unit);
    assert_eq!(sim.stat(unit, Stat::Stamina), 100.0);
    assert_eq!(sim.stat(unit, Stat::Health), 1000.0);
    // After finalization the total is recomputed: stamina floors and health follows it.
    sim.activate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 111.0);
    assert_eq!(sim.stat(unit, Stat::Health), 1110.0);
    assert_eq!(sim.stat(unit, Stat::SpellCritPercent), 3.0);
    sim.deactivate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 100.0);
    assert_eq!(sim.stat(unit, Stat::Health), 1000.0);
    assert_eq!(sim.stat(unit, Stat::SpellCritPercent), 0.0);
}

#[test]
fn attaching_a_stat_buff_to_an_active_aura_adds_it_at_once() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Active", 10 * SECOND));
    sim.activate(aura);
    sim.attach_stat_buff(aura, Stat::Strength, 5.0);
    assert_eq!(sim.stat(unit, Stat::Strength), 5.0);
    assert!(sim.aura(aura).on_gain.is_some() && sim.aura(aura).on_expire.is_some());
}

#[test]
fn stat_dependency_buffs_turn_on_with_the_aura() {
    let (mut sim, unit) = sim_with_unit();
    let dep = sim.new_dynamic_multiply_stat(unit, Stat::Stamina, 2.0);
    sim.unit_mut(unit).stats[Stat::Stamina] = 100.0;
    let aura = sim.register_aura(unit, aura_config("Dependency", 10 * SECOND));
    sim.attach_stat_dependency(aura, dep);
    finalize(&mut sim, unit);
    assert_eq!(sim.stat(unit, Stat::Health), 1000.0);
    sim.activate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 200.0);
    assert_eq!(sim.stat(unit, Stat::Health), 2000.0);
    sim.deactivate(aura);
    assert_eq!(sim.stat(unit, Stat::Stamina), 100.0);
}

#[test]
fn stacking_auras_add_the_bonus_per_stack_change() {
    let (mut sim, unit) = sim_with_unit();
    let mut bonus = Stats::default();
    bonus[Stat::Strength] = 7.0;
    bonus[Stat::Agility] = -1.0;
    let stacking = sim.make_stacking_aura(
        unit,
        StackingStatAura {
            aura: AuraConfig {
                max_stacks: 5,
                ..aura_config("Stacking", 20 * SECOND)
            },
            bonus_per_stack: bonus,
        },
    );
    assert_eq!(stacking.buffed_stat_types, vec![Stat::Strength]);
    finalize(&mut sim, unit);
    sim.activate(stacking.aura);
    sim.set_stacks(stacking.aura, 3);
    assert_eq!(sim.stat(unit, Stat::Strength), 21.0);
    assert_eq!(sim.stat(unit, Stat::Agility), -3.0);
    sim.set_stacks(stacking.aura, 1);
    assert_eq!(sim.stat(unit, Stat::Strength), 7.0);
    sim.deactivate(stacking.aura);
    assert_eq!(sim.stat(unit, Stat::Strength), 0.0);
    assert_eq!(sim.stat(unit, Stat::Agility), 0.0);
}

#[test]
fn temporary_stats_auras_buff_and_tell_the_listeners() {
    let (mut sim, unit) = sim_with_unit();
    let heard: Rc<std::cell::RefCell<Vec<Stats>>> = Rc::default();
    let listener_heard = Rc::clone(&heard);
    sim.unit_mut(unit).on_temporary_stats_changes.push(Rc::new(
        move |_: &mut Sim, _: AuraId, change: &Stats| {
            listener_heard.borrow_mut().push(*change);
        },
    ));
    let mut buffs = Stats::default();
    buffs[Stat::Strength] = 20.0;
    buffs[Stat::Health] = 100.0;
    let aura =
        sim.new_temporary_stats_aura(unit, "Temporary", &ActionId::item(5), buffs, 10 * SECOND);
    assert_eq!(aura.buffed_stat_types, vec![Stat::Strength, Stat::Health]);
    // A health bonus goes through the max health update, not the stats the listeners hear.
    finalize(&mut sim, unit);
    sim.activate(aura.aura);
    assert_eq!(sim.stat(unit, Stat::Strength), 20.0);
    assert_eq!(sim.stat(unit, Stat::Health), 100.0);
    sim.deactivate(aura.aura);
    assert_eq!(sim.stat(unit, Stat::Strength), 0.0);
    assert_eq!(sim.stat(unit, Stat::Health), 0.0);
    let heard = heard.borrow();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[0][Stat::Strength], 20.0);
    assert_eq!(heard[0][Stat::Health], 0.0);
    assert_eq!(heard[1][Stat::Strength], -20.0);
    assert_eq!(sim.aura(aura.aura).action_id, Some(ActionId::item(5)));
}

#[test]
fn a_wrapped_temporary_stats_aura_lets_the_caller_edit_the_config() {
    let (mut sim, unit) = sim_with_unit();
    let mut buffs = Stats::default();
    buffs[Stat::Strength] = 1.0;
    let aura = sim.new_temporary_stats_aura_wrapped(
        unit,
        "Wrapped",
        &ActionId::spell(9),
        buffs,
        SECOND,
        Some(&|config: &mut AuraConfig| {
            config.max_stacks = 4;
            config.tag = "tag".to_string();
        }),
    );
    assert_eq!(sim.aura(aura.aura).max_stacks, 4);
    assert_eq!(sim.aura(aura.aura).tag, "tag");
}

#[test]
fn a_stat_multiplier_aura_toggles_its_dependencies_and_reports_the_change() {
    let (mut sim, unit) = sim_with_unit();
    sim.unit_mut(unit).stats[Stat::AttackPower] = 1000.0;
    let heard: Rc<std::cell::RefCell<Vec<Stats>>> = Rc::default();
    let listener_heard = Rc::clone(&heard);
    sim.unit_mut(unit).on_temporary_stats_changes.push(Rc::new(
        move |_: &mut Sim, _: AuraId, change: &Stats| {
            listener_heard.borrow_mut().push(*change);
        },
    ));
    let aura = sim.new_temporary_stat_multiplier_aura(
        unit,
        aura_config("Multiplier", 15 * SECOND),
        &[StatMultiplier {
            stat: Stat::AttackPower,
            multiplier: 1.1,
        }],
    );
    assert_eq!(aura.buffed_stat_types, vec![Stat::AttackPower]);
    finalize(&mut sim, unit);
    sim.activate(aura.aura);
    assert_eq!(sim.stat(unit, Stat::AttackPower), 1000.0 * 1.1);
    sim.deactivate(aura.aura);
    assert_eq!(sim.stat(unit, Stat::AttackPower), 1000.0);
    let heard = heard.borrow();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[0][Stat::AttackPower], 1000.0 * 1.1 - 1000.0);
    assert_eq!(heard[1][Stat::AttackPower], 1000.0 - 1000.0 * 1.1);
}

#[test]
fn a_stat_buff_aura_infers_its_cooldown_type_and_checks_its_condition() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Aura", SECOND));
    let mut buff = StatBuffAura::new(aura, vec![Stat::Strength]);
    assert_eq!(buff.infer_cd_type(), cooldown_type::DPS);
    buff.buffed_stat_types.push(Stat::Armor);
    assert_eq!(buff.infer_cd_type(), cooldown_type::SURVIVAL);
    assert!(buff.buffs_matching_stat(&[Stat::Armor, Stat::Spirit]));
    assert!(!buff.buffs_matching_stat(&[Stat::Spirit]));
    assert!(buff.can_proc(&sim));
    buff.custom_proc_condition = Some(Rc::new(|sim: &Sim, aura: AuraId| sim.aura(aura).active));
    assert!(!buff.can_proc(&sim));
    sim.activate(aura);
    assert!(buff.can_proc(&sim));
    buff.is_swapped = true;
    assert!(!buff.can_proc(&sim));
}

#[test]
fn buffed_stat_types_are_the_positive_stats_in_order() {
    let mut stats = Stats::default();
    stats[Stat::Spirit] = 1.0;
    stats[Stat::Strength] = 2.0;
    stats[Stat::Agility] = -3.0;
    assert_eq!(
        buffed_stat_types(&stats),
        vec![Stat::Strength, Stat::Spirit]
    );
}

#[test]
fn temporary_buffs_with_stacks_build_the_window_only_when_stacks_come_in_time_or_events() {
    let (mut sim, unit) = sim_with_unit();
    let mut bonus = Stats::default();
    bonus[Stat::Strength] = 2.0;
    let mut config = TemporaryStatBuffWithStacksConfig {
        stacking_aura_label: "Stacks".to_string(),
        aura_label: "Window".to_string(),
        action_id: ActionId::spell(3),
        bonus_per_stack: bonus,
        max_stacks: 4,
        duration: 12 * SECOND,
        ..TemporaryStatBuffWithStacksConfig::default()
    };
    let (stacking, window) = sim.new_temporary_stat_buff_with_stacks(unit, &config);
    assert!(window.is_none());
    assert_eq!(sim.aura(stacking.aura).label, "Stacks");
    assert_eq!(sim.aura(stacking.aura).action_id, Some(ActionId::spell(3)));
    assert_eq!(sim.aura(stacking.aura).duration, 12 * SECOND);
    assert_eq!(sim.aura(stacking.aura).max_stacks, 4);

    config.stacking_aura_label = "Event stacks".to_string();
    config.stacking_aura_action_id = ActionId::spell(4);
    config.aura_label = "Event window".to_string();
    config.stacks_from_event = true;
    let (stacking, window) = sim.new_temporary_stat_buff_with_stacks(unit, &config);
    let window = window.expect("a window aura");
    assert_eq!(sim.aura(stacking.aura).duration, NEVER_EXPIRES);
    assert_eq!(sim.aura(stacking.aura).action_id, Some(ActionId::spell(4)));
    assert_eq!(sim.aura(window).duration, 12 * SECOND);
    assert!(sim.aura(window).on_gain.is_some() && sim.aura(window).on_expire.is_some());

    // The window owns the stacking aura's lifetime.
    sim.activate(window);
    assert!(sim.aura(stacking.aura).active);
    sim.deactivate(window);
    assert!(!sim.aura(stacking.aura).active);

    config.stacks_from_event = false;
    config.time_per_stack = SECOND;
    config.stacking_aura_label = "Timed stacks".to_string();
    config.aura_label = "Timed window".to_string();
    let (stacking, window) = sim.new_temporary_stat_buff_with_stacks(unit, &config);
    assert_eq!(sim.aura(stacking.aura).label, "Timed stacks");
    assert!(window.is_some());

    // Without a stacking label the child takes the window's label, as in Go, where a window
    // aura with the same label panics.
    config.stacking_aura_label.clear();
    config.aura_label = "Same label".to_string();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sim.new_temporary_stat_buff_with_stacks(unit, &config);
    }));
    assert!(result.is_err());
}

#[test]
fn block_prepull_records_the_encounter_start_callback() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Blocked", SECOND));
    assert_eq!(sim.block_prepull(aura), aura);
    assert!(sim.aura(aura).events.on_encounter_start);
    assert_eq!(sim.aura(aura).callback_names(), vec!["on_encounter_start"]);
}

#[test]
fn make_permanent_restores_the_duration_runs_the_old_reset_then_activates() {
    let (mut sim, unit) = sim_with_unit();
    let order: Rc<std::cell::RefCell<Vec<&'static str>>> = Rc::default();
    let old_order = Rc::clone(&order);
    let aura = sim.register_aura(
        unit,
        AuraConfig {
            on_reset: Some(Rc::new(move |sim: &mut Sim, aura: AuraId| {
                old_order.borrow_mut().push("old");
                assert_eq!(sim.aura(aura).duration, NEVER_EXPIRES);
                assert!(!sim.aura(aura).active);
            })),
            ..aura_config("Permanent", SECOND)
        },
    );
    assert_eq!(sim.make_permanent(aura), aura);
    assert_eq!(sim.aura(aura).duration, NEVER_EXPIRES);
    sim.aura_mut(aura).duration = SECOND;
    sim.reset_aura(aura);
    assert_eq!(*order.borrow(), vec!["old"]);
    assert!(sim.aura(aura).active);
    assert_eq!(sim.aura(aura).duration, NEVER_EXPIRES);
}

#[test]
fn pseudo_stat_buffs_scale_and_restore_the_named_field() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Pseudo", 5 * SECOND));
    sim.attach_multiplicative_pseudo_stat_buff(
        aura,
        PseudoStatField::SchoolDamageTakenMultiplier(SchoolIndex::Fire),
        0.85,
    );
    sim.attach_additive_pseudo_stat_buff(aura, PseudoStatField::BaseDodgeChance, 0.02);
    sim.attach_multiplicative_pseudo_stat_buff(
        aura,
        PseudoStatField::Custom(|stats| &mut stats.armor_multiplier),
        2.0,
    );
    let fire = school_array_index(SchoolIndex::Fire);
    let pseudo = |sim: &Sim| sim.unit(unit).pseudo_stats.clone();
    sim.activate(aura);
    assert_eq!(pseudo(&sim).school_damage_taken_multiplier[fire], 0.85);
    assert_eq!(
        pseudo(&sim).school_damage_taken_multiplier[school_array_index(SchoolIndex::Frost)],
        1.0
    );
    assert_eq!(pseudo(&sim).base_dodge_chance, 0.02);
    assert_eq!(pseudo(&sim).armor_multiplier, 2.0);
    sim.deactivate(aura);
    assert_eq!(
        pseudo(&sim).school_damage_taken_multiplier[fire],
        1.0 * 0.85 / 0.85
    );
    assert_eq!(pseudo(&sim).base_dodge_chance, 0.02 - 0.02);
    assert_eq!(pseudo(&sim).armor_multiplier, 1.0);

    // A buff attached to an active aura applies at once.
    let active = sim.register_aura(unit, aura_config("Active pseudo", SECOND));
    sim.activate(active);
    sim.attach_multiplicative_pseudo_stat_buff(active, PseudoStatField::DamageDealtMultiplier, 1.5);
    sim.attach_additive_pseudo_stat_buff(active, PseudoStatField::BonusHealingTaken, 3.0);
    assert_eq!(pseudo(&sim).damage_dealt_multiplier, 1.5);
    assert_eq!(pseudo(&sim).bonus_healing_taken, 3.0);
}

#[test]
fn reduced_crit_taken_buffs_update_the_base_and_the_total() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Crit", 5 * SECOND));
    sim.attach_reduced_crit_taken_percent_buff(aura, 3.0);
    sim.activate(aura);
    let pseudo = sim.unit(unit).pseudo_stats.clone();
    assert_eq!(pseudo.base_reduced_crit_taken_percent, 3.0);
    assert_eq!(pseudo.reduced_crit_taken_percent, 3.0);
    sim.deactivate(aura);
    assert_eq!(sim.unit(unit).pseudo_stats.reduced_crit_taken_percent, 0.0);
}

#[test]
fn speed_buffs_multiply_on_gain_and_divide_on_expire() {
    let (mut sim, unit) = sim_with_unit();
    let cast = sim.register_aura(unit, aura_config("Cast", 5 * SECOND));
    sim.attach_multiply_cast_speed(cast, 1.25);
    let attack = sim.register_aura(unit, aura_config("Attack", 5 * SECOND));
    sim.attach_multiply_attack_speed(attack, 1.1);
    let melee = sim.register_aura(unit, aura_config("Melee", 5 * SECOND));
    sim.attach_multiply_melee_speed(melee, 1.2);
    let ranged = sim.register_aura(unit, aura_config("Ranged", 5 * SECOND));
    sim.attach_multiply_ranged_speed(ranged, 1.3);
    let haste = sim.register_aura(unit, aura_config("Haste", 5 * SECOND));
    sim.attach_multiply_ranged_haste(haste, 1.4);
    finalize(&mut sim, unit);

    sim.activate(cast);
    assert_eq!(sim.unit(unit).pseudo_stats.cast_speed_multiplier, 1.25);
    assert_eq!(sim.unit(unit).cast_speed, 1.0 / 1.25);
    sim.deactivate(cast);
    assert_eq!(
        sim.unit(unit).pseudo_stats.cast_speed_multiplier,
        1.25 * (1.0 / 1.25)
    );

    sim.activate(attack);
    assert_eq!(sim.unit(unit).melee_attack_speed, 1.1);
    assert_eq!(sim.unit(unit).ranged_attack_speed, 1.1);
    assert_eq!(sim.unit(unit).melee_and_ranged_haste, 1.1);
    sim.deactivate(attack);

    sim.activate(melee);
    assert_eq!(sim.unit(unit).pseudo_stats.melee_speed_multiplier, 1.2);
    assert_eq!(sim.unit(unit).melee_attack_speed, 1.2);
    assert_eq!(sim.unit(unit).ranged_attack_speed, 1.0 * 1.1 * (1.0 / 1.1));
    sim.deactivate(melee);

    sim.activate(ranged);
    assert_eq!(sim.unit(unit).pseudo_stats.ranged_speed_multiplier, 1.3);
    sim.deactivate(ranged);

    sim.activate(haste);
    assert_eq!(sim.unit(unit).pseudo_stats.ranged_haste_multiplier, 1.4);
    assert_eq!(sim.unit(unit).pseudo_stats.ranged_speed_multiplier, 1.4);
    sim.deactivate(haste);
    assert_eq!(
        sim.unit(unit).pseudo_stats.ranged_haste_multiplier,
        1.4 * (1.0 / 1.4)
    );
}

#[test]
fn damage_done_by_caster_slots_are_set_while_the_aura_lasts() {
    let (mut sim, attacker) = sim_with_unit();
    let mut enemy = Unit::new(UnitType::Enemy, "enemy".to_string());
    enemy.reaction_time = 1;
    let defender = sim.add_unit(enemy);
    let aura = sim.register_aura(defender, aura_config("Debuff", 5 * SECOND));
    sim.attach_ddbc(aura, 1, 3, attacker);
    sim.state = EnvState::Finalized;
    assert_eq!(sim.damage_done_by_caster_handlers(attacker, defender), 0);
    sim.activate(aura);
    assert_eq!(sim.damage_done_by_caster_handlers(attacker, defender), 1);
    assert_eq!(sim.damage_done_by_caster[&(attacker, defender)].len(), 3);
    sim.deactivate(aura);
    assert_eq!(sim.damage_done_by_caster_handlers(attacker, defender), 0);
}

#[test]
fn a_periodic_action_cannot_be_attached_to_an_active_aura() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Periodic", 5 * SECOND));
    sim.attach_periodic_action(
        aura,
        PeriodicActionOptions {
            period: SECOND,
            ..PeriodicActionOptions::default()
        },
    );
    assert_eq!(
        sim.aura(aura).callback_names(),
        vec!["on_gain", "on_expire"]
    );
    sim.activate(aura);
    sim.deactivate(aura);
    let active = sim.register_aura(unit, aura_config("Active", 5 * SECOND));
    sim.activate(active);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        sim.attach_periodic_action(active, PeriodicActionOptions::default());
    }));
    assert!(result.is_err());
}

#[test]
fn a_fixed_uptime_aura_registers_a_reset_effect() {
    let (mut sim, unit) = sim_with_unit();
    let aura = sim.register_aura(unit, aura_config("Uptime", 5 * SECOND));
    assert!(sim.unit(unit).reset_effects.is_empty());
    sim.apply_fixed_uptime_aura(aura, 0.5, SECOND, 2 * SECOND);
    assert_eq!(sim.unit(unit).reset_effects.len(), 1);
}

#[test]
fn a_damage_absorption_aura_sizes_its_shield_and_stacks_on_activation() {
    let (mut sim, unit) = sim_with_unit();
    let absorption = sim.new_damage_absorption_aura(
        unit,
        AbsorptionAuraConfig {
            aura: aura_config("Shield", 0),
            shield_strength_calculator: Some(Rc::new(|_: &Sim, _: UnitId| 250.7)),
            ..AbsorptionAuraConfig::default()
        },
    );
    assert_eq!(sim.aura(absorption.aura).duration, NEVER_EXPIRES);
    assert_eq!(sim.unit(unit).dynamic_damage_taken_modifiers, 1);
    absorption.activate(&mut sim);
    assert!(sim.aura(absorption.aura).active);
    assert_eq!(absorption.shield_strength(), 250.7);
    assert_eq!(sim.aura(absorption.aura).max_stacks, 250);
    assert_eq!(sim.aura(absorption.aura).stacks, 250);
    sim.deactivate(absorption.aura);
    assert_eq!(absorption.shield_strength(), 0.0);

    let weak = sim.new_damage_absorption_aura(
        unit,
        AbsorptionAuraConfig {
            aura: aura_config("Weak shield", SECOND),
            shield_strength_calculator: Some(Rc::new(|_: &Sim, _: UnitId| 0.2)),
            ..AbsorptionAuraConfig::default()
        },
    );
    weak.activate(&mut sim);
    assert_eq!(sim.aura(weak.aura).stacks, 1);
}

#[test]
fn ally_absorption_arrays_cover_every_unit_that_is_not_an_enemy() {
    let (mut sim, unit) = sim_with_unit();
    let mut enemy = Unit::new(UnitType::Enemy, "enemy".to_string());
    enemy.reaction_time = 1;
    enemy.unit_index = 0;
    let enemy = sim.add_unit(enemy);
    sim.unit_mut(unit).unit_index = 1;
    sim.env_units = vec![enemy, unit];
    let array = sim.new_ally_damage_absorption_aura_array(|sim: &mut Sim, target: UnitId| {
        sim.new_damage_absorption_aura(
            target,
            AbsorptionAuraConfig {
                aura: aura_config("Ally shield", SECOND),
                shield_strength_calculator: Some(Rc::new(|_: &Sim, _: UnitId| 10.0)),
                ..AbsorptionAuraConfig::default()
            },
        )
    });
    assert!(!array.is_empty());
    assert!(array.get(&sim, enemy).is_none());
    assert!(array.get(&sim, unit).is_some());
    assert_eq!(array.find_label(&sim), "Ally shield");
    assert!(DamageAbsorptionAuraArray::default().is_empty());
}
