//! Game data imported from the pinned Go reference by `tools/rust_data.py`.
//!
//! Each table is a JSON Lines file under `data/`, one row per line sorted by its key, with the
//! key first. A table is indexed on first use by reading only each line's key, and a row is
//! parsed only when asked for, so preparing a request touches the few rows it needs.

mod forever_client_spells;
pub(crate) mod spells;
pub(crate) mod tables;

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::contracts::request::Message;

/// A JSON Lines table indexed by the integer key that starts each line.
pub(crate) struct Table {
    name: &'static str,
    text: &'static str,
    index: OnceLock<Vec<(i64, usize, usize)>>,
}

impl Table {
    pub(crate) const fn new(name: &'static str, text: &'static str) -> Table {
        Table {
            name,
            text,
            index: OnceLock::new(),
        }
    }

    fn index(&self) -> &[(i64, usize, usize)] {
        self.index.get_or_init(|| {
            let mut index = Vec::new();
            let mut start = 0;
            for line in self.text.split_inclusive('\n') {
                let end = start + line.trim_end_matches('\n').len();
                if end > start {
                    let key = leading_key(&self.text[start..end])
                        .unwrap_or_else(|| panic!("data/{}: a row without a key", self.name));
                    index.push((key, start, end));
                }
                start += line.len();
            }
            assert!(
                index.windows(2).all(|pair| pair[0].0 < pair[1].0),
                "data/{} is not sorted by its key",
                self.name
            );
            index
        })
    }

    /// The row with this key, as its JSON text.
    pub(crate) fn row(&self, key: i64) -> Option<&'static str> {
        let index = self.index();
        let at = index.binary_search_by_key(&key, |entry| entry.0).ok()?;
        let (_, start, end) = index[at];
        Some(&self.text[start..end])
    }

    /// Every key, in order.
    pub(crate) fn keys(&self) -> impl Iterator<Item = i64> + '_ {
        self.index().iter().map(|entry| entry.0)
    }
}

/// The integer value of a line's first member, `{"key":123,...`.
fn leading_key(line: &str) -> Option<i64> {
    let rest = line.strip_prefix("{\"")?;
    let rest = &rest[rest.find("\":")? + 2..];
    let end = rest.find([',', '}'])?;
    rest[..end].parse().ok()
}

macro_rules! table {
    ($name:ident, $file:literal) => {
        pub(crate) static $name: Table =
            Table::new($file, include_str!(concat!("../data/", $file)));
    };
}

table!(ITEMS, "items.jsonl");
// The live client's equippable items that ITEMS lacks, from `tools/rust_client_items.py`.
table!(CLIENT_ITEMS, "forever-client-items.jsonl");
// The creature type attack power bonuses of CLIENT_ITEMS' items, which an item row cannot state
// (`tools/rust_client_items.py`, `src/prepare/forever_items.rs`).
table!(CLIENT_ITEM_BONUSES, "forever-client-item-bonuses.jsonl");
// The live client's rows for items whose ITEMS row the live game has since changed, read before
// ITEMS under Forever's rules (`src/prepare/items.rs`).
table!(LIVE_ITEMS, "forever-live-items.jsonl");
table!(RANDOM_SUFFIXES, "random-suffixes.jsonl");
table!(ENCHANTS, "enchants.jsonl");
table!(RAND_PROP_POINTS, "rand-prop-points.jsonl");
table!(CONSUMABLES, "consumables.jsonl");
table!(SPELL_EFFECTS, "spell-effects.jsonl");
table!(SPELL_ROWS, "spells.jsonl");

/// A database row as its proto message, such as a `proto.SimItem` from [`ITEMS`].
/// The decoded rows read so far, by table, message type and key.
type RowCache = HashMap<(&'static str, &'static str, i64), Option<std::rc::Rc<Message>>>;

pub(crate) fn proto_row(
    table: &Table,
    type_name: &'static str,
    key: i64,
) -> Option<std::rc::Rc<Message>> {
    // Preparation reads the same rows for every reset simulation it builds; each row is read
    // once per thread.
    thread_local! {
        static ROWS: std::cell::RefCell<RowCache> =
            std::cell::RefCell::new(HashMap::new());
    }
    ROWS.with(|rows| {
        rows.borrow_mut()
            .entry((table.name, type_name, key))
            .or_insert_with(|| {
                let text = table.row(key)?;
                Some(std::rc::Rc::new(
                    Message::from_json_text(type_name, text)
                        .unwrap_or_else(|err| panic!("data/{} row {key}: {err}", table.name)),
                ))
            })
            .clone()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_rows_by_key() {
        let item = proto_row(&ITEMS, "proto.SimItem", 647).expect("Destiny");
        assert_eq!(item.str("name"), "Destiny");
        assert!(proto_row(&ITEMS, "proto.SimItem", 1).is_none());
        let enchant = proto_row(&ENCHANTS, "proto.SimEnchant", 15).expect("Light Armor Kit");
        assert_eq!(enchant.str("name"), "Light Armor Kit");
        assert!(ITEMS.keys().count() > 10_000);
    }

    #[test]
    fn client_items_are_rows_the_reference_database_lacks() {
        let ids: Vec<i64> = CLIENT_ITEMS.keys().collect();
        assert!(ids.len() > 300);
        for id in ids {
            assert!(ITEMS.row(id).is_none(), "item {id} is in both tables");
            let item = proto_row(&CLIENT_ITEMS, "proto.SimItem", id).expect("a client item");
            assert_eq!(item.i32("id"), id as i32);
            assert!(!item.str("name").is_empty());
        }
        let cloak = proto_row(&CLIENT_ITEMS, "proto.SimItem", 1733).expect("Worn Cloak");
        assert_eq!(cloak.str("name"), "Worn Cloak");
    }

    /// A live row replaces a reference row: the same item, slot and kind, so only its numbers move.
    #[test]
    fn live_items_replace_reference_rows_of_the_same_item() {
        let ids: Vec<i64> = LIVE_ITEMS.keys().collect();
        assert!(!ids.is_empty());
        for id in ids {
            assert!(CLIENT_ITEMS.row(id).is_none(), "item {id} is a client item");
            let live = proto_row(&LIVE_ITEMS, "proto.SimItem", id).expect("a live item");
            let reference = proto_row(&ITEMS, "proto.SimItem", id)
                .unwrap_or_else(|| panic!("live item {id} has no reference row"));
            assert_ne!(
                LIVE_ITEMS.row(id),
                ITEMS.row(id),
                "live item {id} repeats its reference row"
            );
            for field in ["name", "set_name"] {
                assert_eq!(live.str(field), reference.str(field), "item {id} {field}");
            }
            for field in [
                "type",
                "armor_type",
                "weapon_type",
                "hand_type",
                "ranged_weapon_type",
            ] {
                assert_eq!(
                    live.enum_name(field),
                    reference.enum_name(field),
                    "item {id} {field}"
                );
            }
        }
    }

    #[test]
    fn reads_a_leading_key() {
        assert_eq!(leading_key(r#"{"id":25,"name":"x"}"#), Some(25));
        assert_eq!(leading_key(r#"{"ilvl":-3}"#), Some(-3));
        assert_eq!(leading_key(r#"{"name":"x"}"#), None);
    }
}
