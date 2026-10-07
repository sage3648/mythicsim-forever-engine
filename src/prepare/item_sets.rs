//! Go sim/core/item_sets.go: item sets, the set bonuses an equipment activates and the status
//! aura each one registers.
//!
//! Go registers a set with `core.NewItemSet` in a package `init`, and keeps only the sets whose
//! items the loaded database holds. Rust reads that final list, in Go's order, from
//! `data/go-tables.json` (`item_sets`), and each class or shared module contributes the sets it
//! implements as [`ItemSet`] values: the list of everything contributed is
//! [`implemented_sets`]. A set Go registers whose bonus the player reaches, and that no module
//! implements, refuses the request.

use crate::contracts::prepared_v2::ActionId;
use crate::data::tables::{tables, ItemSetRow};

use super::env::Environment;
use super::sim::{AuraId, Sim, UnitId};
use super::Refusal;

/// Go `ApplySetBonus`: applies one set bonus, given the status aura that stands for it. Go also
/// hands it the agent; the environment holds that here.
pub(crate) type ApplySetBonus = fn(&mut Environment, AuraId);

/// Go `ItemSet`: a set a module implements.
pub(crate) struct ItemSet {
    pub id: i32,
    pub name: &'static str,
    pub alternative_name: &'static str,
    /// Go `Bonuses`: the set piece requirement and the function applying the bonus.
    pub bonuses: &'static [(i32, ApplySetBonus)],
    /// Go `RequiredProfession`: the proto name of the profession the character must have for
    /// the bonuses to activate, or empty for none.
    pub required_profession: &'static str,
}

/// Go `SetBonus`: a bonus the equipment activates.
struct SetBonus {
    set: &'static ItemSet,
    pieces: i32,
    effect: ApplySetBonus,
}

/// The sets every module implements, searched by name. Classes contribute theirs through
/// `crate::classes::item_sets`.
pub(crate) fn implemented_sets() -> Vec<&'static ItemSet> {
    let mut sets = crate::classes::item_sets();
    sets.extend(super::forever_item_sets::ITEM_SETS.iter());
    sets
}

/// Go's search for the set an item belongs to: by its set ID first, so sets with different
/// names that share an ID all count together, then by name.
fn find_set<'a>(
    sets: &'a [ItemSetRow],
    set_id: i32,
    set_name: &str,
) -> Option<(usize, &'a ItemSetRow)> {
    if set_id > 0 {
        if let Some(found) = sets.iter().enumerate().find(|(_, set)| set.id == set_id) {
            return Some(found);
        }
    }
    sets.iter()
        .enumerate()
        .find(|(_, set)| set.name == set_name || set.alternative_name == set_name)
}

/// Go `Equipment.getSetBonuses`: the bonuses the equipped items reach, in the order the items
/// reach them. `items` is each equipped item's set ID, set name and item ID, in slot order. A
/// bonus Go has and no module implements is a refusal.
fn set_bonuses(
    go_sets: &[ItemSetRow],
    ours: &[&'static ItemSet],
    items: &[(i32, &str, i32)],
) -> Result<Vec<SetBonus>, Refusal> {
    let mut counts = vec![0; go_sets.len()];
    let mut active = Vec::new();
    for &(set_id, set_name, item_id) in items {
        if set_name.is_empty() {
            continue;
        }
        let Some((index, go_set)) = find_set(go_sets, set_id, set_name) else {
            continue;
        };
        counts[index] += 1;
        let pieces = counts[index];
        if !go_set.bonus_pieces.contains(&pieces) {
            continue;
        }
        let implemented = ours
            .iter()
            .find(|set| set.name == go_set.name && set.id == go_set.id)
            .and_then(|set| {
                set.bonuses
                    .iter()
                    .find(|(count, _)| *count == pieces)
                    .map(|(_, effect)| (*set, *effect))
            });
        match implemented {
            Some((set, effect)) => active.push(SetBonus {
                set,
                pieces,
                effect,
            }),
            None => {
                return Err(Refusal::new(
                    "item_set",
                    format!(
                        "set bonus {} {}P is not prepared yet (item {item_id})",
                        go_set.name, pieces
                    ),
                ))
            }
        }
    }
    Ok(active)
}

/// Go `applyItemSetBonusEffects`. Item swapping is refused before this runs, so only the
/// equipped bonuses exist.
pub(crate) fn apply_item_set_bonus_effects(env: &mut Environment) -> Result<(), Refusal> {
    let unit = env.player;
    let equipped: Vec<(i32, String, i32)> = env
        .sim
        .character(unit)
        .equipment
        .iter()
        .map(|item| (item.set_id, item.set_name.clone(), item.id))
        .collect();
    let items: Vec<(i32, &str, i32)> = equipped
        .iter()
        .map(|(id, name, item)| (*id, name.as_str(), *item))
        .collect();
    for bonus in set_bonuses(&tables().item_sets, &implemented_sets(), &items)? {
        if !bonus.set.required_profession.is_empty()
            && !env.sim.has_profession(unit, bonus.set.required_profession)
        {
            continue;
        }
        let aura = make_set_bonus_status_aura(&mut env.sim, unit, bonus.set.name, bonus.pieces);
        (bonus.effect)(env, aura);
    }
    Ok(())
}

/// Go `makeSetBonusStatusAura` for a bonus active at the start: the permanent "<set> <n>P"
/// aura of the gear build phase.
fn make_set_bonus_status_aura(sim: &mut Sim, unit: UnitId, set_name: &str, pieces: i32) -> AuraId {
    sim.register_permanent_gear_aura(unit, format!("{set_name} {pieces}P"))
}

impl Sim {
    /// Go `Aura.ExposeToAPL`: adds a spell ID to the set bonus so a rotation can name it. Only
    /// the aura's action ID is set, as Go's is; its metrics keep the empty ID.
    pub(crate) fn expose_to_apl(&mut self, aura: AuraId, spell_id: i32) -> AuraId {
        self.aura_mut(aura).action_id = Some(ActionId {
            spell_id,
            ..ActionId::default()
        });
        aura
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(id: i32, name: &str, alternative_name: &str, pieces: &[i32]) -> ItemSetRow {
        ItemSetRow {
            id,
            name: name.to_string(),
            alternative_name: alternative_name.to_string(),
            bonus_pieces: pieces.to_vec(),
        }
    }

    fn nothing(_: &mut Environment, _: AuraId) {}

    static OURS: &[ItemSet] = &[
        ItemSet {
            id: 7,
            name: "Alpha",
            alternative_name: "",
            bonuses: &[(2, nothing), (4, nothing)],
            required_profession: "",
        },
        ItemSet {
            id: 0,
            name: "Beta",
            alternative_name: "",
            bonuses: &[(3, nothing)],
            required_profession: "ProfessionEngineering",
        },
    ];

    fn ours() -> Vec<&'static ItemSet> {
        OURS.iter().collect()
    }

    #[test]
    fn a_bonus_is_active_from_the_piece_that_reaches_it() {
        let go = [set(7, "Alpha", "", &[2, 4])];
        let items = [
            (7, "Alpha", 1),
            (0, "", 2),
            (7, "Alpha", 3),
            (7, "Alpha", 4),
        ];
        let active = set_bonuses(&go, &ours(), &items).expect("implemented");
        let reached: Vec<(&str, i32)> = active.iter().map(|b| (b.set.name, b.pieces)).collect();
        assert_eq!(reached, [("Alpha", 2)]);
    }

    #[test]
    fn a_set_id_counts_sets_of_different_names_together() {
        let go = [set(7, "Alpha", "", &[2])];
        let items = [(7, "Alpha", 1), (7, "Alpha Variant", 2)];
        let active = set_bonuses(&go, &ours(), &items).expect("implemented");
        assert_eq!(active.len(), 1);
    }

    #[test]
    fn a_set_without_an_id_is_found_by_name_or_alternative_name() {
        let go = [set(0, "Beta", "Beta Alt", &[3])];
        let items = [(0, "Beta", 1), (0, "Beta Alt", 2), (0, "Beta", 3)];
        let active = set_bonuses(&go, &ours(), &items).expect("implemented");
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].pieces, 3);
    }

    #[test]
    fn items_of_an_unregistered_set_count_for_nothing() {
        let go = [set(7, "Alpha", "", &[2])];
        let items = [(9, "Gamma", 1), (9, "Gamma", 2), (9, "Gamma", 3)];
        assert!(set_bonuses(&go, &ours(), &items)
            .expect("nothing reached")
            .is_empty());
    }

    #[test]
    fn a_registered_set_nobody_implements_refuses_once_its_bonus_is_reached() {
        let go = [set(11, "Gamma", "", &[2, 3])];
        let one = [(11, "Gamma", 1)];
        assert!(set_bonuses(&go, &ours(), &one)
            .expect("no bonus yet")
            .is_empty());
        let two = [(11, "Gamma", 1), (11, "Gamma", 2)];
        let refusal = match set_bonuses(&go, &ours(), &two) {
            Err(refusal) => refusal,
            Ok(_) => panic!("a bonus nobody implements must refuse"),
        };
        assert_eq!(refusal.code, "item_set");
        assert!(refusal.reason.contains("Gamma 2P"));
    }

    #[test]
    fn a_bonus_the_module_lacks_for_a_reached_count_refuses() {
        let go = [set(7, "Alpha", "", &[2, 3])];
        let items = [(7, "Alpha", 1), (7, "Alpha", 2), (7, "Alpha", 3)];
        assert!(set_bonuses(&go, &ours(), &items).is_err());
    }
}
