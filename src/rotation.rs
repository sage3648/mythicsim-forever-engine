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

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Const(Const),
    Compare {
        op: CompareOp,
        lhs: Box<Value>,
        rhs: Box<Value>,
    },
    And(Vec<Value>),
    CurrentManaPercent,
    RemainingTime,
    AuraIsKnown(ActionId),
    AuraIsActive(ActionId),
}

impl Value {
    /// Visit this value and every nested value, parents first.
    pub fn visit(&self, f: &mut impl FnMut(&Value)) {
        f(self);
        match self {
            Value::Compare { lhs, rhs, .. } => {
                lhs.visit(f);
                rhs.visit(f);
            }
            Value::And(values) => values.iter().for_each(|value| value.visit(f)),
            _ => {}
        }
    }

    /// The Go type before coercion.
    pub fn value_type(&self) -> ValueType {
        match self {
            Value::Const(constant) => constant.value_type,
            Value::Compare { .. }
            | Value::And(_)
            | Value::AuraIsKnown(_)
            | Value::AuraIsActive(_) => ValueType::Bool,
            Value::CurrentManaPercent => ValueType::Float,
            Value::RemainingTime => ValueType::Duration,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    CastSpell(ActionId),
    AutocastOtherCooldowns,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// One-based position in the request's priority list, hidden items included.
    pub position: usize,
    pub condition: Option<Value>,
    pub action: Action,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rotation {
    pub priority_list: Vec<Item>,
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
            "priorityList" => {}
            "prepullActions" | "groups" | "valueVariables" if is_empty(value) => {}
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
    if reasons.is_empty() {
        Ok(parsed)
    } else {
        Err(reasons)
    }
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
        "and" => {
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
            if reasons.is_empty() {
                Ok(Value::And(values))
            } else {
                Err(reasons)
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
        "auraIsKnown" | "auraIsActive" => {
            // sourceUnit and includeReactionTime are not modeled.
            only(&["auraId"])?;
            let id = config
                .get("auraId")
                .ok_or_else(|| vec![format!("{name} has no auraId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            Ok(if name == "auraIsKnown" {
                Value::AuraIsKnown(id)
            } else {
                Value::AuraIsActive(id)
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
    CurrentManaPercent,
    RemainingTime,
    AuraIsActive(R),
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
            Compiled::Compare { .. } | Compiled::And(_) | Compiled::AuraIsActive(_) => {
                ValueType::Bool
            }
            Compiled::CurrentManaPercent => ValueType::Float,
            Compiled::RemainingTime => ValueType::Duration,
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

/// How `auraIsActive` reads an aura the character cannot have.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissingAura {
    /// The pinned reference: no value, so the term drops out of its parent.
    Dropped,
    /// Community fix ElliotWood/Forever#622: a constant false.
    Inactive,
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

fn bool_const<R>(value: bool) -> Compiled<R> {
    Compiled::Const(parse_const(if value { "true" } else { "false" }).expect("bool constant"))
}

/// Go `newAPLValue` for the supported subset. `aura` resolves an action ID on the
/// casting player as Go `GetAuraByID` does, or returns `None` when the character lacks it.
fn compile_value<R>(
    value: &Value,
    aura: &dyn Fn(&ActionId) -> Option<R>,
    missing: MissingAura,
) -> Option<Compiled<R>> {
    Some(match value {
        Value::Const(constant) => Compiled::Const(constant.clone()),
        Value::CurrentManaPercent => Compiled::CurrentManaPercent,
        Value::RemainingTime => Compiled::RemainingTime,
        Value::AuraIsKnown(id) => bool_const(aura(id).is_some()),
        Value::AuraIsActive(id) => match (aura(id), missing) {
            (Some(resolved), _) => Compiled::AuraIsActive(resolved),
            (None, MissingAura::Dropped) => return None,
            (None, MissingAura::Inactive) => bool_const(false),
        },
        Value::Compare { op, lhs, rhs } => {
            let lhs = compile_value(lhs, aura, missing)?;
            let rhs = compile_value(rhs, aura, missing)?;
            let to = lhs.value_type().max(rhs.value_type());
            Compiled::Compare {
                op: *op,
                lhs: Box::new(lhs.coerce(to)),
                rhs: Box::new(rhs.coerce(to)),
            }
        }
        Value::And(values) => {
            let mut compiled: Vec<_> = values
                .iter()
                .filter_map(|value| compile_value(value, aura, missing))
                .map(|value| value.coerce(ValueType::Bool))
                .collect();
            match compiled.len() {
                0 => return None,
                1 => compiled.pop().expect("one value"),
                // Go short-circuits an And holding a constant false to that constant.
                _ => match compiled.iter().position(|v| v.const_bool() == Some(false)) {
                    Some(index) => compiled.swap_remove(index),
                    None => Compiled::And(compiled),
                },
            }
        }
    })
}

/// Go `newAPLAction`'s condition handling for one action.
pub fn compile_condition<R>(
    condition: Option<&Value>,
    aura: &dyn Fn(&ActionId) -> Option<R>,
    missing: MissingAura,
) -> CompiledCondition<R> {
    let compiled = condition
        .and_then(|value| compile_value(value, aura, missing))
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
    fn unsupported_operators_are_named() {
        let rotation = serde_json::json!({
            "type": "TypeAPL",
            "priorityList": [
                {"action": {"castSpell": {"spellId": {"spellId": 25304}},
                            "condition": {"dotIsActive": {"spellId": {"spellId": 1}}}}},
                {"action": {"wait": {"duration": {"const": {"val": "1s"}}}}}
            ]
        });
        let reasons = parse(&rotation).unwrap_err();
        assert_eq!(
            reasons,
            [
                "rotation item 1: value dotIsActive is unsupported",
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
        let lacks = |_: &ActionId| None::<()>;
        (
            compile_condition(condition, &lacks, MissingAura::Dropped),
            compile_condition(condition, &lacks, MissingAura::Inactive),
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
}
