package core

import (
	"fmt"

	"github.com/wowsims/classic/sim/core/proto"
)

// Action groups and value variables, ported from upstream wowsims/forever.
//
// Unlike upstream (which builds each group once and patches placeholders in afterwards),
// every group reference and variable reference is expanded inline at parse time with its
// own copy of the value/action tree. That gives each reference its own variable bindings
// without upstream's group-duplication pass.
// ponytail: no per-evaluation cache for variables (upstream has one); add it if a
// variable-heavy APL shows up in profiles.

// A binding scope: the variables visible inside one group reference.
type aplVarScope map[string]*proto.APLValue

// Resolves a variable name to a value. Group scopes are searched innermost first, then
// (unless placeholderOnly) the rotation's global variables. The bound value is parsed in
// the scope it was written in, so binding x = variable_ref(x) refers to the outer x.
func (rot *APLRotation) resolveAPLVariable(name string, placeholderOnly bool) (APLValue, bool) {
	for i := len(rot.varScopes) - 1; i >= 0; i-- {
		if val, ok := rot.varScopes[i][name]; ok {
			saved := rot.varScopes
			rot.varScopes = saved[:i]
			defer func() { rot.varScopes = saved }()
			return rot.newAPLValue(val), true
		}
	}
	if placeholderOnly {
		return nil, false
	}
	val, ok := rot.valueVariables[name]
	if !ok {
		return nil, false
	}
	key := "var:" + name
	if rot.expanding[key] {
		rot.ValidationWarning("Value variable '%s' refers to itself", name)
		return nil, true
	}
	rot.expanding[key] = true
	saved := rot.varScopes
	rot.varScopes = nil
	defer func() {
		delete(rot.expanding, key)
		rot.varScopes = saved
	}()
	return rot.newAPLValue(val), true
}

func (rot *APLRotation) newValueVariableRef(config *proto.APLValueVariableRef) APLValue {
	value, found := rot.resolveAPLVariable(config.Name, false)
	if !found {
		rot.ValidationWarning("Value variable '%s' not found", config.Name)
	} else if value == nil {
		rot.ValidationWarning("Value variable '%s' is empty or invalid", config.Name)
	}
	return value
}

func (rot *APLRotation) newValueVariablePlaceholder(config *proto.APLValueVariablePlaceholder) APLValue {
	if config.Name == "" {
		rot.ValidationWarning("Variable Placeholder must have a name")
		return nil
	}
	value, found := rot.resolveAPLVariable(config.Name, true)
	if !found {
		rot.ValidationWarning("Variable placeholder '%s' must be filled by the group reference", config.Name)
	}
	if value == nil {
		rot.missingPlaceholder = true
	}
	return value
}

type APLActionGroupReference struct {
	defaultAPLActionImpl
	groupName string
	actions   []*APLAction
}

func (rot *APLRotation) newActionGroupReference(config *proto.APLActionGroupReference) APLActionImpl {
	if config.GroupName == "" {
		rot.ValidationWarning("Group reference must provide a group name")
		return nil
	}
	group := rot.groups[config.GroupName]
	if group == nil {
		rot.ValidationWarning("Group reference '%s' not found", config.GroupName)
		return nil
	}
	key := "group:" + config.GroupName
	if rot.expanding[key] {
		rot.ValidationWarning("Group '%s' references itself", config.GroupName)
		return nil
	}
	rot.expanding[key] = true
	rot.usedGroups[config.GroupName] = true

	scope := aplVarScope{}
	for _, v := range group.Variables {
		scope[v.Name] = v.Value
	}
	for _, v := range config.Variables {
		scope[v.Name] = v.Value
	}
	rot.varScopes = append(rot.varScopes, scope)
	defer func() {
		rot.varScopes = rot.varScopes[:len(rot.varScopes)-1]
		delete(rot.expanding, key)
	}()

	// An unfilled placeholder would silently drop a condition, so the whole reference is invalid.
	outerMissing := rot.missingPlaceholder
	rot.missingPlaceholder = false
	defer func() { rot.missingPlaceholder = outerMissing }()

	var actions []*APLAction
	for _, item := range group.Actions {
		if item.Hide {
			continue
		}
		if action := rot.newAPLAction(item.Action); action != nil {
			actions = append(actions, action)
		}
	}
	if rot.missingPlaceholder {
		return nil
	}
	if len(actions) == 0 {
		rot.ValidationWarning("Group '%s' has no valid actions", config.GroupName)
		return nil
	}
	return &APLActionGroupReference{
		groupName: config.GroupName,
		actions:   actions,
	}
}
func (action *APLActionGroupReference) GetInnerActions() []*APLAction {
	return Flatten(MapSlice(action.actions, func(a *APLAction) []*APLAction { return a.GetAllActions() }))
}
func (action *APLActionGroupReference) IsReady(sim *Simulation) bool {
	for _, a := range action.actions {
		if a.IsReady(sim) {
			return true
		}
	}
	return false
}
func (action *APLActionGroupReference) Execute(sim *Simulation) {
	for _, a := range action.actions {
		if a.IsReady(sim) {
			a.Execute(sim)
			return
		}
	}
}
func (action *APLActionGroupReference) String() string {
	return fmt.Sprintf("Group Reference(%s)", action.groupName)
}

type APLValueActionGroupUsed struct {
	DefaultAPLValueImpl
	rot  *APLRotation
	name string
}

func (rot *APLRotation) newValueActionGroupUsed(config *proto.APLValueActionGroupUsed) APLValue {
	return &APLValueActionGroupUsed{rot: rot, name: config.Name}
}
func (value *APLValueActionGroupUsed) Type() proto.APLValueType {
	return proto.APLValueType_ValueTypeBool
}

// Groups are all expanded at parse time, so by the time the sim runs this is fixed.
func (value *APLValueActionGroupUsed) GetBool(_ *Simulation) bool {
	return value.rot.usedGroups[value.name]
}
func (value *APLValueActionGroupUsed) String() string {
	return fmt.Sprintf("Action Group Is Used(%s)", value.name)
}
