//! The class independent item sets of the live Forever client that the reference does not
//! register, whose pieces `data/forever-client-items.jsonl` holds (`tools/rust_client_items.py`,
//! `CLIENT_SETS`). The Tier 1 class sets are in each class's items module
//! (`crate::classes::client_item_sets`).
//!
//! Each bonus is its ItemSetSpell row's spell, read from wago.tools `product=wowf` on 12 November
//! 2026 (the tables of build 1.60.1.70888), beside the line the Wowhead Forever tooltips of the
//! pieces give it on the same day. A bonus whose row is a plain stat is applied to the status
//! aura its piece count registers, as the reference's sets are (`forever_item_sets`). A bonus
//! that is a proc stays unprepared, and an equipment that reaches it is refused by name
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
//!   ItemSetSpell 8249, Krol'dok Resolve (1324298), a proc that casts 1324296. Not prepared: the
//!   rows state no chance for the tooltip's "chance on block" (SpellAuraOptions 248630 has
//!   ProcChance 100).
//! - (5) "Taking damage has a high chance to increase your Stamina by 1 for 30 sec, stacking up
//!   to 20 times. (3s cooldown)" ItemSetSpell 8250, Ogre Dominance (1324309), a proc that casts
//!   1324308. Not prepared: the "high chance" is not a number the rows state plainly.
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
//!   that casts 1324683. Not prepared: the rows state no chance (SpellAuraOptions 248691 has
//!   ProcChance 100, the tooltip "a chance").

use super::character::constants::DEFENSE_RATING_PER_DEFENSE_LEVEL;
use super::env::Environment;
use super::item_sets::{ClientItemSet, ItemSet};
use super::sim::AuraId;
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

/// The client's sets, with the bonuses applied and the piece counts that are not prepared yet.
pub(crate) static CLIENT_ITEM_SETS: &[ClientItemSet] = &[
    ClientItemSet {
        set: ItemSet {
            id: 2135,
            name: "Krol'dok Battlegear",
            alternative_name: "",
            bonuses: &[(2, kroldok_battlegear_2), (3, kroldok_battlegear_3)],
            required_profession: "",
        },
        not_prepared: &[4, 5],
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
            ],
            required_profession: "",
        },
        not_prepared: &[5],
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
        // ItemSetSpell thresholds of every set: 2, 3, 4 and 5 pieces.
        let sets = client_sets();
        assert!(
            sets.len() >= CLIENT_ITEM_SETS.len() + 12,
            "the classes' sets are included"
        );
        for client in sets {
            let mut counts: Vec<i32> = client.set.bonuses.iter().map(|(n, _)| *n).collect();
            counts.extend(client.not_prepared);
            counts.sort_unstable();
            assert_eq!(counts, [2, 3, 4, 5], "{}", client.set.name);
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
