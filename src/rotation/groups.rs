//! Go apl.go `newAPLRotation` and apl_action_group_reference.go: the groups a rotation's
//! references run.
//!
//! Go builds one instance of a group for each reference to it, and for the group itself when
//! nothing references it, by repeating the group's configuration as it finds references to it
//! (a second reference to a group builds another instance, and so does a reference inside a
//! group). A reference then binds the first instance nothing else is bound to when the actions
//! are finalized, in a depth first order the references' own finalizing sets. Binding fills the
//! placeholders of the instance's conditions with the reference's variables and gives the
//! conditions' variable references the values of the group's variables.
//!
//! [`bind`] repeats that on the parsed rotation. Only a condition holds a placeholder or takes
//! a group's variable in Go: the values of an action's own fields (a move's range, a channel's
//! interrupt condition) keep their placeholders and the rotation's variables, and a rotation
//! that depends on that is refused.

use std::collections::{BTreeMap, BTreeSet};

use super::{
    math_operand_types, Action, CompareOp, Group, Item, MathOp, Rotation, Value, ValueType,
};

/// More instances than a rotation could mean; Go builds one more for each reference a group
/// holds to itself, without end.
const MAX_INSTANCES: usize = 256;

/// One reference: in the priority list by its position in it, or in an instance by the position
/// in the instance's items.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reference {
    Top(usize),
    Nested { instance: usize, item: usize },
}

/// One instance of a group.
#[derive(Clone, Debug, PartialEq)]
pub struct Instance {
    pub name: String,
    /// The group's variables, which the variables of the reference that binds it add to.
    variables: Vec<(String, Value)>,
    /// The group's actions, their conditions as the reference that binds the instance left
    /// them.
    pub items: Vec<Item>,
    /// The reference bound to the instance, which runs it.
    pub referenced_by: Option<Reference>,
}

/// What binding the references of a rotation to the instances of its groups settles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Binding {
    pub instances: Vec<Instance>,
    /// The instance each reference runs.
    pub bound: BTreeMap<Reference, usize>,
}

impl Binding {
    /// The instance a reference runs, `None` for one that found no group, which is never ready.
    pub fn instance(&self, reference: Reference) -> Option<usize> {
        self.bound.get(&reference).copied()
    }
}

/// Go `newAPLRotation`'s group handling and the references' `Finalize`: the rotation with its
/// `actionGroupUsed` values settled, and the instances bound. `live` says whether the priority
/// list's item at an index (counting the items the rotation holds) survived Go's building: a
/// reference whose condition is a constant false is pruned and takes no part.
pub fn bind(
    rotation: &Rotation,
    live: &dyn Fn(usize) -> bool,
) -> Result<(Rotation, Binding), String> {
    let named_reference = |item: &Item| match &item.action {
        Action::GroupReference { name, .. } if !name.is_empty() => Some(name.clone()),
        _ => None,
    };
    let top: Vec<(usize, String)> = rotation
        .priority_list
        .iter()
        .enumerate()
        .filter(|(index, _)| live(*index))
        .filter_map(|(index, item)| named_reference(item).map(|name| (index, name)))
        .collect();

    // The groups' configurations, which Go repeats for each reference past the first.
    let mut configs: Vec<&Group> = rotation.groups.iter().collect();
    let mut instances: Vec<Instance> = Vec::new();
    let mut matched: BTreeSet<Reference> = BTreeSet::new();
    let mut index = 0;
    while index < configs.len() {
        if instances.len() >= MAX_INSTANCES {
            return Err("a group refers to itself without end".into());
        }
        let config = configs[index];
        instances.push(Instance {
            name: config.name.clone(),
            variables: config.variables.clone(),
            items: config.items.clone(),
            referenced_by: None,
        });
        // A group referenced more than once gets another instance for each reference past the
        // first.
        let mut found = false;
        for (position, name) in &top {
            if *name == config.name && matched.insert(Reference::Top(*position)) {
                if found {
                    configs.push(config);
                }
                found = true;
                instances[index].referenced_by = Some(Reference::Top(*position));
            }
        }
        // So does a group a group's actions refer to.
        for (item_index, item) in config.items.iter().enumerate() {
            let Some(name) = named_reference(item) else {
                continue;
            };
            let reference = Reference::Nested {
                instance: index,
                item: item_index,
            };
            for other in configs.clone() {
                if name == other.name && !matched.contains(&reference) {
                    configs.push(other);
                    matched.insert(reference);
                }
            }
        }
        index += 1;
    }

    // Finalizing: the groups' actions in order, then the priority list's.
    let mut binding = Binding {
        instances,
        bound: BTreeMap::new(),
    };
    let mut finalizing: Vec<Reference> = Vec::new();
    for instance in 0..binding.instances.len() {
        for item in 0..binding.instances[instance].items.len() {
            if named_reference(&binding.instances[instance].items[item]).is_some() {
                finalize(
                    &mut binding,
                    rotation,
                    Reference::Nested { instance, item },
                    &mut finalizing,
                )?;
            }
        }
    }
    for (position, _) in &top {
        finalize(
            &mut binding,
            rotation,
            Reference::Top(*position),
            &mut finalizing,
        )?;
    }

    // Go `APLValueActionGroupUsed.Finalize`: a group of the name holds a reference.
    let used: BTreeSet<String> = binding
        .instances
        .iter()
        .filter(|instance| instance.referenced_by.is_some())
        .map(|instance| instance.name.clone())
        .collect();
    let mark = |value: &mut Value| {
        value.visit_mut(&mut |value| {
            if let Value::ActionGroupUsed { name, used: flag } = value {
                *flag = used.contains(name);
            }
        })
    };
    let mut settled = rotation.clone();
    for item in settled.priority_list.iter_mut().chain(
        binding
            .instances
            .iter_mut()
            .flat_map(|i| i.items.iter_mut()),
    ) {
        if let Some(condition) = &mut item.condition {
            mark(condition);
        }
    }
    for prepull in &mut settled.prepull {
        if let Some(condition) = &mut prepull.condition {
            mark(condition);
        }
    }
    // A placeholder that is left reads as nothing in Go, and is not reproduced. A group nothing
    // references never runs, and its placeholders are never read.
    for instance in binding
        .instances
        .iter()
        .filter(|instance| instance.referenced_by.is_some())
    {
        if instance
            .items
            .iter()
            .any(|item| item.condition.as_ref().is_some_and(value_has_placeholder))
        {
            return Err("a placeholder of a group's condition is left unfilled".into());
        }
    }
    Ok((settled, binding))
}

/// Go `APLActionGroupReference.Finalize`: bind the first instance of the name nothing else is
/// bound to, fill its conditions' placeholders and give them the group's variables, then
/// finalize the instance's own actions.
fn finalize(
    binding: &mut Binding,
    rotation: &Rotation,
    reference: Reference,
    finalizing: &mut Vec<Reference>,
) -> Result<(), String> {
    // The references finalizing now, so that one that holds itself ends.
    if finalizing.contains(&reference) {
        return Ok(());
    }
    finalizing.push(reference);
    let result = finalize_once(binding, rotation, reference, finalizing);
    finalizing.pop();
    result
}

fn finalize_once(
    binding: &mut Binding,
    rotation: &Rotation,
    reference: Reference,
    finalizing: &mut Vec<Reference>,
) -> Result<(), String> {
    let action = match reference {
        Reference::Top(position) => &rotation.priority_list[position].action,
        Reference::Nested { instance, item } => &binding.instances[instance].items[item].action,
    };
    let Action::GroupReference { name, variables } = action.clone() else {
        return Ok(());
    };
    let found = binding.instances.iter().position(|instance| {
        instance.name == name
            && (instance.referenced_by.is_none() || instance.referenced_by == Some(reference))
    });
    let Some(group) = found else {
        // Go reports the missing group and the reference is never ready.
        binding.bound.remove(&reference);
        return Ok(());
    };
    binding.instances[group].referenced_by = Some(reference);
    let first_bind = binding.bound.insert(reference, group).is_none();
    if first_bind {
        let mut variables_of_group = binding.instances[group].variables.clone();
        // Every placeholder of the conditions needs a variable of the reference.
        let mut placeholders: BTreeSet<String> = BTreeSet::new();
        for item in &binding.instances[group].items {
            if let Some(condition) = &item.condition {
                condition.visit(&mut |value| {
                    if let Value::Placeholder(placeholder) = value {
                        placeholders.insert(placeholder.clone());
                    }
                });
            }
            for value in item.action.values() {
                if value_has_placeholder(value)
                    && !matches!(item.action, Action::GroupReference { .. })
                {
                    return Err(format!(
                        "group {name:?} holds a placeholder in the value of an action, which Go leaves unfilled"
                    ));
                }
            }
        }
        let provided: BTreeMap<String, Value> = variables.iter().cloned().collect();
        if let Some(missing) = placeholders.iter().find(|p| !provided.contains_key(*p)) {
            return Err(format!(
                "group {name:?} requires variable placeholder {missing:?} to be filled"
            ));
        }
        for item in &mut binding.instances[group].items {
            if let Some(condition) = item.condition.take() {
                item.condition = Some(condition.replace_placeholders(&provided)?);
            }
        }
        for (variable, value) in &variables {
            variables_of_group.retain(|(existing, _)| existing != variable);
            variables_of_group.push((variable.clone(), value.clone()));
        }
        let scope: BTreeMap<String, Value> = variables_of_group.into_iter().collect();
        for item in &mut binding.instances[group].items {
            if let Some(condition) = &mut item.condition {
                condition.override_variables(&scope);
            }
        }
    }
    // Finalizing the reference finalizes the instance's actions, which may refer to groups.
    for item in 0..binding.instances[group].items.len() {
        if matches!(
            &binding.instances[group].items[item].action,
            Action::GroupReference { name, .. } if !name.is_empty()
        ) {
            finalize(
                binding,
                rotation,
                Reference::Nested {
                    instance: group,
                    item,
                },
                finalizing,
            )?;
        }
    }
    Ok(())
}

fn value_has_placeholder(value: &Value) -> bool {
    let mut found = false;
    value.visit(&mut |inner| found |= matches!(inner, Value::Placeholder(_)));
    found
}

impl Value {
    /// Visit this value and every nested value, parents first, to change them.
    pub fn visit_mut(&mut self, f: &mut impl FnMut(&mut Value)) {
        f(self);
        match self {
            Value::Compare { lhs, rhs, .. } | Value::Math { lhs, rhs, .. } => {
                lhs.visit_mut(f);
                rhs.visit_mut(f);
            }
            Value::And(values) | Value::Or(values) => {
                values.iter_mut().for_each(|value| value.visit_mut(f))
            }
            Value::Not(value) => value.visit_mut(f),
            Value::AuraShouldRefresh { max_overlap, .. } => max_overlap.visit_mut(f),
            Value::Variable { resolved, .. } => resolved.visit_mut(f),
            _ => {}
        }
    }

    /// Go `reResolveVariableRefs`: a variable reference takes the value of the group's variable
    /// of its name, as far as the values reach their inner values. The new value is not
    /// searched for variables itself.
    fn override_variables(&mut self, scope: &BTreeMap<String, Value>) {
        match self {
            Value::Variable { name, resolved } => {
                if let Some(replacement) = scope.get(name) {
                    **resolved = replacement.clone();
                }
            }
            Value::Compare { lhs, rhs, .. } | Value::Math { lhs, rhs, .. } => {
                lhs.override_variables(scope);
                rhs.override_variables(scope);
            }
            Value::And(values) | Value::Or(values) => values
                .iter_mut()
                .for_each(|value| value.override_variables(scope)),
            Value::Not(value) => value.override_variables(scope),
            Value::AuraShouldRefresh { max_overlap, .. } => max_overlap.override_variables(scope),
            _ => {}
        }
    }

    /// Go `replacePlaceholders`: a placeholder becomes the value its reference gave it. Only the
    /// values Go rebuilds hold the replacement: a comparison, math, and, or and not; any other
    /// value keeps its own. Go does not check the new operands as it builds the value, so a
    /// replacement that those checks would refuse, or a logical operand of another type that Go
    /// reads with a getter it lacks, is refused here.
    pub fn replace_placeholders(self, provided: &BTreeMap<String, Value>) -> Result<Value, String> {
        let replace = |value: Value| value.replace_placeholders(provided);
        Ok(match self {
            Value::Placeholder(name) => provided
                .get(&name)
                .cloned()
                .ok_or_else(|| format!("variable placeholder {name:?} is not filled"))?,
            Value::Compare { op, lhs, rhs } => {
                let (lhs, rhs) = (replace(*lhs)?, replace(*rhs)?);
                let to = lhs.value_type().max(rhs.value_type());
                if to == ValueType::Bool && !matches!(op, CompareOp::Eq | CompareOp::Ne) {
                    return Err(
                        "a placeholder filled so that booleans are compared in order".into(),
                    );
                }
                Value::Compare {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            Value::Math { op, lhs, rhs } => {
                let (lhs, rhs) = (replace(*lhs)?, replace(*rhs)?);
                check_math(op, &lhs, &rhs)?;
                Value::Math {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            Value::And(values) => Value::And(logical_operands(values, provided)?),
            Value::Or(values) => Value::Or(logical_operands(values, provided)?),
            Value::Not(value) => {
                let value = replace(*value)?;
                check_logical(&value)?;
                Value::Not(Box::new(value))
            }
            other => other,
        })
    }
}

fn logical_operands(
    values: Vec<Value>,
    provided: &BTreeMap<String, Value>,
) -> Result<Vec<Value>, String> {
    values
        .into_iter()
        .map(|value| {
            let value = value.replace_placeholders(provided)?;
            check_logical(&value)?;
            Ok(value)
        })
        .collect()
}

/// A logical operand is read as a boolean: Go coerces the ones it builds, and not the one a
/// placeholder was replaced with.
fn check_logical(value: &Value) -> Result<(), String> {
    if matches!(value, Value::Const(_)) || value.value_type() == ValueType::Bool {
        Ok(())
    } else {
        Err("a placeholder in a logical operator filled with a value that is not a boolean".into())
    }
}

/// The checks `newValueMath` makes as it builds the value, which Go does not repeat after a
/// placeholder is replaced.
fn check_math(op: MathOp, lhs: &Value, rhs: &Value) -> Result<(), String> {
    let (lhs_type, rhs_type) = math_operand_types(op, lhs.value_type(), rhs.value_type());
    let numeric = |t: ValueType| matches!(t, ValueType::Int | ValueType::Float);
    if matches!(lhs_type, ValueType::Bool | ValueType::String)
        || matches!(rhs_type, ValueType::Bool | ValueType::String)
        || (op == MathOp::Mul && lhs_type == ValueType::Duration && rhs_type == ValueType::Duration)
        || (op == MathOp::Div && numeric(lhs_type) && rhs_type == ValueType::Duration)
    {
        return Err("a placeholder filled so that the math is not a value".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rotation::parse;
    use serde_json::json;

    fn cast(spell: i32) -> serde_json::Value {
        json!({"action": {"castSpell": {"spellId": {"spellId": spell}}}})
    }

    fn reference(name: &str) -> serde_json::Value {
        json!({"action": {"groupReference": {"groupName": name}}})
    }

    fn bound(rotation: serde_json::Value) -> Result<(Rotation, Binding), String> {
        let rotation = parse(&rotation).map_err(|reasons| reasons.join("; "))?;
        bind(&rotation, &|_| true)
    }

    fn spells(instance: &Instance) -> Vec<i32> {
        instance
            .items
            .iter()
            .filter_map(|item| match &item.action {
                Action::CastSpell { spell, .. } => Some(spell.spell_id),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_reference_binds_the_instance_of_its_group() {
        let (_, binding) = bound(json!({"type": "TypeAPL",
            "priorityList": [cast(1), reference("g"), cast(2)],
            "groups": [{"name": "g", "actions": [cast(3), cast(4)]}]}))
        .unwrap();
        assert_eq!(binding.instances.len(), 1);
        assert_eq!(binding.instance(Reference::Top(1)), Some(0));
        assert_eq!(binding.instance(Reference::Top(0)), None);
        assert_eq!(spells(&binding.instances[0]), [3, 4]);
        assert_eq!(binding.instances[0].referenced_by, Some(Reference::Top(1)));
    }

    #[test]
    fn a_second_reference_builds_another_instance_and_binds_in_gos_order() {
        let (_, binding) = bound(json!({"type": "TypeAPL",
            "priorityList": [reference("g"), cast(1), reference("g")],
            "groups": [{"name": "g", "actions": [cast(3)]}]}))
        .unwrap();
        assert_eq!(binding.instances.len(), 2);
        // The first instance keeps the last reference that matched it, so the first reference
        // finds the second instance, which nothing is bound to.
        assert_eq!(binding.instance(Reference::Top(2)), Some(0));
        assert_eq!(binding.instance(Reference::Top(0)), Some(1));
    }

    #[test]
    fn a_reference_inside_a_group_builds_the_instance_it_binds() {
        let (_, binding) = bound(json!({"type": "TypeAPL",
            "priorityList": [reference("outer")],
            "groups": [
                {"name": "outer", "actions": [cast(1), reference("inner")]},
                {"name": "inner", "actions": [cast(2)]}]}))
        .unwrap();
        // The inner group is built twice: for the config and for the reference that names it.
        assert_eq!(binding.instances.len(), 3);
        assert_eq!(binding.instance(Reference::Top(0)), Some(0));
        let nested = Reference::Nested {
            instance: 0,
            item: 1,
        };
        // The reference finds the first instance nothing is bound to, which is the config's; the
        // copy Go built for it stays unreferenced.
        assert_eq!(binding.instance(nested), Some(1));
        assert!(binding.instances[2].referenced_by.is_none());
    }

    #[test]
    fn a_pruned_reference_binds_nothing_and_an_unfound_one_is_never_ready() {
        let rotation = parse(&json!({"type": "TypeAPL",
            "priorityList": [reference("g"), reference("nowhere")],
            "groups": [{"name": "g", "actions": [cast(3)]}]}))
        .unwrap();
        let (_, binding) = bind(&rotation, &|index| index != 0).unwrap();
        assert_eq!(binding.instance(Reference::Top(0)), None);
        assert_eq!(binding.instance(Reference::Top(1)), None);
        assert_eq!(binding.instances[0].referenced_by, None);
    }

    #[test]
    fn placeholders_take_the_values_of_the_reference() {
        let rotation = json!({"type": "TypeAPL",
            "priorityList": [{"action": {"groupReference": {"groupName": "g", "variables": [
                {"name": "when", "value": {"const": {"val": "5s"}}}]}}}],
            "groups": [{"name": "g", "actions": [{"action": {
                "castSpell": {"spellId": {"spellId": 3}},
                "condition": {"cmp": {"op": "OpGe", "lhs": {"currentTime": {}},
                    "rhs": {"variablePlaceholder": {"name": "when"}}}}}}]}]});
        let (_, binding) = bound(rotation.clone()).unwrap();
        let Some(Value::Compare { rhs, .. }) = &binding.instances[0].items[0].condition else {
            panic!("a comparison");
        };
        assert!(matches!(**rhs, Value::Const(_)));
        // A reference that leaves the placeholder unfilled is refused.
        let mut unfilled = rotation;
        unfilled["priorityList"][0]["action"]["groupReference"]["variables"] = json!([]);
        assert!(bound(unfilled)
            .unwrap_err()
            .contains("requires variable placeholder"));
    }

    #[test]
    fn a_placeholder_in_an_operand_go_does_not_check_again_is_refused() {
        let group = |condition: serde_json::Value| {
            json!({"type": "TypeAPL",
                "priorityList": [{"action": {"groupReference": {"groupName": "g", "variables": [
                    {"name": "p", "value": {"currentRage": {}}}]}}}],
                "groups": [{"name": "g", "actions": [{"action": {
                    "castSpell": {"spellId": {"spellId": 3}}, "condition": condition}}]}]})
        };
        let placeholder = json!({"variablePlaceholder": {"name": "p"}});
        // A float is no boolean for an and.
        assert!(bound(group(json!({"and": {"vals": [placeholder.clone()]}}))).is_err());
        // A float compares in order.
        let compared =
            json!({"cmp": {"op": "OpGt", "lhs": placeholder, "rhs": {"const": {"val": "1"}}}});
        assert!(bound(group(compared)).is_ok());
    }

    #[test]
    fn a_groups_variable_replaces_the_rotations_in_its_conditions() {
        let rotation = json!({"type": "TypeAPL",
            "valueVariables": [{"name": "open", "value": {"const": {"val": "true"}}}],
            "priorityList": [reference("g"), {"action": {
                "castSpell": {"spellId": {"spellId": 1}},
                "condition": {"variableRef": {"name": "open"}}}}],
            "groups": [{"name": "g", "variables": [
                {"name": "open", "value": {"const": {"val": "false"}}}], "actions": [{"action": {
                "castSpell": {"spellId": {"spellId": 3}},
                "condition": {"variableRef": {"name": "open"}}}}]}]});
        let (settled, binding) = bound(rotation).unwrap();
        let resolved = |condition: &Option<Value>| match condition {
            Some(Value::Variable { resolved, .. }) => match **resolved {
                Value::Const(ref constant) => constant.boolean,
                _ => panic!("a constant"),
            },
            _ => panic!("a variable"),
        };
        assert!(!resolved(&binding.instances[0].items[0].condition));
        assert!(resolved(&settled.priority_list[1].condition));
    }

    #[test]
    fn a_group_is_used_when_a_reference_is_bound_to_it() {
        let used = |name: &str| {
            json!({"action": {"castSpell": {"spellId": {"spellId": 1}},
                "condition": {"actionGroupUsed": {"name": name}}}})
        };
        let (settled, _) = bound(json!({"type": "TypeAPL",
            "priorityList": [reference("g"), used("g"), used("unused")],
            "groups": [{"name": "g", "actions": [cast(3)]}, {"name": "unused", "actions": [cast(4)]}]}))
        .unwrap();
        let flag = |index: usize| match &settled.priority_list[index].condition {
            Some(Value::ActionGroupUsed { used, .. }) => *used,
            _ => panic!("a group used value"),
        };
        assert!(flag(1));
        assert!(!flag(2));
    }

    #[test]
    fn a_group_that_names_itself_does_not_end() {
        let error = bound(json!({"type": "TypeAPL",
            "priorityList": [reference("g")],
            "groups": [{"name": "g", "actions": [reference("g")]}]}))
        .unwrap_err();
        assert!(error.contains("without end"), "{error}");
    }
}
