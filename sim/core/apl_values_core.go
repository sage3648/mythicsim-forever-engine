package core

import (
	"fmt"
	"time"

	"github.com/wowsims/classic/sim/core/proto"
)

type APLValueDotIsActive struct {
	DefaultAPLValueImpl
	dot *Dot
}

func (rot *APLRotation) newValueDotIsActive(config *proto.APLValueDotIsActive) APLValue {
	dot := rot.GetAPLDot(rot.GetTargetUnit(config.TargetUnit), config.SpellId)
	if dot == nil {
		return nil
	}
	return &APLValueDotIsActive{
		dot: dot,
	}
}
func (value *APLValueDotIsActive) Type() proto.APLValueType {
	return proto.APLValueType_ValueTypeBool
}
func (value *APLValueDotIsActive) GetBool(sim *Simulation) bool {
	return value.dot.IsActive()
}
func (value *APLValueDotIsActive) String() string {
	return fmt.Sprintf("Dot Is Active(%s)", value.dot.Spell.ActionID)
}

type APLValueDotRemainingTime struct {
	DefaultAPLValueImpl
	dot *Dot
}

func (rot *APLRotation) newValueDotRemainingTime(config *proto.APLValueDotRemainingTime) APLValue {
	dot := rot.GetAPLDot(rot.GetTargetUnit(config.TargetUnit), config.SpellId)
	if dot == nil {
		return nil
	}
	return &APLValueDotRemainingTime{
		dot: dot,
	}
}
func (value *APLValueDotRemainingTime) Type() proto.APLValueType {
	return proto.APLValueType_ValueTypeDuration
}
func (value *APLValueDotRemainingTime) GetDuration(sim *Simulation) time.Duration {
	return value.dot.RemainingDuration(sim)
}
func (value *APLValueDotRemainingTime) String() string {
	return fmt.Sprintf("Dot Remaining Time(%s)", value.dot.Spell.ActionID)
}

type APLValueDotTimeToNextTick struct {
	DefaultAPLValueImpl
	dot *Dot
}

func (rot *APLRotation) newValueDotTimeToNextTick(config *proto.APLValueDotTimeToNextTick) APLValue {
	dot := rot.GetAPLDot(rot.GetTargetUnit(config.TargetUnit), config.SpellId)
	if dot == nil {
		return nil
	}
	return &APLValueDotTimeToNextTick{dot: dot}
}
func (value *APLValueDotTimeToNextTick) Type() proto.APLValueType {
	return proto.APLValueType_ValueTypeDuration
}

// 0 when the dot is not ticking (upstream returns a stale value there).
func (value *APLValueDotTimeToNextTick) GetDuration(sim *Simulation) time.Duration {
	if !value.dot.IsActive() {
		return 0
	}
	return max(0, value.dot.TimeUntilNextTick(sim))
}
func (value *APLValueDotTimeToNextTick) String() string {
	return fmt.Sprintf("Dot Time To Next Tick(%s)", value.dot.Spell.ActionID)
}
