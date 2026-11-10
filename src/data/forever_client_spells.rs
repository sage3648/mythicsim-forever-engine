//! Spell rows of the live Forever client that the pinned store (`data/spells.jsonl`) lacks, for
//! the effect items `src/prepare/forever_effect_items.rs` models. Under Forever's rules only those
//! items name them; no reference row does.
//!
//! The client ships part of each row encrypted, so each field below says where it comes from:
//!
//! - wago.tools `product=wowf` (10 November 2026): SpellName, SpellEffect (Serpentbane's three
//!   readable effects), SpellCategories (defense type, mechanic) and ItemEffect (the item's
//!   trigger and cooldowns, in `forever_effect_items.rs`).
//! - The item's Wowhead Forever tooltip (10 November 2026), for what the encrypted rows hold:
//!   SpellMisc (school, duration, attributes) and the Rock on Stick weapons' SpellEffect rows,
//!   whose wago rows are unreadable (853501, 856584: a trigger spell of 8192 and a variance of 18).
//!
//! Dreadfrost Saber's two rows (1294939 and 1294940) are not encrypted: every field of theirs is
//! the client's, read from wago.tools `product=wowf` on 10 November 2026 (the tables of build
//! 1.60.1.70707, which it serves).
//!
//! A field the client hides and no tooltip states is left at zero: no range check, no missile
//! travel, no spell power or attack power share. The one exception is `PERIODIC_CAN_CRIT`, which
//! every Forever-added physical and Fire periodic damage row in the pinned store carries (19 of
//! the 20 at ids from 1280000; Burning Shell is the one without).

use super::spells::{Effect, Spell};

/// `ATTR_EX_8_PERIODIC_CAN_CRIT`, in the ninth attribute word.
const PERIODIC_CAN_CRIT: u32 = 0x200;

/// The rows, by id.
pub(crate) fn rows() -> Vec<Spell> {
    let mut attr = [0; 17];
    attr[8] = PERIODIC_CAN_CRIT;
    vec![
        // Serpentbane Crown's use (ItemEffect 234016): "Inflict 89 to 112 Fire damage to your
        // current target and an additional 225 damage over 10 sec. Inflicts 2 times as much
        // damage to Beast and Serpentine targets."
        Spell {
            id: 1295039,
            name: "Serpentbane".to_string(),
            // Tooltip: Fire damage.
            school: 4,
            attr,
            // Tooltip: over 10 sec, five ticks of the effect's 2 sec period.
            duration_ms: 10_000,
            // SpellCategories 184090: magic.
            defense_type: 1,
            effects: vec![
                // SpellEffect 1316823: School Damage of 100, variance 0.23 (88.5 to 111.5, the
                // tooltip's 89 to 112), no coefficient.
                Effect {
                    id: 1_316_823,
                    spell_id: 1_295_039,
                    index: 0,
                    effect_type: 2,
                    base_points: 100.0,
                    variance: f64::from(0.23_f32),
                    pvp_mult: 1.0,
                    chain_amp: 1.0,
                    target: [6, 0],
                    ..Effect::default()
                },
                // SpellEffect 1353171: Periodic Damage of 45 every 2 sec (225 over 10 sec), no
                // coefficient.
                Effect {
                    id: 1_353_171,
                    spell_id: 1_295_039,
                    index: 1,
                    effect_type: 6,
                    aura: 3,
                    base_points: 45.0,
                    period_ms: 2000,
                    pvp_mult: 1.0,
                    chain_amp: 1.0,
                    target: [6, 0],
                    ..Effect::default()
                },
                // SpellEffect 1353172: no effect type, base points 2: the "2 times as much
                // damage" against Beasts, which the server applies and the export restates
                // (`forever_effect_items::CREATURE_TYPE_MULTIPLIER`).
                Effect {
                    id: 1_353_172,
                    spell_id: 1_295_039,
                    index: 2,
                    base_points: 2.0,
                    pvp_mult: 1.0,
                    chain_amp: 1.0,
                    ..Effect::default()
                },
            ],
            ..Spell::default()
        },
        rock_to_the_face(1_306_816, "Bigger Rock to the Face", 853_501, 14.0),
        rock_to_the_face(1_309_967, "Rock to the Face", 856_584, 7.0),
        dreadfrost_saber_trigger(),
        dreadfrost_saber_damage(),
    ]
}

/// `ATTR_EX_15` bit 13, which both of Dreadfrost Saber's rows carry (SpellMisc `Attributes_15`
/// 8192), as Fiery Plate Gauntlets' two rows do.
const ATTR_15_BIT_13: u32 = 0x2000;

/// Dreadfrost Saber's equip spell (ItemEffect 234011, trigger type 1, on equip): "Adds 1 Frost
/// damage to your melee attacks." The same shape as Fiery Plate Gauntlets' pinned row 7721: an
/// aura on the wearer that casts another spell on a hit.
fn dreadfrost_saber_trigger() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 842335: Attributes_0 448 (a passive aura) and Attributes_15 8192, both row
    // 7721's own.
    attr[0] = 448;
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_294_939,
        name: "Dreadfrost Saber".to_string(),
        // SpellMisc 842335: SchoolMask 1, the aura's own. The damage is the other row's.
        school: 1,
        attr,
        // SpellAuraOptions 248286: ProcChance 100, ProcTypeMask 20 (melee swings and melee
        // abilities dealt, as row 7721) and, in the second word, 32: the main hand's hits only,
        // as Iceblade Hacker's and Warblade of Caer Darrow's "Melee attacks with this weapon"
        // (pinned rows 1298413 and 1298500) and Bloodthrill's "Main Hand melee attacks".
        proc_chance: 100,
        proc_flags: [20, 32],
        // A chance of 100 beside a tooltip that states none: the aura fires whenever its
        // condition is met (`proc_chance_source::ALWAYS`, as the importer wrote for row 7721).
        proc_chance_source: 2,
        // SpellEffect 1316647: Apply Aura (6) of Proc Trigger Spell (42) on the caster, base
        // points 1 (the tooltip's "$s1"), triggering 1294940.
        effects: vec![Effect {
            id: 1_316_647,
            spell_id: 1_294_939,
            index: 0,
            effect_type: 6,
            aura: 42,
            base_points: 1.0,
            pvp_mult: 1.0,
            trigger_id: 1_294_940,
            chain_amp: 1.0,
            target: [1, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}

/// The spell Dreadfrost Saber's aura casts: "Deals 1 Frost Damage." The same shape as Fiery
/// Plate Gauntlets' Fire Strike (pinned row 7714).
fn dreadfrost_saber_damage() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 842336: Attributes_15 8192 and nothing else.
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_294_940,
        name: "Dreadfrost Saber".to_string(),
        // SpellMisc 842336: SchoolMask 16, Frost.
        school: 16,
        attr,
        // SpellMisc 842336: RangeIndex 11, SpellRange's 15 yards.
        max_range: 15.0,
        // SpellCategories 188896: magic.
        defense_type: 1,
        // SpellEffect 1316648: School Damage (2) of 1 to the target (6), with no variance and no
        // spell power or attack power coefficient.
        effects: vec![Effect {
            id: 1_316_648,
            spell_id: 1_294_940,
            index: 0,
            effect_type: 2,
            base_points: 1.0,
            pvp_mult: 1.0,
            chain_amp: 1.0,
            target: [6, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}

/// Bigger Rock on Bigger Stick's (ItemEffect 236115) and Rock on Stick's (236700) chance on hit:
/// "Bash the target in the face, causing N Physical damage every 2 sec for 20 sec." Its
/// SpellEffect row is encrypted, so the effect is the tooltip's: a Periodic Damage aura of N
/// every 2 sec for 20 sec, with no coefficient.
fn rock_to_the_face(id: i32, name: &str, effect_id: i32, tick: f64) -> Spell {
    let mut attr = [0; 17];
    attr[8] = PERIODIC_CAN_CRIT;
    Spell {
        id,
        name: name.to_string(),
        // Tooltip: Physical damage.
        school: 1,
        attr,
        // Tooltip: for 20 sec.
        duration_ms: 20_000,
        // SpellCategories 187338 and 187341: melee, mechanic 15 (bleed).
        defense_type: 2,
        mechanic: 15,
        effects: vec![Effect {
            id: effect_id,
            spell_id: id,
            index: 0,
            effect_type: 6,
            aura: 3,
            base_points: tick,
            period_ms: 2000,
            pvp_mult: 1.0,
            chain_amp: 1.0,
            target: [6, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}
