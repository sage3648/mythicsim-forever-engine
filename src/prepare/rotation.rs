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
use super::sim::SpellId;
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
}

struct Builder<'a> {
    env: &'a Environment,
    /// Go `rot.prunedActions`: impls of actions a constant false condition removed.
    pruned_casts: Vec<SpellId>,
}

/// Whether a value is a constant expression Go may fold: no part of it reads the simulation.
fn is_constant_expression(value: &Message) -> bool {
    match value.oneof("value") {
        None => true,
        Some((field, Value::Message(inner))) => match field {
            "const" => true,
            "and" | "or" => inner
                .messages("vals")
                .into_iter()
                .all(is_constant_expression),
            "not" => inner.message("val").is_none_or(is_constant_expression),
            "cmp" | "math" => {
                inner.message("lhs").is_none_or(is_constant_expression)
                    && inner.message("rhs").is_none_or(is_constant_expression)
            }
            "min" | "max" => inner
                .messages("vals")
                .into_iter()
                .all(is_constant_expression),
            _ => false,
        },
        Some(_) => false,
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
        if let Some(condition) = config.message("condition") {
            if is_constant_expression(condition) {
                let simple_true = condition
                    .message("const")
                    .is_some_and(|c| c.str("val").eq_ignore_ascii_case("true"));
                if !simple_true {
                    return Err(Refusal::new(
                        "rotation",
                        "a rotation condition Go folds to a constant is not prepared yet"
                            .to_string(),
                    ));
                }
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
            return Ok(built.then(|| Built { casts: Vec::new() }));
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
                Ok(Some(Built { casts: vec![spell] }))
            }
            "channel_spell" => {
                let Some(id) = action.message("spell_id") else {
                    return Ok(None);
                };
                let id = proto_to_action_id(id);
                // Go `newActionChannelSpell`: without an interrupt condition it is a cast
                // action, which removes its spell from the major cooldowns; with one it is a
                // channel action, which does not.
                let casts = action.message("interrupt_if").is_none();
                let spell = if casts {
                    apl_cast_spell(self.env, &id)
                } else {
                    apl_spell(self.env, &id)
                };
                let Some(spell) = spell else {
                    return Ok(None);
                };
                if !casts
                    && !self
                        .env
                        .sim
                        .spell(spell)
                        .flags
                        .matches(SpellFlag::CHANNELED)
                {
                    return Ok(None);
                }
                if !target_resolves(self.env, action.message("target"))? {
                    return Ok(None);
                }
                Ok(Some(Built {
                    casts: if casts { vec![spell] } else { Vec::new() },
                }))
            }
            "sequence" | "strict_sequence" => {
                let mut casts = Vec::new();
                for inner in action.messages("actions") {
                    if let Some(built) = self.action(Some(inner))? {
                        casts.extend(built.casts);
                    }
                }
                Ok(Some(Built { casts }))
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
                Ok(Some(Built { casts: Vec::new() }))
            }
            "autocast_other_cooldowns" | "wait" | "wait_until" => {
                Ok(Some(Built { casts: Vec::new() }))
            }
            other => Err(Refusal::new(
                "rotation",
                format!("rotation action {other} is not prepared yet"),
            )),
        }
    }
}

/// Every `proto.APLValue` inside a message, depth first.
fn values<'a>(message: &'a Message, out: &mut Vec<&'a Message>) {
    if message.type_name() == "proto.APLValue" {
        out.push(message);
    }
    for name in message.set_fields() {
        match message.get(name) {
            Some(Value::Message(inner)) => values(inner, out),
            Some(Value::List(items)) => {
                for item in items {
                    if let Value::Message(inner) = item {
                        values(inner, out);
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
fn register_value_observers(env: &mut Environment, rotation: &Message) -> Result<(), Refusal> {
    let mut found = Vec::new();
    values(rotation, &mut found);
    for value in found {
        let Some((kind, Value::Message(config))) = value.oneof("value") else {
            continue;
        };
        match kind {
            "aura_num_stacks" => {
                let Some(id) = config.message("aura_id") else {
                    continue;
                };
                let id = proto_to_action_id(id);
                let unit = match config.message("source_unit").map(|u| u.enum_name("type")) {
                    None => env.player,
                    Some(kind) if kind == "Unknown" || kind == "Self" => env.player,
                    Some(kind) if kind == "CurrentTarget" => env.encounter.targets[0],
                    Some(kind) => {
                        return Err(Refusal::new(
                            "rotation",
                            format!("an aura stack read on a {kind} unit is not prepared yet"),
                        ))
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
                    None => {
                        return Err(Refusal::new(
                            "rotation",
                            format!("the rotation reads the stacks of {id}, which the unit lacks, as a constant"),
                        ))
                    }
                    Some(aura) if env.sim.aura(aura).max_stacks > 0 => {
                        env.sim.apply_on_stacks_change(aura, std::rc::Rc::new(|_: &mut super::sim::Sim, _, _, _| {}));
                        env.sim.apply_on_reset(aura, std::rc::Rc::new(|_: &mut super::sim::Sim, _| {}));
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
    {
        let mut builder = Builder {
            env,
            pruned_casts: Vec::new(),
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
    }
    register_value_observers(env, rotation)?;
    env.prepull_actions += prepull;
    let player = env.player;
    for spell in casts.into_iter().chain(pruned) {
        let action = env.sim.spell(spell).action_id.clone();
        env.sim.remove_initial_major_cooldown(player, &action);
    }
    Ok(())
}
