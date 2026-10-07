//! Go sim/core/racials_test.go, disable_racials_test.go and the weapon specialization checks of
//! racial_test.go, on a warrior built through the same construction Go's tests use.

use super::*;
use crate::contracts::request::{Message, Request};
use crate::prepare::agent::{ClassSpellName, PrepAgent};
use crate::prepare::sim::{school_array_index, RageBar, SpellId};

struct FakeAgent {
    talents: Message,
}

impl PrepAgent for FakeAgent {
    fn talents(&self) -> &Message {
        &self.talents
    }
    fn class_spells(&self) -> &'static [ClassSpellName] {
        &[]
    }
}

/// A warrior with a rage bar and two spells for Eureka! to act on: an ability the class tags
/// and a spell it does not.
fn factory(sim: &mut Sim, unit: UnitId, _player: &Message) -> Result<Box<dyn PrepAgent>, Refusal> {
    sim.unit_mut(unit).rage_bar = RageBar {
        enabled: true,
        max_rage: 100.0,
        ..RageBar::default()
    };
    for (id, flags, class_spell_mask) in [(1, SpellFlag::NONE, 1), (2, SpellFlag::APL, 0)] {
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: ActionId::spell(id),
                class_spell_mask,
                proc_mask: ProcMask::MELEE_MH_SPECIAL,
                spell_school: school::PHYSICAL,
                flags,
                damage_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
    }
    Ok(Box::new(FakeAgent {
        talents: Message::empty("proto.WarriorTalents"),
    }))
}

fn environment(race: &str, player: &str, mob_type: &str) -> Environment {
    let request = format!(
        r#"{{"simOptions":{{"iterations":1,"randomSeed":"100"}},
        "raid":{{"parties":[{{"players":[{{"name":"Warrior","class":"ClassWarrior",
          "race":"{race}","buffs":{{}},"consumables":{{}}{player}}}]}}]}},
        "encounter":{{"duration":180,"targets":[{{"level":63,"mobType":"{mob_type}"}}]}}}}"#
    );
    let request = Request::from_json(request.as_bytes()).expect("a valid request");
    match Environment::new(request.message(), factory) {
        Ok(env) => env,
        Err(refusal) => panic!("{refusal}"),
    }
}

fn warrior(race: &str, player: &str) -> Environment {
    environment(race, player, "MobTypeElemental")
}

/// The equipment of one main hand weapon: Worn Mace (36), Worn Shortsword (25) and Worn Axe (37)
/// have no effects. Go equips by the item's own slot, so a second main hand weapon replaces the
/// first.
fn weapon(id: i32) -> String {
    let empty = "{},".repeat(14);
    format!(r#","equipment":{{"items":[{empty}{{"id":{id}}}]}}"#)
}

fn stat(env: &Environment, stat: Stat) -> f64 {
    env.sim.stat(env.player, stat)
}

fn aura(env: &Environment, label: &str) -> Option<AuraId> {
    env.sim.get_aura(env.player, label)
}

fn spell(env: &Environment, id: i32) -> Option<SpellId> {
    env.sim.get_spell(env.player, &ActionId::spell(id))
}

fn physical_taken(env: &Environment) -> f64 {
    env.sim
        .unit(env.player)
        .pseudo_stats
        .school_damage_taken_multiplier[school_array_index(SchoolIndex::Physical)]
}

#[test]
fn disabling_racials_drops_them_but_keeps_the_base_stats() {
    let with = warrior("RaceOrc", "");
    let without = warrior("RaceOrc", r#","disableRacials":true"#);
    assert!(aura(&with, "Blood Fury").is_some());
    assert!(aura(&without, "Blood Fury").is_none());
    assert_eq!(
        with.sim.character(with.player).base_stats,
        without.sim.character(without.player).base_stats
    );

    // The human's spirit racial is a multiplier on top of the base stats, so it goes too.
    let human = warrior("RaceHuman", "");
    let human_without = warrior("RaceHuman", r#","disableRacials":true"#);
    assert!(stat(&human, Stat::Spirit) > stat(&human_without, Stat::Spirit));
    assert_eq!(
        stat(&human_without, Stat::Spirit),
        human_without.sim.character(human_without.player).base_stats[Stat::Spirit]
    );
}

#[test]
fn blood_fury_is_percent_based_and_reports_its_temporary_stats() {
    let mut env = warrior("RaceOrc", "");
    let unit = env.player;
    let blood_fury = aura(&env, "Blood Fury").expect("Blood Fury");
    let heard: Rc<std::cell::RefCell<Vec<Stats>>> = Rc::default();
    let listener_heard = Rc::clone(&heard);
    env.sim
        .unit_mut(unit)
        .on_temporary_stats_changes
        .push(Rc::new(move |_: &mut Sim, aura: AuraId, change: &Stats| {
            assert_eq!(aura, blood_fury);
            listener_heard.borrow_mut().push(*change);
        }));

    let before = env.sim.stats(unit);
    env.sim.activate(blood_fury);
    let during = env.sim.stats(unit);
    env.sim.deactivate(blood_fury);
    let after = env.sim.stats(unit);

    let heard = heard.borrow();
    assert_eq!(heard.len(), 2);
    assert_eq!(heard[0], during.subtract(&before));
    assert_eq!(heard[1], after.subtract(&during));
    assert_eq!(after, before);
    let attack_power = before[Stat::AttackPower];
    assert!(attack_power > 0.0);
    assert!((during[Stat::AttackPower] - attack_power * 1.1).abs() <= 1.0);
    assert!((heard[0][Stat::AttackPower] - attack_power * 0.1).abs() <= 1.0);
    assert_eq!(heard[1][Stat::AttackPower], -heard[0][Stat::AttackPower]);

    let spell_id = spell(&env, 20572).expect("Blood Fury's spell");
    assert_eq!(env.sim.spell(spell_id).related_self_buff, Some(blood_fury));
    let icd = env.sim.aura(blood_fury).icd.expect("an ICD");
    assert_eq!(icd, env.sim.spell(spell_id).cd);
    assert_eq!(icd.duration, 2 * MINUTE);
}

#[test]
fn berserking_is_a_flat_ten_percent_with_no_cost() {
    let mut env = warrior("RaceTroll", "");
    let unit = env.player;
    let before = env.sim.unit(unit).pseudo_stats.attack_speed_multiplier;
    let berserking = aura(&env, "Berserking").expect("Berserking");
    env.sim.activate(berserking);
    assert_eq!(
        env.sim.unit(unit).pseudo_stats.attack_speed_multiplier,
        before * 1.1
    );
    env.sim.deactivate(berserking);
    let spell_id = spell(&env, 20554).expect("Berserking's spell");
    assert!(env.sim.spell(spell_id).cost.is_none());
    let mcds = &env.sim.character(unit).initial_major_cooldowns;
    assert_eq!(mcds.len(), 1);
    assert_eq!(mcds[0].cooldown_type, cooldown_type::DPS);
}

#[test]
fn elunes_light_grants_ten_crit_on_use() {
    let mut env = warrior("RaceNightElf", "");
    let before = stat(&env, Stat::PhysicalCritPercent);
    let elunes_light = aura(&env, "Elune's Light").expect("Elune's Light");
    env.sim.activate(elunes_light);
    assert_eq!(stat(&env, Stat::PhysicalCritPercent), before + 10.0);
    env.sim.deactivate(elunes_light);
    assert_eq!(stat(&env, Stat::PhysicalCritPercent), before);
    let unit = env.player;
    let spell_id = spell(&env, 1259799).expect("Elune's Light's spell");
    assert!(env.sim.spell(spell_id).flags.matches(SpellFlag::MCD));
    assert_eq!(
        env.sim.character(unit).initial_major_cooldowns[0].cooldown_type,
        cooldown_type::DPS
    );
    assert_eq!(
        env.sim.aura(elunes_light).icd.map(|icd| icd.duration),
        Some(3 * MINUTE)
    );
}

#[test]
fn stoneform_takes_ten_percent_of_physical_damage_while_it_lasts() {
    let mut env = warrior("RaceDwarf", "");
    let unit = env.player;
    assert_eq!(physical_taken(&env), 1.0);
    let stoneform = aura(&env, "Stoneform").expect("Stoneform");
    env.sim.activate(stoneform);
    assert_eq!(physical_taken(&env), 0.9);
    env.sim.deactivate(stoneform);
    assert_eq!(physical_taken(&env), 1.0 * 0.9 / 0.9);
    let mcds = &env.sim.character(unit).initial_major_cooldowns;
    assert_eq!(mcds[0].cooldown_type, cooldown_type::SURVIVAL);
    // A spell on the 1.5 sec GCD is not reactive.
    let spell_id = spell(&env, 20594).expect("Stoneform's spell");
    assert!(!env.sim.spell(spell_id).flags.matches(SpellFlag::REACTIVE));
}

#[test]
fn shatter_curse_is_a_survival_cooldown_on_the_six_magic_schools() {
    let mut env = warrior("RaceOrc", "");
    let unit = env.player;
    let shatter_curse = aura(&env, "Shatter Curse").expect("Shatter Curse");
    env.sim.activate(shatter_curse);
    let taken = env
        .sim
        .unit(unit)
        .pseudo_stats
        .school_damage_taken_multiplier;
    for index in [
        SchoolIndex::Arcane,
        SchoolIndex::Fire,
        SchoolIndex::Frost,
        SchoolIndex::Holy,
        SchoolIndex::Nature,
        SchoolIndex::Shadow,
    ] {
        assert_eq!(taken[school_array_index(index)], 0.85);
    }
    assert_eq!(taken[school_array_index(SchoolIndex::Physical)], 1.0);
    let survival = env
        .sim
        .character(unit)
        .initial_major_cooldowns
        .iter()
        .filter(|mcd| mcd.cooldown_type == cooldown_type::SURVIVAL)
        .count();
    assert_eq!(survival, 1);
}

#[test]
fn tauren_endurance_grants_one_percent_hit() {
    let human = warrior("RaceHuman", "");
    let tauren = warrior("RaceTauren", "");
    assert_eq!(
        stat(&tauren, Stat::PhysicalHitPercent),
        stat(&human, Stat::PhysicalHitPercent) + 1.0
    );
    assert_eq!(
        stat(&tauren, Stat::SpellHitPercent),
        stat(&human, Stat::SpellHitPercent) + 1.0
    );
}

#[test]
fn touch_of_the_grave_uses_the_melee_variant() {
    let env = warrior("RaceUndead", "");
    let id = aura(&env, "Touch of the Grave").expect("Touch of the Grave");
    let aura = env.sim.aura(id);
    assert_eq!(
        aura.action_id_for_proc.as_ref().map(|id| id.spell_id),
        Some(1260189)
    );
    assert!(aura.events.on_spell_hit_dealt);
    assert_eq!(aura.icd.map(|icd| icd.duration), Some(SECOND));
    assert!(aura.icd.and_then(|icd| icd.timer).is_some());
    assert_eq!(aura.duration, NEVER_EXPIRES);
    assert!(aura.active);
    assert!(spell(&env, 1260198).is_some());
}

#[test]
fn skyborne_racials_speed_up_swings_and_hit_elementals_harder() {
    let human = warrior("RaceHuman", "");
    let human_pseudo = human.sim.unit(human.player).pseudo_stats.clone();
    for race in ["RaceHighOrderSkyborne", "RaceWindshaperSkyborne"] {
        let env = warrior(race, "");
        let unit = env.player;
        let pseudo = &env.sim.unit(unit).pseudo_stats;
        assert_eq!(
            pseudo.attack_speed_multiplier,
            human_pseudo.attack_speed_multiplier * 1.01
        );
        assert_eq!(
            pseudo.cast_speed_multiplier,
            human_pseudo.cast_speed_multiplier * 1.01
        );
        let table = env.attack_table(unit, env.encounter.targets[0]);
        assert_eq!(table.damage_dealt_multiplier, 1.05);
        assert_eq!(table.crit_multiplier, 1.0);

        let read_ley_line = spell(&env, 1259705);
        let skysight = spell(&env, 1259686);
        assert_eq!(read_ley_line.is_some(), race == "RaceHighOrderSkyborne");
        assert_eq!(skysight.is_some(), race == "RaceWindshaperSkyborne");
        // Client 1.60.1.70058 SpellMisc: Read Ley Line casts in 2 sec, Skysight in 0.5 sec.
        if let Some(id) = read_ley_line {
            assert_eq!(env.sim.spell(id).default_cast.cast_time, 2 * SECOND);
        }
        if let Some(id) = skysight {
            assert_eq!(env.sim.spell(id).default_cast.cast_time, 500 * MILLISECOND);
        }
    }
    // The table of a target that is not an elemental is untouched.
    let troll = environment("RaceHighOrderSkyborne", "", "MobTypeHumanoid");
    let table = troll.attack_table(troll.player, troll.encounter.targets[0]);
    assert_eq!(table.damage_dealt_multiplier, 1.0);
}

#[test]
fn beast_slaying_applies_only_to_beasts() {
    let troll = environment("RaceTroll", "", "MobTypeBeast");
    let table = troll.attack_table(troll.player, troll.encounter.targets[0]);
    assert_eq!(table.damage_dealt_multiplier, 1.05);
    let elemental = warrior("RaceTroll", "");
    let table = elemental.attack_table(elemental.player, elemental.encounter.targets[0]);
    assert_eq!(table.damage_dealt_multiplier, 1.0);
}

#[test]
fn energized_and_elemental_blessing_change_regen_and_movement() {
    let mut env = warrior("RaceWindshaperSkyborne", "");
    let unit = env.player;
    let blessing = aura(&env, "Elemental Blessing").expect("Elemental Blessing");
    let before = env.sim.unit(unit).pseudo_stats.movement_speed_multiplier;
    env.sim.activate(blessing);
    assert_eq!(
        env.sim.unit(unit).pseudo_stats.movement_speed_multiplier,
        before * 1.1
    );
    env.sim.deactivate(blessing);

    let mut env = warrior("RaceHighOrderSkyborne", "");
    let energized = aura(&env, "Energized").expect("Energized");
    // A warrior has no mana bar, so the aura changes nothing; a mana bar doubles the regen.
    env.sim.activate(energized);
    assert_eq!(env.sim.unit(env.player).mana_bar.mana_regen_multiplier, 0.0);
    env.sim.deactivate(energized);
    let unit = env.player;
    env.sim.unit_mut(unit).mana_bar.enabled = true;
    env.sim.unit_mut(unit).mana_bar.mana_regen_multiplier = 1.0;
    env.sim.activate(energized);
    assert_eq!(env.sim.unit(unit).mana_bar.mana_regen_multiplier, 2.0);
    env.sim.deactivate(energized);
    assert_eq!(env.sim.unit(unit).mana_bar.mana_regen_multiplier, 1.0);
}

#[test]
fn expansive_mind_and_eureka_for_a_gnome_warrior() {
    let mut env = warrior("RaceGnome", "");
    let unit = env.player;
    assert_eq!(env.sim.unit(unit).rage_bar.max_rage, 105.0);

    let eureka = aura(&env, "Eureka!").expect("Eureka!");
    env.sim.activate(eureka);
    assert!(env.sim.aura(eureka).active);
    assert_eq!(env.sim.aura(eureka).stacks, 3);

    let ability = spell(&env, 1).expect("the class ability");
    let item_spell = spell(&env, 2).expect("the item spell");
    assert_eq!(env.sim.spell(item_spell).damage_multiplier, 1.0);
    assert!((env.sim.spell(ability).damage_multiplier - 1.1).abs() < 1e-4);

    env.sim.set_stacks(eureka, 0);
    assert!(!env.sim.aura(eureka).active);
    assert!((env.sim.spell(ability).damage_multiplier - 1.0).abs() < 1e-4);
    assert!(env.sim.aura(eureka).events.on_cast_complete);
}

#[test]
fn weapon_specialization_follows_the_weapons_and_the_override() {
    let crit = |env: &Environment| stat(env, Stat::PhysicalCritPercent);
    let spell_crit = |env: &Environment| stat(env, Stat::SpellCritPercent);
    let naked = warrior("RaceHuman", "");

    // A sword raises both crits by the racial's 2%.
    let swordsman = warrior("RaceHuman", &weapon(25));
    assert_eq!(crit(&swordsman) - crit(&naked), 2.0);
    assert_eq!(spell_crit(&swordsman) - spell_crit(&naked), 2.0);
    // No sword, no crit.
    let mace = warrior("RaceHuman", &weapon(36));
    assert_eq!(crit(&mace), crit(&naked));

    // Withholding the specialization drops the crit even with the weapon equipped.
    let withheld = warrior(
        "RaceHuman",
        &format!("{},\"disableWeaponSpecialization\":true", weapon(25)),
    );
    assert_eq!(crit(&withheld), crit(&naked));

    // The orc's axe and the dwarf's mace give 1%.
    let orc = warrior("RaceOrc", "");
    let orc_axe = warrior("RaceOrc", &weapon(37));
    assert_eq!(crit(&orc_axe) - crit(&orc), 1.0);
    let dwarf = warrior("RaceDwarf", "");
    let dwarf_mace = warrior("RaceDwarf", &weapon(36));
    assert_eq!(crit(&dwarf_mace) - crit(&dwarf), 1.0);

    // The aura is permanent, active only with the weapon, and listed either way.
    let id = aura(&naked, "Sword Specialization").expect("Sword Specialization");
    assert!(!naked.sim.aura(id).active);
    let id = aura(&swordsman, "Sword Specialization").expect("Sword Specialization");
    assert!(swordsman.sim.aura(id).active);
    assert_eq!(swordsman.sim.aura(id).duration, NEVER_EXPIRES);
}

/// A warrior that registers a survival on-use cooldown with no GCD, as an item trinket does.
fn survival_factory(
    sim: &mut Sim,
    unit: UnitId,
    player: &Message,
) -> Result<Box<dyn PrepAgent>, Refusal> {
    let timer = sim.new_timer(unit);
    let mut stats = Stats::default();
    stats[Stat::Armor] = 100.0;
    let aura = sim.register_temporary_stats_on_use_cd(
        unit,
        "Shield Wall Trinket",
        stats,
        10 * SECOND,
        SpellConfig {
            action_id: ActionId::item(1234),
            cast: CastConfig {
                cd: Cooldown {
                    timer: Some(timer),
                    duration: 5 * MINUTE,
                },
                ..CastConfig::default()
            },
            ..SpellConfig::default()
        },
    );
    assert_eq!(aura.infer_cd_type(), cooldown_type::SURVIVAL);
    factory(sim, unit, player)
}

#[test]
fn a_survival_on_use_cooldown_without_a_gcd_is_reactive() {
    let request = r#"{"simOptions":{"iterations":1,"randomSeed":"100"},
        "raid":{"parties":[{"players":[{"name":"Tank","class":"ClassWarrior",
          "race":"RaceHuman","buffs":{},"consumables":{}}]}]},
        "encounter":{"duration":180,"targets":[{"level":63}]}}"#;
    let request = Request::from_json(request.as_bytes()).expect("a valid request");
    let env = match Environment::new(request.message(), survival_factory) {
        Ok(env) => env,
        Err(refusal) => panic!("{refusal}"),
    };
    let unit = env.player;
    let id = env
        .sim
        .get_spell(unit, &ActionId::item(1234))
        .expect("the on-use spell");
    let spell = env.sim.spell(id);
    assert!(spell.flags.matches(SpellFlag::MCD));
    assert!(spell.flags.matches(SpellFlag::APL));
    assert!(spell.flags.matches(SpellFlag::REACTIVE));
    assert!(spell.flags.matches(SpellFlag::NO_ON_CAST_COMPLETE));
    let aura = env
        .sim
        .get_aura(unit, "Shield Wall Trinket")
        .expect("the aura");
    assert_eq!(spell.related_self_buff, Some(aura));
    assert_eq!(env.sim.aura(aura).icd, Some(spell.cd));
    let mcd = env
        .sim
        .character(unit)
        .initial_major_cooldowns
        .iter()
        .find(|mcd| mcd.spell == id)
        .expect("the major cooldown");
    assert_eq!(mcd.cooldown_type, cooldown_type::SURVIVAL);
    assert!(mcd.timings.is_empty());
}
