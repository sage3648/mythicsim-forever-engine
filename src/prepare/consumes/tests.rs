//! Go consumes.go on a player built through the same construction as Go's consume tests: the
//! stats, auras, spells and major cooldowns each consumable registers.

use super::*;
use crate::contracts::request::Request;
use crate::prepare::agent::{ClassSpellName, PrepAgent};
use crate::prepare::consumable_effects::{consumable_item_effect, dragonbreath_chili_effect};

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

fn factory(sim: &mut Sim, unit: UnitId, _player: &Message) -> Result<Box<dyn PrepAgent>, Refusal> {
    sim.enable_mana_bar(unit);
    Ok(Box::new(FakeAgent {
        talents: Message::empty("proto.WarriorTalents"),
    }))
}

fn build(class: &str, profession: &str, consumables: &str) -> Environment {
    let request = format!(
        r#"{{"simOptions":{{"iterations":1,"randomSeed":"100"}},
        "raid":{{"parties":[{{"players":[{{"name":"Player","class":"{class}",
          "race":"RaceHuman","profession1":"{profession}","buffs":{{}},
          "consumables":{consumables}}}]}}]}},
        "encounter":{{"duration":180,"targets":[{{"level":63,"mobType":"MobTypeElemental"}}]}}}}"#
    );
    let request = Request::from_json(request.as_bytes()).expect("a valid request");
    match Environment::new(request.message(), factory) {
        Ok(env) => env,
        Err(refusal) => panic!("{refusal}"),
    }
}

fn player(consumables: &str) -> Environment {
    build("ClassWarrior", "Alchemy", consumables)
}

fn stat(env: &Environment, stat: Stat) -> f64 {
    env.sim.stat(env.player, stat)
}

fn item_spell(env: &Environment, item: i32) -> Option<SpellId> {
    env.sim.get_spell(env.player, &ActionId::item(item))
}

fn cooldowns(env: &Environment) -> Vec<MajorCooldown> {
    env.sim
        .character(env.player)
        .initial_major_cooldowns
        .clone()
}

fn base(consumables: &str) -> (Environment, Environment) {
    (player(consumables), player("{}"))
}

#[test]
fn flasks_elixirs_food_and_classic_buffs_add_their_client_stats() {
    let (with, without) = base(
        r#"{"flaskId":13512,"battleElixirId":13452,"guardianElixirId":20007,"foodId":18254,
        "zanzaId":8423,"strengthBuffId":12451,"attackPowerBuffId":12460,"alcoholId":18284,
        "spellPowerElixirId":13454,"schoolElixirId":17708,"defenseElixirId":13445}"#,
    );
    let gain = |s: Stat| stat(&with, s) - stat(&without, s);
    // Flask of Supreme Power 150 and Greater Arcane Elixir 35 spell damage.
    assert_eq!(gain(Stat::SpellDamage), 185.0);
    assert_eq!(gain(Stat::MP5), 12.0);
    assert_eq!(gain(Stat::FrostDamage), 15.0);
    // Greater Defense 450 and 2 armor for each of Mongoose's 25 agility.
    assert_eq!(gain(Stat::Armor), 450.0 + 2.0 * 25.0);
    // Mongoose: agility 25 and melee crit rating 28.
    assert_eq!(gain(Stat::Agility), 25.0);
    assert_eq!(gain(Stat::MeleeCritRating), 28.0);
    // Juju Power.
    assert_eq!(gain(Stat::Strength), 30.0);
    // Juju Might.
    assert_eq!(gain(Stat::AttackPower), 40.0);
    assert_eq!(gain(Stat::RangedAttackPower), 40.0);
    // Runn Tum Tuber 15, Cerebral Cortex Compound 25 and Kreeg's Stout -5 intellect.
    assert_eq!(gain(Stat::Intellect), 35.0);
}

#[test]
fn a_consumable_missing_from_the_database_adds_nothing() {
    let (with, without) = base(r#"{"flaskId":999999999,"foodId":999999998}"#);
    assert_eq!(
        with.sim.stats(with.player),
        without.sim.stats(without.player)
    );
}

#[test]
fn scrolls_bid_in_the_shared_stat_category_and_are_permanent() {
    let env = player(
        r#"{"scrollStr":true,"scrollAgi":true,"scrollInt":true,"scrollSpi":true,"scrollArm":true}"#,
    );
    let unit = env.player;
    for (label, category, priority) in [
        ("Scroll of Agility IV", "StatBuffAgilityAdd", 17.0),
        ("Scroll of Strength IV", "StatBuffStrengthAdd", 17.0),
        ("Scroll of Intellect IV", "StatBuffIntellectAdd", 16.0),
        ("Scroll of Spirit IV", "StatBuffSpiritAdd", 15.0),
        ("Scroll of Protection IV", "StatBuffArmorAdd", 240.0),
    ] {
        let aura = env.sim.get_aura(unit, label).expect(label);
        let a = env.sim.aura(aura);
        assert!(a.active, "{label}");
        assert_eq!(a.duration, NEVER_EXPIRES);
        assert_eq!(a.exclusive_effects.len(), 1);
        let effect = &env.sim.effects[a.exclusive_effects[0].0];
        assert_eq!(env.sim.categories[effect.category.0].name, category);
        assert_eq!(effect.priority, priority);
    }
    let without = player("{}");
    assert_eq!(
        stat(&env, Stat::Strength) - stat(&without, Stat::Strength),
        17.0
    );
    // 240 and 2 armor for each of the scroll's 17 agility.
    assert_eq!(
        stat(&env, Stat::Armor) - stat(&without, Stat::Armor),
        240.0 + 2.0 * 17.0
    );
}

#[test]
fn bogling_root_adds_one_physical_damage() {
    let (with, without) = base(r#"{"boglingRoot":true}"#);
    assert_eq!(
        stat(&with, Stat::PhysicalDamage) - stat(&without, Stat::PhysicalDamage),
        1.0
    );
}

#[test]
fn static_imbues_add_their_stats_and_the_dense_stones_their_weapon_damage() {
    let (with, without) = base(r#"{"mhImbueId":25122}"#);
    let gain = |s: Stat| stat(&with, s) - stat(&without, s);
    assert_eq!(gain(Stat::SpellDamage), 36.0);
    assert_eq!(gain(Stat::HealingPower), 36.0);
    assert_eq!(gain(Stat::SpellCritPercent), 1.0);

    let (with, without) = base(r#"{"mhImbueId":22756}"#);
    assert_eq!(
        stat(&with, Stat::PhysicalCritPercent) - stat(&without, Stat::PhysicalCritPercent),
        2.0
    );
    assert_eq!(
        stat(&with, Stat::RangedCritPercent) - stat(&without, Stat::RangedCritPercent),
        -2.0
    );

    let with = player(r#"{"mhImbueId":16138,"ohImbueId":16622}"#);
    let without = player("{}");
    let weapons = |env: &Environment| env.sim.unit(env.player).auto_attacks.clone();
    let (a, b) = (weapons(&with), weapons(&without));
    assert_eq!(a.mh.base_damage_min - b.mh.base_damage_min, 8.0);
    assert_eq!(a.mh.base_damage_max - b.mh.base_damage_max, 8.0);
    assert_eq!(a.oh.base_damage_min - b.oh.base_damage_min, 8.0);
    assert_eq!(with.sim.mh_imbue_flat_weapon_damage(with.player), 8.0);
    assert_eq!(without.sim.mh_imbue_flat_weapon_damage(without.player), 0.0);
}

#[test]
fn bonuses_against_a_mob_type_reach_every_attack_table_after_finalization() {
    let env = player(r#"{"battleElixirId":9224,"mhImbueId":28891}"#);
    let index = env.sim.unit(env.player).unit_index as usize;
    assert!(!env.attack_tables[index].is_empty());
    for table in &env.attack_tables[index] {
        let demon = table.mob_type_bonus_stats["MobTypeDemon"];
        assert_eq!(demon[Stat::AttackPower], 265.0);
        assert_eq!(demon[Stat::RangedAttackPower], 265.0);
        let undead = table.mob_type_bonus_stats["MobTypeUndead"];
        assert_eq!(undead[Stat::AttackPower], 100.0);
    }
    // Demonslaying is a mob type bonus, not stats.
    assert_eq!(
        stat(&env, Stat::AttackPower),
        stat(&player("{}"), Stat::AttackPower)
    );
}

#[test]
fn gift_of_arthas_debuffs_each_enemy_and_triggers_on_hits_taken() {
    let env = player(r#"{"guardianElixirId":9088}"#);
    let unit = env.player;
    let without = player("{}");
    assert_eq!(
        stat(&env, Stat::ShadowResistance) - stat(&without, Stat::ShadowResistance),
        10.0
    );
    let target = env.encounter.targets[0];
    assert!(env
        .sim
        .get_aura(target, "Gift of Arthas (Player)")
        .is_some());
    let spell = env
        .sim
        .get_spell(unit, &ActionId::spell(11374))
        .expect("the proc spell");
    let spell = env.sim.spell(spell);
    assert_eq!(spell.spell_school, school::NATURE);
    assert_eq!(spell.proc_mask, ProcMask::EMPTY);
    assert_eq!(spell.flat_threat_bonus, 90.0);
    let trigger = env
        .sim
        .get_aura(unit, "Gift of Arthas - Trigger")
        .expect("the trigger");
    let trigger = env.sim.aura(trigger);
    assert_eq!(trigger.icd.expect("an icd").duration, 3 * SECOND);
    assert!(trigger.events.on_spell_hit_taken);
    assert!(trigger.active);
}

#[test]
fn dragonbreath_chili_registers_its_fire_spell_and_melee_trigger() {
    let env = player(r#"{"dragonbreathChili":true}"#);
    let unit = env.player;
    let spell = env
        .sim
        .get_spell(unit, &ActionId::spell(15851))
        .expect("the chili spell");
    let spell = env.sim.spell(spell);
    assert_eq!(spell.spell_school, school::FIRE);
    assert_eq!(spell.defense_type, DefenseType::Magic);
    assert_eq!(spell.proc_mask, ProcMask::SPELL_DAMAGE_PROC);
    assert_eq!(spell.bonus_coefficient, 1.0);
    assert!(spell
        .flags
        .matches(SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::PASSIVE_SPELL));
    let aura = env
        .sim
        .get_aura(unit, "Dragonbreath Chili")
        .expect("the chili aura");
    let aura = env.sim.aura(aura);
    assert_eq!(aura.action_id_for_proc, Some(ActionId::spell(15852)));
    assert_eq!(aura.icd.expect("an icd").duration, 10 * SECOND);
    assert!(aura.events.on_spell_hit_dealt);

    let effect = dragonbreath_chili_effect(&env).expect("the effect");
    assert_eq!(effect["kind"], "dragonbreath_chili");
    assert_eq!(effect["proc_chance"], 0.05);
    assert_eq!(effect["delay_ns"], 10 * crate::prepare::sim::MILLISECOND);
    // The chili's own spell and the unit's melee autos are the only spells the trigger reads
    // from a player with no class spells: the melee autos.
    assert!(effect["trigger_spells"].is_array());
    assert!(player("{}")
        .sim
        .get_aura(unit, "Dragonbreath Chili")
        .is_none());
}

#[test]
fn only_the_selected_potion_is_a_major_cooldown_but_every_listed_potion_is_a_spell() {
    let env = player(r#"{"potId":13444,"potions":[3827,13444,13442,13446,13455,1710]}"#);
    let mcds = cooldowns(&env);
    assert_eq!(mcds.len(), 1);
    let mcd = &mcds[0];
    assert_eq!(env.sim.spell(mcd.spell).action_id, ActionId::item(13444));
    assert_eq!(mcd.cooldown_type, cooldown_type::MANA);
    assert_eq!(mcd.priority, 0);
    let flags = env.sim.spell(mcd.spell).flags;
    assert!(flags.matches(SpellFlag::COMBAT_POTION));
    assert!(flags.matches(SpellFlag::POTION | SpellFlag::ENCOUNTER_ONLY | SpellFlag::APL));
    assert!(flags.matches(SpellFlag::MCD));

    let potions = [3827, 13444, 13442, 13446, 13455, 1710];
    let shared = env
        .sim
        .spell(item_spell(&env, 3827).unwrap())
        .shared_cd
        .timer;
    assert!(shared.is_some());
    for potion in potions {
        let spell = env
            .sim
            .spell(item_spell(&env, potion).expect("a potion spell"));
        assert!(spell.flags.matches(SpellFlag::POTION), "{potion}");
        // Every potion shares the potion category's timer and has its own.
        assert_eq!(spell.shared_cd.timer, shared);
        assert_eq!(spell.shared_cd.duration, 2 * MINUTE);
        assert!(spell.cd.timer.is_some() && spell.cd.timer != shared);
    }
    // Only the selected potion is a combat potion.
    assert!(!env
        .sim
        .spell(item_spell(&env, 3827).unwrap())
        .flags
        .matches(SpellFlag::COMBAT_POTION));
}

#[test]
fn a_potion_with_a_buff_registers_a_stat_aura_and_its_cooldown_type() {
    // Greater Stoneshield: armor for 120 s, a survival cooldown.
    let env = player(r#"{"potId":13455,"potions":[13455]}"#);
    let mcd = &cooldowns(&env)[0];
    assert_eq!(mcd.cooldown_type, cooldown_type::SURVIVAL);
    let spell = env.sim.spell(mcd.spell);
    let aura = spell.related_self_buff.expect("a self buff");
    assert_eq!(env.sim.aura(aura).label, "Greater Stoneshield Potion");
    assert_eq!(env.sim.aura(aura).duration, 120 * SECOND);

    // Mighty Rage Potion: strength for 20 s, and a rage gain, which is a DPS cooldown.
    let env = player(r#"{"potId":13442,"potions":[13442]}"#);
    assert_eq!(cooldowns(&env)[0].cooldown_type, cooldown_type::DPS);

    // A healing potion's effect is a heal, which a potion does not count as a resource gain, so
    // its cooldown has no type.
    let env = player(r#"{"potId":13446,"potions":[13446]}"#);
    assert_eq!(cooldowns(&env)[0].cooldown_type, cooldown_type::UNKNOWN);
}

#[test]
fn potions_describe_themselves_as_the_exporter_does() {
    let mut env = player(r#"{"potId":13444,"potions":[13444,13442]}"#);
    let mut unrepresented = Vec::new();
    let mana = item_spell(&env, 13444).unwrap();
    let effect = consumable_item_effect(&mut env, mana, 13444, &mut unrepresented).unwrap();
    assert_eq!(effect["kind"], "potion_mana");
    assert_eq!(effect["rng_label"], "Major Mana Potion");
    assert_eq!(effect["gains"][0]["min"], 1800.0);
    assert_eq!(effect["stone_multiplier"], 1.0);
    assert_eq!(effect["regen_window_seconds"], 5.0);

    let rage = item_spell(&env, 13442).unwrap();
    let effect = consumable_item_effect(&mut env, rage, 13442, &mut unrepresented).unwrap();
    assert_eq!(effect["kind"], "potion_resource");
    assert_eq!(effect["gains"][0]["resource"], "ResourceTypeRage");
    assert_eq!(effect["aura"], "Mighty Rage Potion");
    assert_eq!(
        effect["gain_log"],
        "Gained {\"Strength\": 60.000,} from {ItemID: 13442}."
    );
    assert_eq!(
        effect["expire_log"],
        "Lost {\"Strength\": 60.000,} from fading {ItemID: 13442}."
    );
    assert!(unrepresented.is_empty());
}

#[test]
fn every_conjured_item_is_a_major_cooldown_and_unknown_ids_are_skipped() {
    // Demonic Rune restores mana, Thistle Tea energy; 999 is not in the database.
    let env = player(r#"{"conjuredId":12662,"conjuredItems":[12662,7676,999]}"#);
    let mcds = cooldowns(&env);
    assert_eq!(mcds.len(), 2);
    let rune = &mcds[0];
    assert_eq!(env.sim.spell(rune.spell).action_id, ActionId::item(12662));
    assert_eq!(rune.cooldown_type, cooldown_type::MANA);
    let tea = &mcds[1];
    assert_eq!(env.sim.spell(tea.spell).action_id, ActionId::item(7676));
    assert_eq!(tea.cooldown_type, cooldown_type::DPS);
    for mcd in &mcds {
        let spell = env.sim.spell(mcd.spell);
        assert!(spell.flags.matches(SpellFlag::CONJURED | SpellFlag::APL));
        assert_eq!(spell.shared_cd.duration, 2 * MINUTE);
    }
    // Thistle Tea has its own 5 minute cooldown; the rune the 2 minute default.
    assert_eq!(env.sim.spell(tea.spell).cd.duration, 300 * SECOND);
    assert_eq!(env.sim.spell(rune.spell).cd.duration, 2 * MINUTE);
    // Both are in spell category 1153.
    assert_eq!(
        env.sim.spell(tea.spell).shared_cd.timer,
        env.sim.spell(rune.spell).shared_cd.timer
    );
}

#[test]
fn conjured_items_describe_themselves_and_which_one_is_selected() {
    let mut env = player(r#"{"conjuredId":7676,"conjuredItems":[12662,7676]}"#);
    let mut unrepresented = Vec::new();
    let rune = item_spell(&env, 12662).unwrap();
    let effect = consumable_item_effect(&mut env, rune, 12662, &mut unrepresented).unwrap();
    assert_eq!(effect["kind"], "conjured_mana");
    assert_eq!(effect["selected"], false);
    let tea = item_spell(&env, 7676).unwrap();
    let effect = consumable_item_effect(&mut env, tea, 7676, &mut unrepresented).unwrap();
    assert_eq!(effect["kind"], "conjured_energy");
    assert_eq!(effect["selected"], true);
    assert_eq!(effect["spill"], 10.0);
    assert!(unrepresented.is_empty());
}

fn explosive_mcds(env: &Environment) -> Vec<(i32, i32, u32)> {
    cooldowns(env)
        .iter()
        .map(|mcd| {
            (
                env.sim.spell(mcd.spell).action_id.item_id,
                mcd.priority,
                mcd.cooldown_type,
            )
        })
        .collect()
}

#[test]
fn explosives_follow_the_professions_and_class_that_may_throw_them() {
    let explosive = cooldown_type::DPS | cooldown_type::EXPLOSIVE;
    // Anyone throws a SAF-T bomb, Ez-Thro Dynamite II and Crystal Charge.
    for (id, item) in [(1269161, 260793), (18588, 18588), (15239, 11566)] {
        let env = build(
            "ClassWarrior",
            "Alchemy",
            &format!(r#"{{"explosiveId":{id}}}"#),
        );
        assert_eq!(
            explosive_mcds(&env),
            vec![(item, COOLDOWN_PRIORITY_LOW + 10, explosive)]
        );
    }
    // Thorium Grenades and Dense Dynamite need an engineer.
    for (id, item) in [(19769, 15993), (23063, 18641), (18641, 18641)] {
        let consumables = format!(r#"{{"explosiveId":{id}}}"#);
        assert!(explosive_mcds(&build("ClassWarrior", "Alchemy", &consumables)).is_empty());
        let engineer = build("ClassWarrior", "Engineering", &consumables);
        assert_eq!(
            explosive_mcds(&engineer),
            vec![(item, COOLDOWN_PRIORITY_LOW + 10, explosive)]
        );
    }
    // Scroll of Cryoblast is a mage's.
    let consumables = r#"{"explosiveId":440212}"#;
    assert!(explosive_mcds(&build("ClassWarrior", "Engineering", consumables)).is_empty());
    let mage = build("ClassMage", "Alchemy", consumables);
    assert_eq!(
        explosive_mcds(&mage),
        vec![(217495, COOLDOWN_PRIORITY_LOW + 10, explosive)]
    );
    let spell = mage.sim.spell(item_spell(&mage, 217495).unwrap());
    assert_eq!(spell.spell_school, school::FROST);
}

#[test]
fn an_explosive_spell_is_a_one_minute_shared_magic_cast() {
    let env = build("ClassWarrior", "Engineering", r#"{"explosiveId":19769}"#);
    let spell = env.sim.spell(item_spell(&env, 15993).unwrap());
    assert_eq!(spell.defense_type, DefenseType::Magic);
    assert_eq!(spell.proc_mask, ProcMask::EMPTY);
    assert_eq!(spell.missile_speed, 25.0);
    assert_eq!(spell.default_cast.cast_time, SECOND);
    assert_eq!(spell.bonus_hit_percent, 100.0);
    assert!(spell.flags.matches(SpellFlag::EXPLOSIVE));
    assert_eq!(spell.shared_cd.duration, MINUTE);
    assert_eq!(spell.cd.timer, None);
    assert_eq!(spell.cast_kind, crate::prepare::spell::CastKind::Full);
}

#[test]
fn the_goblin_sapper_needs_an_engineer_and_registers_its_self_hit_first() {
    let consumables = r#"{"goblinSapper":true,"explosiveId":1269161}"#;
    let plain = build("ClassWarrior", "Alchemy", consumables);
    assert!(item_spell(&plain, 10646).is_none());

    let env = build("ClassWarrior", "Engineering", consumables);
    let book: Vec<ActionId> = env
        .sim
        .unit(env.player)
        .spellbook
        .iter()
        .map(|spell| env.sim.spell(*spell).action_id.clone())
        .collect();
    let sapper = ActionId::item(10646);
    let self_hit = ActionId {
        tag: 1,
        ..sapper.clone()
    };
    let self_position = book
        .iter()
        .position(|id| *id == self_hit)
        .expect("the self hit");
    let sapper_position = book
        .iter()
        .position(|id| *id == sapper)
        .expect("the sapper");
    assert!(self_position < sapper_position);
    let self_spell = env.sim.get_spell(env.player, &self_hit).unwrap();
    assert_eq!(env.sim.spell(self_spell).proc_mask, ProcMask::SPELL_DAMAGE);

    let spell = env.sim.spell(item_spell(&env, 10646).unwrap());
    assert_eq!(spell.cd.duration, 5 * MINUTE);
    assert_eq!(spell.default_cast.cast_time, 0);
    let mcds = explosive_mcds(&env);
    assert_eq!(
        mcds[0],
        (
            10646,
            COOLDOWN_PRIORITY_LOW + 20,
            cooldown_type::DPS | cooldown_type::EXPLOSIVE
        )
    );
    assert_eq!(mcds.len(), 2);
}

#[test]
fn explosives_describe_themselves_as_the_exporter_does() {
    let mut env = build("ClassWarrior", "Engineering", r#"{"explosiveId":1269334}"#);
    let mut unrepresented = Vec::new();
    let spell = item_spell(&env, 260817).unwrap();
    let effect = consumable_item_effect(&mut env, spell, 260817, &mut unrepresented).unwrap();
    assert_eq!(effect["kind"], "basic_explosive");
    assert_eq!(effect["min_damage"], 225.0);
    assert_eq!(effect["max_damage"], 675.0);
    assert_eq!(effect["aoe_cap_multiplier"], 1.0);
    assert!(unrepresented.is_empty());
}
