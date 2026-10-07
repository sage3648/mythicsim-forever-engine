//! The shared item and enchant effects on a character built the way Go's tests build one.

use super::*;
use crate::prepare::item_test_support::{aura, environment, spell};
use crate::prepare::sim::NEVER_EXPIRES;

/// The item IDs the generated tables and the hand-written effects register, which are the
/// forever package's items in `registered_effects.txt`.
const FOREVER_ITEMS: [i32; 148] = [
    833, 871, 1131, 1168, 1204, 1447, 1979, 2802, 2824, 4262, 4264, 4696, 5443, 6469, 6472, 6622,
    6898, 6907, 6972, 7133, 7284, 7507, 7508, 7515, 7747, 7939, 8348, 8367, 9397, 9449, 9458, 9509,
    10543, 10721, 10760, 10761, 11302, 11744, 11809, 11810, 11811, 11815, 11861, 11905, 12532,
    12631, 12632, 12794, 12798, 12805, 13143, 13164, 13171, 13204, 13246, 13286, 13361, 13401,
    13955, 13966, 13984, 14152, 14487, 14554, 14555, 15107, 15108, 15866, 15867, 15873, 16768,
    17074, 17076, 17111, 17759, 18047, 18340, 18351, 18371, 18383, 18820, 18825, 18826, 19024,
    19099, 19100, 19288, 19324, 19339, 19343, 19344, 19345, 19952, 20071, 20072, 20525, 20636,
    21115, 21116, 21117, 21118, 21119, 21120, 21180, 21190, 21473, 21670, 21891, 22061, 22191,
    22268, 22678, 22954, 23027, 23041, 23042, 23046, 23047, 23206, 23207, 23558, 23570, 219345,
    226881, 234562, 234588, 249469, 249470, 267369, 270226, 272439, 272591, 272838, 272999, 273003,
    273137, 273840, 274159, 274290, 274759, 276631, 279876, 282778, 285275, 285276, 285277, 285278,
    286541,
];

/// The enchants of the same files.
const FOREVER_ENCHANTS: [i32; 15] = [
    30, 32, 33, 36, 44, 63, 663, 664, 803, 1898, 7941, 7942, 8216, 8220, 8721,
];

#[test]
fn the_registrations_are_the_forever_packages_ids() {
    let (items, enchants) = registered_ids();
    assert_eq!(items, FOREVER_ITEMS.to_vec());
    assert_eq!(enchants, FOREVER_ENCHANTS.to_vec());
}

#[test]
fn every_registration_is_one_go_registers() {
    let tables = crate::data::tables::tables();
    let (items, enchants) = registered_ids();
    for item in items {
        assert!(
            tables.item_effect_ids.contains(&item),
            "item {item} is not registered by Go"
        );
    }
    for enchant in enchants {
        assert!(
            tables.enchant_effect_ids.contains(&enchant),
            "enchant {enchant} is not registered by Go"
        );
    }
}

#[test]
fn hand_of_justice_listens_to_landed_melee_hits_behind_a_two_second_cooldown() {
    let env = environment(&[(11815, 0)]);
    let id = aura(&env, "Hand of Justice");
    let aura = env.sim.aura(id);
    assert_eq!(aura.action_id_for_proc, Some(ActionId::spell(15600)));
    assert_eq!(aura.icd.map(|icd| icd.duration), Some(2 * SECOND));
    assert_eq!(
        aura.callback_names(),
        vec!["on_reset", "on_spell_hit_dealt"]
    );
}

#[test]
fn a_weapon_proc_rolls_its_procs_per_minute_at_the_weapons_swing_speed() {
    // Bonereaver's Edge: two procs a minute, on the main hand that swings every 2.6 seconds.
    let env = environment(&[(17076, 0)]);
    let trigger = env.sim.aura(aura(&env, "Bonereaver's Edge Proc"));
    let dpm = trigger.dpm.as_ref().expect("a proc manager");
    assert_eq!(dpm.proc_chances, vec![2.6 * (2.0 / 60.0)]);
    assert_eq!(dpm.proc_masks, vec![ProcMask::MELEE_MH]);
    let stacks = env.sim.aura(aura(&env, "Bonereaver's Edge"));
    assert_eq!(stacks.max_stacks, 3);
    assert_eq!(stacks.duration, 10 * SECOND);
}

#[test]
fn a_proc_that_stacks_a_debuff_registers_it_on_every_enemy() {
    let env = environment(&[(13204, 0)]);
    let target = env.encounter.targets[0];
    let debuff = env
        .sim
        .aura(env.sim.get_aura(target, "Puncture Armor").unwrap());
    assert_eq!(debuff.max_stacks, 3);
    assert_eq!(debuff.duration, 30 * SECOND);
    assert!(debuff.on_stacks_change.is_some());
}

#[test]
fn on_use_items_in_one_category_share_a_timer_and_keep_their_own_cooldowns() {
    // Helm of Fire and Smokey's Lighter are both offensive trinket category 1141 uses.
    let env = environment(&[(8348, 0), (13171, 0)]);
    let helm = env.sim.spell(spell(&env, &ActionId::item(8348)));
    let lighter = env.sim.spell(spell(&env, &ActionId::item(13171)));
    assert_eq!(helm.shared_cd.timer, lighter.shared_cd.timer);
    assert!(helm.shared_cd.timer.is_some());
    assert_ne!(helm.cd.timer, lighter.cd.timer);
    assert_eq!(helm.cd.duration, 300 * SECOND);
    assert_eq!(helm.shared_cd.duration, 10 * SECOND);
    assert_eq!(lighter.cd.duration, 90 * SECOND);
    let cooldowns: Vec<_> = env
        .sim
        .character(env.player)
        .initial_major_cooldowns
        .iter()
        .map(|mcd| env.sim.spell(mcd.spell).action_id.item_id)
        .collect();
    assert_eq!(cooldowns, vec![8348, 13171]);
}

#[test]
fn a_stat_on_use_item_gets_a_temporary_stats_aura_and_its_cooldown_as_the_aura_icd() {
    // Golden Banana: 16 of each of five stats for 15 seconds, every five minutes.
    let env = environment(&[(270226, 0)]);
    let id = env
        .sim
        .get_aura(env.player, "Monkey Business (1287571)")
        .unwrap();
    let aura = env.sim.aura(id);
    assert_eq!(aura.duration, 15 * SECOND);
    assert_eq!(aura.icd.map(|icd| icd.duration), Some(300 * SECOND));
    let banana = env.sim.spell(spell(&env, &ActionId::item(270226)));
    assert_eq!(banana.related_self_buff, Some(id));
}

#[test]
fn a_scope_adds_to_the_ranged_weapons_damage() {
    // Sniper Scope: +7 damage on a bow whose damage is 10 to 20 before it.
    let env = environment(&[(25, 664)]);
    let ranged = &env.sim.unit(env.player).auto_attacks.ranged;
    assert_eq!(ranged.base_damage_min, 17.0);
    assert_eq!(ranged.base_damage_max, 27.0);
}

#[test]
fn an_item_with_haste_pseudo_stats_multiplies_the_wearers_speeds() {
    // Enchant Gloves - Minor Haste: one percent melee, ranged and spell haste.
    let env = environment(&[(203, 931)]);
    let id = aura(&env, "Enchant 931 Speed (ItemSlotHands)");
    assert!(env.sim.aura(id).active);
    let unit = env.sim.unit(env.player);
    assert_eq!(unit.pseudo_stats.melee_speed_multiplier, 1.01);
    assert_eq!(unit.pseudo_stats.ranged_speed_multiplier, 1.01);
    assert_eq!(unit.pseudo_stats.cast_speed_multiplier, 1.01);
}

#[test]
fn an_equip_effect_is_checked_on_reset_against_the_equipped_item() {
    // Tortoise Armor's equip aura: damage taken from spells reduced by 5, active while worn.
    let env = environment(&[(6907, 0)]);
    let aura = env.sim.aura(aura(&env, "Tortoise Armor"));
    assert!(aura.active);
    assert_eq!(aura.duration, NEVER_EXPIRES);
    assert!(aura.on_reset.is_some());
}

#[test]
fn a_spell_data_proc_names_its_buff_after_the_item() {
    // Draconic Infused Emblem: a chance on a hit for a stat buff aura of its own.
    let env = environment(&[(22268, 0)]);
    let trigger = env.sim.aura(aura(&env, "Draconic Infused Emblem"));
    assert_eq!(trigger.action_id_for_proc, Some(ActionId::item(22268)));
    let buff = env.sim.aura(aura(&env, "Draconic Infused Emblem Proc"));
    assert_eq!(buff.action_id, Some(ActionId::spell(1318930)));
    assert_eq!(buff.icd, trigger.icd);
}

#[test]
fn an_item_for_another_class_is_not_applied() {
    // Robe of the Archmage is a mage's: its use is not registered for a warrior.
    let env = environment(&[(14152, 0)]);
    assert!(env
        .sim
        .get_spell(env.player, &ActionId::item(14152))
        .is_none());
}

mod exports {
    use super::*;
    use crate::prepare::export_items::item_proc_effects;
    use serde_json::json;

    fn effects(env: &Environment) -> (Vec<serde_json::Value>, Vec<String>) {
        let mut unrepresented = Vec::new();
        (item_proc_effects(env, &mut unrepresented), unrepresented)
    }

    /// The spellbook position of the main hand auto attack.
    fn main_hand_auto(env: &Environment) -> usize {
        let action = ActionId {
            other_id: "OtherActionAttack".to_string(),
            tag: 1,
            ..ActionId::default()
        };
        let spell = spell(env, &action);
        env.sim
            .unit(env.player)
            .spellbook
            .iter()
            .position(|registered| *registered == spell)
            .expect("in the spellbook")
    }

    #[test]
    fn hand_of_justice_is_described_as_an_extra_attack_proc() {
        let env = environment(&[(11815, 0)]);
        let (effects, unrepresented) = effects(&env);
        assert!(unrepresented.is_empty());
        assert_eq!(
            effects,
            vec![
                json!({"kind": "extra_attack_proc", "trigger_aura": "Hand of Justice",
                        "proc_chance": 0.01, "attacks": 1})
            ]
        );
    }

    #[test]
    fn puncture_armor_is_read_stack_by_stack_from_a_separate_reset() {
        let env = environment(&[(13204, 0)]);
        let (effects, unrepresented) = effects(&env);
        assert!(unrepresented.is_empty());
        assert_eq!(effects.len(), 1);
        let effect = &effects[0];
        assert_eq!(effect["kind"], "armor_debuff_proc");
        assert_eq!(effect["trigger_aura"], "Bashguuder Proc");
        assert_eq!(effect["aura"], "Puncture Armor");
        assert_eq!(
            effect["armor_by_stacks"],
            json!([0.0, -100.0, -200.0, -300.0])
        );
        // Two procs a minute of the main hand's 2.6 second swing, heard by its auto attack.
        assert_eq!(
            effect["chances"],
            json!([{"spell": main_hand_auto(&env), "chance": 2.6 * (2.0 / 60.0)}])
        );
    }

    #[test]
    fn a_weapon_enchant_that_casts_damage_rolls_on_the_enchanted_hand() {
        // Fiery Weapon: six procs a minute of the main hand's swing.
        let env = environment(&[(25, 803)]);
        let (effects, unrepresented) = effects(&env);
        assert!(unrepresented.is_empty(), "{unrepresented:?}");
        assert_eq!(effects.len(), 1);
        let effect = &effects[0];
        assert_eq!(effect["kind"], "spell_data_damage_proc");
        assert_eq!(effect["trigger_aura"], "Enchant Weapon - Fiery Weapon");
        assert_eq!(effect["landed_only"], true);
        assert_eq!(
            effect["chances"],
            json!([{"spell": main_hand_auto(&env), "chance": 2.6 * (6.0 / 60.0)}])
        );
    }
}
