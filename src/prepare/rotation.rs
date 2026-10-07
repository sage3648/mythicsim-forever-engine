//! Go sim/core/apl.go `newAPLRotation`, for what building the rotation changes in the
//! prepared state: the prepull actions it registers and the major cooldowns it removes
//! because it casts them itself.
//!
//! Go builds every action and value of the rotation. Most of that only reads the simulation;
//! preparation follows the part that changes it. An action whose spell the player does not
//! know is dropped, as Go's constructors return nil for it; a condition Go folds to a constant
//! is refused, since only Go's constant folding can say whether it prunes the action.

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::{Message, Value};

use super::agent::proto_to_action_id;
use super::env::Environment;
use super::sim::{SpellId, UnitId};
use super::spell::SpellFlag;
use super::Refusal;

/// Go `GetAPLSpell`.
fn apl_spell(env: &Environment, id: &ActionId) -> Option<SpellId> {
    let sim = &env.sim;
    let book = &sim.unit(env.player).spellbook;
    if id.other_id == "OtherActionPotion" && id.spell_id == 0 && id.item_id == 0 {
        return book
            .iter()
            .copied()
            .find(|s| sim.spell(*s).flags.matches(SpellFlag::COMBAT_POTION));
    }
    book.iter()
        .copied()
        .find(|s| &sim.spell(*s).action_id == id && sim.spell(*s).flags.matches(SpellFlag::APL))
        .or_else(|| sim.get_spell(env.player, id))
}

/// Go `GetAPLCastSpell`.
fn apl_cast_spell(env: &Environment, id: &ActionId) -> Option<SpellId> {
    let sim = &env.sim;
    if let Some(spell) = sim.get_spell(env.player, id) {
        if !sim.spell(spell).flags.matches(SpellFlag::APL) {
            if let Some(queued) = sim.unit(env.player).spellbook.iter().copied().find(|s| {
                let other = &sim.spell(*s).action_id;
                other.same_action_ignore_tag(id) && sim.spell(*s).flags.matches(SpellFlag::APL)
            }) {
                return Some(queued);
            }
        }
    }
    apl_spell(env, id)
}

/// Whether an action's target reference resolves: Go `GetTargetUnit(...).Get() != nil`.
fn target_resolves(env: &Environment, target: Option<&Message>) -> Result<bool, Refusal> {
    let Some(target) = target else {
        return Ok(true);
    };
    let kind = target.enum_name("type");
    match kind.as_str() {
        "Unknown" | "CurrentTarget" | "Self" => Ok(true),
        "Target" => Ok((target.i32("index") as usize) < env.encounter.targets.len()),
        "Player" => Ok(target.i32("index") == 0),
        // Go's `NextActiveTargetUnit` and `PreviousActiveTargetUnit` always name a target of
        // the player's current one.
        "NextTarget" | "PreviousTarget" => Ok(env
            .sim
            .unit(env.player)
            .current_target
            .is_some_and(|t| env.encounter.targets.contains(&t))),
        other => Err(Refusal::new(
            "rotation",
            format!("a rotation target of type {other} is not prepared yet"),
        )),
    }
}

/// Go `spell.CurDot() != nil`: the spell's dot on its caster's current target, or its related
/// dot spell's.
fn has_cur_dot(env: &Environment, spell: SpellId) -> Result<bool, Refusal> {
    let s = env.sim.spell(spell);
    if s.dots.is_empty() {
        return match s.related_dot_spell {
            Some(related) => has_cur_dot(env, related),
            None => Ok(false),
        };
    }
    let Some(target) = env.sim.unit(s.unit).current_target else {
        return Err(Refusal::new(
            "rotation",
            "a dot read without a current target".to_string(),
        ));
    };
    let index = env.sim.unit(target).unit_index as usize;
    Ok(s.dots.get(index).copied().flatten().is_some())
}

/// One built action: Go's `APLAction` reduced to what construction needs.
struct Built {
    /// Spells a cast action presses, including those of inner actions.
    casts: Vec<SpellId>,
    /// The spell of a cast action itself: what Go's `removeFromMajorCooldowns` reads of a
    /// pruned action's impl.
    direct: Option<SpellId>,
}

impl Built {
    fn new(casts: Vec<SpellId>) -> Self {
        Built {
            casts,
            direct: None,
        }
    }
}

struct Builder<'a> {
    env: &'a Environment,
    /// Go `rot.prunedActions`: impls of actions a constant false condition removed.
    pruned_casts: Vec<SpellId>,
    /// The values Go constructs: those of every action whose impl it builds, and their
    /// conditions. An action Go drops for its impl never builds its condition.
    built_values: Vec<Message>,
}

/// A value's type as far as Go's constructors check it: only a boolean matters, to Go
/// `newValueCompare`'s check that booleans are only compared for equality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ty {
    Bool,
    Other,
    Unknown,
}

/// What Go's constructor returns for a value: nil, a value read during the fight, or an
/// `APLValueConst` with its type and `boolVal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Fold {
    Nil,
    Dyn(Ty),
    Const(Ty, bool),
}

impl Fold {
    const FALSE: Fold = Fold::Const(Ty::Bool, false);
    const TRUE: Fold = Fold::Const(Ty::Bool, true);

    fn ty(self) -> Option<Ty> {
        match self {
            Fold::Nil => None,
            Fold::Dyn(ty) | Fold::Const(ty, _) => Some(ty),
        }
    }
}

/// Every result Go's constructor may give for a value. Preparation does not follow every
/// value's constructor, so a value it does not follow may be nil or read during the fight;
/// the set keeps every possibility, and a decision that depends on which one is refused.
type Folds = std::collections::BTreeSet<Fold>;

fn one(fold: Fold) -> Folds {
    Folds::from([fold])
}

/// Go `coerceTo(value, ValueTypeBool)`: a constant keeps its `boolVal` with the new type.
fn coerce_bool(folds: &Folds) -> Folds {
    folds
        .iter()
        .map(|fold| match *fold {
            Fold::Nil => Fold::Nil,
            Fold::Dyn(_) => Fold::Dyn(Ty::Bool),
            Fold::Const(_, b) => Fold::Const(Ty::Bool, b),
        })
        .collect()
}

/// Go `newValueConst`: `boolVal` is whether the text is empty, unless it says true or false.
fn const_fold(text: &str) -> Fold {
    if text.eq_ignore_ascii_case("true") {
        Fold::TRUE
    } else if text.eq_ignore_ascii_case("false") {
        Fold::FALSE
    } else {
        Fold::Const(Ty::Other, !text.is_empty())
    }
}

/// The non-nil values of a list Go filters, as the possible outcomes of Go `newValueAnd`,
/// `newValueOr`, `newValueMin` and `newValueMax`: nil for none, the value itself for one, and
/// for more, whether any of them is the constant `short` (false for And, true for Or).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Gathered {
    Empty,
    One(Fold),
    Many { short: bool },
}

fn gather(children: &[Folds], short: Option<Fold>) -> std::collections::BTreeSet<Gathered> {
    let mut states = std::collections::BTreeSet::from([Gathered::Empty]);
    for child in children {
        let mut next = std::collections::BTreeSet::new();
        for state in &states {
            for fold in child {
                let is_short = Some(*fold) == short;
                next.insert(match (*state, *fold) {
                    (state, Fold::Nil) => state,
                    (Gathered::Empty, fold) => Gathered::One(fold),
                    (Gathered::One(first), _) => Gathered::Many {
                        short: Some(first) == short || is_short,
                    },
                    (Gathered::Many { short: any }, _) => Gathered::Many {
                        short: any || is_short,
                    },
                });
            }
        }
        states = next;
    }
    states
}

/// Go `newValueAnd` (short constant false) and `newValueOr` (short constant true).
fn fold_logic(children: &[Folds], short: Fold) -> Folds {
    let children: Vec<Folds> = children.iter().map(coerce_bool).collect();
    gather(&children, Some(short))
        .into_iter()
        .map(|state| match state {
            Gathered::Empty => Fold::Nil,
            Gathered::One(fold) => fold,
            Gathered::Many { short: true } => short,
            Gathered::Many { short: false } => Fold::Dyn(Ty::Bool),
        })
        .collect()
}

/// The unit a value's source unit reference names while the rotation is built: `None` for a
/// reference Go resolves to no unit, an error for one preparation does not follow.
fn source_unit(env: &Environment, reference: Option<&Message>) -> Result<Option<UnitId>, ()> {
    let kind = reference.map_or_else(|| "Unknown".to_string(), |r| r.enum_name("type"));
    match kind.as_str() {
        "Unknown" | "Self" => Ok(Some(env.player)),
        "CurrentTarget" => Ok(env.sim.unit(env.player).current_target),
        // Go `NextActiveTarget` and `PreviousActiveTarget` of the current target: every target
        // is enabled while the rotation is built, as preparation refuses one disabled at start.
        "NextTarget" | "PreviousTarget" => {
            let targets = &env.encounter.targets;
            let Some(current) = env.sim.unit(env.player).current_target else {
                return Err(());
            };
            let Some(index) = targets.iter().position(|t| *t == current) else {
                return Err(());
            };
            let next = if kind == "NextTarget" {
                (index + 1) % targets.len()
            } else {
                (index + targets.len() - 1) % targets.len()
            };
            Ok(Some(targets[next]))
        }
        "Target" => {
            let index = reference.map_or(0, |r| r.i32("index"));
            Ok(usize::try_from(index)
                .ok()
                .and_then(|i| env.encounter.targets.get(i).copied()))
        }
        // Go `Environment.GetUnit`: a pet of the owner the reference names, by its index.
        "Pet" => {
            let index = reference.map_or(0, |r| r.i32("index"));
            let owner = reference
                .and_then(|r| r.message("owner"))
                .map(|owner| (owner.enum_name("type"), owner.i32("index")));
            let owner_is_player = match owner {
                Some((kind, _)) if kind == "Self" => true,
                Some((kind, index)) if kind == "Player" => index == 0,
                _ => false,
            };
            Ok(owner_is_player
                .then(|| {
                    usize::try_from(index)
                        .ok()
                        .and_then(|i| env.sim.unit(env.player).pets.get(i).copied())
                })
                .flatten())
        }
        _ => Err(()),
    }
}

/// Go `GetAPLAura(GetSourceUnit(...), id).Get()`: whether the aura exists, or `None` when
/// preparation does not follow the reference.
fn aura_known(env: &Environment, config: &Message) -> Option<bool> {
    let unit = source_unit(env, config.message("source_unit")).ok()?;
    let id = proto_to_action_id(config.message("aura_id")?);
    Some(unit.is_some_and(|unit| {
        env.sim
            .unit(unit)
            .auras
            .iter()
            .any(|aura| env.sim.aura(*aura).action_id.as_ref() == Some(&id))
    }))
}

/// Go `GetAPLAura(...).Get().MaxStacks`: `Some(None)` for an aura the unit lacks, `None` when
/// preparation does not follow the reference.
fn aura_stacks(env: &Environment, config: &Message) -> Option<Option<i32>> {
    let unit = source_unit(env, config.message("source_unit")).ok()?;
    let id = proto_to_action_id(config.message("aura_id")?);
    Some(unit.and_then(|unit| {
        env.sim
            .unit(unit)
            .auras
            .iter()
            .find(|aura| env.sim.aura(**aura).action_id.as_ref() == Some(&id))
            .map(|aura| env.sim.aura(*aura).max_stacks)
    }))
}

/// Go `spell.Dot(target)`, following the related dot spell.
fn spell_dot(env: &Environment, spell: SpellId, target: UnitId) -> bool {
    let s = env.sim.spell(spell);
    if s.dots.is_empty() {
        return s
            .related_dot_spell
            .is_some_and(|related| spell_dot(env, related, target));
    }
    let index = env.sim.unit(target).unit_index as usize;
    s.dots.get(index).copied().flatten().is_some()
}

/// Go `NewDotReference(GetTargetUnit(...), id).Get() != nil`, or `None` when preparation does
/// not follow the reference.
fn dot_exists(env: &Environment, config: &Message) -> Option<bool> {
    let Some(id) = config.message("spell_id") else {
        return Some(false);
    };
    let reference = config.message("target_unit");
    let kind = reference.map_or_else(|| "Unknown".to_string(), |r| r.enum_name("type"));
    let target = match kind.as_str() {
        "Unknown" | "CurrentTarget" => env.sim.unit(env.player).current_target,
        "Target" => {
            let index = reference.map_or(0, |r| r.i32("index"));
            usize::try_from(index)
                .ok()
                .and_then(|i| env.encounter.targets.get(i).copied())
        }
        _ => return None,
    };
    let Some(target) = target else {
        return Some(false);
    };
    let Some(spell) = apl_spell(env, &proto_to_action_id(id)) else {
        return Some(false);
    };
    Some(env.sim.spell(spell).aoe_dot.is_some() || spell_dot(env, spell, target))
}

/// Go's value constructors, as far as constant folding goes: `newAPLValue` for a condition.
fn fold(env: &Environment, value: &Message) -> Folds {
    let unknown = || Folds::from([Fold::Nil, Fold::Dyn(Ty::Unknown)]);
    let Some((kind, Value::Message(config))) = value.oneof("value") else {
        return one(Fold::Nil);
    };
    // An aura value: nil without an aura id, `missing` for an aura the unit lacks.
    let aura_value = |missing: Fold, present: Fold| -> Folds {
        if config.message("aura_id").is_none() {
            return one(Fold::Nil);
        }
        match aura_known(env, config) {
            Some(true) => one(present),
            Some(false) => one(missing),
            None => Folds::from([missing, present]),
        }
    };
    match kind {
        "const" => one(const_fold(config.str("val"))),
        "and" => fold_logic(
            &config
                .messages("vals")
                .into_iter()
                .map(|v| fold(env, v))
                .collect::<Vec<_>>(),
            Fold::FALSE,
        ),
        "or" => fold_logic(
            &config
                .messages("vals")
                .into_iter()
                .map(|v| fold(env, v))
                .collect::<Vec<_>>(),
            Fold::TRUE,
        ),
        "not" => {
            let inner = config
                .message("val")
                .map_or_else(|| one(Fold::Nil), |v| fold(env, v));
            coerce_bool(&inner)
                .into_iter()
                .map(|fold| match fold {
                    Fold::Const(_, b) => Fold::Const(Ty::Bool, !b),
                    other => other,
                })
                .collect()
        }
        "min" | "max" => {
            let children: Vec<Folds> = config
                .messages("vals")
                .into_iter()
                .map(|v| fold(env, v))
                .collect();
            // coerceAllToSameType may change a type; a lone value is returned as it is.
            gather(&children, None)
                .into_iter()
                .map(|state| match state {
                    Gathered::Empty => Fold::Nil,
                    Gathered::One(Fold::Const(_, b)) => Fold::Const(Ty::Unknown, b),
                    Gathered::One(_) | Gathered::Many { .. } => Fold::Dyn(Ty::Unknown),
                })
                .collect()
        }
        "cmp" | "math" => {
            let side = |name: &str| {
                config
                    .message(name)
                    .map_or_else(|| one(Fold::Nil), |v| fold(env, v))
            };
            let (lhs, rhs) = (side("lhs"), side("rhs"));
            let mut out = Folds::new();
            for l in &lhs {
                for r in &rhs {
                    if *l == Fold::Nil || *r == Fold::Nil {
                        out.insert(Fold::Nil);
                        continue;
                    }
                    let equality =
                        kind == "cmp" && matches!(config.enum_name("op").as_str(), "OpEq" | "OpNe");
                    let checked = [l.ty(), r.ty()].into_iter().all(|ty| ty == Some(Ty::Other));
                    // Go nils comparisons of booleans other than for equality and math on
                    // types it rejects; only two other-typed operands of a comparison are
                    // certain to give a value.
                    if !(kind == "cmp" && (equality || checked)) {
                        out.insert(Fold::Nil);
                    }
                    out.insert(Fold::Dyn(if kind == "cmp" {
                        Ty::Bool
                    } else {
                        Ty::Unknown
                    }));
                }
            }
            out
        }
        "aura_is_known" => match aura_known(env, config) {
            Some(known) => one(Fold::Const(Ty::Bool, known)),
            None => Folds::from([Fold::FALSE, Fold::TRUE]),
        },
        "spell_is_known" => match config.message("spell_id") {
            Some(id) => one(Fold::Const(
                Ty::Bool,
                apl_spell(env, &proto_to_action_id(id)).is_some(),
            )),
            None => unknown(),
        },
        "aura_is_active" => aura_value(Fold::FALSE, Fold::Dyn(Ty::Bool)),
        "aura_is_inactive" => aura_value(Fold::TRUE, Fold::Dyn(Ty::Bool)),
        // Go's constant "0ms": its boolVal is true, as the text is not empty.
        "aura_remaining_time" => aura_value(Fold::Const(Ty::Other, true), Fold::Dyn(Ty::Other)),
        // Go's constant "0" for an aura the unit lacks, and nil for one without stacks.
        "aura_num_stacks" => {
            if config.message("aura_id").is_none() {
                return one(Fold::Nil);
            }
            match aura_stacks(env, config) {
                Some(None) => one(Fold::Const(Ty::Other, true)),
                Some(Some(0)) => one(Fold::Nil),
                Some(Some(_)) => one(Fold::Dyn(Ty::Other)),
                None => Folds::from([
                    Fold::Const(Ty::Other, true),
                    Fold::Nil,
                    Fold::Dyn(Ty::Other),
                ]),
            }
        }
        // Values Go always builds.
        "remaining_time"
        | "remaining_time_percent"
        | "current_time"
        | "current_time_percent"
        | "number_targets" => one(Fold::Dyn(Ty::Other)),
        // Nil for a unit without mana.
        "current_mana_percent" => match source_unit(env, config.message("source_unit")) {
            Ok(Some(unit)) if env.sim.unit(unit).mana_bar.enabled => one(Fold::Dyn(Ty::Other)),
            Ok(_) => one(Fold::Nil),
            Err(()) => unknown(),
        },
        // Go `NewDotReference`: nil when the spell has no dot on the target.
        "dot_is_active" | "dot_remaining_time" | "dot_time_to_next_tick" => {
            let ty = if kind == "dot_is_active" {
                Ty::Bool
            } else {
                Ty::Other
            };
            match dot_exists(env, config) {
                Some(true) => one(Fold::Dyn(ty)),
                Some(false) => one(Fold::Nil),
                None => Folds::from([Fold::Nil, Fold::Dyn(ty)]),
            }
        }
        _ => unknown(),
    }
}

impl Builder<'_> {
    /// Go `newAPLAction`: `None` for an action Go's constructor returns nil for or prunes.
    fn action(&mut self, config: Option<&Message>) -> Result<Option<Built>, Refusal> {
        let Some(config) = config else {
            return Ok(None);
        };
        let Some(built) = self.action_impl(config)? else {
            return Ok(None);
        };
        let mut found = Vec::new();
        values(config, &mut found);
        self.built_values.extend(found.into_iter().cloned());
        // Go prunes an action whose condition folds to a constant false, and keeps its impl
        // so the spell it casts still leaves the major cooldowns.
        if let Some(condition) = config.message("condition") {
            let folded = coerce_bool(&fold(self.env, condition));
            let pruned = folded.iter().filter(|f| **f == Fold::FALSE).count();
            if pruned == folded.len() {
                self.pruned_casts.extend(built.direct);
                return Ok(None);
            }
            if pruned > 0 {
                return Err(Refusal::new(
                    "rotation",
                    "a rotation condition Go may fold to a constant is not prepared yet"
                        .to_string(),
                ));
            }
        }
        Ok(Some(built))
    }

    fn action_impl(&mut self, config: &Message) -> Result<Option<Built>, Refusal> {
        // Go asks the agent first: a class builds its own actions.
        if let Some(built) =
            self.env
                .agent
                .custom_apl_action(&self.env.sim, self.env.player, config)
        {
            return Ok(built.then(|| Built::new(Vec::new())));
        }
        let Some((kind, Value::Message(action))) = config.oneof("action") else {
            return Err(Refusal::new(
                "rotation",
                "a rotation action without a kind".to_string(),
            ));
        };
        match kind {
            "cast_spell" | "cast_friendly_spell" => {
                let Some(id) = action.message("spell_id") else {
                    return Ok(None);
                };
                let id = proto_to_action_id(id);
                let spell = if kind == "cast_spell" {
                    apl_cast_spell(self.env, &id)
                } else {
                    apl_spell(self.env, &id)
                };
                let Some(spell) = spell else {
                    return Ok(None);
                };
                if !target_resolves(self.env, action.message("target"))? {
                    return Ok(None);
                }
                Ok(Some(Built {
                    casts: vec![spell],
                    direct: Some(spell),
                }))
            }
            // Go `newActionSequence` drops the steps that are nil and is nil without any;
            // `newActionStrictSequence` is nil when any step is, and prunes the others.
            "sequence" | "strict_sequence" => {
                let mut built = Vec::new();
                let mut missing = false;
                for inner in action.messages("actions") {
                    match self.action(Some(inner))? {
                        Some(step) => built.push(step),
                        None => missing = true,
                    }
                }
                if kind == "strict_sequence" && missing {
                    self.pruned_casts
                        .extend(built.iter().filter_map(|step| step.direct));
                    return Ok(None);
                }
                if built.is_empty() {
                    return Ok(None);
                }
                Ok(Some(Built::new(
                    built.into_iter().flat_map(|step| step.casts).collect(),
                )))
            }
            // Go `newActionMultidot`: dropped unless the spell has a dot on the current
            // target; it removes no major cooldown.
            "multidot" => {
                let Some(id) = action.message("spell_id") else {
                    return Ok(None);
                };
                let Some(spell) = apl_spell(self.env, &proto_to_action_id(id)) else {
                    return Ok(None);
                };
                if !has_cur_dot(self.env, spell)? {
                    return Ok(None);
                }
                Ok(Some(Built::new(Vec::new())))
            }
            // Go builds these without changing the simulation; a move reads its range only
            // when it runs.
            "autocast_other_cooldowns" | "wait" | "wait_until" | "move" | "move_duration" => {
                Ok(Some(Built::new(Vec::new())))
            }
            other => Err(Refusal::new(
                "rotation",
                format!("rotation action {other} is not prepared yet"),
            )),
        }
    }
}

/// Every `proto.APLValue` inside a message, depth first.
/// Every `proto.APLValue` inside an action, depth first, leaving out its inner actions, which
/// Go builds, or drops, on their own.
fn values<'a>(message: &'a Message, out: &mut Vec<&'a Message>) {
    if message.type_name() == "proto.APLValue" {
        out.push(message);
    }
    let walk = |inner: &'a Message, out: &mut Vec<&'a Message>| {
        if inner.type_name() != "proto.APLAction" {
            values(inner, out);
        }
    };
    for name in message.set_fields() {
        match message.get(name) {
            Some(Value::Message(inner)) => walk(inner, out),
            Some(Value::List(items)) => {
                for item in items {
                    if let Value::Message(inner) = item {
                        walk(inner, out);
                    }
                }
            }
            _ => {}
        }
    }
}

/// What building the rotation's values registers: apl_values_aura.go `newValueAuraNumStacks`
/// adds a stack change and a reset callback to the aura it reads, and apl_values_dot.go's base
/// value reads register an aura, which preparation refuses for now.
fn register_value_observers(env: &mut Environment, built: &[Message]) -> Result<(), Refusal> {
    for value in built {
        let Some((kind, Value::Message(config))) = value.oneof("value") else {
            continue;
        };
        match kind {
            "aura_num_stacks" => {
                let Some(id) = config.message("aura_id") else {
                    continue;
                };
                let id = proto_to_action_id(id);
                let unit = match source_unit(env, config.message("source_unit")) {
                    Ok(Some(unit)) => unit,
                    // Go reads a unit it cannot resolve as the constant 0 and registers nothing.
                    Ok(None) => continue,
                    Err(()) => {
                        let kind = config
                            .message("source_unit")
                            .map(|u| u.enum_name("type"))
                            .unwrap_or_default();
                        return Err(Refusal::new(
                            "rotation",
                            format!("an aura stack read on a {kind} unit is not prepared yet"),
                        ));
                    }
                };
                let aura = env
                    .sim
                    .unit(unit)
                    .auras
                    .iter()
                    .copied()
                    .find(|aura| env.sim.aura(*aura).action_id.as_ref() == Some(&id));
                match aura {
                    // Go reads an aura the unit lacks as the constant 0 and registers nothing.
                    None => {}
                    Some(aura) if env.sim.aura(aura).max_stacks > 0 => {
                        env.sim.apply_on_stacks_change(
                            aura,
                            std::rc::Rc::new(|_: &mut super::sim::Sim, _, _, _| {}),
                        );
                        env.sim.apply_on_reset(
                            aura,
                            std::rc::Rc::new(|_: &mut super::sim::Sim, _| {}),
                        );
                    }
                    Some(_) => {}
                }
            }
            "dot_percent_increase"
            | "dot_crit_percent_increase"
            | "dot_tick_rate_percent_increase"
                if config.bool("use_base_value") =>
            {
                return Err(Refusal::new(
                    "rotation",
                    "a dot increase read against its base value is not prepared yet".to_string(),
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Go `newAPLRotation` at finalization.
pub(crate) fn build_rotation(
    env: &mut Environment,
    rotation: Option<&Message>,
) -> Result<(), Refusal> {
    let Some(rotation) = rotation else {
        return Ok(());
    };
    if rotation.enum_name("type") != "TypeAPL" {
        return Err(Refusal::new(
            "rotation",
            "only APL rotations are prepared".to_string(),
        ));
    }
    if !rotation.messages("groups").is_empty() || !rotation.messages("value_variables").is_empty() {
        return Err(Refusal::new(
            "rotation",
            "rotation groups and variables are not prepared yet".to_string(),
        ));
    }
    let mut prepull = 0;
    let mut casts = Vec::new();
    let pruned;
    let built_values;
    {
        let mut builder = Builder {
            env,
            pruned_casts: Vec::new(),
            built_values: Vec::new(),
        };
        for item in rotation.messages("prepull_actions") {
            if item.bool("hide") {
                continue;
            }
            let Some(at) = item.message("do_at_value") else {
                continue;
            };
            let Some(Value::Message(constant)) = at.get("const") else {
                return Err(Refusal::new(
                    "rotation",
                    "a prepull time that is not a constant".to_string(),
                ));
            };
            let do_at = crate::rotation::parse_const(constant.str("val"))
                .map_err(|err| Refusal::new("rotation", err))?
                .duration_ns;
            if do_at > 0 {
                continue;
            }
            if builder.action(item.message("action"))?.is_some() {
                prepull += 1;
            }
        }
        for item in rotation.messages("priority_list") {
            if item.bool("hide") {
                continue;
            }
            if let Some(built) = builder.action(item.message("action"))? {
                casts.extend(built.casts);
            }
        }
        pruned = builder.pruned_casts;
        built_values = builder.built_values;
    }
    register_value_observers(env, &built_values)?;
    env.prepull_actions += prepull;
    let player = env.player;
    for spell in casts.into_iter().chain(pruned) {
        let action = env.sim.spell(spell).action_id.clone();
        env.sim.remove_initial_major_cooldown(player, &action);
    }
    Ok(())
}
