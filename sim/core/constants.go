package core

import (
	"time"
)

const CharacterMaxLevel = 60

const GCDMin = time.Second * 1
const GCDDefault = time.Millisecond * 1500
const SpellBatchWindow = time.Millisecond * 10

const DefaultAttackPowerPerDPS = 14.0
const ArmorPenPerPercentArmor = 13.99

const MaxMeleeAttackDistance = 5
const MinRangedAttackDistance = 12

// How often a ranged auto that came due while moving checks whether it can fire.
const RangedAutoRetryInterval = time.Millisecond * 500

const MissDodgeParryBlockCritChancePerDefense = 0.04

const DefenseRatingToChanceReduction = (1.0 / DefenseRatingPerDefense) * MissDodgeParryBlockCritChancePerDefense / 100

const ResilienceRatingPerCritDamageReductionPercent = ResilienceRatingPerCritReductionChance / 2.2

// Updated based on formulas supplied by InDebt on WoWSims Discord
const EnemyAutoAttackAPCoefficient = 1.0 / (14.0 * 177.0)

// Forever bosses take upstream wowsims/forever's value (their 62695f774). Neither number comes
// from the client; the tie-breaker (client > beta logs > their code > ours) picks theirs.
// Against 1/(14 x 177) a boss auto attack lands ~5.6% harder.
const ForeverEnemyAutoAttackAPCoefficient = 0.00052

const AverageMagicPartialResistPerLevelMultiplier = 0.02

// IDs for items used in core
const (
	ItemIDBraidedEterniumChain  = 24114
	ItemIDChainOfTheTwilightOwl = 24121
	ItemIDEyeOfTheNight         = 24116
	ItemIDJadePendantOfBlasting = 20966
	ItemIDTheLightningCapacitor = 28785
)

type Hand bool

const MainHand Hand = true
const OffHand Hand = false

type DefenseType byte

const (
	DefenseTypeNone DefenseType = iota
	DefenseTypeMagic
	DefenseTypeMelee
	DefenseTypeRanged

	DefenseTypeLen
)
