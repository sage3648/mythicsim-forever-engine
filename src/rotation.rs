//! A strict subset of Go's APL rotation language.
//!
//! Parsing accepts the protojson rotation carried by prepared v2 and records every
//! operator outside the implemented subset as a named reason. Constant parsing and type
//! coercion follow sim/core/apl_values_operators.go at the pinned revision, including
//! Go `time.ParseDuration` for duration constants.

use serde_json::{Map, Value as Json};

use crate::contracts::prepared_v2::ActionId;

/// Go `APLValueType`, in coercion priority order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValueType {
    Int,
    Float,
    Duration,
    String,
    Bool,
}

/// Go `APLValueConst`: every representation is kept because a coerced constant reads
/// the field of its new type, not a conversion of its original value.
#[derive(Clone, Debug, PartialEq)]
pub struct Const {
    pub value_type: ValueType,
    pub int: i32,
    pub float: f64,
    pub duration_ns: i64,
    pub string: String,
    pub boolean: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompareOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Go `APLValueMath_MathOperator`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl MathOp {
    /// Go `APLValueMath.Type`, from the operand types after `newValueMath` coerced them.
    pub fn result_type(self, lhs: ValueType, rhs: ValueType) -> ValueType {
        match self {
            MathOp::Add | MathOp::Sub => lhs,
            MathOp::Mul if lhs == ValueType::Duration || rhs == ValueType::Duration => {
                ValueType::Duration
            }
            MathOp::Div if lhs == ValueType::Duration && rhs == ValueType::Duration => {
                ValueType::Float
            }
            _ if lhs == ValueType::Float || rhs == ValueType::Float => ValueType::Float,
            _ => lhs,
        }
    }

    /// The getter types Go's `APLValueMath` reads its operands with, when it is read with
    /// the getter of its own type.
    fn operand_getters(self, lhs: ValueType, rhs: ValueType) -> (ValueType, ValueType) {
        match (self, self.result_type(lhs, rhs)) {
            (MathOp::Add | MathOp::Sub, result) => (result, result),
            // A duration product or quotient reads each operand with its own getter.
            (MathOp::Mul | MathOp::Div, ValueType::Duration) => (lhs, rhs),
            (MathOp::Div, ValueType::Float)
                if lhs == ValueType::Duration && rhs == ValueType::Duration =>
            {
                (lhs, rhs)
            }
            (_, result) => (result, result),
        }
    }
}

/// Operand types after `newValueMath`, which coerces addition and subtraction operands to
/// the higher of their two types.
fn math_operand_types(op: MathOp, lhs: ValueType, rhs: ValueType) -> (ValueType, ValueType) {
    match op {
        MathOp::Add | MathOp::Sub => (lhs.max(rhs), lhs.max(rhs)),
        MathOp::Mul | MathOp::Div => (lhs, rhs),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Const(Const),
    Compare {
        op: CompareOp,
        lhs: Box<Value>,
        rhs: Box<Value>,
    },
    And(Vec<Value>),
    Or(Vec<Value>),
    Not(Box<Value>),
    Math {
        op: MathOp,
        lhs: Box<Value>,
        rhs: Box<Value>,
    },
    CurrentManaPercent,
    CurrentMana,
    RemainingTime,
    CurrentTime,
    NumberTargets,
    AuraIsKnown(ActionId),
    AuraIsActive(ActionId),
    /// `auraIsActive` with the current target as its source unit.
    TargetAuraIsActive(ActionId),
    AuraNumStacks(ActionId),
    AuraRemainingTime(ActionId),
    DotIsActive(ActionId),
    DotRemainingTime(ActionId),
    SpellIsKnown(ActionId),
    SpellIsReady(ActionId),
    SpellCastTime(ActionId),
    SpellTimeToReady(ActionId),
    DotTimeToNextTick(ActionId),
    GcdIsReady,
    /// Go `APLValueSpellCanCast`: `CanCastOrQueue` on the current target.
    SpellCanCast(ActionId),
    /// Go `APLValueAutoTimeToNext` for the melee swings.
    AutoTimeToNext(AutoType),
}

/// The swings Go `APLValueAutoTimeToNext` reads, by its `APLValueAutoAttackType`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoType {
    /// `MeleeAuto`: the earlier of the two hands.
    Melee,
    MainHand,
    OffHand,
}

impl Value {
    /// Visit this value and every nested value, parents first.
    pub fn visit(&self, f: &mut impl FnMut(&Value)) {
        f(self);
        match self {
            Value::Compare { lhs, rhs, .. } | Value::Math { lhs, rhs, .. } => {
                lhs.visit(f);
                rhs.visit(f);
            }
            Value::And(values) | Value::Or(values) => {
                values.iter().for_each(|value| value.visit(f))
            }
            Value::Not(value) => value.visit(f),
            _ => {}
        }
    }

    /// This value with `numberTargets` read as the one target the runtime supports, a
    /// constant that folds.
    pub fn with_one_target(&self) -> Value {
        let map = |value: &Value| Box::new(value.with_one_target());
        match self {
            Value::NumberTargets => Value::Const(parse_const("1").expect("int constant")),
            Value::Compare { op, lhs, rhs } => Value::Compare {
                op: *op,
                lhs: map(lhs),
                rhs: map(rhs),
            },
            Value::Math { op, lhs, rhs } => Value::Math {
                op: *op,
                lhs: map(lhs),
                rhs: map(rhs),
            },
            Value::And(values) => Value::And(values.iter().map(Value::with_one_target).collect()),
            Value::Or(values) => Value::Or(values.iter().map(Value::with_one_target).collect()),
            Value::Not(value) => Value::Not(map(value)),
            other => other.clone(),
        }
    }

    /// The Go type before coercion.
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Const(constant) => constant.value_type,
            Value::Compare { .. }
            | Value::And(_)
            | Value::Or(_)
            | Value::Not(_)
            | Value::AuraIsKnown(_)
            | Value::AuraIsActive(_)
            | Value::TargetAuraIsActive(_)
            | Value::DotIsActive(_)
            | Value::SpellIsKnown(_)
            | Value::SpellIsReady(_)
            | Value::SpellCanCast(_)
            | Value::GcdIsReady => ValueType::Bool,
            Value::AuraNumStacks(_) | Value::NumberTargets => ValueType::Int,
            Value::AuraRemainingTime(_)
            | Value::DotRemainingTime(_)
            | Value::SpellCastTime(_)
            | Value::SpellTimeToReady(_)
            | Value::DotTimeToNextTick(_)
            | Value::AutoTimeToNext(_)
            | Value::RemainingTime
            | Value::CurrentTime => ValueType::Duration,
            Value::CurrentManaPercent | Value::CurrentMana => ValueType::Float,
            Value::Math { op, lhs, rhs } => {
                let (lhs, rhs) = math_operand_types(*op, lhs.value_type(), rhs.value_type());
                op.result_type(lhs, rhs)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    CastSpell(ActionId),
    AutocastOtherCooldowns,
    /// Go `APLActionStrictSequence`: casts that run in order once the first is ready.
    StrictSequence(Vec<ActionId>),
    /// Go `APLActionChannelSpell`: a channel the rotation may interrupt.
    ChannelSpell {
        spell: ActionId,
        interrupt_if: Option<Value>,
        allow_recast: bool,
    },
}

impl Action {
    /// The spells the action names, in order, as Go `GetAllActions` visits casts.
    pub fn spells(&self) -> Vec<&ActionId> {
        match self {
            Action::CastSpell(id) => vec![id],
            Action::AutocastOtherCooldowns => Vec::new(),
            Action::StrictSequence(ids) => ids.iter().collect(),
            Action::ChannelSpell { spell, .. } => vec![spell],
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// One-based position in the request's priority list, hidden items included.
    pub position: usize,
    pub condition: Option<Value>,
    pub action: Action,
}

/// Go `APLPrepullAction`: an action at a fixed time before the pull.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepull {
    /// One-based position in the request's prepull list, hidden actions included.
    pub position: usize,
    /// Nanoseconds relative to the pull, never positive.
    pub do_at_ns: i64,
    pub action: Action,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rotation {
    pub priority_list: Vec<Item>,
    pub prepull: Vec<Prepull>,
}

/// Parse a protojson `APLRotation`. Returns every unsupported construct, not only the first.
pub fn parse(rotation: &Json) -> Result<Rotation, Vec<String>> {
    let mut reasons = Vec::new();
    let mut parsed = Rotation::default();
    let Some(object) = rotation.as_object() else {
        return Err(vec!["rotation must be an object".into()]);
    };
    for (key, value) in object {
        match key.as_str() {
            "type" if value == "TypeAPL" => {}
            "type" => reasons.push(format!("rotation type {value} is unsupported")),
            "priorityList" | "prepullActions" => {}
            "groups" | "valueVariables" if is_empty(value) => {}
            other => reasons.push(format!("rotation field {other} is unsupported")),
        }
    }
    if object.get("type").is_none() {
        reasons.push("rotation type is missing".into());
    }
    for (index, item) in object
        .get("priorityList")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        match parse_item(item, index + 1) {
            Ok(Some(item)) => parsed.priority_list.push(item),
            Ok(None) => {}
            Err(mut item_reasons) => {
                for reason in &mut item_reasons {
                    *reason = format!("rotation item {}: {reason}", index + 1);
                }
                reasons.extend(item_reasons);
            }
        }
    }
    for (index, item) in object
        .get("prepullActions")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        match parse_prepull(item, index + 1) {
            Ok(Some(prepull)) => parsed.prepull.push(prepull),
            Ok(None) => {}
            Err(reason) => reasons.push(format!("prepull action {}: {reason}", index + 1)),
        }
    }
    if reasons.is_empty() {
        Ok(parsed)
    } else {
        Err(reasons)
    }
}

/// Go `newAPLRotation`'s prepull parsing: a hidden action or one after the pull is skipped.
fn parse_prepull(item: &Json, position: usize) -> Result<Option<Prepull>, String> {
    let object = item.as_object().ok_or("prepull action must be an object")?;
    for key in object.keys() {
        if !matches!(key.as_str(), "action" | "doAtValue" | "hide") {
            return Err(format!("field {key} is unsupported"));
        }
    }
    if object.get("hide").and_then(Json::as_bool) == Some(true) {
        return Ok(None);
    }
    let do_at = match object.get("doAtValue").map(parse_value) {
        Some(Ok(Value::Const(constant))) => constant,
        Some(Ok(_)) => return Err("a do-at time other than a constant is unsupported".into()),
        Some(Err(reasons)) => return Err(reasons.join("; ")),
        None => return Err("no do-at time".into()),
    };
    // Go `GetDuration` on the constant, as its type converts it.
    let do_at_ns = match do_at.value_type {
        ValueType::Duration | ValueType::Int | ValueType::Float => do_at.duration_ns,
        _ => return Err("a do-at time that is not a duration is unsupported".into()),
    };
    if do_at_ns > 0 {
        return Ok(None);
    }
    let action = object
        .get("action")
        .and_then(Json::as_object)
        .ok_or("no action")?;
    let action = match single(action, &["uuid"])? {
        ("castSpell", config) => Action::CastSpell(parse_cast_spell(config)?),
        (name, _) => return Err(format!("action {name} is unsupported")),
    };
    Ok(Some(Prepull {
        position,
        do_at_ns,
        action,
    }))
}

fn is_empty(value: &Json) -> bool {
    match value {
        Json::Array(items) => items.is_empty(),
        Json::Object(fields) => fields.is_empty(),
        Json::Null => true,
        _ => false,
    }
}

fn single<'a>(
    object: &'a Map<String, Json>,
    ignored: &[&str],
) -> Result<(&'a str, &'a Json), String> {
    let mut fields = object
        .iter()
        .filter(|(key, _)| !ignored.contains(&key.as_str()));
    match (fields.next(), fields.next()) {
        (Some((key, value)), None) => Ok((key.as_str(), value)),
        _ => Err("expected exactly one operator".into()),
    }
}

fn parse_item(item: &Json, position: usize) -> Result<Option<Item>, Vec<String>> {
    let object = item
        .as_object()
        .ok_or_else(|| vec!["item must be an object".to_string()])?;
    for key in object.keys() {
        if key != "action" && key != "hide" {
            return Err(vec![format!("item field {key} is unsupported")]);
        }
    }
    if object.get("hide").and_then(Json::as_bool) == Some(true) {
        return Ok(None);
    }
    let action = object
        .get("action")
        .and_then(Json::as_object)
        .ok_or_else(|| vec!["item has no action".to_string()])?;
    let mut reasons = Vec::new();
    let condition = match action.get("condition") {
        Some(condition) => match parse_value(condition) {
            Ok(value) => Some(value),
            Err(mut value_reasons) => {
                reasons.append(&mut value_reasons);
                None
            }
        },
        None => None,
    };
    let parsed = match single(action, &["condition", "uuid"]) {
        Ok(("castSpell", config)) => parse_cast_spell(config).map(Action::CastSpell),
        Ok(("autocastOtherCooldowns", config)) if is_empty(config) => {
            Ok(Action::AutocastOtherCooldowns)
        }
        Ok(("strictSequence", config)) => parse_strict_sequence(config),
        Ok(("channelSpell", config)) => parse_channel_spell(config),
        Ok((name, _)) => Err(format!("action {name} is unsupported")),
        Err(err) => Err(err),
    };
    match parsed {
        Ok(action) if reasons.is_empty() => Ok(Some(Item {
            position,
            condition,
            action,
        })),
        Ok(_) => Err(reasons),
        Err(reason) => {
            reasons.push(reason);
            Err(reasons)
        }
    }
}

/// Go `newActionStrictSequence` for sequences of unconditional casts.
fn parse_strict_sequence(config: &Json) -> Result<Action, String> {
    let object = config
        .as_object()
        .ok_or("strictSequence must be an object")?;
    for key in object.keys() {
        if key != "actions" {
            return Err(format!("strictSequence field {key} is unsupported"));
        }
    }
    let mut spells = Vec::new();
    for action in object
        .get("actions")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        let action = action
            .as_object()
            .ok_or("strictSequence action must be an object")?;
        match single(action, &["uuid"])? {
            ("castSpell", cast) => spells.push(parse_cast_spell(cast)?),
            (name, _) => return Err(format!("strictSequence action {name} is unsupported")),
        }
    }
    Ok(Action::StrictSequence(spells))
}

/// Go `newActionChannelSpell` for the current target.
fn parse_channel_spell(config: &Json) -> Result<Action, String> {
    let object = config.as_object().ok_or("channelSpell must be an object")?;
    let mut interrupt_if = None;
    let mut allow_recast = false;
    for (key, value) in object {
        match key.as_str() {
            "spellId" => {}
            "interruptIf" => {
                interrupt_if = Some(parse_value(value).map_err(|reasons| reasons.join("; "))?)
            }
            "allowRecast" => {
                allow_recast = value.as_bool().ok_or("allowRecast must be a boolean")?
            }
            other => return Err(format!("channelSpell field {other} is unsupported")),
        }
    }
    let spell = parse_action_id(object.get("spellId").ok_or("channelSpell has no spellId")?)?;
    Ok(Action::ChannelSpell {
        spell,
        interrupt_if,
        allow_recast,
    })
}

fn parse_cast_spell(config: &Json) -> Result<ActionId, String> {
    let object = config.as_object().ok_or("castSpell must be an object")?;
    for key in object.keys() {
        if key != "spellId" {
            // A target reference other than the default current target is not modeled.
            return Err(format!("castSpell field {key} is unsupported"));
        }
    }
    parse_action_id(object.get("spellId").ok_or("castSpell has no spellId")?)
}

/// Parse a protojson `ActionID`.
pub fn parse_action_id(value: &Json) -> Result<ActionId, String> {
    let object = value.as_object().ok_or("action id must be an object")?;
    let mut id = ActionId::default();
    for (key, field) in object {
        match key.as_str() {
            "spellId" => id.spell_id = json_i32(field)?,
            "itemId" => id.item_id = json_i32(field)?,
            "tag" => id.tag = json_i32(field)?,
            // Go `ProtoToActionID` ignores the rank.
            "rank" => {
                json_i32(field)?;
            }
            "otherId" => {
                id.other_id = field
                    .as_str()
                    .ok_or("otherId must be a string")?
                    .to_string()
            }
            other => return Err(format!("action id field {other} is unsupported")),
        }
    }
    let kinds = [id.spell_id != 0, id.item_id != 0, !id.other_id.is_empty()];
    if kinds.iter().filter(|set| **set).count() != 1 {
        return Err("action id must name exactly one spell, item or other action".into());
    }
    Ok(id)
}

fn json_i32(value: &Json) -> Result<i32, String> {
    value
        .as_i64()
        .and_then(|number| i32::try_from(number).ok())
        .ok_or_else(|| format!("invalid integer {value}"))
}

fn parse_value(value: &Json) -> Result<Value, Vec<String>> {
    let object = value
        .as_object()
        .ok_or_else(|| vec!["value must be an object".to_string()])?;
    let (name, config) = single(object, &["uuid"]).map_err(|err| vec![err])?;
    let fields = config.as_object();
    let only = |allowed: &[&str]| -> Result<(), Vec<String>> {
        match fields {
            Some(fields) => match fields.keys().find(|key| !allowed.contains(&key.as_str())) {
                Some(key) => Err(vec![format!("{name} field {key} is unsupported")]),
                None => Ok(()),
            },
            None => Err(vec![format!("{name} must be an object")]),
        }
    };
    match name {
        "const" => {
            only(&["val"])?;
            let text = config.get("val").and_then(Json::as_str).unwrap_or_default();
            parse_const(text).map(Value::Const).map_err(|err| vec![err])
        }
        "cmp" => {
            only(&["op", "lhs", "rhs"])?;
            let op = match config.get("op").and_then(Json::as_str) {
                Some("OpEq") => CompareOp::Eq,
                Some("OpNe") => CompareOp::Ne,
                Some("OpLt") => CompareOp::Lt,
                Some("OpLe") => CompareOp::Le,
                Some("OpGt") => CompareOp::Gt,
                Some("OpGe") => CompareOp::Ge,
                other => {
                    return Err(vec![format!(
                        "comparison operator {other:?} is unsupported"
                    )])
                }
            };
            let lhs = config
                .get("lhs")
                .ok_or_else(|| vec!["cmp has no lhs".to_string()]);
            let rhs = config
                .get("rhs")
                .ok_or_else(|| vec!["cmp has no rhs".to_string()]);
            let (lhs, rhs) = (lhs.and_then(parse_value), rhs.and_then(parse_value));
            match (lhs, rhs) {
                // Go drops such a comparison, which silently removes the condition.
                (Ok(lhs), Ok(rhs))
                    if lhs.value_type().max(rhs.value_type()) == ValueType::Bool
                        && !matches!(op, CompareOp::Eq | CompareOp::Ne) =>
                {
                    Err(vec!["ordered comparison of booleans is unsupported".into()])
                }
                (Ok(lhs), Ok(rhs)) => Ok(Value::Compare {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }),
                (lhs, rhs) => Err(lhs.err().into_iter().chain(rhs.err()).flatten().collect()),
            }
        }
        "math" => {
            only(&["op", "lhs", "rhs"])?;
            let op = match config.get("op").and_then(Json::as_str) {
                Some("OpAdd") => MathOp::Add,
                Some("OpSub") => MathOp::Sub,
                Some("OpMul") => MathOp::Mul,
                Some("OpDiv") => MathOp::Div,
                other => return Err(vec![format!("math operator {other:?} is unsupported")]),
            };
            let lhs = config
                .get("lhs")
                .ok_or_else(|| vec!["math has no lhs".to_string()]);
            let rhs = config
                .get("rhs")
                .ok_or_else(|| vec!["math has no rhs".to_string()]);
            let (lhs, rhs) = match (lhs.and_then(parse_value), rhs.and_then(parse_value)) {
                (Ok(lhs), Ok(rhs)) => (lhs, rhs),
                (lhs, rhs) => {
                    return Err(lhs.err().into_iter().chain(rhs.err()).flatten().collect())
                }
            };
            // Go panics when it reads an operand with a getter its type lacks. Constants and
            // the coerced operands of a sum or difference answer every getter.
            let (lhs_type, rhs_type) = math_operand_types(op, lhs.value_type(), rhs.value_type());
            let (lhs_getter, rhs_getter) = op.operand_getters(lhs_type, rhs_type);
            let answers = |value: &Value, getter: ValueType| {
                matches!(value, Value::Const(_))
                    || matches!(op, MathOp::Add | MathOp::Sub)
                    || value.value_type() == getter
            };
            if !answers(&lhs, lhs_getter) || !answers(&rhs, rhs_getter) {
                return Err(vec![
                    "math that reads an operand as another type is unsupported".into(),
                ]);
            }
            Ok(Value::Math {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            })
        }
        "and" | "or" => {
            only(&["vals"])?;
            let mut values = Vec::new();
            let mut reasons = Vec::new();
            for value in config
                .get("vals")
                .and_then(Json::as_array)
                .into_iter()
                .flatten()
            {
                match parse_value(value) {
                    Ok(value) => values.push(value),
                    Err(mut value_reasons) => reasons.append(&mut value_reasons),
                }
            }
            if !reasons.is_empty() {
                Err(reasons)
            } else if name == "and" {
                Ok(Value::And(values))
            } else {
                Ok(Value::Or(values))
            }
        }
        "currentManaPercent" => {
            only(&[])?;
            Ok(Value::CurrentManaPercent)
        }
        "remainingTime" => {
            only(&[])?;
            Ok(Value::RemainingTime)
        }
        "currentMana" => {
            only(&[])?;
            Ok(Value::CurrentMana)
        }
        "currentTime" => {
            only(&[])?;
            Ok(Value::CurrentTime)
        }
        "numberTargets" => {
            only(&[])?;
            Ok(Value::NumberTargets)
        }
        "gcdIsReady" => {
            only(&[])?;
            Ok(Value::GcdIsReady)
        }
        "autoTimeToNext" => {
            only(&["autoType"])?;
            match config.get("autoType").and_then(Json::as_str) {
                Some("MeleeAuto") => Ok(Value::AutoTimeToNext(AutoType::Melee)),
                Some("MainHandAuto") => Ok(Value::AutoTimeToNext(AutoType::MainHand)),
                Some("OffHandAuto") => Ok(Value::AutoTimeToNext(AutoType::OffHand)),
                other => Err(vec![format!(
                    "autoTimeToNext auto type {other:?} is unsupported"
                )]),
            }
        }
        "dotIsActive" | "dotRemainingTime" | "dotTimeToNextTick" | "spellIsKnown"
        | "spellIsReady" | "spellCastTime" | "spellTimeToReady" | "spellCanCast" => {
            // A target unit other than the current target is not modeled.
            only(&["spellId"])?;
            let id = config
                .get("spellId")
                .ok_or_else(|| vec![format!("{name} has no spellId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            Ok(match name {
                "dotIsActive" => Value::DotIsActive(id),
                "dotRemainingTime" => Value::DotRemainingTime(id),
                "dotTimeToNextTick" => Value::DotTimeToNextTick(id),
                "spellIsKnown" => Value::SpellIsKnown(id),
                "spellIsReady" => Value::SpellIsReady(id),
                "spellTimeToReady" => Value::SpellTimeToReady(id),
                "spellCanCast" => Value::SpellCanCast(id),
                _ => Value::SpellCastTime(id),
            })
        }
        "not" => {
            only(&["val"])?;
            let value = config
                .get("val")
                .ok_or_else(|| vec!["not has no val".to_string()])?;
            Ok(Value::Not(Box::new(parse_value(value)?)))
        }
        "auraIsActive" if fields.is_some_and(|fields| fields.contains_key("sourceUnit")) => {
            // Go GetSourceUnit: the player itself, or the current target, of the one in scope.
            only(&["auraId", "sourceUnit"])?;
            let id = config
                .get("auraId")
                .ok_or_else(|| vec![format!("{name} has no auraId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            let source = config.get("sourceUnit").and_then(Json::as_object);
            let kind = source.and_then(|unit| match unit.keys().find(|key| *key != "type") {
                Some(_) => None,
                None => unit.get("type").and_then(Json::as_str),
            });
            match kind {
                Some("Self") => Ok(Value::AuraIsActive(id)),
                Some("CurrentTarget") => Ok(Value::TargetAuraIsActive(id)),
                _ => Err(vec![format!(
                    "{name} sourceUnit {} is unsupported",
                    config.get("sourceUnit").cloned().unwrap_or_default()
                )]),
            }
        }
        "auraIsKnown" | "auraIsActive" | "auraNumStacks" | "auraRemainingTime" => {
            // sourceUnit, except on auraIsActive, and includeReactionTime are not modeled.
            only(&["auraId"])?;
            let id = config
                .get("auraId")
                .ok_or_else(|| vec![format!("{name} has no auraId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            Ok(match name {
                "auraIsKnown" => Value::AuraIsKnown(id),
                "auraIsActive" => Value::AuraIsActive(id),
                "auraNumStacks" => Value::AuraNumStacks(id),
                _ => Value::AuraRemainingTime(id),
            })
        }
        other => Err(vec![format!("value {other} is unsupported")]),
    }
}

/// Go `DurationFromSeconds`.
pub fn duration_from_seconds(seconds: f64) -> i64 {
    (1_000_000_000_f64 * seconds) as i64
}

/// Go `newValueConst`.
pub fn parse_const(text: &str) -> Result<Const, String> {
    let mut constant = Const {
        value_type: ValueType::String,
        int: 0,
        float: 0.0,
        duration_ns: 0,
        string: text.to_string(),
        boolean: !text.is_empty(),
    };
    if text.eq_ignore_ascii_case("true") {
        constant.boolean = true;
        constant.value_type = ValueType::Bool;
        return Ok(constant);
    }
    if text.eq_ignore_ascii_case("false") {
        constant.boolean = false;
        constant.value_type = ValueType::Bool;
        return Ok(constant);
    }
    if let Some(duration) = parse_go_duration(text) {
        constant.duration_ns = duration;
        constant.value_type = ValueType::Duration;
        return Ok(constant);
    }
    if let Some(int) = parse_go_atoi(text) {
        constant.int = int;
        constant.float = f64::from(int);
        constant.duration_ns = duration_from_seconds(constant.float);
        constant.value_type = ValueType::Int;
        return Ok(constant);
    }
    if text.len() > 1 && text.ends_with('%') {
        if let Some(float) = parse_go_float(&text[..text.len() - 1]) {
            constant.float = float / 100.0;
            constant.duration_ns = duration_from_seconds(float / 100.0);
            constant.value_type = ValueType::Float;
            return Ok(constant);
        }
    }
    if let Some(float) = parse_go_float(text) {
        constant.float = float;
        constant.duration_ns = duration_from_seconds(float);
        constant.value_type = ValueType::Float;
        return Ok(constant);
    }
    Err(format!("string constant {text:?} is unsupported"))
}

/// Go `strconv.Atoi` for the int32 values the APL stores.
fn parse_go_atoi(text: &str) -> Option<i32> {
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    // Go keeps int32(Atoi(...)); values outside int32 would wrap there, so reject them.
    text.parse::<i64>().ok().and_then(|v| i32::try_from(v).ok())
}

/// Go `strconv.ParseFloat` for plain decimal and exponent forms. Special values such as
/// "inf", "nan" and hexadecimal floats are rejected rather than reinterpreted.
fn parse_go_float(text: &str) -> Option<f64> {
    let body = text.strip_prefix(['+', '-']).unwrap_or(text);
    if body.is_empty()
        || !body
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-'))
        || !body
            .bytes()
            .next()
            .is_some_and(|b| b.is_ascii_digit() || b == b'.')
    {
        return None;
    }
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

/// Go `time.ParseDuration`, ported from the standard library's integer algorithm.
pub fn parse_go_duration(text: &str) -> Option<i64> {
    let mut s = text.as_bytes();
    let mut negative = false;
    if let Some((&sign, rest)) = s.split_first() {
        if sign == b'-' || sign == b'+' {
            negative = sign == b'-';
            s = rest;
        }
    }
    if s == b"0" {
        return Some(0);
    }
    if s.is_empty() {
        return None;
    }
    let mut total: u64 = 0;
    while !s.is_empty() {
        if !(s[0] == b'.' || s[0].is_ascii_digit()) {
            return None;
        }
        let (mut value, rest, int_digits) = leading_int(s)?;
        s = rest;
        let mut fraction = 0u64;
        let mut scale = 1f64;
        let mut fraction_digits = false;
        if let Some((&b'.', rest)) = s.split_first() {
            s = rest;
            let (f, rest, digits, sc) = leading_fraction(s);
            fraction = f;
            scale = sc;
            fraction_digits = digits;
            s = rest;
        }
        if !int_digits && !fraction_digits {
            return None;
        }
        let unit_length = s
            .iter()
            .position(|b| *b == b'.' || b.is_ascii_digit())
            .unwrap_or(s.len());
        if unit_length == 0 {
            return None;
        }
        let unit: u64 = match &s[..unit_length] {
            b"ns" => 1,
            // "us", then U+00B5 and U+03BC micro signs followed by "s".
            b"us" | [0xc2, 0xb5, b's'] | [0xce, 0xbc, b's'] => 1_000,
            b"ms" => 1_000_000,
            b"s" => 1_000_000_000,
            b"m" => 60_000_000_000,
            b"h" => 3_600_000_000_000,
            _ => return None,
        };
        s = &s[unit_length..];
        if value > (1u64 << 63) / unit {
            return None;
        }
        value *= unit;
        if fraction > 0 {
            value = value.checked_add((fraction as f64 * (unit as f64 / scale)) as u64)?;
            if value > 1u64 << 63 {
                return None;
            }
        }
        total = total.checked_add(value)?;
        if total > 1u64 << 63 {
            return None;
        }
    }
    if negative {
        return Some((total as i64).wrapping_neg());
    }
    if total > (1u64 << 63) - 1 {
        return None;
    }
    Some(total as i64)
}

fn leading_int(s: &[u8]) -> Option<(u64, &[u8], bool)> {
    let mut value: u64 = 0;
    let mut index = 0;
    while index < s.len() && s[index].is_ascii_digit() {
        if value > (1u64 << 63) / 10 {
            return None;
        }
        value = value * 10 + u64::from(s[index] - b'0');
        if value > 1u64 << 63 {
            return None;
        }
        index += 1;
    }
    Some((value, &s[index..], index > 0))
}

fn leading_fraction(s: &[u8]) -> (u64, &[u8], bool, f64) {
    let mut value: u64 = 0;
    let mut scale = 1f64;
    let mut overflow = false;
    let mut index = 0;
    while index < s.len() && s[index].is_ascii_digit() {
        if !overflow {
            if value > (1u64 << 63) / 10 {
                overflow = true;
            } else {
                let next = value * 10 + u64::from(s[index] - b'0');
                if next > 1u64 << 63 {
                    overflow = true;
                } else {
                    value = next;
                    scale *= 10.0;
                }
            }
        }
        index += 1;
    }
    (value, &s[index..], index > 0, scale)
}

/// A compiled condition: Go's constant folding and type coercion applied. `R` names an
/// aura: a runtime handle, or the action ID when only the compiled shape matters.
#[derive(Clone, Debug, PartialEq)]
pub enum Compiled<R> {
    Const(Const),
    Compare {
        op: CompareOp,
        lhs: Box<Compiled<R>>,
        rhs: Box<Compiled<R>>,
    },
    And(Vec<Compiled<R>>),
    Or(Vec<Compiled<R>>),
    Not(Box<Compiled<R>>),
    /// Go `APLValueMath`.
    Math {
        op: MathOp,
        lhs: Box<Compiled<R>>,
        rhs: Box<Compiled<R>>,
    },
    CurrentManaPercent,
    CurrentMana,
    RemainingTime,
    CurrentTime,
    NumberTargets,
    AuraIsActive(R),
    AuraNumStacks(R),
    AuraRemainingTime(R),
    /// A spell by its position in the player's spellbook, with the dot it names.
    DotIsActive(usize),
    DotRemainingTime(usize),
    SpellIsReady(usize),
    SpellCastTime(usize),
    SpellTimeToReady(usize),
    DotTimeToNextTick(usize),
    GcdIsReady,
    SpellCanCast(usize),
    AutoTimeToNext(AutoType),
    /// Go `APLValueCoerced`.
    Coerced {
        to: ValueType,
        inner: Box<Compiled<R>>,
    },
}

impl<R> Compiled<R> {
    pub fn value_type(&self) -> ValueType {
        match self {
            Compiled::Const(constant) => constant.value_type,
            Compiled::Compare { .. }
            | Compiled::And(_)
            | Compiled::Or(_)
            | Compiled::Not(_)
            | Compiled::AuraIsActive(_)
            | Compiled::DotIsActive(_)
            | Compiled::SpellIsReady(_)
            | Compiled::SpellCanCast(_)
            | Compiled::GcdIsReady => ValueType::Bool,
            Compiled::AuraNumStacks(_) | Compiled::NumberTargets => ValueType::Int,
            Compiled::AuraRemainingTime(_)
            | Compiled::AutoTimeToNext(_)
            | Compiled::DotRemainingTime(_)
            | Compiled::SpellCastTime(_)
            | Compiled::SpellTimeToReady(_)
            | Compiled::DotTimeToNextTick(_)
            | Compiled::RemainingTime
            | Compiled::CurrentTime => ValueType::Duration,
            Compiled::CurrentManaPercent | Compiled::CurrentMana => ValueType::Float,
            Compiled::Math { op, lhs, rhs } => op.result_type(lhs.value_type(), rhs.value_type()),
            Compiled::Coerced { to, .. } => *to,
        }
    }

    fn const_bool(&self) -> Option<bool> {
        match self {
            Compiled::Const(constant) if constant.value_type == ValueType::Bool => {
                Some(constant.boolean)
            }
            _ => None,
        }
    }

    /// Go `coerceTo`: a constant changes type in place; anything else is wrapped.
    fn coerce(self, to: ValueType) -> Self {
        if self.value_type() == to {
            self
        } else if let Compiled::Const(mut constant) = self {
            constant.value_type = to;
            Compiled::Const(constant)
        } else {
            Compiled::Coerced {
                to,
                inner: Box::new(self),
            }
        }
    }
}

impl<R: Clone> Compiled<R> {
    /// What the value means, with comparisons of constants evaluated and And and Or
    /// simplified. Go keeps such comparisons, so this is only for comparing compilations.
    fn folded(&self) -> Compiled<R> {
        match self {
            Compiled::Compare { op, lhs, rhs } => {
                let (lhs, rhs) = (lhs.folded(), rhs.folded());
                match (&lhs, &rhs) {
                    (Compiled::Const(a), Compiled::Const(b)) if a.value_type == b.value_type => {
                        let result = match a.value_type {
                            ValueType::Int => Some(op.apply(a.int, b.int)),
                            ValueType::Float => Some(op.apply(a.float, b.float)),
                            ValueType::Duration => Some(op.apply(a.duration_ns, b.duration_ns)),
                            ValueType::Bool => match op {
                                CompareOp::Eq => Some(a.boolean == b.boolean),
                                CompareOp::Ne => Some(a.boolean != b.boolean),
                                _ => None,
                            },
                            ValueType::String => None,
                        };
                        match result {
                            Some(value) => bool_const(value),
                            None => Compiled::Compare {
                                op: *op,
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                            },
                        }
                    }
                    _ => Compiled::Compare {
                        op: *op,
                        lhs: Box::new(lhs),
                        rhs: Box::new(rhs),
                    },
                }
            }
            Compiled::And(values) => simplify(values, true, Compiled::And),
            Compiled::Not(value) => {
                let value = value.folded();
                match value.const_bool() {
                    Some(constant) => bool_const(!constant),
                    None => Compiled::Not(Box::new(value)),
                }
            }
            Compiled::Or(values) => simplify(values, false, Compiled::Or),
            Compiled::Coerced { to, inner } => Compiled::Coerced {
                to: *to,
                inner: Box::new(inner.folded()),
            },
            // Go never folds arithmetic; its operands still fold.
            Compiled::Math { op, lhs, rhs } => Compiled::Math {
                op: *op,
                lhs: Box::new(lhs.folded()),
                rhs: Box::new(rhs.folded()),
            },
            other => other.clone(),
        }
    }
}

/// Fold And (`identity` true) or Or (`identity` false): identity terms drop out, the
/// other constant decides, and one remaining term stands for the operator.
fn simplify<R: Clone>(
    values: &[Compiled<R>],
    identity: bool,
    build: fn(Vec<Compiled<R>>) -> Compiled<R>,
) -> Compiled<R> {
    let mut kept = Vec::new();
    for value in values.iter().map(Compiled::folded) {
        match value.const_bool() {
            Some(constant) if constant == identity => {}
            Some(_) => return bool_const(!identity),
            None => kept.push(value),
        }
    }
    match kept.len() {
        0 => bool_const(identity),
        1 => kept.pop().expect("one value"),
        _ => build(kept),
    }
}

impl CompareOp {
    fn apply<T: PartialOrd>(self, lhs: T, rhs: T) -> bool {
        match self {
            CompareOp::Eq => lhs == rhs,
            CompareOp::Ne => lhs != rhs,
            CompareOp::Lt => lhs < rhs,
            CompareOp::Le => lhs <= rhs,
            CompareOp::Gt => lhs > rhs,
            CompareOp::Ge => lhs >= rhs,
        }
    }
}

/// How `auraIsActive` and `auraNumStacks` read an aura the character cannot have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingAura {
    /// The pinned reference: no value, so the term drops out of its parent.
    Dropped,
    /// Community fix ElliotWood/Forever#622: inactive, with no stacks.
    Inactive,
}

/// An aura a rotation names, as Go `GetAPLAura` finds it on the casting player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FoundAura<R> {
    pub aura: R,
    pub max_stacks: i32,
}

/// How compilation resolves the names a rotation uses, as Go does on the casting player.
pub struct Lookup<'a, R> {
    /// Go `GetAuraByID`: the aura, or `None` when the character lacks it.
    pub aura: &'a dyn Fn(&ActionId) -> Option<FoundAura<R>>,
    /// Go `GetAuraByID` on the current target.
    pub target_aura: &'a dyn Fn(&ActionId) -> Option<FoundAura<R>>,
    /// Go `GetAPLSpell`: the spellbook position of the spell, or `None` when unknown.
    pub spell: &'a dyn Fn(&ActionId) -> Option<usize>,
    /// Go `GetAPLDot`: the spellbook position of the spell when it has a dot on the target.
    pub dot: &'a dyn Fn(&ActionId) -> Option<usize>,
}

/// What Go `newAPLAction` makes of an action's condition.
#[derive(Clone, Debug, PartialEq)]
pub enum CompiledCondition<R> {
    /// A constant false condition removes the action.
    Pruned,
    /// No condition, a constant true, or a condition that compiled to no value.
    Always,
    When(Compiled<R>),
}

impl<R: Clone + PartialEq> CompiledCondition<R> {
    /// Whether two compilations act the same, though Go may keep constant comparisons.
    pub fn same_meaning(&self, other: &Self) -> bool {
        self.folded() == other.folded()
    }

    /// Whether the condition can never hold, though Go may keep evaluating it.
    pub fn never_holds(&self) -> bool {
        self.folded() == CompiledCondition::Pruned
    }

    fn folded(&self) -> Self {
        match self {
            CompiledCondition::When(value) => {
                let value = value.folded();
                match value.const_bool() {
                    Some(true) => CompiledCondition::Always,
                    Some(false) => CompiledCondition::Pruned,
                    None => CompiledCondition::When(value),
                }
            }
            other => other.clone(),
        }
    }
}

fn bool_const<R>(value: bool) -> Compiled<R> {
    Compiled::Const(parse_const(if value { "true" } else { "false" }).expect("bool constant"))
}

/// Go `newValueAnd` and `newValueOr`: terms without a value drop out, one term stands for
/// the operator, and a constant that decides the result replaces it.
fn fold<R>(
    values: &[Value],
    lookup: &Lookup<R>,
    missing: MissingAura,
    deciding: bool,
    build: fn(Vec<Compiled<R>>) -> Compiled<R>,
) -> Option<Compiled<R>> {
    let mut compiled: Vec<_> = values
        .iter()
        .filter_map(|value| compile_value(value, lookup, missing))
        .map(|value| value.coerce(ValueType::Bool))
        .collect();
    match compiled.len() {
        0 => None,
        1 => compiled.pop(),
        _ => Some(
            match compiled
                .iter()
                .position(|v| v.const_bool() == Some(deciding))
            {
                Some(index) => compiled.swap_remove(index),
                None => build(compiled),
            },
        ),
    }
}

/// Go `newAPLValue` for the supported subset. A spell or dot the character lacks gives no
/// value, so the term drops out of its parent; community #622 changes only auras.
fn compile_value<R>(
    value: &Value,
    lookup: &Lookup<R>,
    missing: MissingAura,
) -> Option<Compiled<R>> {
    let aura = lookup.aura;
    Some(match value {
        Value::Const(constant) => Compiled::Const(constant.clone()),
        Value::CurrentManaPercent => Compiled::CurrentManaPercent,
        Value::CurrentMana => Compiled::CurrentMana,
        Value::RemainingTime => Compiled::RemainingTime,
        Value::CurrentTime => Compiled::CurrentTime,
        Value::NumberTargets => Compiled::NumberTargets,
        Value::DotIsActive(id) => Compiled::DotIsActive((lookup.dot)(id)?),
        Value::DotRemainingTime(id) => Compiled::DotRemainingTime((lookup.dot)(id)?),
        // Go `newValueSpellIsKnown` is a constant.
        Value::SpellIsKnown(id) => bool_const((lookup.spell)(id).is_some()),
        Value::SpellIsReady(id) => Compiled::SpellIsReady((lookup.spell)(id)?),
        Value::SpellCastTime(id) => Compiled::SpellCastTime((lookup.spell)(id)?),
        Value::SpellTimeToReady(id) => Compiled::SpellTimeToReady((lookup.spell)(id)?),
        Value::SpellCanCast(id) => Compiled::SpellCanCast((lookup.spell)(id)?),
        Value::AutoTimeToNext(auto) => Compiled::AutoTimeToNext(*auto),
        Value::DotTimeToNextTick(id) => Compiled::DotTimeToNextTick((lookup.dot)(id)?),
        Value::GcdIsReady => Compiled::GcdIsReady,
        Value::AuraIsKnown(id) => bool_const(aura(id).is_some()),
        Value::AuraIsActive(id) => match (aura(id), missing) {
            (Some(found), _) => Compiled::AuraIsActive(found.aura),
            (None, MissingAura::Dropped) => return None,
            (None, MissingAura::Inactive) => bool_const(false),
        },
        Value::TargetAuraIsActive(id) => match ((lookup.target_aura)(id), missing) {
            (Some(found), _) => Compiled::AuraIsActive(found.aura),
            (None, MissingAura::Dropped) => return None,
            (None, MissingAura::Inactive) => bool_const(false),
        },
        Value::AuraNumStacks(id) => match (aura(id), missing) {
            // Go warns that the aura does not stack and drops the value, fix or not.
            (Some(found), _) if found.max_stacks == 0 => return None,
            (Some(found), _) => Compiled::AuraNumStacks(found.aura),
            (None, MissingAura::Dropped) => return None,
            (None, MissingAura::Inactive) => {
                Compiled::Const(parse_const("0").expect("int constant"))
            }
        },
        Value::Compare { op, lhs, rhs } => {
            let lhs = compile_value(lhs, lookup, missing)?;
            let rhs = compile_value(rhs, lookup, missing)?;
            let to = lhs.value_type().max(rhs.value_type());
            Compiled::Compare {
                op: *op,
                lhs: Box::new(lhs.coerce(to)),
                rhs: Box::new(rhs.coerce(to)),
            }
        }
        Value::Math { op, lhs, rhs } => {
            let lhs = compile_value(lhs, lookup, missing)?;
            let rhs = compile_value(rhs, lookup, missing)?;
            let (lhs, rhs) = match op {
                MathOp::Add | MathOp::Sub => {
                    let to = lhs.value_type().max(rhs.value_type());
                    (lhs.coerce(to), rhs.coerce(to))
                }
                MathOp::Mul | MathOp::Div => (lhs, rhs),
            };
            let (lhs_type, rhs_type) = (lhs.value_type(), rhs.value_type());
            let numeric = |t: ValueType| matches!(t, ValueType::Int | ValueType::Float);
            // Go newValueMath warns and gives no value for these.
            if matches!(lhs_type, ValueType::Bool | ValueType::String)
                || matches!(rhs_type, ValueType::Bool | ValueType::String)
                || (*op == MathOp::Mul
                    && lhs_type == ValueType::Duration
                    && rhs_type == ValueType::Duration)
                || (*op == MathOp::Div && numeric(lhs_type) && rhs_type == ValueType::Duration)
            {
                return None;
            }
            Compiled::Math {
                op: *op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            }
        }
        Value::AuraRemainingTime(id) => match (aura(id), missing) {
            (Some(found), _) => Compiled::AuraRemainingTime(found.aura),
            (None, MissingAura::Dropped) => return None,
            (None, MissingAura::Inactive) => {
                Compiled::Const(parse_const("0ms").expect("duration constant"))
            }
        },
        // Go `newValueNot` folds a constant operand.
        Value::Not(value) => {
            let value = compile_value(value, lookup, missing)?.coerce(ValueType::Bool);
            match value.const_bool() {
                Some(constant) => bool_const(!constant),
                None => Compiled::Not(Box::new(value)),
            }
        }
        Value::And(values) => return fold(values, lookup, missing, false, Compiled::And),
        Value::Or(values) => return fold(values, lookup, missing, true, Compiled::Or),
    })
}

/// Go `coerceTo(newAPLValue(value), Bool)`, as `newActionChannelSpell` compiles its
/// interrupt condition: `None` when the value has none.
pub fn compile_bool_value<R>(
    value: Option<&Value>,
    lookup: &Lookup<R>,
    missing: MissingAura,
) -> Option<Compiled<R>> {
    value
        .and_then(|value| compile_value(value, lookup, missing))
        .map(|value| value.coerce(ValueType::Bool))
}

/// Go `newAPLAction`'s condition handling for one action.
pub fn compile_condition<R>(
    condition: Option<&Value>,
    lookup: &Lookup<R>,
    missing: MissingAura,
) -> CompiledCondition<R> {
    let compiled = condition
        .and_then(|value| compile_value(value, lookup, missing))
        .map(|value| value.coerce(ValueType::Bool));
    match compiled {
        None => CompiledCondition::Always,
        Some(value) => match value.const_bool() {
            Some(false) => CompiledCondition::Pruned,
            Some(true) => CompiledCondition::Always,
            None => CompiledCondition::When(value),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_spell(_: &ActionId) -> Option<usize> {
        None
    }

    fn only_auras<'a, R>(aura: &'a dyn Fn(&ActionId) -> Option<FoundAura<R>>) -> Lookup<'a, R> {
        Lookup {
            aura,
            target_aura: aura,
            spell: &no_spell,
            dot: &no_spell,
        }
    }

    #[test]
    fn duration_parser_matches_go_examples() {
        assert_eq!(parse_go_duration("25s"), Some(25_000_000_000));
        assert_eq!(parse_go_duration("1.5s"), Some(1_500_000_000));
        assert_eq!(parse_go_duration("1m30s"), Some(90_000_000_000));
        assert_eq!(parse_go_duration("100ms"), Some(100_000_000));
        assert_eq!(parse_go_duration("-2h"), Some(-7_200_000_000_000));
        assert_eq!(parse_go_duration("0"), Some(0));
        assert_eq!(parse_go_duration(".5s"), Some(500_000_000));
        assert_eq!(parse_go_duration("25"), None);
        assert_eq!(parse_go_duration("80%"), None);
        assert_eq!(parse_go_duration("s"), None);
    }

    #[test]
    fn constants_keep_every_go_representation() {
        let percent = parse_const("80%").unwrap();
        assert_eq!(percent.value_type, ValueType::Float);
        assert_eq!(percent.float, 0.8);
        assert_eq!(percent.duration_ns, 800_000_000);
        let seconds = parse_const("25s").unwrap();
        assert_eq!(seconds.value_type, ValueType::Duration);
        assert_eq!((seconds.duration_ns, seconds.float), (25_000_000_000, 0.0));
        let int = parse_const("25").unwrap();
        assert_eq!(int.value_type, ValueType::Int);
        assert_eq!(
            (int.int, int.float, int.duration_ns),
            (25, 25.0, 25_000_000_000)
        );
        assert_eq!(parse_const("TRUE").unwrap().value_type, ValueType::Bool);
        assert!(parse_const("hello").is_err());
    }

    #[test]
    fn aura_is_active_reads_the_player_or_the_current_target() {
        let item = |source: serde_json::Value| {
            serde_json::json!({"action": {"castSpell": {"spellId": {"spellId": 1}},
                "condition": {"auraIsActive": {"auraId": {"spellId": 2}, "sourceUnit": source}}}})
        };
        let rotation = serde_json::json!({"type": "TypeAPL", "priorityList": [
            item(serde_json::json!({"type": "Self"})),
            item(serde_json::json!({"type": "CurrentTarget"})),
        ]});
        let parsed = parse(&rotation).unwrap();
        let id = ActionId {
            spell_id: 2,
            ..ActionId::default()
        };
        assert_eq!(
            parsed.priority_list[0].condition,
            Some(Value::AuraIsActive(id.clone()))
        );
        assert_eq!(
            parsed.priority_list[1].condition,
            Some(Value::TargetAuraIsActive(id))
        );
        for source in [
            serde_json::json!({"type": "Target", "index": 1}),
            serde_json::json!({"type": "NextTarget"}),
        ] {
            let rotation =
                serde_json::json!({"type": "TypeAPL", "priorityList": [item(source.clone())]});
            assert_eq!(
                parse(&rotation).unwrap_err(),
                [format!(
                    "rotation item 1: auraIsActive sourceUnit {source} is unsupported"
                )]
            );
        }
    }

    #[test]
    fn math_types_follow_go() {
        let math = |op: &str, lhs: serde_json::Value, rhs: serde_json::Value| {
            parse_value(&serde_json::json!({"math": {"op": op, "lhs": lhs, "rhs": rhs}}))
        };
        let remaining = || serde_json::json!({"remainingTime": {}});
        let mana = || serde_json::json!({"currentMana": {}});
        let constant = |val: &str| serde_json::json!({"const": {"val": val}});
        let typed = |value: Result<Value, Vec<String>>| value.unwrap().value_type();
        // A duration times an integer stays a duration; a sum takes the higher type.
        assert_eq!(
            typed(math("OpMul", remaining(), constant("100"))),
            ValueType::Duration
        );
        assert_eq!(
            typed(math("OpAdd", constant("1"), mana())),
            ValueType::Float
        );
        assert_eq!(
            typed(math("OpDiv", remaining(), remaining())),
            ValueType::Float
        );
        assert_eq!(
            typed(math("OpDiv", remaining(), constant("2"))),
            ValueType::Duration
        );
        assert_eq!(
            typed(math("OpMul", constant("2"), constant("3"))),
            ValueType::Int
        );
        // A duration over a float is a float, which Go reads from the duration and panics.
        assert!(math("OpDiv", remaining(), constant("2.5")).is_err());
        assert!(math("OpMul", serde_json::json!({"numberTargets": {}}), mana()).is_err());
        assert!(math("OpMod", mana(), mana()).is_err());
    }

    #[test]
    fn unsupported_operators_are_named() {
        let rotation = serde_json::json!({
            "type": "TypeAPL",
            "priorityList": [
                {"action": {"castSpell": {"spellId": {"spellId": 25304}},
                            "condition": {"spellNumCharges": {"spellId": {"spellId": 1}}}}},
                {"action": {"wait": {"duration": {"const": {"val": "1s"}}}}}
            ]
        });
        let reasons = parse(&rotation).unwrap_err();
        assert_eq!(
            reasons,
            [
                "rotation item 1: value spellNumCharges is unsupported",
                "rotation item 2: action wait is unsupported"
            ]
        );
    }

    /// Conditions over spell 44404, an aura the test character lacks.
    fn conditions(json: serde_json::Value) -> (CompiledCondition<()>, CompiledCondition<()>) {
        let rotation = parse(&serde_json::json!({
            "type": "TypeAPL",
            "priorityList": [{"action": {
                "castSpell": {"spellId": {"spellId": 25345}},
                "condition": json,
            }}],
        }))
        .unwrap();
        let condition = rotation.priority_list[0].condition.as_ref();
        let lacks = |_: &ActionId| None::<FoundAura<()>>;
        (
            compile_condition(condition, &only_auras(&lacks), MissingAura::Dropped),
            compile_condition(condition, &only_auras(&lacks), MissingAura::Inactive),
        )
    }

    #[test]
    fn missing_auras_compile_as_pinned_go_and_community_622() {
        let active = serde_json::json!({"auraIsActive": {"auraId": {"spellId": 44404}}});
        let known = serde_json::json!({"auraIsKnown": {"auraId": {"spellId": 44404}}});
        let low_mana = serde_json::json!({"cmp": {
            "op": "OpLt", "lhs": {"currentManaPercent": {}}, "rhs": {"const": {"val": "20%"}},
        }});
        // Unguarded: pinned Go fires on every pass, the fix never fires.
        assert_eq!(
            conditions(active.clone()),
            (CompiledCondition::Always, CompiledCondition::Pruned)
        );
        // An auraIsKnown guard prunes the action under both readings.
        assert_eq!(
            conditions(serde_json::json!({"and": {"vals": [known, active.clone()]}})),
            (CompiledCondition::Pruned, CompiledCondition::Pruned)
        );
        // Pinned Go keeps the other terms of an And; the fix prunes it.
        let (pinned, fixed) =
            conditions(serde_json::json!({"and": {"vals": [low_mana, active.clone()]}}));
        assert!(matches!(
            pinned,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        assert_eq!(fixed, CompiledCondition::Pruned);
        // Go does not fold comparisons, so the fix leaves a live constant comparison.
        let (pinned, fixed) = conditions(serde_json::json!({"cmp": {
            "op": "OpEq", "lhs": active, "rhs": {"const": {"val": "false"}},
        }}));
        assert_eq!(pinned, CompiledCondition::Always);
        assert!(matches!(
            fixed,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
    }

    #[test]
    fn or_and_stack_counts_fold_as_go_does() {
        let compile = |json: serde_json::Value, max_stacks: i32| {
            let rotation = parse(&serde_json::json!({
                "type": "TypeAPL",
                "priorityList": [{"action": {
                    "castSpell": {"spellId": {"spellId": 25304}},
                    "condition": json,
                }}],
            }))
            .unwrap();
            let condition = rotation.priority_list[0].condition.clone();
            // The character has 400573 and lacks 44404.
            let find = move |id: &ActionId| {
                (id.spell_id == 400573).then_some(FoundAura {
                    aura: (),
                    max_stacks,
                })
            };
            (
                compile_condition(condition.as_ref(), &only_auras(&find), MissingAura::Dropped),
                compile_condition(
                    condition.as_ref(),
                    &only_auras(&find),
                    MissingAura::Inactive,
                ),
            )
        };
        let stacks = |id: i32| {
            serde_json::json!({"cmp": {"op": "OpGe",
                "lhs": {"auraNumStacks": {"auraId": {"spellId": id}}}, "rhs": {"const": {"val": "3"}}}})
        };
        let known = serde_json::json!({"auraIsKnown": {"auraId": {"spellId": 400573}}});
        // A known aura's stacks compare as integers.
        let (pinned, fixed) = compile(stacks(400573), 4);
        assert_eq!(pinned, fixed);
        assert!(matches!(
            pinned,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        // Go drops the stacks of an aura without MaxStacks, with or without the fix.
        assert_eq!(
            compile(stacks(400573), 0),
            (CompiledCondition::Always, CompiledCondition::Always)
        );
        // A missing aura: pinned Go drops the term, the fix compares a constant 0.
        let (pinned, fixed) = compile(stacks(44404), 4);
        assert_eq!(pinned, CompiledCondition::Always);
        assert!(matches!(
            fixed,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        // A constant true decides an Or.
        let (pinned, fixed) = compile(
            serde_json::json!({"or": {"vals": [stacks(400573), known]}}),
            4,
        );
        assert_eq!(
            (pinned, fixed),
            (CompiledCondition::Always, CompiledCondition::Always)
        );
        // Without the stacking aura, pinned Go keeps only the mana term and the fix keeps a
        // constant 0 >= 3 beside it: different shapes, the same meaning.
        let low_mana = serde_json::json!({"cmp": {
            "op": "OpLt", "lhs": {"currentManaPercent": {}}, "rhs": {"const": {"val": "40%"}},
        }});
        let (pinned, fixed) = compile(
            serde_json::json!({"or": {"vals": [stacks(44404), low_mana.clone()]}}),
            4,
        );
        assert_ne!(pinned, fixed);
        assert!(pinned.same_meaning(&fixed));
        // A missing aura that would otherwise fire every time does not mean the same.
        let active = serde_json::json!({"auraIsActive": {"auraId": {"spellId": 44404}}});
        let (pinned, fixed) = compile(active, 4);
        assert!(!pinned.same_meaning(&fixed));
        // One remaining term stands for the Or.
        let (pinned, _) = compile(
            serde_json::json!({"or": {"vals": [stacks(400573), stacks(44404)]}}),
            4,
        );
        assert!(matches!(
            pinned,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
    }

    #[test]
    fn not_and_remaining_time_fold_as_go_does() {
        let compile = |json: serde_json::Value| {
            let rotation = parse(&serde_json::json!({
                "type": "TypeAPL",
                "priorityList": [{"action": {
                    "castSpell": {"spellId": {"spellId": 10207}},
                    "condition": json,
                }}],
            }))
            .unwrap();
            let condition = rotation.priority_list[0].condition.clone();
            // The character has 22959 and lacks 400625.
            let find = |id: &ActionId| {
                (id.spell_id == 22959).then_some(FoundAura {
                    aura: (),
                    max_stacks: 5,
                })
            };
            (
                compile_condition(condition.as_ref(), &only_auras(&find), MissingAura::Dropped),
                compile_condition(
                    condition.as_ref(),
                    &only_auras(&find),
                    MissingAura::Inactive,
                ),
            )
        };
        let known = |id: i32| serde_json::json!({"auraIsKnown": {"auraId": {"spellId": id}}});
        // Go folds Not of a constant.
        assert_eq!(
            compile(serde_json::json!({"not": {"val": known(22959)}})),
            (CompiledCondition::Pruned, CompiledCondition::Pruned)
        );
        assert_eq!(
            compile(serde_json::json!({"not": {"val": known(400625)}})),
            (CompiledCondition::Always, CompiledCondition::Always)
        );
        // Remaining time compares as a duration; a missing aura has none under the fix.
        let remaining = |id: i32| {
            serde_json::json!({"cmp": {"op": "OpLe",
                "lhs": {"auraRemainingTime": {"auraId": {"spellId": id}}}, "rhs": {"const": {"val": "4s"}}}})
        };
        let (pinned, fixed) = compile(remaining(22959));
        assert_eq!(pinned, fixed);
        assert!(matches!(
            pinned,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        let (pinned, fixed) = compile(remaining(400625));
        assert_eq!(pinned, CompiledCondition::Always);
        // 0 <= 4s holds, so the fix acts as the pinned reading here.
        assert!(pinned.same_meaning(&fixed));
    }

    #[test]
    fn sequences_and_channels_parse_with_their_values() {
        let rotation = parse(&serde_json::json!({
            "type": "TypeAPL",
            "priorityList": [
                {"action": {"strictSequence": {"actions": [
                    {"castSpell": {"spellId": {"spellId": 14751}}},
                    {"castSpell": {"spellId": {"spellId": 10947, "rank": 9}}},
                ]}}},
                {"action": {"channelSpell": {
                    "spellId": {"spellId": 18807},
                    "interruptIf": {"and": {"vals": [
                        {"cmp": {"op": "OpLe",
                            "lhs": {"spellTimeToReady": {"spellId": {"spellId": 10947}}},
                            "rhs": {"const": {"val": "0s"}}}},
                        {"cmp": {"op": "OpLe",
                            "lhs": {"dotTimeToNextTick": {"spellId": {"spellId": 18807}}},
                            "rhs": {"const": {"val": "0.05s"}}}},
                        {"gcdIsReady": {}},
                    ]}},
                    "allowRecast": true,
                }}},
            ],
        }))
        .unwrap();
        assert_eq!(
            rotation.priority_list[0].action,
            Action::StrictSequence(vec![ActionId::spell(14751), ActionId::spell(10947)])
        );
        let Action::ChannelSpell {
            spell,
            interrupt_if: Some(Value::And(terms)),
            allow_recast: true,
        } = &rotation.priority_list[1].action
        else {
            panic!("expected an interruptible channel");
        };
        assert_eq!(*spell, ActionId::spell(18807));
        assert_eq!(terms[2], Value::GcdIsReady);
        let types: Vec<ValueType> = terms
            .iter()
            .map(|term| match term {
                Value::Compare { lhs, .. } => lhs.value_type(),
                other => other.value_type(),
            })
            .collect();
        assert_eq!(
            types,
            [ValueType::Duration, ValueType::Duration, ValueType::Bool]
        );
    }
}
