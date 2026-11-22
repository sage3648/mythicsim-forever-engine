//! The class independent item sets of the live Forever client that the reference does not
//! register, whose pieces `data/forever-client-items.jsonl` holds (`tools/rust_client_items.py`,
//! `CLIENT_SETS`). The Tier 1 class sets are in each class's items module
//! (`crate::classes::client_item_sets`).
//!
//! Each bonus is its ItemSetSpell row's spell, read from wago.tools `product=wowf` on 12 November
//! 2026 (the tables of build 1.60.1.70888), beside the line the Wowhead Forever tooltips of the
//! pieces give it on the same day. A bonus whose row is a plain stat is applied to the status
//! aura its piece count registers, as the reference's sets are (`forever_item_sets`). A bonus
//! that is a proc is prepared by the module that models it where one does (`forever_tank_procs`),
//! and stays unprepared otherwise: an equipment that reaches it is refused by name
//! (`item_sets::client_set_bonuses`).
//!
//! Krol'dok Battlegear (ItemSet 2135: Helmet 275006, Pauldrons 287014, Chest 275005, Legplates
//! 287013, Gauntlets 287012):
//!
//! - (2) "+100 Armor." ItemSetSpell 8247, Increased Armor 100 (1324287). SpellEffect 1361064: an
//!   armor aura (674, the type the client moved its armor auras to) of 100 on the wearer.
//! - (3) "Increased Defense +5." ItemSetSpell 8248, Increased Defense (1324288). SpellEffect
//!   1361065: a skill aura (30) of 5 on Defense (skill 95).
//! - (4) "Chance on block to heal for 14 to 23. Healing is doubled in Strongholds and Cities."
//!   ItemSetSpell 8249, Krol'dok Resolve (1324298), a proc that casts 1324296. SpellAuraOptions
//!   248630 has ProcChance 100 beside the tooltip's "chance on block", which is read literally as
//!   the rate: every block heals (`forever_tank_procs`).
//! - (5) "Taking damage has a high chance to increase your Stamina by 1 for 30 sec, stacking up
//!   to 20 times. (3s cooldown)" ItemSetSpell 8250, Ogre Dominance (1324309), a proc that casts
//!   1324308. SpellAuraOptions 248632 has ProcChance 100 and a proc recovery of 3000 ms beside the
//!   tooltip's "high chance", which is read literally as the rate: every landed hit taken adds a
//!   stack, at most once in 3 sec (`forever_tank_procs`).
//!
//! Defias Enforcer's Garb (ItemSet 2136: Mask 287019, Shoulderpads 276741, Chest 287020,
//! Legguards 276746, Prowlers 287021):
//!
//! - (2) "+10 Fire Resistance." ItemSetSpell 8255, Increased Fire Resist 10 (1324673).
//!   SpellEffect 1361599: a resistance aura (22) of 10 on the Fire school (mask 4).
//! - (3) "+15 Stamina." ItemSetSpell 8256, Increased Stamina 10 (1324674). SpellEffect 1361600: a
//!   stat aura (29) of 15 on Stamina (stat 2). The row and the tooltip say 15; only the spell's
//!   name says 10.
//! - (4) "Increased Swords +1." ItemSetSpell 8257, Increased 1H Sword (1324675). SpellEffect
//!   1361601: a skill aura (30) of 1 on Swords (skill 43).
//! - (5) "Melee attacks have a chance to increase your Critical Strike chance by 2% for 20 sec.
//!   This effect is doubled in swamps." ItemSetSpell 8258, Defias Juggernaut (1324684), a proc
//!   (ProcTypeMask 4116 and, in the second word, 32: the main hand's melee hits) that casts
//!   1324683, an A_MOD_CRIT_PCT (290) of 2 for 20 sec that does not stack. SpellAuraOptions 248691
//!   has ProcChance 100 beside the tooltip's "a chance". Logs measure 1 proc per minute on the
//!   landed melee hits of either hand, priced on the hitting weapon's speed: four five piece
//!   wearers on forever.warcraftlogs.com (22 November 2026, reports mTk2BJK7FLQWwcC8,
//!   GVXDntYPzaRmk9yK, TXbC4jMJv8GN2wD3, g1YV283cxqG4pWm6) gained it 548 times in about 111
//!   minutes of melee, on 3.2 to 4.3% of a Rogue's landed hits and 4.6% of a bear's (2.5 sec
//!   swings), with the buff up 35 to 55% of the time. Until then every landed main hand hit
//!   refreshed it. No fight is in a swamp, so the doubling (the buff's Dummy of 4) never applies.
//!   The rows are in the pinned store.
//!
//! Soulforge Armor (ItemSet 516: the Classic Tier 0.5 Paladin pieces 22086 to 22093, which the
//! reference does not register; its Soulforge Armor is the later reissue's set), read the same
//! way on 13 November 2026 beside the Wowhead Forever tooltip of Soulforge Breastplate (22089):
//!
//! - (2) "+8 All Resistances." ItemSetSpell 2317, Increased All Resist 08 (18679): a resistance
//!   aura (22) of 8 on every school (mask 126).
//! - (4) "Chance on melee attack to increase your damage and healing done by magical spells and
//!   effects by up to 65 for 10 sec." ItemSetSpell 2318, Crusader's Wrath (27498), a proc on
//!   melee autoattacks (ProcTypeMask 4) that casts 27499, 65 spell damage (13) and healing (135).
//!   SpellAuraOptions 135342 has ProcChance 100 and no rate, as Lightforge Armor's (5) proc of the
//!   same buff (450625) has; it runs at Lightforge's 3 per minute
//!   (`forever_item_sets::crusaders_wrath`).
//! - (6) "+40 Attack Power." ItemSetSpell 2316, Attack Power 40 (14049): melee (99) and ranged
//!   (124) attack power of 40.
//! - (8) "+200 Armor." ItemSetSpell 2319, Increased Armor 200 (14803): an armor aura (674) of 200.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;

use super::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use super::character::constants::DEFENSE_RATING_PER_DEFENSE_LEVEL;
use super::dbcenums::A_MOD_CRIT_PCT;
use super::env::Environment;
use super::export_items::aura_named;
use super::item_sets::{ClientItemSet, ItemSet};
use super::sim::AuraId;
use super::spell::ProcMask;
use super::spelldata::must_find;
use super::stats::{Stat, Stats};
use super::weapon_skill::WeaponSkill;

/// Krol'dok Battlegear (2): +100 Armor, as Ironweave Battlesuit's +200 Armor is.
fn kroldok_battlegear_2(env: &mut Environment, aura: AuraId) {
    env.sim
        .attach_stats_buff(aura, Stats::from_pairs(&[(Stat::Armor, 100.0)]));
}

/// Krol'dok Battlegear (3): Increased Defense +5, as Imperial Plate's +7 is.
fn kroldok_battlegear_3(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stat_buff(
        aura,
        Stat::DefenseRating,
        5.0 * DEFENSE_RATING_PER_DEFENSE_LEVEL,
    );
}

/// Defias Enforcer's Garb (2): +10 Fire Resistance.
fn defias_enforcers_garb_2(env: &mut Environment, aura: AuraId) {
    env.sim
        .attach_stats_buff(aura, Stats::from_pairs(&[(Stat::FireResistance, 10.0)]));
}

/// Defias Enforcer's Garb (3): +15 Stamina.
fn defias_enforcers_garb_3(env: &mut Environment, aura: AuraId) {
    env.sim
        .attach_stats_buff(aura, Stats::from_pairs(&[(Stat::Stamina, 15.0)]));
}

/// Defias Enforcer's Garb (4): Increased Swords +1, as Defias Leather's Daggers +1 is.
fn defias_enforcers_garb_4(env: &mut Environment, aura: AuraId) {
    env.sim.attach_weapon_skill(aura, WeaponSkill::Swords, 1.0);
}

/// Defias Juggernaut, the proc of Defias Enforcer's Garb (5), and the buff it applies.
const DEFIAS_JUGGERNAUT: i32 = 1_324_684;
const DEFIAS_JUGGERNAUT_BUFF: i32 = 1_324_683;
/// The permanent aura the five pieces register (`item_sets`), which the listener goes by.
pub(crate) const DEFIAS_ENFORCERS_GARB_5: &str = "Defias Enforcer's Garb 5P";
/// The buff's temporary stats aura.
pub(crate) const DEFIAS_JUGGERNAUT_AURA: &str = "Defias Juggernaut";

/// The rate logs measure, in procs per minute of the hitting weapon's speed.
const DEFIAS_JUGGERNAUT_PPM: f64 = 1.0;
/// The hits it rolls on: the melee hits of either hand, white and special.
const DEFIAS_JUGGERNAUT_MASK: ProcMask = ProcMask::MELEE;
/// The listener's name, which the exported proc rolls under.
pub(crate) const DEFIAS_JUGGERNAUT_TRIGGER: &str = "Defias Juggernaut";
/// The buff's spell, for the export's logs.
pub(crate) const DEFIAS_JUGGERNAUT_BUFF_ID: i32 = DEFIAS_JUGGERNAUT_BUFF;

/// The buff's crit: its A_MOD_CRIT_PCT, every kind of crit, as `parse_effects` reads the aura.
pub(crate) fn defias_juggernaut_stats() -> Stats {
    let crit = must_find(DEFIAS_JUGGERNAUT_BUFF)
        .effects
        .iter()
        .find(|effect| effect.aura == A_MOD_CRIT_PCT)
        .map_or(0.0, |effect| effect.base_points);
    Stats::from_pairs(&[
        (Stat::PhysicalCritPercent, crit),
        (Stat::SpellCritPercent, crit),
    ])
}

/// Defias Enforcer's Garb (5): the buff, and a listener on the set's aura that rolls 1 proc per
/// minute on landed melee hits of either hand, through a manager an item swap rebuilds for the
/// weapons in the hands, as Lightforge Armor's Crusader's Wrath does.
fn defias_enforcers_garb_5(env: &mut Environment, aura: AuraId) {
    let unit = env.player;
    env.sim.new_temporary_stats_aura(
        unit,
        DEFIAS_JUGGERNAUT_AURA,
        &ActionId::spell(DEFIAS_JUGGERNAUT_BUFF),
        defias_juggernaut_stats(),
        must_find(DEFIAS_JUGGERNAUT_BUFF).duration(),
    );
    let dpm = Rc::new(
        env.sim
            .new_ppm_manager(unit, DEFIAS_JUGGERNAUT_PPM, DEFIAS_JUGGERNAUT_MASK)
            .rebuilt_by_swap(
                &env.sim,
                unit,
                super::item_swap::ManagerChance::Ppm(DEFIAS_JUGGERNAUT_PPM),
                super::item_swap::ManagerMask::Static(DEFIAS_JUGGERNAUT_MASK),
            ),
    );
    env.sim.attach_proc_trigger(
        aura,
        &ProcTrigger {
            action_id: ActionId::spell(DEFIAS_JUGGERNAUT),
            name: DEFIAS_JUGGERNAUT_TRIGGER.to_string(),
            callback: CallbackMask::ON_SPELL_HIT_DEALT,
            proc_mask: DEFIAS_JUGGERNAUT_MASK,
            outcome: HitOutcome::LANDED,
            dpm: Some(dpm),
            ..ProcTrigger::default()
        },
    );
}

/// The stat auras of the client sets, for the combinations `statAurasEffect` reads.
pub(crate) fn stat_auras(env: &Environment) -> Vec<String> {
    if aura_named(env, DEFIAS_ENFORCERS_GARB_5).is_some() {
        vec![DEFIAS_JUGGERNAUT_AURA.to_string()]
    } else {
        Vec::new()
    }
}

/// Soulforge Armor (2): +8 All Resistances.
fn soulforge_armor_2(env: &mut Environment, aura: AuraId) {
    let mut stats = Stats::default();
    for stat in [
        Stat::ArcaneResistance,
        Stat::FireResistance,
        Stat::FrostResistance,
        Stat::NatureResistance,
        Stat::ShadowResistance,
    ] {
        stats[stat] = 8.0;
    }
    env.sim.attach_stats_buff(aura, stats);
}

/// Soulforge Armor (4): Crusader's Wrath, as Lightforge Armor's (5).
fn soulforge_armor_4(env: &mut Environment, aura: AuraId) {
    super::forever_item_sets::crusaders_wrath(env, aura, SOULFORGE_CRUSADERS_WRATH, 27498);
}

/// The name of Soulforge Armor's Crusader's Wrath proc.
pub(crate) const SOULFORGE_CRUSADERS_WRATH: &str = "Crusader's Wrath Proc - Soulforge Armor";

/// Soulforge Armor (6): +40 Attack Power.
fn soulforge_armor_6(env: &mut Environment, aura: AuraId) {
    env.sim.attach_stats_buff(
        aura,
        Stats::from_pairs(&[(Stat::AttackPower, 40.0), (Stat::RangedAttackPower, 40.0)]),
    );
}

/// Soulforge Armor (8): +200 Armor.
fn soulforge_armor_8(env: &mut Environment, aura: AuraId) {
    env.sim
        .attach_stats_buff(aura, Stats::from_pairs(&[(Stat::Armor, 200.0)]));
}

/// The client's sets, with the bonuses applied and the piece counts that are not prepared yet.
pub(crate) static CLIENT_ITEM_SETS: &[ClientItemSet] = &[
    ClientItemSet {
        set: ItemSet {
            id: 2135,
            name: "Krol'dok Battlegear",
            alternative_name: "",
            bonuses: &[
                (2, kroldok_battlegear_2),
                (3, kroldok_battlegear_3),
                (4, super::forever_tank_procs::kroldok_battlegear_4),
                (5, super::forever_tank_procs::kroldok_battlegear_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
    ClientItemSet {
        set: ItemSet {
            id: 2136,
            name: "Defias Enforcer's Garb",
            alternative_name: "",
            bonuses: &[
                (2, defias_enforcers_garb_2),
                (3, defias_enforcers_garb_3),
                (4, defias_enforcers_garb_4),
                (5, defias_enforcers_garb_5),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
    ClientItemSet {
        set: ItemSet {
            id: 516,
            name: "Soulforge Armor",
            alternative_name: "",
            bonuses: &[
                (2, soulforge_armor_2),
                (4, soulforge_armor_4),
                (6, soulforge_armor_6),
                (8, soulforge_armor_8),
            ],
            required_profession: "",
        },
        not_prepared: &[],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::tables::tables;
    use crate::prepare::item_sets::client_sets;

    #[test]
    fn the_reference_registers_none_of_the_client_sets() {
        // The shared sets and each class's.
        for client in client_sets() {
            let set = &client.set;
            assert!(
                super::super::item_sets::find_set(&tables().item_sets, set.id, set.name).is_none(),
                "{}",
                set.name
            );
        }
    }

    #[test]
    fn every_bonus_of_a_client_set_is_applied_or_named_as_not_prepared() {
        // ItemSetSpell thresholds of every set: 2, 3, 4 and 5 pieces, unless the rows say others.
        let sets = client_sets();
        assert!(
            sets.len() >= CLIENT_ITEM_SETS.len() + 12,
            "the classes' sets are included"
        );
        for client in sets {
            let mut counts: Vec<i32> = client.set.bonuses.iter().map(|(n, _)| *n).collect();
            counts.extend(client.not_prepared);
            counts.sort_unstable();
            // The generated rows where they hold the set (Soulforge Armor: 2, 4, 6 and 8).
            let mut rows: Vec<i32> = super::super::forever_set_spells_generated::SET_BONUSES
                .iter()
                .filter(|(id, _, _)| *id == client.set.id)
                .map(|(_, pieces, _)| *pieces)
                .collect();
            rows.dedup();
            if rows.is_empty() {
                rows = vec![2, 3, 4, 5];
            }
            assert_eq!(counts, rows, "{}", client.set.name);
        }
    }

    #[test]
    fn no_two_client_sets_share_an_id_or_a_name() {
        let sets = client_sets();
        for (i, a) in sets.iter().enumerate() {
            for b in &sets[i + 1..] {
                assert_ne!(a.set.id, b.set.id, "{}", a.set.name);
                assert_ne!(a.set.name, b.set.name);
            }
        }
    }
}
