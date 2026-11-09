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
    ]
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
