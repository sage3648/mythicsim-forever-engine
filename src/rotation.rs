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

/// Go `ShamanTotems_TotemType`, the totem slot a Shaman value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Totem {
    Earth,
    Air,
    Fire,
    Water,
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
    /// Go sim/shaman/apl_values.go `APLValueTotemRemainingTime`; no totem type gives no value.
    TotemRemainingTime {
        totem: Option<Totem>,
        include_reaction_time: bool,
    },
    CurrentManaPercent,
    /// Go `APLValueCurrentHealthPercent` of the player.
    CurrentHealthPercent,
    RemainingTimePercent,
    CurrentMana,
    CurrentEnergy,
    MaxEnergy,
    CurrentComboPoints,
    TimeToNextEnergyTick,
    /// Go `APLValueCurrentRage`.
    CurrentRage,
    /// Go `APLValueIsExecutePhase`, by its percent threshold.
    IsExecutePhase(i32),
    RemainingTime,
    CurrentTime,
    NumberTargets,
    AuraIsKnown(ActionId),
    /// `auraIsKnown` with one of the player's pets, by its position among them, as its
    /// source unit.
    PetAuraIsKnown {
        pet: usize,
        id: ActionId,
    },
    AuraIsActive(ActionId),
    /// `auraIsActive` with the current target as its source unit.
    TargetAuraIsActive(ActionId),
    AuraNumStacks(ActionId),
    /// `auraNumStacks` with the current target as its source unit.
    TargetAuraNumStacks(ActionId),
    AuraRemainingTime(ActionId),
    /// `auraRemainingTime` with the current target as its source unit.
    TargetAuraRemainingTime(ActionId),
    DotIsActive(ActionId),
    DotRemainingTime(ActionId),
    SpellIsKnown(ActionId),
    SpellIsReady(ActionId),
    SpellCastTime(ActionId),
    SpellTimeToReady(ActionId),
    DotTimeToNextTick(ActionId),
    GcdIsReady,
    /// Go `APLValueAuraShouldRefresh`: the aura, whether it is on the current target, and the
    /// overlap a refresh allows.
    AuraShouldRefresh {
        id: ActionId,
        target: bool,
        max_overlap: Box<Value>,
    },
    /// Go `APLValueFrontOfTarget`.
    FrontOfTarget,
    /// Go `APLValueMaxMana`.
    MaxMana,
    /// Go `APLValueSpellCanCast`: `CanCastOrQueue` on the current target.
    SpellCanCast(ActionId),
    /// Go `APLValueAutoTimeToNext`.
    AutoTimeToNext(AutoAttackType),
    /// Go `APLValueAutoSwingTime`: a hand's current swing duration.
    AutoSwingTime(SwingType),
    /// Go `APLValueSpellCurrentCost`.
    SpellCurrentCost(ActionId),
}

/// Go `APLValueAutoSwingTime_SwingType`: which swing a value reads. Unknown reads as none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwingType {
    Unknown,
    MainHand,
    OffHand,
    Ranged,
}

/// Go `APLValueAutoAttackType`: which auto attack a value reads. Unknown reads as any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoAttackType {
    Any,
    Melee,
    MainHand,
    OffHand,
    Ranged,
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
            Value::AuraShouldRefresh { max_overlap, .. } => max_overlap.visit(f),
            _ => {}
        }
    }

    /// This value with `numberTargets` read as the fight's target count, a constant that
    /// folds.
    pub fn with_targets(&self, count: usize) -> Value {
        let map = |value: &Value| Box::new(value.with_targets(count));
        match self {
            Value::NumberTargets => {
                Value::Const(parse_const(&count.to_string()).expect("int constant"))
            }
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
            Value::And(values) => Value::And(
                values
                    .iter()
                    .map(|value| value.with_targets(count))
                    .collect(),
            ),
            Value::Or(values) => Value::Or(
                values
                    .iter()
                    .map(|value| value.with_targets(count))
                    .collect(),
            ),
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
            | Value::PetAuraIsKnown { .. }
            | Value::AuraIsActive(_)
            | Value::TargetAuraIsActive(_)
            | Value::AuraShouldRefresh { .. }
            | Value::FrontOfTarget
            | Value::DotIsActive(_)
            | Value::SpellIsKnown(_)
            | Value::SpellIsReady(_)
            | Value::IsExecutePhase(_)
            | Value::SpellCanCast(_)
            | Value::GcdIsReady => ValueType::Bool,
            Value::AuraNumStacks(_)
            | Value::TargetAuraNumStacks(_)
            | Value::NumberTargets
            | Value::CurrentComboPoints => ValueType::Int,
            Value::AuraRemainingTime(_)
            | Value::TargetAuraRemainingTime(_)
            | Value::DotRemainingTime(_)
            | Value::SpellCastTime(_)
            | Value::SpellTimeToReady(_)
            | Value::DotTimeToNextTick(_)
            | Value::AutoTimeToNext(_)
            | Value::AutoSwingTime(_)
            | Value::RemainingTime
            | Value::TotemRemainingTime { .. }
            | Value::CurrentTime
            | Value::TimeToNextEnergyTick => ValueType::Duration,
            Value::CurrentManaPercent
            | Value::CurrentHealthPercent
            | Value::CurrentMana
            | Value::RemainingTimePercent
            | Value::CurrentEnergy
            | Value::CurrentRage
            | Value::MaxEnergy
            | Value::SpellCurrentCost(_)
            | Value::MaxMana => ValueType::Float,
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
    /// Go `APLActionCastFriendlySpell` at the player, the one player in scope.
    CastAtPlayer(ActionId),
    AutocastOtherCooldowns,
    /// Go `APLActionStrictSequence`: casts that run in order once the first is ready.
    StrictSequence(Vec<ActionId>),
    /// Go `APLActionSequence`: casts that run one step at a time, each when it is ready, and
    /// never again once done.
    Sequence(Vec<ActionId>),
    /// Go `APLActionChannelSpell`: a channel the rotation may interrupt.
    ChannelSpell {
        spell: ActionId,
        interrupt_if: Option<Value>,
        allow_recast: bool,
    },
    /// Go `APLActionActivateAura` on one of the player's auras, parsed only as a prepull action.
    ActivateAura(ActionId),
    /// Go `APLActionMove`, parsed only as a prepull action: the range from the target to move
    /// to, which Go reads as the constant's float value.
    Move(f64),
    /// Go `APLActionMultidot`: the spell on the first of up to `max_dots` targets whose dot
    /// is down or runs out within `max_overlap`.
    Multidot {
        spell: ActionId,
        max_dots: i32,
        max_overlap: Option<Value>,
    },
}

impl Action {
    /// The spells the action names, in order, as Go `GetAllActions` visits casts.
    pub fn spells(&self) -> Vec<&ActionId> {
        match self {
            Action::CastSpell(id) | Action::CastAtPlayer(id) => vec![id],
            Action::AutocastOtherCooldowns => Vec::new(),
            Action::StrictSequence(ids) | Action::Sequence(ids) => ids.iter().collect(),
            Action::ChannelSpell { spell, .. } | Action::Multidot { spell, .. } => vec![spell],
            Action::ActivateAura(_) | Action::Move(_) => Vec::new(),
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
    /// Go compiles a prepull action's condition only to prune it: a constant false drops the
    /// action, and anything else never stops it from running.
    pub condition: Option<Value>,
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
    let condition = match action.get("condition") {
        Some(condition) => Some(parse_value(condition).map_err(|reasons| reasons.join("; "))?),
        None => None,
    };
    let action = match single(action, &["uuid", "condition"])? {
        ("castSpell", config) => Action::CastSpell(parse_cast_spell(config)?),
        ("activateAura", config) => {
            let object = config.as_object().ok_or("activateAura must be an object")?;
            if let Some(key) = object.keys().find(|key| *key != "auraId") {
                return Err(format!("activateAura field {key} is unsupported"));
            }
            Action::ActivateAura(parse_action_id(
                object.get("auraId").ok_or("activateAura has no auraId")?,
            )?)
        }
        ("move", config) => {
            let object = config.as_object().ok_or("move must be an object")?;
            if let Some(key) = object.keys().find(|key| *key != "rangeFromTarget") {
                return Err(format!("move field {key} is unsupported"));
            }
            let range = object.get("rangeFromTarget").ok_or("move has no range")?;
            match parse_value(range) {
                Ok(Value::Const(constant)) => Action::Move(constant.float),
                Ok(_) => return Err("a move range other than a constant is unsupported".into()),
                Err(reasons) => return Err(reasons.join("; ")),
            }
        }
        (name, _) => return Err(format!("action {name} is unsupported")),
    };
    Ok(Some(Prepull {
        position,
        do_at_ns,
        action,
        condition,
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
        Ok(("castFriendlySpell", config)) => parse_cast_friendly_spell(config),
        Ok(("autocastOtherCooldowns", config)) if is_empty(config) => {
            Ok(Action::AutocastOtherCooldowns)
        }
        Ok(("strictSequence", config)) => parse_strict_sequence(config),
        Ok(("sequence", config)) => parse_sequence(config),
        Ok(("channelSpell", config)) => parse_channel_spell(config),
        Ok(("multidot", config)) => parse_multidot(config),
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

/// Go `newActionSequence` for sequences of unconditional casts. The name only matters to a
/// reset sequence action, which is unsupported.
fn parse_sequence(config: &Json) -> Result<Action, String> {
    let object = config.as_object().ok_or("sequence must be an object")?;
    for key in object.keys() {
        if key != "actions" && key != "name" {
            return Err(format!("sequence field {key} is unsupported"));
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
            .ok_or("sequence action must be an object")?;
        match single(action, &["uuid"])? {
            ("castSpell", cast) => spells.push(parse_cast_spell(cast)?),
            (name, _) => return Err(format!("sequence action {name} is unsupported")),
        }
    }
    Ok(Action::Sequence(spells))
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

/// Go `newActionCastFriendlySpell`: no target means the current target, as for `castSpell`;
/// the first player of the raid, or the unit itself, is the player.
fn parse_cast_friendly_spell(config: &Json) -> Result<Action, String> {
    let object = config
        .as_object()
        .ok_or("castFriendlySpell must be an object")?;
    for key in object.keys() {
        if key != "spellId" && key != "target" {
            return Err(format!("castFriendlySpell field {key} is unsupported"));
        }
    }
    let spell = parse_action_id(
        object
            .get("spellId")
            .ok_or("castFriendlySpell has no spellId")?,
    )?;
    let Some(target) = object.get("target") else {
        return Ok(Action::CastSpell(spell));
    };
    let target = target
        .as_object()
        .ok_or("castFriendlySpell target must be an object")?;
    for key in target.keys() {
        if key != "type" && key != "index" {
            return Err(format!(
                "castFriendlySpell target field {key} is unsupported"
            ));
        }
    }
    let index = match target.get("index") {
        Some(index) => json_i32(index)?,
        None => 0,
    };
    match (target.get("type").and_then(Json::as_str), index) {
        (Some("Player"), 0) | (Some("Self"), _) => Ok(Action::CastAtPlayer(spell)),
        (None | Some("CurrentTarget"), _) => Ok(Action::CastSpell(spell)),
        (kind, index) => Err(format!(
            "castFriendlySpell target {} {index} is unsupported",
            kind.unwrap_or("Unknown")
        )),
    }
}

/// Go `newActionMultidot`: its spell, dot count and overlap.
fn parse_multidot(config: &Json) -> Result<Action, String> {
    let object = config.as_object().ok_or("multidot must be an object")?;
    let mut spell = None;
    let mut max_dots = 0;
    let mut max_overlap = None;
    for (key, field) in object {
        match key.as_str() {
            "spellId" => spell = Some(parse_action_id(field)?),
            "maxDots" => max_dots = json_i32(field)?,
            "maxOverlap" => {
                max_overlap = Some(parse_value(field).map_err(|reasons| reasons.join("; "))?)
            }
            other => return Err(format!("multidot field {other} is unsupported")),
        }
    }
    Ok(Action::Multidot {
        spell: spell.ok_or("multidot has no spellId")?,
        max_dots,
        max_overlap,
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
        // Without a source unit, the player itself.
        "currentHealthPercent" => {
            only(&[])?;
            Ok(Value::CurrentHealthPercent)
        }
        "remainingTime" => {
            only(&[])?;
            Ok(Value::RemainingTime)
        }
        "remainingTimePercent" => {
            only(&[])?;
            Ok(Value::RemainingTimePercent)
        }
        "currentMana" => {
            only(&[])?;
            Ok(Value::CurrentMana)
        }
        "currentTime" => {
            only(&[])?;
            Ok(Value::CurrentTime)
        }
        "currentEnergy" => {
            only(&[])?;
            Ok(Value::CurrentEnergy)
        }
        "maxEnergy" => {
            only(&[])?;
            Ok(Value::MaxEnergy)
        }
        "currentRage" => {
            only(&[])?;
            Ok(Value::CurrentRage)
        }
        "isExecutePhase" => {
            only(&["threshold"])?;
            match config.get("threshold").and_then(Json::as_str) {
                Some("E20") => Ok(Value::IsExecutePhase(20)),
                Some("E25") => Ok(Value::IsExecutePhase(25)),
                Some("E35") => Ok(Value::IsExecutePhase(35)),
                Some("E45") => Ok(Value::IsExecutePhase(45)),
                Some("E90") => Ok(Value::IsExecutePhase(90)),
                other => Err(vec![format!(
                    "isExecutePhase threshold {other:?} is unsupported"
                )]),
            }
        }
        "currentComboPoints" => {
            only(&[])?;
            Ok(Value::CurrentComboPoints)
        }
        "timeToNextEnergyTick" => {
            only(&[])?;
            Ok(Value::TimeToNextEnergyTick)
        }
        "numberTargets" => {
            only(&[])?;
            Ok(Value::NumberTargets)
        }
        "totemRemainingTime" => {
            only(&["totemType", "includeReactionTime"])?;
            let totem = match config.get("totemType").and_then(Json::as_str) {
                None | Some("TypeUnknownTotem") => None,
                Some("Earth") => Some(Totem::Earth),
                Some("Air") => Some(Totem::Air),
                Some("Fire") => Some(Totem::Fire),
                Some("Water") => Some(Totem::Water),
                Some(other) => return Err(vec![format!("totem type {other} is unsupported")]),
            };
            let include_reaction_time = match config.get("includeReactionTime") {
                None => false,
                Some(value) => value
                    .as_bool()
                    .ok_or_else(|| vec!["includeReactionTime must be a boolean".to_string()])?,
            };
            Ok(Value::TotemRemainingTime {
                totem,
                include_reaction_time,
            })
        }
        "gcdIsReady" => {
            only(&[])?;
            Ok(Value::GcdIsReady)
        }
        "autoTimeToNext" => {
            only(&["autoType"])?;
            // protojson writes the enum by name and omits the zero value, UnknownAuto, which
            // Go's switch reads as any auto attack.
            let kind = match config.get("autoType").map(|kind| kind.as_str()) {
                None | Some(Some("UnknownAuto" | "AnyAuto")) => AutoAttackType::Any,
                Some(Some("MeleeAuto")) => AutoAttackType::Melee,
                Some(Some("MainHandAuto")) => AutoAttackType::MainHand,
                Some(Some("OffHandAuto")) => AutoAttackType::OffHand,
                Some(Some("RangedAuto")) => AutoAttackType::Ranged,
                Some(other) => {
                    return Err(vec![format!(
                        "autoTimeToNext autoType {other:?} is unsupported"
                    )])
                }
            };
            Ok(Value::AutoTimeToNext(kind))
        }
        "autoSwingTime" => {
            only(&["autoType"])?;
            // protojson writes the enum by name and omits the zero value, Unknown.
            let kind = match config.get("autoType").map(|kind| kind.as_str()) {
                None | Some(Some("Unknown")) => SwingType::Unknown,
                Some(Some("MainHand")) => SwingType::MainHand,
                Some(Some("OffHand")) => SwingType::OffHand,
                Some(Some("Ranged")) => SwingType::Ranged,
                Some(other) => {
                    return Err(vec![format!(
                        "autoSwingTime autoType {other:?} is unsupported"
                    )])
                }
            };
            Ok(Value::AutoSwingTime(kind))
        }
        "spellCurrentCost" => {
            only(&["spellId"])?;
            let id = config
                .get("spellId")
                .ok_or_else(|| vec!["spellCurrentCost has no spellId".to_string()])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            Ok(Value::SpellCurrentCost(id))
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
        "auraIsActive" | "auraNumStacks" | "auraRemainingTime"
            if fields.is_some_and(|fields| fields.contains_key("sourceUnit")) =>
        {
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
            match (kind, name) {
                (Some("Self"), "auraIsActive") => Ok(Value::AuraIsActive(id)),
                (Some("Self"), "auraNumStacks") => Ok(Value::AuraNumStacks(id)),
                (Some("Self"), _) => Ok(Value::AuraRemainingTime(id)),
                (Some("CurrentTarget"), "auraIsActive") => Ok(Value::TargetAuraIsActive(id)),
                (Some("CurrentTarget"), "auraNumStacks") => Ok(Value::TargetAuraNumStacks(id)),
                (Some("CurrentTarget"), _) => Ok(Value::TargetAuraRemainingTime(id)),
                _ => Err(vec![format!(
                    "{name} sourceUnit {} is unsupported",
                    config.get("sourceUnit").cloned().unwrap_or_default()
                )]),
            }
        }
        "auraIsKnown" if fields.is_some_and(|fields| fields.contains_key("sourceUnit")) => {
            // Go GetSourceUnit: the player itself, or a pet of the player by its index.
            only(&["auraId", "sourceUnit"])?;
            let id = config
                .get("auraId")
                .ok_or_else(|| vec![format!("{name} has no auraId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            let unsupported = || {
                vec![format!(
                    "{name} sourceUnit {} is unsupported",
                    config.get("sourceUnit").cloned().unwrap_or_default()
                )]
            };
            let source = config
                .get("sourceUnit")
                .and_then(Json::as_object)
                .ok_or_else(unsupported)?;
            let kind = source.get("type").and_then(Json::as_str);
            let owner_is_self =
                source
                    .get("owner")
                    .and_then(Json::as_object)
                    .is_some_and(|owner| {
                        owner.len() == 1 && owner.get("type").and_then(Json::as_str) == Some("Self")
                    });
            let known_keys = source
                .keys()
                .all(|key| ["type", "index", "owner"].contains(&key.as_str()));
            match kind {
                Some("Self") if source.len() == 1 => Ok(Value::AuraIsKnown(id)),
                Some("Pet") if owner_is_self && known_keys => {
                    let pet = match source.get("index") {
                        None => 0,
                        Some(index) => index
                            .as_u64()
                            .and_then(|index| usize::try_from(index).ok())
                            .ok_or_else(unsupported)?,
                    };
                    Ok(Value::PetAuraIsKnown { pet, id })
                }
                _ => Err(unsupported()),
            }
        }
        "auraShouldRefresh" => {
            only(&["auraId", "maxOverlap", "sourceUnit"])?;
            let id = config
                .get("auraId")
                .ok_or_else(|| vec![format!("{name} has no auraId")])
                .and_then(|id| parse_action_id(id).map_err(|err| vec![err]))?;
            // Go `GetTargetUnit`: no unit reference means the current target.
            let target = match config.get("sourceUnit") {
                None => true,
                Some(source) => match source.as_object() {
                    Some(unit) if unit.keys().all(|key| key == "type") => {
                        match unit.get("type").and_then(Json::as_str) {
                            Some("Self") => false,
                            Some("CurrentTarget") => true,
                            _ => {
                                return Err(vec![format!(
                                    "{name} sourceUnit {source} is unsupported"
                                )])
                            }
                        }
                    }
                    _ => return Err(vec![format!("{name} sourceUnit {source} is unsupported")]),
                },
            };
            let max_overlap = match config.get("maxOverlap") {
                Some(value) => parse_value(value)?,
                // Go defaults a missing overlap to a constant 0ms.
                None => Value::Const(parse_const("0ms").expect("duration constant")),
            };
            Ok(Value::AuraShouldRefresh {
                id,
                target,
                max_overlap: Box::new(max_overlap),
            })
        }
        "frontOfTarget" => {
            only(&[])?;
            Ok(Value::FrontOfTarget)
        }
        "maxMana" => {
            only(&[])?;
            Ok(Value::MaxMana)
        }
        "auraIsKnown" | "auraIsActive" | "auraNumStacks" | "auraRemainingTime" => {
            // sourceUnit, except on auraIsActive and auraNumStacks, and includeReactionTime
            // are not modeled.
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
    TotemRemainingTime {
        totem: Totem,
        include_reaction_time: bool,
    },
    CurrentManaPercent,
    /// Go `APLValueCurrentHealthPercent` of the player.
    CurrentHealthPercent,
    RemainingTimePercent,
    CurrentMana,
    CurrentEnergy,
    MaxEnergy,
    MaxMana,
    FrontOfTarget,
    /// Go `APLValueAuraShouldRefresh` with the overlap coerced to a duration.
    AuraShouldRefresh {
        aura: R,
        overlap: Box<Compiled<R>>,
    },
    CurrentComboPoints,
    TimeToNextEnergyTick,
    CurrentRage,
    IsExecutePhase(i32),
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
    AutoTimeToNext(AutoAttackType),
    AutoSwingTime(SwingType),
    SpellCurrentCost(usize),
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
            | Compiled::IsExecutePhase(_)
            | Compiled::SpellCanCast(_)
            | Compiled::GcdIsReady => ValueType::Bool,
            Compiled::AuraNumStacks(_) | Compiled::NumberTargets | Compiled::CurrentComboPoints => {
                ValueType::Int
            }
            Compiled::AuraRemainingTime(_)
            | Compiled::AutoTimeToNext(_)
            | Compiled::AutoSwingTime(_)
            | Compiled::DotRemainingTime(_)
            | Compiled::SpellCastTime(_)
            | Compiled::SpellTimeToReady(_)
            | Compiled::DotTimeToNextTick(_)
            | Compiled::RemainingTime
            | Compiled::TotemRemainingTime { .. }
            | Compiled::CurrentTime
            | Compiled::TimeToNextEnergyTick => ValueType::Duration,
            Compiled::CurrentManaPercent
            | Compiled::CurrentHealthPercent
            | Compiled::CurrentMana
            | Compiled::RemainingTimePercent
            | Compiled::CurrentEnergy
            | Compiled::CurrentRage
            | Compiled::MaxEnergy
            | Compiled::SpellCurrentCost(_)
            | Compiled::MaxMana => ValueType::Float,
            Compiled::FrontOfTarget | Compiled::AuraShouldRefresh { .. } => ValueType::Bool,
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
    /// Go `GetAuraByID` on the player's pet at a position among its pets: whether it has the
    /// aura, false when there is no such pet.
    pub pet_aura_known: &'a dyn Fn(usize, &ActionId) -> bool,
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
    #[cfg(test)]
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
    deciding: bool,
    build: fn(Vec<Compiled<R>>) -> Compiled<R>,
) -> Option<Compiled<R>> {
    let mut compiled: Vec<_> = values
        .iter()
        .filter_map(|value| compile_value(value, lookup))
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
/// value, so the term drops out of its parent. An aura the character lacks reads as inactive,
/// with no stacks and no time left (ElliotWood/Forever#622).
fn compile_value<R>(value: &Value, lookup: &Lookup<R>) -> Option<Compiled<R>> {
    let aura = lookup.aura;
    Some(match value {
        Value::Const(constant) => Compiled::Const(constant.clone()),
        Value::CurrentManaPercent => Compiled::CurrentManaPercent,
        Value::CurrentHealthPercent => Compiled::CurrentHealthPercent,
        Value::RemainingTimePercent => Compiled::RemainingTimePercent,
        Value::CurrentMana => Compiled::CurrentMana,
        Value::CurrentEnergy => Compiled::CurrentEnergy,
        Value::CurrentRage => Compiled::CurrentRage,
        Value::IsExecutePhase(threshold) => Compiled::IsExecutePhase(*threshold),
        Value::MaxEnergy => Compiled::MaxEnergy,
        Value::MaxMana => Compiled::MaxMana,
        Value::FrontOfTarget => Compiled::FrontOfTarget,
        // Go `newValueAuraShouldRefresh`: no value without the aura.
        Value::AuraShouldRefresh {
            id,
            target,
            max_overlap,
        } => {
            let found = if *target {
                (lookup.target_aura)(id)
            } else {
                aura(id)
            }?;
            let overlap = compile_value(max_overlap, lookup)?.coerce(ValueType::Duration);
            Compiled::AuraShouldRefresh {
                aura: found.aura,
                overlap: Box::new(overlap),
            }
        }
        Value::CurrentComboPoints => Compiled::CurrentComboPoints,
        Value::TimeToNextEnergyTick => Compiled::TimeToNextEnergyTick,
        Value::RemainingTime => Compiled::RemainingTime,
        Value::CurrentTime => Compiled::CurrentTime,
        Value::NumberTargets => Compiled::NumberTargets,
        Value::TotemRemainingTime {
            totem,
            include_reaction_time,
        } => Compiled::TotemRemainingTime {
            totem: (*totem)?,
            include_reaction_time: *include_reaction_time,
        },
        Value::DotIsActive(id) => Compiled::DotIsActive((lookup.dot)(id)?),
        Value::DotRemainingTime(id) => Compiled::DotRemainingTime((lookup.dot)(id)?),
        // Go `newValueSpellIsKnown` is a constant.
        Value::SpellIsKnown(id) => bool_const((lookup.spell)(id).is_some()),
        Value::SpellIsReady(id) => Compiled::SpellIsReady((lookup.spell)(id)?),
        Value::SpellCastTime(id) => Compiled::SpellCastTime((lookup.spell)(id)?),
        Value::SpellTimeToReady(id) => Compiled::SpellTimeToReady((lookup.spell)(id)?),
        Value::SpellCanCast(id) => Compiled::SpellCanCast((lookup.spell)(id)?),
        Value::AutoTimeToNext(auto) => Compiled::AutoTimeToNext(*auto),
        Value::AutoSwingTime(kind) => Compiled::AutoSwingTime(*kind),
        // Go GetAPLSpell: an unknown spell gives no value.
        Value::SpellCurrentCost(id) => Compiled::SpellCurrentCost((lookup.spell)(id)?),
        Value::DotTimeToNextTick(id) => Compiled::DotTimeToNextTick((lookup.dot)(id)?),
        Value::GcdIsReady => Compiled::GcdIsReady,
        Value::AuraIsKnown(id) => bool_const(aura(id).is_some()),
        Value::PetAuraIsKnown { pet, id } => bool_const((lookup.pet_aura_known)(*pet, id)),
        Value::AuraIsActive(id) => match aura(id) {
            Some(found) => Compiled::AuraIsActive(found.aura),
            None => bool_const(false),
        },
        Value::TargetAuraIsActive(id) => match (lookup.target_aura)(id) {
            Some(found) => Compiled::AuraIsActive(found.aura),
            None => bool_const(false),
        },
        Value::TargetAuraNumStacks(id) => match (lookup.target_aura)(id) {
            Some(found) if found.max_stacks == 0 => return None,
            Some(found) => Compiled::AuraNumStacks(found.aura),
            None => Compiled::Const(parse_const("0").expect("int constant")),
        },
        Value::AuraNumStacks(id) => match aura(id) {
            // Go warns that the aura does not stack and drops the value.
            Some(found) if found.max_stacks == 0 => return None,
            Some(found) => Compiled::AuraNumStacks(found.aura),
            None => Compiled::Const(parse_const("0").expect("int constant")),
        },
        Value::Compare { op, lhs, rhs } => {
            let lhs = compile_value(lhs, lookup)?;
            let rhs = compile_value(rhs, lookup)?;
            let to = lhs.value_type().max(rhs.value_type());
            Compiled::Compare {
                op: *op,
                lhs: Box::new(lhs.coerce(to)),
                rhs: Box::new(rhs.coerce(to)),
            }
        }
        Value::Math { op, lhs, rhs } => {
            let lhs = compile_value(lhs, lookup)?;
            let rhs = compile_value(rhs, lookup)?;
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
        Value::AuraRemainingTime(id) => match aura(id) {
            Some(found) => Compiled::AuraRemainingTime(found.aura),
            None => Compiled::Const(parse_const("0ms").expect("duration constant")),
        },
        Value::TargetAuraRemainingTime(id) => match (lookup.target_aura)(id) {
            Some(found) => Compiled::AuraRemainingTime(found.aura),
            None => Compiled::Const(parse_const("0ms").expect("duration constant")),
        },
        // Go `newValueNot` folds a constant operand.
        Value::Not(value) => {
            let value = compile_value(value, lookup)?.coerce(ValueType::Bool);
            match value.const_bool() {
                Some(constant) => bool_const(!constant),
                None => Compiled::Not(Box::new(value)),
            }
        }
        Value::And(values) => return fold(values, lookup, false, Compiled::And),
        Value::Or(values) => return fold(values, lookup, true, Compiled::Or),
    })
}

/// Go `coerceTo(newAPLValue(value), Bool)`, as `newActionChannelSpell` compiles its
/// interrupt condition: `None` when the value has none.
pub fn compile_bool_value<R>(value: Option<&Value>, lookup: &Lookup<R>) -> Option<Compiled<R>> {
    value
        .and_then(|value| compile_value(value, lookup))
        .map(|value| value.coerce(ValueType::Bool))
}

/// Go `coerceTo(newAPLValue(value), Duration)`, as `newActionMultidot` compiles its overlap:
/// `None` when the value has none.
pub fn compile_duration_value<R>(value: Option<&Value>, lookup: &Lookup<R>) -> Option<Compiled<R>> {
    value
        .and_then(|value| compile_value(value, lookup))
        .map(|value| value.coerce(ValueType::Duration))
}

/// Go `newAPLAction`'s condition handling for one action.
pub fn compile_condition<R>(condition: Option<&Value>, lookup: &Lookup<R>) -> CompiledCondition<R> {
    let compiled = condition
        .and_then(|value| compile_value(value, lookup))
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
            pet_aura_known: &|_, _| false,
        }
    }

    /// Go `GetSourceUnit` on a pet of the player reads that pet's auras, a constant.
    #[test]
    fn aura_is_known_reads_a_pet_of_the_player() {
        let pet = serde_json::json!({"auraIsKnown": {
            "auraId": {"spellId": 1293696},
            "sourceUnit": {"type": "Pet", "index": 1, "owner": {"type": "Self"}}
        }});
        let value = parse_value(&pet).unwrap();
        let id = ActionId {
            spell_id: 1293696,
            ..ActionId::default()
        };
        assert_eq!(
            value,
            Value::PetAuraIsKnown {
                pet: 1,
                id: id.clone()
            }
        );
        let none = |_: &ActionId| -> Option<FoundAura<ActionId>> { None };
        let known = |pet: usize, aura: &ActionId| pet == 1 && *aura == id;
        let lookup = Lookup {
            pet_aura_known: &known,
            ..only_auras(&none)
        };
        assert_eq!(
            compile_condition(Some(&value), &lookup),
            CompiledCondition::Always
        );
        let other = serde_json::json!({"auraIsKnown": {
            "auraId": {"spellId": 1293696},
            "sourceUnit": {"type": "Pet", "index": 0, "owner": {"type": "CurrentTarget"}}
        }});
        assert!(parse_value(&other).is_err());
    }

    /// Go `APLValueAutoSwingTime` and `APLValueSpellCurrentCost`, read by the upstream Rogue
    /// and Feral presets.
    #[test]
    fn swing_time_and_current_cost_parse() {
        let swing = serde_json::json!({"autoSwingTime": {"autoType": "MainHand"}});
        assert_eq!(
            parse_value(&swing),
            Ok(Value::AutoSwingTime(SwingType::MainHand))
        );
        assert_eq!(
            Value::AutoSwingTime(SwingType::MainHand).value_type(),
            ValueType::Duration
        );
        let unknown = serde_json::json!({"autoSwingTime": {}});
        assert_eq!(
            parse_value(&unknown),
            Ok(Value::AutoSwingTime(SwingType::Unknown))
        );
        let cost = serde_json::json!({"spellCurrentCost": {"spellId": {"spellId": 9830}}});
        let id = ActionId {
            spell_id: 9830,
            ..ActionId::default()
        };
        assert_eq!(parse_value(&cost), Ok(Value::SpellCurrentCost(id.clone())));
        assert_eq!(Value::SpellCurrentCost(id).value_type(), ValueType::Float);
    }

    /// Go `APLActionMultidot` in the priority list and `APLActionActivateAura` among the
    /// prepull actions, as the upstream Balance presets use them.
    #[test]
    fn multidot_and_prepull_aura_activation_parse() {
        let rotation = serde_json::json!({
            "type": "TypeAPL",
            "prepullActions": [{"action": {"activateAura": {"auraId": {"spellId": 24858}}},
                                "doAtValue": {"const": {"val": "-10s"}}}],
            "priorityList": [{"action": {"multidot": {"spellId": {"spellId": 9835}, "maxDots": 3,
                                                      "maxOverlap": {"const": {"val": "0ms"}}}}}]
        });
        let parsed = parse(&rotation).unwrap();
        let id = |spell_id| ActionId {
            spell_id,
            ..ActionId::default()
        };
        assert_eq!(parsed.prepull[0].action, Action::ActivateAura(id(24858)));
        let Action::Multidot {
            spell, max_dots, ..
        } = &parsed.priority_list[0].action
        else {
            panic!("expected a multidot");
        };
        assert_eq!((spell, *max_dots), (&id(9835), 3));
        assert_eq!(parsed.priority_list[0].action.spells(), vec![&id(9835)]);
        let item = serde_json::json!({"action": {"activateAura": {"auraId": {"spellId": 24858}}}});
        assert!(parse_item(&item, 1).is_err());
    }

    /// Go `APLActionMove` is read among the prepull actions only, with the range as the float
    /// the constant holds: a duration constant holds none, so it moves to zero yards. A range
    /// that is not a constant stays unsupported, and so does a move in the priority list.
    #[test]
    fn prepull_move_parses_its_constant_range() {
        let prepull = |range: serde_json::Value| {
            serde_json::json!({
                "type": "TypeAPL",
                "prepullActions": [{"action": {"move": {"rangeFromTarget": range}},
                                    "doAtValue": {"const": {"val": "-5s"}}}]
            })
        };
        for (range, yards) in [("25", 25.0), ("7.5", 7.5), ("30s", 0.0), ("true", 0.0)] {
            let parsed = parse(&prepull(serde_json::json!({"const": {"val": range}}))).unwrap();
            assert_eq!(parsed.prepull[0].action, Action::Move(yards), "{range}");
            assert_eq!(parsed.prepull[0].do_at_ns, -5_000_000_000);
            assert!(parsed.prepull[0].action.spells().is_empty());
        }
        let variable = prepull(serde_json::json!({"currentRage": {}}));
        assert_eq!(
            parse(&variable).unwrap_err(),
            ["prepull action 1: a move range other than a constant is unsupported"]
        );
        let item =
            serde_json::json!({"action": {"move": {"rangeFromTarget": {"const": {"val": "5"}}}}});
        assert!(parse_item(&item, 1).is_err());
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

    /// The condition over spell 44404, an aura the test character lacks.
    fn condition(json: serde_json::Value) -> CompiledCondition<()> {
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
        compile_condition(condition, &only_auras(&lacks))
    }

    #[test]
    fn missing_auras_read_as_inactive_as_community_622() {
        let active = serde_json::json!({"auraIsActive": {"auraId": {"spellId": 44404}}});
        let known = serde_json::json!({"auraIsKnown": {"auraId": {"spellId": 44404}}});
        let low_mana = serde_json::json!({"cmp": {
            "op": "OpLt", "lhs": {"currentManaPercent": {}}, "rhs": {"const": {"val": "20%"}},
        }});
        // Unguarded, the action never fires; the pinned reference before #622 dropped the
        // term and fired it on every pass.
        assert_eq!(condition(active.clone()), CompiledCondition::Pruned);
        // An auraIsKnown guard prunes the action too.
        assert_eq!(
            condition(serde_json::json!({"and": {"vals": [known, active.clone()]}})),
            CompiledCondition::Pruned
        );
        // The constant false decides an And.
        assert_eq!(
            condition(serde_json::json!({"and": {"vals": [low_mana, active.clone()]}})),
            CompiledCondition::Pruned
        );
        // Go does not fold comparisons, so a live constant comparison remains.
        assert!(matches!(
            condition(serde_json::json!({"cmp": {
                "op": "OpEq", "lhs": active, "rhs": {"const": {"val": "false"}},
            }})),
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
            compile_condition(condition.as_ref(), &only_auras(&find))
        };
        let stacks = |id: i32| {
            serde_json::json!({"cmp": {"op": "OpGe",
                "lhs": {"auraNumStacks": {"auraId": {"spellId": id}}}, "rhs": {"const": {"val": "3"}}}})
        };
        let known = serde_json::json!({"auraIsKnown": {"auraId": {"spellId": 400573}}});
        // A known aura's stacks compare as integers.
        assert!(matches!(
            compile(stacks(400573), 4),
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        // Go drops the stacks of an aura without MaxStacks.
        assert_eq!(compile(stacks(400573), 0), CompiledCondition::Always);
        // A missing aura compares a constant 0, which never reaches 3.
        let missing = compile(stacks(44404), 4);
        assert!(matches!(
            missing,
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        assert!(missing.never_holds());
        // A constant true decides an Or.
        assert_eq!(
            compile(
                serde_json::json!({"or": {"vals": [stacks(400573), known]}}),
                4,
            ),
            CompiledCondition::Always
        );
        // Without the stacking aura, the constant 0 >= 3 beside the mana term means the mana
        // term alone.
        let low_mana = serde_json::json!({"cmp": {
            "op": "OpLt", "lhs": {"currentManaPercent": {}}, "rhs": {"const": {"val": "40%"}},
        }});
        let with_missing = compile(
            serde_json::json!({"or": {"vals": [stacks(44404), low_mana.clone()]}}),
            4,
        );
        assert_ne!(with_missing, compile(low_mana.clone(), 4));
        assert!(with_missing.same_meaning(&compile(low_mana, 4)));
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
            compile_condition(condition.as_ref(), &only_auras(&find))
        };
        let known = |id: i32| serde_json::json!({"auraIsKnown": {"auraId": {"spellId": id}}});
        // Go folds Not of a constant.
        assert_eq!(
            compile(serde_json::json!({"not": {"val": known(22959)}})),
            CompiledCondition::Pruned
        );
        assert_eq!(
            compile(serde_json::json!({"not": {"val": known(400625)}})),
            CompiledCondition::Always
        );
        // Remaining time compares as a duration; a missing aura has none left.
        let remaining = |id: i32| {
            serde_json::json!({"cmp": {"op": "OpLe",
                "lhs": {"auraRemainingTime": {"auraId": {"spellId": id}}}, "rhs": {"const": {"val": "4s"}}}})
        };
        assert!(matches!(
            compile(remaining(22959)),
            CompiledCondition::When(Compiled::Compare { .. })
        ));
        // 0 <= 4s always holds.
        assert!(compile(remaining(400625)).same_meaning(&CompiledCondition::Always));
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

    #[test]
    fn a_named_sequence_and_a_target_aura_remaining_time_parse() {
        let rotation = parse(&serde_json::json!({
            "type": "TypeAPL",
            "priorityList": [
                {"action": {
                    "condition": {"cmp": {"op": "OpLe",
                        "lhs": {"auraRemainingTime": {"auraId": {"spellId": 11581},
                            "sourceUnit": {"type": "CurrentTarget"}}},
                        "rhs": {"const": {"val": "1.5s"}}}},
                    "sequence": {"name": "Stance into Reck", "actions": [
                        {"castSpell": {"spellId": {"spellId": 2458}}},
                        {"castSpell": {"spellId": {"spellId": 1719}}},
                    ]},
                }},
            ],
        }))
        .unwrap();
        let item = &rotation.priority_list[0];
        assert_eq!(
            item.action,
            Action::Sequence(vec![ActionId::spell(2458), ActionId::spell(1719)])
        );
        let Some(Value::Compare { lhs, .. }) = &item.condition else {
            panic!("expected a comparison");
        };
        assert_eq!(
            **lhs,
            Value::TargetAuraRemainingTime(ActionId::spell(11581))
        );
    }

    /// Go `newActionCastFriendlySpell`: the first player or the unit itself is the player, no
    /// target is the current target, and any other unit is not modeled.
    #[test]
    fn a_friendly_cast_names_the_player_or_the_current_target() {
        let cast = |target: Option<serde_json::Value>| {
            let mut config = serde_json::json!({"spellId": {"spellId": 25292}});
            if let Some(target) = target {
                config["target"] = target;
            }
            parse(&serde_json::json!({
                "type": "TypeAPL",
                "priorityList": [{"action": {
                    "condition": {"cmp": {"op": "OpLt", "lhs": {"currentHealthPercent": {}},
                        "rhs": {"const": {"val": "60%"}}}},
                    "castFriendlySpell": config,
                }}],
            }))
            .map(|rotation| rotation.priority_list[0].action.clone())
        };
        let spell = ActionId::spell(25292);
        assert_eq!(
            cast(Some(serde_json::json!({"type": "Player", "index": 0}))),
            Ok(Action::CastAtPlayer(spell.clone()))
        );
        assert_eq!(
            cast(Some(serde_json::json!({"type": "Self"}))),
            Ok(Action::CastAtPlayer(spell.clone()))
        );
        assert_eq!(cast(None), Ok(Action::CastSpell(spell)));
        assert!(cast(Some(serde_json::json!({"type": "Player", "index": 1}))).is_err());
    }
}
