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
//! Maw of Min'loth's two rows (1295064 and 1295065) are not encrypted either: every field of
//! theirs is the client's, read from wago.tools `product=wowf` on 11 November 2026.
//!
//! Nor is Toofbreaker's row (1320269): every field of it is the client's, read from wago.tools
//! `product=wowf` on 11 November 2026 (the tables of build 1.60.1.70888).
//!
//! Nor are Pyromancer's Baton's Ablaze (1320925) and Sword of Dismemberment's Dismember
//! (1324687): every field of theirs is the client's, read from wago.tools `product=wowf` on 13
//! November 2026 (the tables of build 1.60.1.70888).
//!
//! Nor is Lightcrest Hammer's Smite (1299631): every field of it is the client's, read from
//! wago.tools `product=wowf` on 13 November 2026 (the tables of build 1.60.1.70888).
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
        maw_of_minloth_trigger(),
        maw_of_minloth_leech(),
        toofbreak(),
        ablaze(),
        dismember(),
        smite(),
    ]
}

/// Lightcrest Hammer's chance on hit (ItemEffect 234878, trigger type 2, no rate and no cooldown
/// stated): "Smites a target for 105 Holy damage. Deals 2 times as much damage to Undead." The
/// row is its own trigger, a Holy hit on the magic table.
fn smite() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 846916: Attributes_15 8192 and nothing else.
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_299_631,
        name: "Smite".to_string(),
        // SpellMisc 846916: SchoolMask 2, Holy.
        school: 2,
        attr,
        // SpellLevels 152461: SpellLevel 30.
        spell_level: 30,
        // SpellMisc 846916: RangeIndex 6, SpellRange's 100 yards.
        max_range: 100.0,
        // SpellCategories 185137: magic.
        defense_type: 1,
        effects: vec![
            // SpellEffect 1323965: School Damage (2) of 105 to the target (6), with no variance
            // and no spell power or attack power coefficient.
            Effect {
                id: 1_323_965,
                spell_id: 1_299_631,
                index: 0,
                effect_type: 2,
                base_points: 105.0,
                spell_level: 30,
                pvp_mult: 1.0,
                chain_amp: 1.0,
                target: [6, 0],
                ..Effect::default()
            },
            // SpellEffect 1323966: a Dummy (3) with base points 2 on the target (6): the "2 times
            // as much damage" to Undead, which the server applies and preparation restates
            // (`forever_effect_items::CREATURE_TYPE_DAMAGE_ITEMS`).
            Effect {
                id: 1_323_966,
                spell_id: 1_299_631,
                index: 1,
                effect_type: 3,
                base_points: 2.0,
                spell_level: 30,
                pvp_mult: 1.0,
                chain_amp: 1.0,
                target: [6, 0],
                ..Effect::default()
            },
        ],
        ..Spell::default()
    }
}

/// Pyromancer's Baton's chance on hit (ItemEffect 238191, trigger type 2, no rate and no cooldown
/// stated): "Burns the target for 30 Fire damage every 3 sec for 9 sec." The row is its own
/// trigger, as a reference chance on hit weapon's is, and has the shape of Flaming Incinerator's
/// Incinerator (pinned row 1294537): a Fire damage over time alone, on the magic table.
fn ablaze() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 866630: Attributes_8 512 (`PERIODIC_CAN_CRIT`) and Attributes_15 8192, and
    // nothing else.
    attr[8] = PERIODIC_CAN_CRIT;
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_320_925,
        name: "Ablaze".to_string(),
        // SpellMisc 866630: SchoolMask 4, Fire.
        school: 4,
        attr,
        // SpellMisc 866630: DurationIndex 105, SpellDuration's 9000 ms.
        duration_ms: 9000,
        // SpellMisc 866630: RangeIndex 577, SpellRange's 8 yards.
        max_range: 8.0,
        // SpellCategories 189172: magic defense, magic dispel.
        defense_type: 1,
        dispel_type: 1,
        // SpellAuraOptions 248366: ProcChance 101 and nothing else, as Incinerator's row. The
        // aura procs nothing; the importer writes a chance of 101 as `ALWAYS`.
        proc_chance: 101,
        proc_chance_source: 2,
        // SpellEffect 1355874: Apply Aura (6) of Periodic Damage (3) of 30 every 3000 ms on the
        // target (6), with no variance and no spell power or attack power coefficient.
        effects: vec![Effect {
            id: 1_355_874,
            spell_id: 1_320_925,
            index: 0,
            effect_type: 6,
            aura: 3,
            base_points: 30.0,
            period_ms: 3000,
            pvp_mult: 1.0,
            chain_amp: 1.0,
            target: [6, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}

/// Sword of Dismemberment's chance on hit (ItemEffect 238815, trigger type 2, no rate and no
/// cooldown stated): "Dismember the target, inflicting 335 to 1005 Physical damage." The row is
/// its own trigger, as Toofbreak's is, a Physical hit on the melee table.
fn dismember() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 870298: Attributes_3 65536, Attributes_4 8388608 and Attributes_15 8192.
    attr[3] = 65_536;
    attr[4] = 8_388_608;
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_324_687,
        name: "Dismember".to_string(),
        // SpellMisc 870298: SchoolMask 1, Physical.
        school: 1,
        attr,
        // SpellLevels 154036: SpellLevel 1.
        spell_level: 1,
        // SpellMisc 870298: RangeIndex 13, SpellRange's 50000 yards ("Anywhere").
        max_range: 50_000.0,
        // SpellCategories 189951: melee.
        defense_type: 2,
        // SpellEffect 1361623: School Damage (2) of 670 to the target (6), variance 1 (335 to
        // 1005, the tooltip's), with no spell power or attack power coefficient.
        effects: vec![Effect {
            id: 1_361_623,
            spell_id: 1_324_687,
            index: 0,
            effect_type: 2,
            base_points: 670.0,
            spell_level: 1,
            variance: 1.0,
            pvp_mult: 1.0,
            chain_amp: 1.0,
            target: [6, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}

/// Toofbreaker's chance on hit (ItemEffect 238089, trigger type 2, no rate and no cooldown
/// stated): "Bash the target, causing 42 Physical damage. Deals 2 times as much damage if the
/// target is facing you." The row is its own trigger, as a reference chance on hit weapon's is.
fn toofbreak() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 866009: Attributes_0 262144 and Attributes_15 8192.
    attr[0] = 262_144;
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_320_269,
        name: "Toofbreak".to_string(),
        // SpellMisc 866009: SchoolMask 1, Physical.
        school: 1,
        attr,
        // SpellMisc 866009: RangeIndex 6, SpellRange's 100 yards.
        max_range: 100.0,
        // SpellCooldowns 101719: RecoveryTime 100. A proc's spell has no cooldown of its own
        // here (`resolve_spell::proc`), so nothing reads it.
        cooldown_ms: 100,
        // SpellCategories 189061: melee.
        defense_type: 2,
        effects: vec![
            // SpellEffect 1354774: School Damage (2) of 42 to the target (6), with no variance
            // and no spell power or attack power coefficient.
            Effect {
                id: 1_354_774,
                spell_id: 1_320_269,
                index: 0,
                effect_type: 2,
                base_points: 42.0,
                pvp_mult: 1.0,
                chain_amp: 1.0,
                target: [6, 0],
                ..Effect::default()
            },
            // SpellEffect 1354775: a Dummy (3) with base points 2 and no target: the "2 times
            // as much damage" to a target facing the wearer, which the server applies and
            // preparation restates (`forever_effect_items::FACING_DAMAGE_ITEMS`).
            Effect {
                id: 1_354_775,
                spell_id: 1_320_269,
                index: 1,
                effect_type: 3,
                base_points: 2.0,
                pvp_mult: 1.0,
                chain_amp: 1.0,
                ..Effect::default()
            },
        ],
        ..Spell::default()
    }
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

/// Maw of Min'loth's equip spell (ItemEffect 234017, trigger type 1, on equip): "When struck in
/// melee combat, has a 10% chance of stealing 40 life from the target enemy." The same shape as
/// Skullflame Shield's Drain Life (pinned row 18815): an aura on the wearer that casts a leech on
/// the attacker when a melee hit lands on the wearer.
fn maw_of_minloth_trigger() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 842470: Attributes_0 64 (a passive), row 18815's own, and nothing else.
    attr[0] = 64;
    Spell {
        id: 1_295_064,
        name: "Maw of Min'loth".to_string(),
        // SpellMisc 842470: SchoolMask 1, the aura's own. The leech is the other row's.
        school: 1,
        attr,
        // SpellAuraOptions 247408: ProcChance 10 (the tooltip's 10%), ProcCategoryRecovery 3000
        // (the tooltip's 3s cooldown) and ProcTypeMask 40: melee swings and melee abilities
        // taken, row 18815's own mask.
        proc_chance: 10,
        proc_flags: [40, 0],
        icd_ms: 3000,
        // The chance column is the roll (`proc_chance_source::COLUMN`, the default).
        // SpellEffect 1316865: Apply Aura (6) of Proc Trigger Spell (42) on the caster, base
        // points 0, triggering 1295065.
        ref_ids: vec![1_295_065],
        effects: vec![Effect {
            id: 1_316_865,
            spell_id: 1_295_064,
            index: 0,
            effect_type: 6,
            aura: 42,
            pvp_mult: 1.0,
            trigger_id: 1_295_065,
            chain_amp: 1.0,
            target: [1, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}

/// The spell Maw of Min'loth's aura casts: a Health Leech of 40. The same shape as Skullflame
/// Shield's pinned row 18817, in the Physical school on the melee table where that one is Shadow
/// on the magic table.
fn maw_of_minloth_leech() -> Spell {
    let mut attr = [0; 17];
    // SpellMisc 842471: Attributes_0 262160 and Attributes_15 8192.
    attr[0] = 262_160;
    attr[15] = ATTR_15_BIT_13;
    Spell {
        id: 1_295_065,
        name: "Maw of Min'loth".to_string(),
        // SpellMisc 842471: SchoolMask 1, Physical.
        school: 1,
        attr,
        // SpellMisc 842471: RangeIndex 11, SpellRange's 15 yards.
        max_range: 15.0,
        // SpellCategories 184098: melee.
        defense_type: 2,
        // SpellEffect 1316866: Health Leech (9) of 40 from the target (6), with no variance and
        // no spell power or attack power coefficient; EffectAmplitude 1, the share of the damage
        // that heals the wearer.
        effects: vec![Effect {
            id: 1_316_866,
            spell_id: 1_295_065,
            index: 0,
            effect_type: 9,
            base_points: 40.0,
            pvp_mult: 1.0,
            amplitude: 1.0,
            chain_amp: 1.0,
            target: [6, 0],
            ..Effect::default()
        }],
        ..Spell::default()
    }
}
