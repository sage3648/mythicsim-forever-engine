//! The client's own spell enums, as Go's `sim/core/dbcenums` declares them: what an effect
//! does, which aura it applies, the attribute bits a spell carries and the proc bits its aura
//! listens to.
//!
//! Every constant keeps Go's name and numeric value. Go's enum types are plain integers here,
//! because the rows in [`crate::data::spells`] store the columns as `i32`; the aliases below
//! only say which enum a value belongs to. The generated stringers (`Named`, `String`) are not
//! ported.

#![allow(non_upper_case_globals)]

/// Go `SpellEffectType`: `SpellEffect.Effect`.
pub(crate) type SpellEffectType = i32;
/// Go `EffectAuraType`.
pub(crate) type EffectAuraType = i32;
/// Go `ImplicitTarget`: `ImplicitTarget_0/_1`.
pub(crate) type ImplicitTarget = i32;
/// Go `Mechanic`: `SpellCategories.Mechanic` and `EffectMechanic`.
pub(crate) type Mechanic = i32;
/// Go `PowerType`: `SpellPower.PowerType`.
pub(crate) type PowerType = i32;
/// Go `SpellModOp`: the misc value of `A_ADD_FLAT_MODIFIER` and `A_ADD_PCT_MODIFIER`.
pub(crate) type SpellModOp = i32;
/// Go `ShapeshiftForm`: `SpellShapeshiftForm.ID`. Zero is no form.
pub(crate) type ShapeshiftForm = u8;

/// Go `EffectAuraType.IsProcTrigger`: the two auras that cast their trigger spell when the proc
/// fires.
pub(crate) fn is_proc_trigger(aura: EffectAuraType) -> bool {
    aura == A_PROC_TRIGGER_SPELL || aura == A_PROC_TRIGGER_SPELL_WITH_VALUE
}

/// Go `PowerType.InTenths`: rage runs 0-1000 where the sim counts 0-100, so a cost of 150 is 15
/// rage. Every other bar is stated in whole points.
pub(crate) fn power_in_tenths(power: PowerType) -> bool {
    power == POWER_RAGE
}

/// The forms whose `SpellShapeshiftForm.Flags` set bit 1 (Go `stanceForms`).
const STANCE_FORMS: u64 = 0x68a70002;

/// Go `ShapeshiftForm.Mask`: the bit a `SpellShapeshift` mask names this form by. No form has
/// no bit. A form past bit 63 shifts out to zero, as Go's shift does.
pub(crate) fn form_mask(form: ShapeshiftForm) -> u64 {
    if form == 0 {
        return 0;
    }
    1u64.checked_shl(u32::from(form) - 1).unwrap_or(0)
}

/// Go `ShapeshiftForm.IsStance`: a stance, in which a spell that names no form can be cast,
/// rather than a shapeshift, in which it cannot.
pub(crate) fn form_is_stance(form: ShapeshiftForm) -> bool {
    STANCE_FORMS & form_mask(form) != 0
}

// SpellEffectType, Go dbcenums/effects.go
pub(crate) const E_NONE: SpellEffectType = 0;
pub(crate) const E_INSTAKILL: SpellEffectType = 1;
pub(crate) const E_SCHOOL_DAMAGE: SpellEffectType = 2;
pub(crate) const E_DUMMY: SpellEffectType = 3;
pub(crate) const E_PORTAL_TELEPORT: SpellEffectType = 4;
pub(crate) const E_UNK_ITEM_MOD: SpellEffectType = 5;
pub(crate) const E_APPLY_AURA: SpellEffectType = 6;
pub(crate) const E_ENVIRONMENTAL_DAMAGE: SpellEffectType = 7;
pub(crate) const E_POWER_DRAIN: SpellEffectType = 8;
pub(crate) const E_HEALTH_LEECH: SpellEffectType = 9;
pub(crate) const E_HEAL: SpellEffectType = 10;
pub(crate) const E_BIND: SpellEffectType = 11;
pub(crate) const E_PORTAL: SpellEffectType = 12;
pub(crate) const E_TELEPORT_TO_RETURN_POINT: SpellEffectType = 13;
pub(crate) const E_INCREASE_CURRENCY_CAP: SpellEffectType = 14;
pub(crate) const E_TELEPORT_WITH_SPELL_VISUAL_KIT_LOADING_SCREEN: SpellEffectType = 15;
pub(crate) const E_QUEST_COMPLETE: SpellEffectType = 16;
pub(crate) const E_WEAPON_DAMAGE_NOSCHOOL: SpellEffectType = 17;
pub(crate) const E_RESURRECT: SpellEffectType = 18;
pub(crate) const E_ADD_EXTRA_ATTACKS: SpellEffectType = 19;
pub(crate) const E_DODGE: SpellEffectType = 20;
pub(crate) const E_EVADE: SpellEffectType = 21;
pub(crate) const E_PARRY: SpellEffectType = 22;
pub(crate) const E_BLOCK: SpellEffectType = 23;
pub(crate) const E_CREATE_ITEM: SpellEffectType = 24;
pub(crate) const E_WEAPON: SpellEffectType = 25;
pub(crate) const E_DEFENSE: SpellEffectType = 26;
pub(crate) const E_PERSISTENT_AREA_AURA: SpellEffectType = 27;
pub(crate) const E_SUMMON: SpellEffectType = 28;
pub(crate) const E_LEAP: SpellEffectType = 29;
pub(crate) const E_ENERGIZE: SpellEffectType = 30;
pub(crate) const E_WEAPON_PERCENT_DAMAGE: SpellEffectType = 31;
pub(crate) const E_TRIGGER_MISSILE: SpellEffectType = 32;
pub(crate) const E_OPEN_LOCK: SpellEffectType = 33;
pub(crate) const E_SUMMON_CHANGE_ITEM: SpellEffectType = 34;
pub(crate) const E_APPLY_AREA_AURA_PARTY: SpellEffectType = 35;
pub(crate) const E_LEARN_SPELL: SpellEffectType = 36;
pub(crate) const E_SPELL_DEFENSE: SpellEffectType = 37;
pub(crate) const E_DISPEL: SpellEffectType = 38;
pub(crate) const E_LANGUAGE: SpellEffectType = 39;
pub(crate) const E_DUAL_WIELD: SpellEffectType = 40;
pub(crate) const E_JUMP: SpellEffectType = 41;
pub(crate) const E_JUMP_DEST: SpellEffectType = 42;
pub(crate) const E_TELEPORT_UNITS_FACE_CASTER: SpellEffectType = 43;
pub(crate) const E_SKILL_STEP: SpellEffectType = 44;
pub(crate) const E_PLAY_MOVIE: SpellEffectType = 45;
pub(crate) const E_SPAWN: SpellEffectType = 46;
pub(crate) const E_TRADE_SKILL: SpellEffectType = 47;
pub(crate) const E_STEALTH: SpellEffectType = 48;
pub(crate) const E_DETECT: SpellEffectType = 49;
pub(crate) const E_TRANS_DOOR: SpellEffectType = 50;
pub(crate) const E_FORCE_CRITICAL_HIT: SpellEffectType = 51;
pub(crate) const E_SET_MAX_BATTLE_PET_COUNT: SpellEffectType = 52;
pub(crate) const E_ENCHANT_ITEM: SpellEffectType = 53;
pub(crate) const E_ENCHANT_ITEM_TEMPORARY: SpellEffectType = 54;
pub(crate) const E_TAMECREATURE: SpellEffectType = 55;
pub(crate) const E_SUMMON_PET: SpellEffectType = 56;
pub(crate) const E_LEARN_PET_SPELL: SpellEffectType = 57;
pub(crate) const E_WEAPON_DAMAGE: SpellEffectType = 58;
pub(crate) const E_CREATE_RANDOM_ITEM: SpellEffectType = 59;
pub(crate) const E_PROFICIENCY: SpellEffectType = 60;
pub(crate) const E_SEND_EVENT: SpellEffectType = 61;
pub(crate) const E_POWER_BURN: SpellEffectType = 62;
pub(crate) const E_THREAT: SpellEffectType = 63;
pub(crate) const E_TRIGGER_SPELL: SpellEffectType = 64;
pub(crate) const E_APPLY_AREA_AURA_RAID: SpellEffectType = 65;
pub(crate) const E_RECHARGE_ITEM: SpellEffectType = 66;
pub(crate) const E_HEAL_MAX_HEALTH: SpellEffectType = 67;
pub(crate) const E_INTERRUPT_CAST: SpellEffectType = 68;
pub(crate) const E_DISTRACT: SpellEffectType = 69;
pub(crate) const E_PULL: SpellEffectType = 70;
pub(crate) const E_PICKPOCKET: SpellEffectType = 71;
pub(crate) const E_ADD_FARSIGHT: SpellEffectType = 72;
pub(crate) const E_UNTRAIN_TALENTS: SpellEffectType = 73;
pub(crate) const E_APPLY_GLYPH: SpellEffectType = 74;
pub(crate) const E_HEAL_MECHANICAL: SpellEffectType = 75;
pub(crate) const E_SUMMON_OBJECT_WILD: SpellEffectType = 76;
pub(crate) const E_SCRIPT_EFFECT: SpellEffectType = 77;
pub(crate) const E_ATTACK: SpellEffectType = 78;
pub(crate) const E_SANCTUARY: SpellEffectType = 79;
pub(crate) const E_MODIFY_FOLLOWER_ITEM_LEVEL: SpellEffectType = 80;
pub(crate) const E_PUSH_ABILITY_TO_ACTION_BAR: SpellEffectType = 81;
pub(crate) const E_BIND_SIGHT: SpellEffectType = 82;
pub(crate) const E_DUEL: SpellEffectType = 83;
pub(crate) const E_STUCK: SpellEffectType = 84;
pub(crate) const E_SUMMON_PLAYER: SpellEffectType = 85;
pub(crate) const E_ACTIVATE_OBJECT: SpellEffectType = 86;
pub(crate) const E_GAMEOBJECT_DAMAGE: SpellEffectType = 87;
pub(crate) const E_GAMEOBJECT_REPAIR: SpellEffectType = 88;
pub(crate) const E_GAMEOBJECT_SET_DESTRUCTION_STATE: SpellEffectType = 89;
pub(crate) const E_KILL_CREDIT: SpellEffectType = 90;
pub(crate) const E_THREAT_ALL: SpellEffectType = 91;
pub(crate) const E_ENCHANT_HELD_ITEM: SpellEffectType = 92;
pub(crate) const E_FORCE_DESELECT: SpellEffectType = 93;
pub(crate) const E_SELF_RESURRECT: SpellEffectType = 94;
pub(crate) const E_SKINNING: SpellEffectType = 95;
pub(crate) const E_CHARGE: SpellEffectType = 96;
pub(crate) const E_CAST_BUTTON: SpellEffectType = 97;
pub(crate) const E_KNOCK_BACK: SpellEffectType = 98;
pub(crate) const E_DISENCHANT: SpellEffectType = 99;
pub(crate) const E_INEBRIATE: SpellEffectType = 100;
pub(crate) const E_FEED_PET: SpellEffectType = 101;
pub(crate) const E_DISMISS_PET: SpellEffectType = 102;
pub(crate) const E_REPUTATION: SpellEffectType = 103;
pub(crate) const E_SUMMON_OBJECT_SLOT1: SpellEffectType = 104;
pub(crate) const E_SURVEY: SpellEffectType = 105;
pub(crate) const E_CHANGE_RAID_MARKER: SpellEffectType = 106;
pub(crate) const E_SHOW_CORPSE_LOOT: SpellEffectType = 107;
pub(crate) const E_DISPEL_MECHANIC: SpellEffectType = 108;
pub(crate) const E_RESURRECT_PET: SpellEffectType = 109;
pub(crate) const E_DESTROY_ALL_TOTEMS: SpellEffectType = 110;
pub(crate) const E_DURABILITY_DAMAGE: SpellEffectType = 111;
pub(crate) const E_CANCEL_CONVERSATION: SpellEffectType = 113;
pub(crate) const E_ATTACK_ME: SpellEffectType = 114;
pub(crate) const E_DURABILITY_DAMAGE_PCT: SpellEffectType = 115;
pub(crate) const E_SKIN_PLAYER_CORPSE: SpellEffectType = 116;
pub(crate) const E_SPIRIT_HEAL: SpellEffectType = 117;
pub(crate) const E_SKILL: SpellEffectType = 118;
pub(crate) const E_APPLY_AREA_AURA_PET: SpellEffectType = 119;
pub(crate) const E_TELEPORT_GRAVEYARD: SpellEffectType = 120;
pub(crate) const E_NORMALIZED_WEAPON_DMG: SpellEffectType = 121;
pub(crate) const E_SEND_TAXI: SpellEffectType = 123;
pub(crate) const E_PULL_TOWARDS: SpellEffectType = 124;
pub(crate) const E_MODIFY_THREAT_PERCENT: SpellEffectType = 125;
pub(crate) const E_STEAL_BENEFICIAL_BUFF: SpellEffectType = 126;
pub(crate) const E_PROSPECTING: SpellEffectType = 127;
pub(crate) const E_APPLY_AREA_AURA_FRIEND: SpellEffectType = 128;
pub(crate) const E_APPLY_AREA_AURA_ENEMY: SpellEffectType = 129;
pub(crate) const E_REDIRECT_THREAT: SpellEffectType = 130;
pub(crate) const E_PLAY_SOUND: SpellEffectType = 131;
pub(crate) const E_PLAY_MUSIC: SpellEffectType = 132;
pub(crate) const E_UNLEARN_SPECIALIZATION: SpellEffectType = 133;
pub(crate) const E_KILL_CREDIT2: SpellEffectType = 134;
pub(crate) const E_CALL_PET: SpellEffectType = 135;
pub(crate) const E_HEAL_PCT: SpellEffectType = 136;
pub(crate) const E_ENERGIZE_PCT: SpellEffectType = 137;
pub(crate) const E_LEAP_BACK: SpellEffectType = 138;
pub(crate) const E_CLEAR_QUEST: SpellEffectType = 139;
pub(crate) const E_FORCE_CAST: SpellEffectType = 140;
pub(crate) const E_FORCE_CAST_WITH_VALUE: SpellEffectType = 141;
pub(crate) const E_TRIGGER_SPELL_WITH_VALUE: SpellEffectType = 142;
pub(crate) const E_APPLY_AREA_AURA_OWNER: SpellEffectType = 143;
pub(crate) const E_KNOCK_BACK_DEST: SpellEffectType = 144;
pub(crate) const E_PULL_TOWARDS_DEST: SpellEffectType = 145;
pub(crate) const E_RESTORE_GARRISON_TROOP_VITALITY: SpellEffectType = 146;
pub(crate) const E_QUEST_FAIL: SpellEffectType = 147;
pub(crate) const E_TRIGGER_MISSILE_SPELL_WITH_VALUE: SpellEffectType = 148;
pub(crate) const E_CHARGE_DEST: SpellEffectType = 149;
pub(crate) const E_QUEST_START: SpellEffectType = 150;
pub(crate) const E_TRIGGER_SPELL_2: SpellEffectType = 151;
pub(crate) const E_SUMMON_RAF_FRIEND: SpellEffectType = 152;
pub(crate) const E_CREATE_TAMED_PET: SpellEffectType = 153;
pub(crate) const E_DISCOVER_TAXI: SpellEffectType = 154;
pub(crate) const E_TITAN_GRIP: SpellEffectType = 155;
pub(crate) const E_ENCHANT_ITEM_PRISMATIC: SpellEffectType = 156;
pub(crate) const E_CREATE_LOOT: SpellEffectType = 157;
pub(crate) const E_MILLING: SpellEffectType = 158;
pub(crate) const E_ALLOW_RENAME_PET: SpellEffectType = 159;
pub(crate) const E_FORCE_CAST_2: SpellEffectType = 160;
pub(crate) const E_TALENT_SPEC_COUNT: SpellEffectType = 161;
pub(crate) const E_TALENT_SPEC_SELECT: SpellEffectType = 162;
pub(crate) const E_OBLITERATE_ITEM: SpellEffectType = 163;
pub(crate) const E_REMOVE_AURA: SpellEffectType = 164;
pub(crate) const E_DAMAGE_FROM_MAX_HEALTH_PCT: SpellEffectType = 165;
pub(crate) const E_GIVE_CURRENCY: SpellEffectType = 166;
pub(crate) const E_UPDATE_PLAYER_PHASE: SpellEffectType = 167;
pub(crate) const E_ALLOW_CONTROL_PET: SpellEffectType = 168;
pub(crate) const E_DESTROY_ITEM: SpellEffectType = 169;
pub(crate) const E_UPDATE_ZONE_AURAS_AND_PHASES: SpellEffectType = 170;
pub(crate) const E_SUMMON_PERSONAL_GAMEOBJECT: SpellEffectType = 171;
pub(crate) const E_RESURRECT_WITH_AURA: SpellEffectType = 172;
pub(crate) const E_UNLOCK_GUILD_VAULT_TAB: SpellEffectType = 173;
pub(crate) const E_APPLY_AURA_ON_PET: SpellEffectType = 174;
pub(crate) const E_SANCTUARY_2: SpellEffectType = 176;
pub(crate) const E_DESPAWN_PERSISTENT_AREA_AURA: SpellEffectType = 177;
pub(crate) const E_CREATE_AREATRIGGER: SpellEffectType = 179;
pub(crate) const E_UPDATE_AREATRIGGER: SpellEffectType = 180;
pub(crate) const E_REMOVE_TALENT: SpellEffectType = 181;
pub(crate) const E_DESPAWN_AREATRIGGER: SpellEffectType = 182;
pub(crate) const E_REPUTATION_2: SpellEffectType = 184;
pub(crate) const E_RANDOMIZE_ARCHAEOLOGY_DIGSITES: SpellEffectType = 187;
pub(crate) const E_SUMMON_STABLED_PET_AS_GUARDIAN: SpellEffectType = 188;
pub(crate) const E_LOOT: SpellEffectType = 189;
pub(crate) const E_CHANGE_PARTY_MEMBERS: SpellEffectType = 190;
pub(crate) const E_TELEPORT_TO_DIGSITE: SpellEffectType = 191;
pub(crate) const E_UNCAGE_BATTLEPET: SpellEffectType = 192;
pub(crate) const E_START_PET_BATTLE: SpellEffectType = 193;
pub(crate) const E_PLAY_SCENE_SCRIPT_PACKAGE: SpellEffectType = 195;
pub(crate) const E_CREATE_SCENE_OBJECT: SpellEffectType = 196;
pub(crate) const E_CREATE_PERSONAL_SCENE_OBJECT: SpellEffectType = 197;
pub(crate) const E_PLAY_SCENE: SpellEffectType = 198;
pub(crate) const E_DESPAWN_SUMMON: SpellEffectType = 199;
pub(crate) const E_HEAL_BATTLEPET_PCT: SpellEffectType = 200;
pub(crate) const E_ENABLE_BATTLE_PETS: SpellEffectType = 201;
/// originally "APPLY_AURA_ON_?"
pub(crate) const E_APPLY_AURA_ON_UNKNOWN: SpellEffectType = 202;
pub(crate) const E_REMOVE_AURA_2: SpellEffectType = 203;
pub(crate) const E_CHANGE_BATTLEPET_QUALITY: SpellEffectType = 204;
pub(crate) const E_LAUNCH_QUEST_CHOICE: SpellEffectType = 205;
pub(crate) const E_ALTER_ITEM: SpellEffectType = 206;
pub(crate) const E_LAUNCH_QUEST_TASK: SpellEffectType = 207;
pub(crate) const E_SET_REPUTATION: SpellEffectType = 208;
pub(crate) const E_LEARN_GARRISON_BUILDING: SpellEffectType = 210;
pub(crate) const E_LEARN_GARRISON_SPECIALIZATION: SpellEffectType = 211;
pub(crate) const E_REMOVE_AURA_BY_SPELL_LABEL: SpellEffectType = 212;
pub(crate) const E_JUMP_TO_DESTINATION_2: SpellEffectType = 213;
pub(crate) const E_CREATE_GARRISON: SpellEffectType = 214;
pub(crate) const E_UPGRADE_CHARACTER_SPELLS: SpellEffectType = 215;
pub(crate) const E_CREATE_SHIPMENT: SpellEffectType = 216;
pub(crate) const E_UPGRADE_GARRISON: SpellEffectType = 217;
pub(crate) const E_CREATE_CONVERSATION: SpellEffectType = 219;
pub(crate) const E_ADD_GARRISON_FOLLOWER: SpellEffectType = 220;
pub(crate) const E_ADD_GARRISON_MISSION: SpellEffectType = 221;
pub(crate) const E_CREATE_HEIRLOOM_ITEM: SpellEffectType = 222;
pub(crate) const E_CHANGE_ITEM_BONUSES: SpellEffectType = 223;
pub(crate) const E_ACTIVATE_GARRISON_BUILDING: SpellEffectType = 224;
pub(crate) const E_GRANT_BATTLEPET_LEVEL: SpellEffectType = 225;
pub(crate) const E_TRIGGER_ACTION_SET: SpellEffectType = 226;
pub(crate) const E_TELEPORT_TO_LFG_DUNGEON: SpellEffectType = 227;
pub(crate) const E_SET_FOLLOWER_QUALITY: SpellEffectType = 229;
pub(crate) const E_INCREASE_FOLLOWER_ITEM_LEVEL: SpellEffectType = 230;
pub(crate) const E_INCREASE_FOLLOWER_EXPERIENCE: SpellEffectType = 231;
pub(crate) const E_REMOVE_PHASE: SpellEffectType = 232;
pub(crate) const E_RANDOMIZE_FOLLOWER_ABILITIES: SpellEffectType = 233;
pub(crate) const E_GIVE_EXPERIENCE: SpellEffectType = 236;
pub(crate) const E_GIVE_RESTED_EXPERIENCE_BONUS: SpellEffectType = 237;
pub(crate) const E_INCREASE_SKILL: SpellEffectType = 238;
pub(crate) const E_END_GARRISON_BUILDING_CONSTRUCTION: SpellEffectType = 239;
pub(crate) const E_GIVE_ARTIFACT_POWER: SpellEffectType = 240;
pub(crate) const E_GIVE_ARTIFACT_POWER_NO_BONUS: SpellEffectType = 242;
pub(crate) const E_APPLY_ENCHANT_ILLUSION: SpellEffectType = 243;
pub(crate) const E_LEARN_FOLLOWER_ABILITY: SpellEffectType = 244;
pub(crate) const E_UPGRADE_HEIRLOOM: SpellEffectType = 245;
pub(crate) const E_FINISH_GARRISON_MISSION: SpellEffectType = 246;
pub(crate) const E_ADD_GARRISON_MISSION_SET: SpellEffectType = 247;
pub(crate) const E_FINISH_SHIPMENT: SpellEffectType = 248;
pub(crate) const E_FORCE_EQUIP_ITEM: SpellEffectType = 249;
pub(crate) const E_TAKE_SCREENSHOT: SpellEffectType = 250;
pub(crate) const E_SET_GARRISON_CACHE_SIZE: SpellEffectType = 251;
pub(crate) const E_TELEPORT_UNITS: SpellEffectType = 252;
pub(crate) const E_GIVE_HONOR: SpellEffectType = 253;
pub(crate) const E_JUMP_CHARGE: SpellEffectType = 254;
pub(crate) const E_LEARN_TRANSMOG_SET: SpellEffectType = 255;
pub(crate) const E_MODIFY_KEYSTONE: SpellEffectType = 258;
pub(crate) const E_RESPEC_AZERITE_EMPOWERED_ITEM: SpellEffectType = 259;
pub(crate) const E_SUMMON_STABLED_PET: SpellEffectType = 260;
pub(crate) const E_SCRAP_ITEM: SpellEffectType = 261;
pub(crate) const E_REPAIR_ITEM: SpellEffectType = 263;
pub(crate) const E_REMOVE_GEM: SpellEffectType = 264;
pub(crate) const E_LEARN_AZERITE_ESSENCE_POWER: SpellEffectType = 265;
pub(crate) const E_SET_ITEM_BONUS_LIST_GROUP_ENTRY: SpellEffectType = 266;
pub(crate) const E_CREATE_PRIVATE_CONVERSATION: SpellEffectType = 267;
pub(crate) const E_APPLY_MOUNT_EQUIPMENT: SpellEffectType = 268;
pub(crate) const E_INCREASE_ITEM_BONUS_LIST_GROUP_STEP: SpellEffectType = 269;
pub(crate) const E_APPLY_AREA_AURA_PARTY_NONRANDOM: SpellEffectType = 271;
pub(crate) const E_SET_COVENANT: SpellEffectType = 272;
pub(crate) const E_CRAFT_RUNEFORGE_LEGENDARY: SpellEffectType = 273;
pub(crate) const E_LEARN_TRANSMOG_ILLUSION: SpellEffectType = 276;
pub(crate) const E_SET_CHROMIE_TIME: SpellEffectType = 277;
pub(crate) const E_LEARN_GARR_TALENT: SpellEffectType = 279;
pub(crate) const E_LEARN_SOULBIND_CONDUIT: SpellEffectType = 281;
pub(crate) const E_CONVERT_ITEMS_TO_CURRENCY: SpellEffectType = 282;
pub(crate) const E_COMPLETE_CAMPAIGN: SpellEffectType = 283;
pub(crate) const E_SEND_CHAT_MESSAGE: SpellEffectType = 284;
pub(crate) const E_MODIFY_KEYSTONE_2: SpellEffectType = 285;
pub(crate) const E_GRANT_BATTLEPET_EXPERIENCE: SpellEffectType = 286;
pub(crate) const E_SET_GARRISON_FOLLOWER_LEVEL: SpellEffectType = 287;
pub(crate) const E_CRAFT_ITEM: SpellEffectType = 288;
pub(crate) const E_MODIFY_AURA_STACKS: SpellEffectType = 289;
pub(crate) const E_MODIFY_COOLDOWN: SpellEffectType = 290;
pub(crate) const E_MODIFY_COOLDOWNS: SpellEffectType = 291;
pub(crate) const E_MODIFY_COOLDOWNS_BY_CATEGORY: SpellEffectType = 292;
pub(crate) const E_MODIFY_CHARGES: SpellEffectType = 293;
pub(crate) const E_CRAFT_LOOT: SpellEffectType = 294;
pub(crate) const E_SALVAGE_ITEM: SpellEffectType = 295;
pub(crate) const E_CRAFT_SALVAGE_ITEM: SpellEffectType = 296;
pub(crate) const E_RECRAFT_ITEM: SpellEffectType = 297;
pub(crate) const E_CANCEL_ALL_PRIVATE_CONVERSATIONS: SpellEffectType = 298;
pub(crate) const E_CRAFT_ENCHANT: SpellEffectType = 301;
pub(crate) const E_GATHERING: SpellEffectType = 302;
pub(crate) const E_CREATE_TRAIT_TREE_CONFIG: SpellEffectType = 303;
pub(crate) const E_CHANGE_ACTIVE_COMBAT_TRAIT_CONFIG: SpellEffectType = 304;
pub(crate) const E_UPDATE_INTERACTIONS: SpellEffectType = 306;
pub(crate) const E_CANCEL_PRELOAD_WORLD: SpellEffectType = 308;
pub(crate) const E_PRELOAD_WORLD: SpellEffectType = 309;
pub(crate) const E_310: SpellEffectType = 310;
pub(crate) const E_ENSURE_WORLD_LOADED: SpellEffectType = 311;
pub(crate) const E_312: SpellEffectType = 312;
pub(crate) const E_CHANGE_ITEM_BONUSES_2: SpellEffectType = 313;
pub(crate) const E_ADD_SOCKET_BONUS: SpellEffectType = 314;
pub(crate) const E_LEARN_TRANSMOG_APPEARANCE_FROM_ITEM_MOD_APPEARANCE_GROUP: SpellEffectType = 315;
pub(crate) const E_KILL_CREDIT_LABEL_1: SpellEffectType = 316;
pub(crate) const E_KILL_CREDIT_LABEL_2: SpellEffectType = 317;
pub(crate) const E_328: SpellEffectType = 328;
pub(crate) const E_329: SpellEffectType = 329;
pub(crate) const E_SET_PLAYER_DATA_ELEMENT_ACCOUNT: SpellEffectType = 335;
pub(crate) const E_SET_PLAYER_DATA_ELEMENT_CHARACTER: SpellEffectType = 336;
pub(crate) const E_SET_PLAYER_DATA_FLAG_ACCOUNT: SpellEffectType = 337;
pub(crate) const E_SET_PLAYER_DATA_FLAG_CHARACTER: SpellEffectType = 338;
pub(crate) const E_UI_ACTION: SpellEffectType = 339;
pub(crate) const E_LEARN_WARBAND_SCENE: SpellEffectType = 341;
pub(crate) const E_ASSIST_ACTION: SpellEffectType = 345;
pub(crate) const E_EQUIP_TRANSMOG_OUTFIT: SpellEffectType = 347;
pub(crate) const E_GIVE_HOUSE_LEVEL: SpellEffectType = 348;
pub(crate) const E_LEARN_HOUSING_INTERIOR: SpellEffectType = 349;
pub(crate) const E_LEARN_HOUSING_EXTERIOR: SpellEffectType = 350;
pub(crate) const E_LEARN_HOUSE_THEME: SpellEffectType = 351;
pub(crate) const E_LEARN_HOUSING_COMPONENT_TEXTURE: SpellEffectType = 352;
pub(crate) const E_CREATE_AREA_TRIGGER_2: SpellEffectType = 353;
pub(crate) const E_SET_NEIGHBORHOOD_INITIATIVE: SpellEffectType = 354;
pub(crate) const E_APPLY_ITEM_BONUS: SpellEffectType = 357;
pub(crate) const E_REMOVE_ITEM_BONUS: SpellEffectType = 358;
pub(crate) const E_359: SpellEffectType = 359;
pub(crate) const E_360: SpellEffectType = 360;

// EffectAuraType, Go dbcenums/auras.go
/// Enum constants defined using the A_ naming convention. Every id is listed: one with no known name
/// is A_<id>, and one that is neither named nor read is commented out.
pub(crate) const A_NONE: EffectAuraType = 0;
pub(crate) const A_BIND_SIGHT: EffectAuraType = 1;
pub(crate) const A_MOD_POSSESS: EffectAuraType = 2;
pub(crate) const A_PERIODIC_DAMAGE: EffectAuraType = 3;
pub(crate) const A_DUMMY: EffectAuraType = 4;
pub(crate) const A_MOD_CONFUSE: EffectAuraType = 5;
pub(crate) const A_MOD_CHARM: EffectAuraType = 6;
pub(crate) const A_MOD_FEAR: EffectAuraType = 7;
pub(crate) const A_PERIODIC_HEAL: EffectAuraType = 8;
pub(crate) const A_MOD_ATTACKSPEED: EffectAuraType = 9;
pub(crate) const A_MOD_THREAT: EffectAuraType = 10;
pub(crate) const A_MOD_TAUNT: EffectAuraType = 11;
pub(crate) const A_MOD_STUN: EffectAuraType = 12;
pub(crate) const A_MOD_DAMAGE_DONE: EffectAuraType = 13;
pub(crate) const A_MOD_DAMAGE_TAKEN: EffectAuraType = 14;
pub(crate) const A_DAMAGE_SHIELD: EffectAuraType = 15;
pub(crate) const A_MOD_STEALTH: EffectAuraType = 16;
pub(crate) const A_MOD_STEALTH_DETECT: EffectAuraType = 17;
pub(crate) const A_MOD_INVISIBILITY: EffectAuraType = 18;
pub(crate) const A_MOD_INVISIBILITY_DETECT: EffectAuraType = 19;
pub(crate) const A_OBS_MOD_HEALTH: EffectAuraType = 20;
pub(crate) const A_OBS_MOD_POWER: EffectAuraType = 21;
pub(crate) const A_MOD_RESISTANCE: EffectAuraType = 22;
pub(crate) const A_PERIODIC_TRIGGER_SPELL: EffectAuraType = 23;
pub(crate) const A_PERIODIC_ENERGIZE: EffectAuraType = 24;
pub(crate) const A_MOD_PACIFY: EffectAuraType = 25;
pub(crate) const A_MOD_ROOT: EffectAuraType = 26;
pub(crate) const A_MOD_SILENCE: EffectAuraType = 27;
pub(crate) const A_REFLECT_SPELLS: EffectAuraType = 28;
pub(crate) const A_MOD_STAT: EffectAuraType = 29;
pub(crate) const A_MOD_SKILL: EffectAuraType = 30;
pub(crate) const A_MOD_INCREASE_SPEED: EffectAuraType = 31;
pub(crate) const A_MOD_INCREASE_MOUNTED_SPEED: EffectAuraType = 32;
pub(crate) const A_MOD_DECREASE_SPEED: EffectAuraType = 33;
pub(crate) const A_MOD_INCREASE_HEALTH: EffectAuraType = 34;
pub(crate) const A_MOD_INCREASE_ENERGY: EffectAuraType = 35;
pub(crate) const A_MOD_SHAPESHIFT: EffectAuraType = 36;
pub(crate) const A_EFFECT_IMMUNITY: EffectAuraType = 37;
pub(crate) const A_STATE_IMMUNITY: EffectAuraType = 38;
pub(crate) const A_SCHOOL_IMMUNITY: EffectAuraType = 39;
pub(crate) const A_DAMAGE_IMMUNITY: EffectAuraType = 40;
pub(crate) const A_DISPEL_IMMUNITY: EffectAuraType = 41;
pub(crate) const A_PROC_TRIGGER_SPELL: EffectAuraType = 42;
pub(crate) const A_PROC_TRIGGER_DAMAGE: EffectAuraType = 43;
pub(crate) const A_TRACK_CREATURES: EffectAuraType = 44;
pub(crate) const A_TRACK_RESOURCES: EffectAuraType = 45;
pub(crate) const A_MOD_PARRY_PERCENT: EffectAuraType = 47;
pub(crate) const A_PERIODIC_TRIGGER_SPELL_FROM_CLIENT: EffectAuraType = 48;
pub(crate) const A_MOD_DODGE_PERCENT: EffectAuraType = 49;
pub(crate) const A_MOD_CRITICAL_HEALING_AMOUNT: EffectAuraType = 50;
pub(crate) const A_MOD_BLOCK_PERCENT: EffectAuraType = 51;
pub(crate) const A_MOD_WEAPON_CRIT_PERCENT: EffectAuraType = 52;
pub(crate) const A_PERIODIC_LEECH: EffectAuraType = 53;
pub(crate) const A_MOD_HIT_CHANCE: EffectAuraType = 54;
pub(crate) const A_MOD_SPELL_HIT_CHANCE: EffectAuraType = 55;
pub(crate) const A_TRANSFORM: EffectAuraType = 56;
pub(crate) const A_MOD_SPELL_CRIT_CHANCE: EffectAuraType = 57;
pub(crate) const A_MOD_INCREASE_SWIM_SPEED: EffectAuraType = 58;
pub(crate) const A_MOD_DAMAGE_DONE_CREATURE: EffectAuraType = 59;
pub(crate) const A_MOD_PACIFY_SILENCE: EffectAuraType = 60;
pub(crate) const A_MOD_SCALE: EffectAuraType = 61;
pub(crate) const A_PERIODIC_HEALTH_FUNNEL: EffectAuraType = 62;
pub(crate) const A_MOD_ADDITIONAL_POWER_COST: EffectAuraType = 63;
pub(crate) const A_PERIODIC_MANA_LEECH: EffectAuraType = 64;
pub(crate) const A_MOD_CASTING_SPEED_NOT_STACK: EffectAuraType = 65;
pub(crate) const A_FEIGN_DEATH: EffectAuraType = 66;
pub(crate) const A_MOD_DISARM: EffectAuraType = 67;
pub(crate) const A_MOD_STALKED: EffectAuraType = 68;
pub(crate) const A_SCHOOL_ABSORB: EffectAuraType = 69;
pub(crate) const A_PERIODIC_WEAPON_PERCENT_DAMAGE: EffectAuraType = 70;
pub(crate) const A_STORE_TELEPORT_RETURN_POINT: EffectAuraType = 71;
pub(crate) const A_MOD_POWER_COST_SCHOOL_PCT: EffectAuraType = 72;
pub(crate) const A_MOD_POWER_COST_SCHOOL: EffectAuraType = 73;
pub(crate) const A_REFLECT_SPELLS_SCHOOL: EffectAuraType = 74;
pub(crate) const A_MOD_LANGUAGE: EffectAuraType = 75;
pub(crate) const A_FAR_SIGHT: EffectAuraType = 76;
pub(crate) const A_MECHANIC_IMMUNITY: EffectAuraType = 77;
pub(crate) const A_MOUNTED: EffectAuraType = 78;
pub(crate) const A_MOD_DAMAGE_PERCENT_DONE: EffectAuraType = 79;
pub(crate) const A_MOD_PERCENT_STAT: EffectAuraType = 80;
pub(crate) const A_SPLIT_DAMAGE_PCT: EffectAuraType = 81;
pub(crate) const A_WATER_BREATHING: EffectAuraType = 82;
pub(crate) const A_MOD_BASE_RESISTANCE: EffectAuraType = 83;
pub(crate) const A_MOD_REGEN: EffectAuraType = 84;
pub(crate) const A_MOD_POWER_REGEN: EffectAuraType = 85;
pub(crate) const A_CHANNEL_DEATH_ITEM: EffectAuraType = 86;
pub(crate) const A_MOD_DAMAGE_PERCENT_TAKEN: EffectAuraType = 87;
pub(crate) const A_MOD_HEALTH_REGEN_PERCENT: EffectAuraType = 88;
pub(crate) const A_PERIODIC_DAMAGE_PERCENT: EffectAuraType = 89;
pub(crate) const A_MOD_DETECT_RANGE: EffectAuraType = 91;
pub(crate) const A_PREVENTS_FLEEING: EffectAuraType = 92;
pub(crate) const A_MOD_UNATTACKABLE: EffectAuraType = 93;
pub(crate) const A_INTERRUPT_REGEN: EffectAuraType = 94;
pub(crate) const A_GHOST: EffectAuraType = 95;
pub(crate) const A_SPELL_MAGNET: EffectAuraType = 96;
pub(crate) const A_MANA_SHIELD: EffectAuraType = 97;
pub(crate) const A_MOD_SKILL_TALENT: EffectAuraType = 98;
pub(crate) const A_MOD_ATTACK_POWER: EffectAuraType = 99;
pub(crate) const A_AURAS_VISIBLE: EffectAuraType = 100;
pub(crate) const A_MOD_RESISTANCE_PCT: EffectAuraType = 101;
pub(crate) const A_MOD_MELEE_ATTACK_POWER_VERSUS: EffectAuraType = 102;
pub(crate) const A_MOD_TOTAL_THREAT: EffectAuraType = 103;
pub(crate) const A_WATER_WALK: EffectAuraType = 104;
pub(crate) const A_FEATHER_FALL: EffectAuraType = 105;
pub(crate) const A_HOVER: EffectAuraType = 106;
pub(crate) const A_ADD_FLAT_MODIFIER: EffectAuraType = 107;
pub(crate) const A_ADD_PCT_MODIFIER: EffectAuraType = 108;
pub(crate) const A_ADD_TARGET_TRIGGER: EffectAuraType = 109;
pub(crate) const A_MOD_POWER_REGEN_PERCENT: EffectAuraType = 110;
pub(crate) const A_INTERCEPT_MELEE_RANGED_ATTACKS: EffectAuraType = 111;
pub(crate) const A_OVERRIDE_CLASS_SCRIPTS: EffectAuraType = 112;
pub(crate) const A_MOD_RANGED_DAMAGE_TAKEN: EffectAuraType = 113;
pub(crate) const A_MOD_RANGED_DAMAGE_TAKEN_PCT: EffectAuraType = 114;
pub(crate) const A_MOD_HEALING: EffectAuraType = 115;
pub(crate) const A_MOD_REGEN_DURING_COMBAT: EffectAuraType = 116;
pub(crate) const A_MOD_MECHANIC_RESISTANCE: EffectAuraType = 117;
pub(crate) const A_MOD_HEALING_PCT: EffectAuraType = 118;
pub(crate) const A_PVP_TALENTS: EffectAuraType = 119;
pub(crate) const A_UNTRACKABLE: EffectAuraType = 120;
pub(crate) const A_EMPATHY: EffectAuraType = 121;
pub(crate) const A_MOD_OFFHAND_DAMAGE_PCT: EffectAuraType = 122;
pub(crate) const A_MOD_TARGET_RESISTANCE: EffectAuraType = 123;
pub(crate) const A_MOD_RANGED_ATTACK_POWER: EffectAuraType = 124;
pub(crate) const A_MOD_MELEE_DAMAGE_TAKEN: EffectAuraType = 125;
pub(crate) const A_MOD_MELEE_DAMAGE_TAKEN_PCT: EffectAuraType = 126;
pub(crate) const A_RANGED_ATTACK_POWER_ATTACKER_BONUS: EffectAuraType = 127;
pub(crate) const A_MOD_FIXATE: EffectAuraType = 128;
pub(crate) const A_MOD_SPEED_ALWAYS: EffectAuraType = 129;
pub(crate) const A_MOD_MOUNTED_SPEED_ALWAYS: EffectAuraType = 130;
pub(crate) const A_MOD_RANGED_ATTACK_POWER_VERSUS: EffectAuraType = 131;
pub(crate) const A_MOD_INCREASE_ENERGY_PERCENT: EffectAuraType = 132;
pub(crate) const A_MOD_INCREASE_HEALTH_PERCENT: EffectAuraType = 133;
pub(crate) const A_MOD_MANA_REGEN_INTERRUPT: EffectAuraType = 134;
pub(crate) const A_MOD_HEALING_DONE: EffectAuraType = 135;
pub(crate) const A_MOD_HEALING_DONE_PERCENT: EffectAuraType = 136;
pub(crate) const A_MOD_TOTAL_STAT_PERCENTAGE: EffectAuraType = 137;
pub(crate) const A_MOD_MELEE_HASTE: EffectAuraType = 138;
pub(crate) const A_FORCE_REACTION: EffectAuraType = 139;
pub(crate) const A_MOD_RANGED_HASTE: EffectAuraType = 140;
pub(crate) const A_MOD_RANGED_AMMO_HASTE: EffectAuraType = 141;
pub(crate) const A_MOD_BASE_RESISTANCE_PCT: EffectAuraType = 142;
pub(crate) const A_MOD_RECOVERY_RATE_BY_SPELL_LABEL: EffectAuraType = 143;
pub(crate) const A_SAFE_FALL: EffectAuraType = 144;
pub(crate) const A_MOD_INCREASE_HEALTH_PERCENT2: EffectAuraType = 145;
pub(crate) const A_ALLOW_TAME_PET_TYPE: EffectAuraType = 146;
pub(crate) const A_MECHANIC_IMMUNITY_MASK: EffectAuraType = 147;
pub(crate) const A_MOD_CHARGE_RECOVERY_RATE: EffectAuraType = 148;
pub(crate) const A_REDUCE_PUSHBACK: EffectAuraType = 149;
pub(crate) const A_MOD_SHIELD_BLOCKVALUE_PCT: EffectAuraType = 150;
pub(crate) const A_TRACK_STEALTHED: EffectAuraType = 151;
pub(crate) const A_MOD_DETECTED_RANGE: EffectAuraType = 152;
pub(crate) const A_MOD_AUTOATTACK_RANGE: EffectAuraType = 153;
pub(crate) const A_MOD_STEALTH_LEVEL: EffectAuraType = 154;
pub(crate) const A_MOD_WATER_BREATHING: EffectAuraType = 155;
pub(crate) const A_MOD_REPUTATION_GAIN: EffectAuraType = 156;
pub(crate) const A_PET_DAMAGE_MULTI: EffectAuraType = 157;
pub(crate) const A_ALLOW_TALENT_SWAPPING: EffectAuraType = 158;
pub(crate) const A_NO_PVP_CREDIT: EffectAuraType = 159;
pub(crate) const A_MOD_AOE_AVOIDANCE: EffectAuraType = 160;
pub(crate) const A_MOD_HEALTH_REGEN_IN_COMBAT: EffectAuraType = 161;
pub(crate) const A_POWER_BURN: EffectAuraType = 162;
pub(crate) const A_MOD_CRIT_DAMAGE_BONUS: EffectAuraType = 163;
pub(crate) const A_FORCE_BREATH_BAR: EffectAuraType = 164;
pub(crate) const A_MELEE_ATTACK_POWER_ATTACKER_BONUS: EffectAuraType = 165;
pub(crate) const A_MOD_ATTACK_POWER_PCT: EffectAuraType = 166;
pub(crate) const A_MOD_RANGED_ATTACK_POWER_PCT: EffectAuraType = 167;
pub(crate) const A_MOD_DAMAGE_DONE_VERSUS: EffectAuraType = 168;
pub(crate) const A_SET_FFA_PVP: EffectAuraType = 169;
pub(crate) const A_DETECT_AMORE: EffectAuraType = 170;
pub(crate) const A_MOD_SPEED_NOT_STACK: EffectAuraType = 171;
pub(crate) const A_MOD_MOUNTED_SPEED_NOT_STACK: EffectAuraType = 172;
pub(crate) const A_MOD_RECHARGE_TIME_PCT_CATEGORY_MASK: EffectAuraType = 173;
pub(crate) const A_MOD_SPELL_DAMAGE_OF_STAT_PERCENT: EffectAuraType = 174;
pub(crate) const A_MOD_SPELL_HEALING_OF_STAT_PERCENT: EffectAuraType = 175;
pub(crate) const A_SPIRIT_OF_REDEMPTION: EffectAuraType = 176;
pub(crate) const A_AOE_CHARM: EffectAuraType = 177;
pub(crate) const A_MOD_MAX_POWER_PCT: EffectAuraType = 178;
pub(crate) const A_MOD_POWER_DISPLAY: EffectAuraType = 179;
pub(crate) const A_MOD_FLAT_SPELL_DAMAGE_VERSUS: EffectAuraType = 180;
pub(crate) const A_MOD_SPELL_CURRENCY_REAGENTS_COUNT_PCT: EffectAuraType = 181;
pub(crate) const A_SUPPRESS_ITEM_PASSIVE_EFFECT_BY_SPELL_LABEL: EffectAuraType = 182;
pub(crate) const A_MOD_CRIT_CHANCE_VERSUS_TARGET_HEALTH: EffectAuraType = 183;
pub(crate) const A_MOD_ATTACKER_MELEE_HIT_CHANCE: EffectAuraType = 184;
pub(crate) const A_MOD_ATTACKER_RANGED_HIT_CHANCE: EffectAuraType = 185;
pub(crate) const A_MOD_ATTACKER_SPELL_HIT_CHANCE: EffectAuraType = 186;
pub(crate) const A_MOD_ATTACKER_MELEE_CRIT_CHANCE: EffectAuraType = 187;
pub(crate) const A_MOD_UI_HEALING_RANGE: EffectAuraType = 188;
pub(crate) const A_MOD_RATING: EffectAuraType = 189;
pub(crate) const A_MOD_FACTION_REPUTATION_GAIN: EffectAuraType = 190;
pub(crate) const A_USE_NORMAL_MOVEMENT_SPEED: EffectAuraType = 191;
pub(crate) const A_MOD_MELEE_RANGED_HASTE: EffectAuraType = 192;
pub(crate) const A_MELEE_SLOW: EffectAuraType = 193;
pub(crate) const A_MOD_TARGET_ABSORB_SCHOOL: EffectAuraType = 194;
pub(crate) const A_LEARN_SPELL: EffectAuraType = 195;
pub(crate) const A_MOD_COOLDOWN: EffectAuraType = 196;
pub(crate) const A_MOD_ATTACKER_SPELL_AND_WEAPON_CRIT_CHANCE: EffectAuraType = 197;
pub(crate) const A_MOD_COMBAT_RATING_FROM_COMBAT_RATING: EffectAuraType = 198;
pub(crate) const A_MOD_INCREASES_SPELL_PCT_TO_HIT: EffectAuraType = 199;
pub(crate) const A_MOD_XP_PCT: EffectAuraType = 200;
pub(crate) const A_FLY: EffectAuraType = 201;
pub(crate) const A_IGNORE_COMBAT_RESULT: EffectAuraType = 202;
pub(crate) const A_PREVENT_INTERRUPT: EffectAuraType = 203;
pub(crate) const A_PREVENT_CORPSE_RELEASE: EffectAuraType = 204;
pub(crate) const A_MOD_CHARGE_COOLDOWN: EffectAuraType = 205;
pub(crate) const A_MOD_INCREASE_VEHICLE_FLIGHT_SPEED: EffectAuraType = 206;
pub(crate) const A_MOD_INCREASE_MOUNTED_FLIGHT_SPEED: EffectAuraType = 207;
pub(crate) const A_MOD_INCREASE_FLIGHT_SPEED: EffectAuraType = 208;
pub(crate) const A_MOD_MOUNTED_FLIGHT_SPEED_ALWAYS: EffectAuraType = 209;
pub(crate) const A_MOD_VEHICLE_SPEED_ALWAYS: EffectAuraType = 210;
pub(crate) const A_MOD_FLIGHT_SPEED_NOT_STACK: EffectAuraType = 211;
pub(crate) const A_MOD_HONOR_GAIN_PCT: EffectAuraType = 212;
pub(crate) const A_MOD_RAGE_FROM_DAMAGE_DEALT: EffectAuraType = 213;
pub(crate) const A_ARENA_PREPARATION: EffectAuraType = 215;
pub(crate) const A_HASTE_SPELLS: EffectAuraType = 216;
pub(crate) const A_MOD_MELEE_HASTE_2: EffectAuraType = 217;
pub(crate) const A_ADD_PCT_MODIFIER_BY_SPELL_LABEL: EffectAuraType = 218;
pub(crate) const A_ADD_FLAT_MODIFIER_BY_SPELL_LABEL: EffectAuraType = 219;
pub(crate) const A_MOD_ABILITY_SCHOOL_MASK: EffectAuraType = 220;
pub(crate) const A_MOD_DETAUNT: EffectAuraType = 221;
pub(crate) const A_REMOVE_TRANSMOG_COST: EffectAuraType = 222;
pub(crate) const A_REMOVE_BARBER_SHOP_COST: EffectAuraType = 223;
pub(crate) const A_LEARN_TALENT: EffectAuraType = 224;
pub(crate) const A_MOD_VISIBILITY_RANGE: EffectAuraType = 225;
pub(crate) const A_PERIODIC_DUMMY: EffectAuraType = 226;
pub(crate) const A_PERIODIC_TRIGGER_SPELL_WITH_VALUE: EffectAuraType = 227;
pub(crate) const A_DETECT_STEALTH: EffectAuraType = 228;
pub(crate) const A_MOD_AOE_DAMAGE_AVOIDANCE: EffectAuraType = 229;
pub(crate) const A_MOD_MAX_HEALTH: EffectAuraType = 230;
pub(crate) const A_PROC_TRIGGER_SPELL_WITH_VALUE: EffectAuraType = 231;
pub(crate) const A_MECHANIC_DURATION_MOD: EffectAuraType = 232;
pub(crate) const A_CHANGE_MODEL_FOR_ALL_HUMANOIDS: EffectAuraType = 233;
pub(crate) const A_MECHANIC_DURATION_MOD_NOT_STACK: EffectAuraType = 234;
pub(crate) const A_MOD_HOVER_NO_HEIGHT_OFFSET: EffectAuraType = 235;
pub(crate) const A_CONTROL_VEHICLE: EffectAuraType = 236;
pub(crate) const A_237: EffectAuraType = 237;
pub(crate) const A_238: EffectAuraType = 238;
pub(crate) const A_MOD_SCALE_2: EffectAuraType = 239;
pub(crate) const A_MOD_EXPERTISE: EffectAuraType = 240;
pub(crate) const A_FORCE_MOVE_FORWARD: EffectAuraType = 241;
pub(crate) const A_MOD_SPELL_DAMAGE_FROM_HEALING: EffectAuraType = 242;
pub(crate) const A_MOD_FACTION: EffectAuraType = 243;
pub(crate) const A_COMPREHEND_LANGUAGE: EffectAuraType = 244;
pub(crate) const A_MOD_AURA_DURATION_BY_DISPEL: EffectAuraType = 245;
pub(crate) const A_MOD_AURA_DURATION_BY_DISPEL_NOT_STACK: EffectAuraType = 246;
pub(crate) const A_CLONE_CASTER: EffectAuraType = 247;
pub(crate) const A_MOD_COMBAT_RESULT_CHANCE: EffectAuraType = 248;
pub(crate) const A_MOD_DAMAGE_PERCENT_DONE_BY_TARGET_AURA_MECHANIC: EffectAuraType = 249;
pub(crate) const A_MOD_INCREASE_HEALTH_2: EffectAuraType = 250;
pub(crate) const A_MOD_ENEMY_DODGE: EffectAuraType = 251;
pub(crate) const A_MOD_SPEED_SLOW_ALL: EffectAuraType = 252;
pub(crate) const A_MOD_BLOCK_CRIT_CHANCE: EffectAuraType = 253;
pub(crate) const A_MOD_DISARM_OFFHAND: EffectAuraType = 254;
pub(crate) const A_MOD_MECHANIC_DAMAGE_TAKEN_PERCENT: EffectAuraType = 255;
pub(crate) const A_NO_REAGENT_USE: EffectAuraType = 256;
pub(crate) const A_MOD_TARGET_RESIST_BY_SPELL_CLASS: EffectAuraType = 257;
pub(crate) const A_OVERRIDE_SUMMONED_OBJECT: EffectAuraType = 258;
pub(crate) const A_MOD_HOT_PCT: EffectAuraType = 259;
pub(crate) const A_SCREEN_EFFECT: EffectAuraType = 260;
pub(crate) const A_PHASE: EffectAuraType = 261;
pub(crate) const A_ABILITY_IGNORE_AURASTATE: EffectAuraType = 262;
pub(crate) const A_DISABLE_CASTING_EXCEPT_ABILITIES: EffectAuraType = 263;
pub(crate) const A_DISABLE_ATTACKING_EXCEPT_ABILITIES: EffectAuraType = 264;
pub(crate) const A_SET_VIGNETTE: EffectAuraType = 266;
pub(crate) const A_MOD_IMMUNE_AURA_APPLY_SCHOOL: EffectAuraType = 267;
pub(crate) const A_MOD_ARMOR_PCT_FROM_STAT: EffectAuraType = 268;
pub(crate) const A_MOD_IGNORE_TARGET_RESIST: EffectAuraType = 269;
pub(crate) const A_MOD_SCHOOL_MASK_DAMAGE_FROM_CASTER: EffectAuraType = 270;
pub(crate) const A_MOD_SPELL_DAMAGE_FROM_CASTER: EffectAuraType = 271;
pub(crate) const A_MOD_BLOCK_VALUE_PCT: EffectAuraType = 272;
pub(crate) const A_X_RAY: EffectAuraType = 273;
pub(crate) const A_MOD_BLOCK_VALUE_FLAT: EffectAuraType = 274;
pub(crate) const A_MOD_IGNORE_SHAPESHIFT: EffectAuraType = 275;
pub(crate) const A_MOD_DAMAGE_DONE_FOR_MECHANIC: EffectAuraType = 276;
pub(crate) const A_MOD_MAX_AFFECTED_TARGETS: EffectAuraType = 277;
pub(crate) const A_MOD_DISARM_RANGED: EffectAuraType = 278;
pub(crate) const A_INITIALIZE_IMAGES: EffectAuraType = 279;
pub(crate) const A_SPELL_AURA_MOD_ARMOR_PENETRATION_PCT: EffectAuraType = 280;
pub(crate) const A_PROVIDE_SPELL_FOCUS: EffectAuraType = 281;
pub(crate) const A_MOD_BASE_HEALTH_PCT: EffectAuraType = 282;
pub(crate) const A_MOD_HEALING_RECEIVED: EffectAuraType = 283;
pub(crate) const A_LINKED: EffectAuraType = 284;
pub(crate) const A_LINKED_2: EffectAuraType = 285;
pub(crate) const A_MOD_RECOVERY_RATE: EffectAuraType = 286;
pub(crate) const A_DEFLECT_SPELLS: EffectAuraType = 287;
pub(crate) const A_IGNORE_HIT_DIRECTION: EffectAuraType = 288;
pub(crate) const A_PREVENT_DURABILITY_LOSS: EffectAuraType = 289;
pub(crate) const A_MOD_CRIT_PCT: EffectAuraType = 290;
pub(crate) const A_MOD_XP_QUEST_PCT: EffectAuraType = 291;
pub(crate) const A_OPEN_STABLE: EffectAuraType = 292;
pub(crate) const A_OVERRIDE_SPELLS: EffectAuraType = 293;
pub(crate) const A_PREVENT_REGENERATE_POWER: EffectAuraType = 294;
pub(crate) const A_MOD_PERIODIC_DAMAGE_TAKEN: EffectAuraType = 295;
pub(crate) const A_SET_VEHICLE_ID: EffectAuraType = 296;
pub(crate) const A_MOD_ROOT_DISABLE_GRAVITY: EffectAuraType = 297;
pub(crate) const A_MOD_STUN_DISABLE_GRAVITY: EffectAuraType = 298;
pub(crate) const A_SHARE_DAMAGE_PCT: EffectAuraType = 300;
pub(crate) const A_SCHOOL_HEAL_ABSORB: EffectAuraType = 301;
pub(crate) const A_MOD_DAMAGE_DONE_VERSUS_AURASTATE: EffectAuraType = 303;
pub(crate) const A_MOD_FAKE_INEBRIATE: EffectAuraType = 304;
pub(crate) const A_MOD_MINIMUM_SPEED: EffectAuraType = 305;
pub(crate) const A_MOD_CRIT_CHANCE_FOR_CASTER: EffectAuraType = 306;
pub(crate) const A_CAST_WHILE_WALKING_BY_SPELL_LABEL: EffectAuraType = 307;
pub(crate) const A_MOD_CRIT_CHANCE_FOR_CASTER_WITH_ABILITIES: EffectAuraType = 308;
pub(crate) const A_MOD_RESILIENCE: EffectAuraType = 309;
pub(crate) const A_MOD_CREATURE_AOE_DAMAGE_AVOIDANCE: EffectAuraType = 310;
pub(crate) const A_IGNORE_COMBAT: EffectAuraType = 311;
pub(crate) const A_ANIM_REPLACEMENT_SET: EffectAuraType = 312;
pub(crate) const A_REPLACE_MOUNT_ANIMATION_SET: EffectAuraType = 313;
pub(crate) const A_PREVENT_RESURRECTION: EffectAuraType = 314;
pub(crate) const A_UNDERWATER_WALKING: EffectAuraType = 315;
pub(crate) const A_SCHOOL_ABSORB_OVERKILL: EffectAuraType = 316;
pub(crate) const A_MOD_SPELL_POWER_PCT: EffectAuraType = 317;
pub(crate) const A_MASTERY: EffectAuraType = 318;
pub(crate) const A_MOD_MELEE_HASTE_3: EffectAuraType = 319;
pub(crate) const A_MOD_RANGED_HASTE_2: EffectAuraType = 320;
pub(crate) const A_MOD_NO_ACTIONS: EffectAuraType = 321;
pub(crate) const A_INTERFERE_TARGETTING: EffectAuraType = 322;
pub(crate) const A_OVERRIDE_UNLOCKED_AZERITE_ESSENCE_RANK: EffectAuraType = 324;
pub(crate) const A_LEARN_PVP_TALENT: EffectAuraType = 325;
pub(crate) const A_PHASE_GROUP: EffectAuraType = 326;
pub(crate) const A_PHASE_ALWAYS_VISIBLE: EffectAuraType = 327;
pub(crate) const A_TRIGGER_SPELL_ON_POWER_PCT: EffectAuraType = 328;
pub(crate) const A_MOD_POWER_GAIN_PCT: EffectAuraType = 329;
pub(crate) const A_CAST_WHILE_WALKING: EffectAuraType = 330;
pub(crate) const A_FORCE_WEATHER: EffectAuraType = 331;
pub(crate) const A_OVERRIDE_ACTIONBAR_SPELLS: EffectAuraType = 332;
pub(crate) const A_OVERRIDE_ACTIONBAR_SPELLS_TRIGGERED: EffectAuraType = 333;
pub(crate) const A_MOD_AUTOATTACK_CRIT_CHANCE: EffectAuraType = 334;
pub(crate) const A_MOUNT_RESTRICTIONS: EffectAuraType = 336;
pub(crate) const A_MOD_VENDOR_ITEMS_PRICES: EffectAuraType = 337;
pub(crate) const A_MOD_DURABILITY_LOSS: EffectAuraType = 338;
pub(crate) const A_MOD_CRIT_CHANCE_FOR_CASTER_PET: EffectAuraType = 339;
pub(crate) const A_MOD_RESURRECTED_HEALTH_BY_GUILD_MEMBER: EffectAuraType = 340;
pub(crate) const A_MOD_SPELL_CATEGORY_COOLDOWN: EffectAuraType = 341;
pub(crate) const A_MOD_MELEE_RANGED_HASTE_2: EffectAuraType = 342;
pub(crate) const A_MOD_MELEE_DAMAGE_FROM_CASTER: EffectAuraType = 343;
pub(crate) const A_MOD_AUTOATTACK_DAMAGE: EffectAuraType = 344;
pub(crate) const A_BYPASS_ARMOR_FOR_CASTER: EffectAuraType = 345;
pub(crate) const A_ENABLE_ALT_POWER: EffectAuraType = 346;
pub(crate) const A_MOD_SPELL_COOLDOWN_BY_HASTE: EffectAuraType = 347;
pub(crate) const A_MOD_MONEY_GAIN: EffectAuraType = 348;
pub(crate) const A_MOD_CURRENCY_GAIN: EffectAuraType = 349;
pub(crate) const A_350: EffectAuraType = 350;
pub(crate) const A_MOD_CURRENCY_GAIN_PCT_CATEGORY: EffectAuraType = 351;
pub(crate) const A_MOD_CAMOUFLAGE: EffectAuraType = 353;
pub(crate) const A_MOD_HEALING_DONE_PCT_VS_TARGET_HEALTH: EffectAuraType = 354;
pub(crate) const A_MOD_CASTING_SPEED: EffectAuraType = 355;
pub(crate) const A_PROVIDE_TOTEM_CATEGORY: EffectAuraType = 356;
pub(crate) const A_ENABLE_BOSS1_UNIT_FRAME: EffectAuraType = 357;
pub(crate) const A_WORGEN_ALTERED_FORM: EffectAuraType = 358;
pub(crate) const A_MOD_HEALING_DONE_VERSUS_AURASTATE: EffectAuraType = 359;
pub(crate) const A_PROC_TRIGGER_SPELL_COPY: EffectAuraType = 360;
pub(crate) const A_OVERRIDE_AUTOATTACK_WITH_MELEE_SPELL: EffectAuraType = 361;
pub(crate) const A_MOD_NEXT_SPELL: EffectAuraType = 363;
pub(crate) const A_MAX_FAR_CLIP_PLANE: EffectAuraType = 365;
pub(crate) const A_OVERRIDE_SPELL_POWER_BY_AP_PCT: EffectAuraType = 366;
pub(crate) const A_OVERRIDE_AUTOATTACK_WITH_RANGED_SPELL: EffectAuraType = 367;
pub(crate) const A_ENABLE_POWER_BAR_TIMER: EffectAuraType = 369;
pub(crate) const A_SPELL_OVERRIDE_NAME_GROUP: EffectAuraType = 370;
pub(crate) const A_OVERRIDE_MOUNT_FROM_SET: EffectAuraType = 372;
pub(crate) const A_MOD_SPEED_NO_CONTROL: EffectAuraType = 373;
pub(crate) const A_MOD_FALL_DAMAGE_PCT: EffectAuraType = 374;
pub(crate) const A_HIDE_MODEL_AND_EQUIPEMENT_SLOTS: EffectAuraType = 375;
pub(crate) const A_MOD_CURRENCY_GAIN_FROM_SOURCE: EffectAuraType = 376;
pub(crate) const A_CAST_WHILE_WALKING_ALL: EffectAuraType = 377;
pub(crate) const A_MOD_POSSESS_PET: EffectAuraType = 378;
pub(crate) const A_MOD_MANA_REGEN_PCT: EffectAuraType = 379;
pub(crate) const A_MOD_DAMAGE_FROM_CASTER_GUARDIAN: EffectAuraType = 380;
pub(crate) const A_MOD_DAMAGE_TAKEN_FROM_CASTER_PET: EffectAuraType = 381;
pub(crate) const A_MOD_PET_STAT_PCT: EffectAuraType = 382;
pub(crate) const A_IGNORE_SPELL_COOLDOWN: EffectAuraType = 383;
pub(crate) const A_MOD_TAXI_FLIGHT_SPEED: EffectAuraType = 388;
pub(crate) const A_BLOCK_SPELLS_IN_FRONT: EffectAuraType = 393;
pub(crate) const A_SHOW_CONFIRMATION_PROMPT: EffectAuraType = 394;
pub(crate) const A_AREA_TRIGGER: EffectAuraType = 395;
pub(crate) const A_TRIGGER_SPELL_ON_POWER_AMOUNT: EffectAuraType = 396;
pub(crate) const A_BATTLEGROUND_PLAYER_POSITION_FACTIONAL: EffectAuraType = 397;
pub(crate) const A_BATTLEGROUND_PLAYER_POSITION: EffectAuraType = 398;
pub(crate) const A_MOD_TIME_RATE: EffectAuraType = 399;
pub(crate) const A_MOD_SKILL_2: EffectAuraType = 400;
pub(crate) const A_ACT_AS_CONTROL_ZONE: EffectAuraType = 401;
pub(crate) const A_MOD_OVERRIDE_POWER_DISPLAY: EffectAuraType = 402;
pub(crate) const A_OVERRIDE_SPELL_VISUAL: EffectAuraType = 403;
pub(crate) const A_OVERRIDE_ATTACK_POWER_BY_SP_PCT: EffectAuraType = 404;
pub(crate) const A_MOD_RATING_PCT: EffectAuraType = 405;
pub(crate) const A_KEYBOUND_OVERRIDE: EffectAuraType = 406;
pub(crate) const A_MOD_FEAR_2: EffectAuraType = 407;
pub(crate) const A_SET_ACTION_BUTTON_SPELL_COUNT: EffectAuraType = 408;
pub(crate) const A_CAN_TURN_WHILE_FALLING: EffectAuraType = 409;
pub(crate) const A_MOD_MAX_CHARGES: EffectAuraType = 411;
pub(crate) const A_MOD_RANGED_ATTACK_DEFLECT_CHANCE: EffectAuraType = 413;
pub(crate) const A_MOD_RANGED_ATTACK_BLOCK_CHANCE_IN_FRONT: EffectAuraType = 414;
pub(crate) const A_MOD_COOLDOWN_BY_HASTE_REGEN: EffectAuraType = 416;
pub(crate) const A_MOD_GLOBAL_COOLDOWN_BY_HASTE_REGEN: EffectAuraType = 417;
pub(crate) const A_MOD_MAX_POWER: EffectAuraType = 418;
pub(crate) const A_MOD_BASE_MANA_PCT: EffectAuraType = 419;
pub(crate) const A_MOD_BATTLE_PET_XP_PCT: EffectAuraType = 420;
pub(crate) const A_MOD_ABSORB_EFFECTS_DONE_PCT: EffectAuraType = 421;
pub(crate) const A_MOD_ABSORB_EFFECTS_TAKEN_PCT: EffectAuraType = 422;
pub(crate) const A_MOD_MANA_COST_PCT: EffectAuraType = 423;
pub(crate) const A_CASTER_IGNORE_LOS: EffectAuraType = 424;
pub(crate) const A_SCALE_PLAYER_LEVEL: EffectAuraType = 427;
pub(crate) const A_LINKED_SUMMON: EffectAuraType = 428;
pub(crate) const A_MOD_SUMMON_DAMAGE: EffectAuraType = 429;
pub(crate) const A_PLAY_SCENE: EffectAuraType = 430;
pub(crate) const A_MOD_OVERRIDE_ZONE_PVP_TYPE: EffectAuraType = 431;
pub(crate) const A_MOD_ENVIRONMENTAL_DAMAGE_TAKEN: EffectAuraType = 436;
pub(crate) const A_MOD_MINIMUM_SPEED_RATE: EffectAuraType = 437;
pub(crate) const A_PRELOAD_PHASE: EffectAuraType = 438;
pub(crate) const A_MOD_MULTISTRIKE_DAMAGE: EffectAuraType = 440;
pub(crate) const A_MOD_MULTISTRIKE_CHANCE: EffectAuraType = 441;
pub(crate) const A_MOD_READINESS: EffectAuraType = 442;
pub(crate) const A_MOD_LEECH: EffectAuraType = 443;
pub(crate) const A_SPELL_AURA_ADVANCED_FLYING: EffectAuraType = 446;
pub(crate) const A_MOD_XP_FROM_CREATURE_TYPE: EffectAuraType = 447;
pub(crate) const A_OVERRIDE_PET_SPECS: EffectAuraType = 451;
pub(crate) const A_CHARGE_RECOVERY_MOD: EffectAuraType = 453;
pub(crate) const A_CHARGE_RECOVERY_MULTIPLIER: EffectAuraType = 454;
pub(crate) const A_MOD_ROOT_2: EffectAuraType = 455;
pub(crate) const A_CHARGE_RECOVERY_AFFECTED_BY_HASTE: EffectAuraType = 456;
pub(crate) const A_CHARGE_RECOVERY_AFFECTED_BY_HASTE_REGEN: EffectAuraType = 457;
pub(crate) const A_IGNORE_DUAL_WIELD_HIT_PENALTY: EffectAuraType = 458;
pub(crate) const A_IGNORE_MOVEMENT_FORCES: EffectAuraType = 459;
pub(crate) const A_RESET_COOLDOWNS_ON_DUEL_START: EffectAuraType = 460;
pub(crate) const A_MOD_HEALING_AND_ABSORB_FROM_CASTER: EffectAuraType = 462;
pub(crate) const A_CONVERT_CRIT_RATING_PCT_TO_PARRY_RATING: EffectAuraType = 463;
pub(crate) const A_MOD_ATTACK_POWER_OF_BONUS_ARMOR: EffectAuraType = 464;
pub(crate) const A_MOD_BONUS_ARMOR: EffectAuraType = 465;
pub(crate) const A_MOD_BONUS_ARMOR_PCT: EffectAuraType = 466;
pub(crate) const A_MOD_STAT_BONUS_PCT: EffectAuraType = 467;
pub(crate) const A_TRIGGER_SPELL_ON_HEALTH_BELOW_PCT: EffectAuraType = 468;
pub(crate) const A_SHOW_CONFIRMATION_PROMPT_WITH_DIFFICULTY: EffectAuraType = 469;
pub(crate) const A_MOD_AURA_TIME_RATE_BY_SPELL_LABEL: EffectAuraType = 470;
pub(crate) const A_MOD_VERSATILITY: EffectAuraType = 471;
pub(crate) const A_PREVENT_DURABILITY_LOSS_FROM_COMBAT: EffectAuraType = 473;
pub(crate) const A_REPLACE_ITEM_BONUS_TREE: EffectAuraType = 474;
pub(crate) const A_ALLOW_USING_GAMEOBJECTS_WHILE_MOUNTED: EffectAuraType = 475;
pub(crate) const A_MOD_CURRENCY_GAIN_LOOTED_PCT: EffectAuraType = 476;
pub(crate) const A_MOD_ARTIFACT_ITEM_LEVEL: EffectAuraType = 480;
pub(crate) const A_CONVERT_CONSUMED_RUNE: EffectAuraType = 481;
pub(crate) const A_SUPPRESS_TRANSFORMS: EffectAuraType = 483;
pub(crate) const A_ALLOW_INTERRUPT_SPELL: EffectAuraType = 484;
pub(crate) const A_MOD_MOVEMENT_FORCE_MAGNITUDE: EffectAuraType = 485;
pub(crate) const A_COSMETIC_MOUNTED: EffectAuraType = 487;
pub(crate) const A_DISABLE_GRAVITY: EffectAuraType = 488;
pub(crate) const A_MOD_ALTERNATIVE_DEFAULT_LANGUAGE: EffectAuraType = 489;
pub(crate) const A_MOD_RESTED_XP_CONSUMPTION: EffectAuraType = 492;
/// duplicate string?
pub(crate) const A_MOD_RESTED_XP_CONSUMPTION_DUP: EffectAuraType = 493;
pub(crate) const A_SET_POWER_POINT_CHARGE: EffectAuraType = 494;
pub(crate) const A_TRIGGER_SPELL_ON_EXPIRE: EffectAuraType = 495;
pub(crate) const A_ALLOW_CHANGING_EQUIPMENT_IN_TORGHAST: EffectAuraType = 496;
pub(crate) const A_MOD_ANIMA_GAIN: EffectAuraType = 497;
pub(crate) const A_CURRENCY_LOSS_PCT_ON_DEATH: EffectAuraType = 498;
/// differentiate duplicate
pub(crate) const A_MOD_RESTED_XP_CONSUMPTION_2: EffectAuraType = 499;
pub(crate) const A_IGNORE_SPELL_CHARGE_COOLDOWN: EffectAuraType = 500;
pub(crate) const A_MOD_CRITICAL_DAMAGE_TAKEN_FROM_CASTER: EffectAuraType = 501;
pub(crate) const A_MOD_VERSATILITY_DAMAGE_DONE_BENEFIT: EffectAuraType = 502;
pub(crate) const A_MOD_VERSATILITY_HEALING_DONE_BENEFIT: EffectAuraType = 503;
pub(crate) const A_MOD_HEALING_TAKEN_FROM_CASTER: EffectAuraType = 504;
pub(crate) const A_MOD_PLAYER_CHOICE_REROLLS: EffectAuraType = 505;
pub(crate) const A_DISABLE_INERTIA: EffectAuraType = 506;
pub(crate) const A_MOD_DAMAGE_TAKEN_FROM_CASTER_BY_LABEL: EffectAuraType = 507;
pub(crate) const A_MODIFIED_RAID_INSTANCE: EffectAuraType = 510;
pub(crate) const A_APPLY_PROFESSION_EFFECT: EffectAuraType = 511;
pub(crate) const A_CONVERT_RUNE: EffectAuraType = 512;
pub(crate) const A_MOD_DRAGONRIDING_AIR_FRICTION: EffectAuraType = 513;
pub(crate) const A_MOD_DRAGONRIDING_MAX_VELOCITY: EffectAuraType = 514;
pub(crate) const A_MOD_DRAGONRIDING_LIFT_COEFFICIENT: EffectAuraType = 515;
pub(crate) const A_MOD_DRAGONRIDING_ADD_IMPULSE_MAX_SPEED: EffectAuraType = 518;
pub(crate) const A_MOD_COOLDOWN_RECOVERY_RATE_ALL: EffectAuraType = 519;
pub(crate) const A_MOD_DRAGONRIDING_BRAKING_RATE: EffectAuraType = 520;
pub(crate) const A_MOD_DRAGONRIDING_PITCHING_RATE_DOWN: EffectAuraType = 521;
pub(crate) const A_MOD_DRAGONRIDING_PITCHING_RATE_UP: EffectAuraType = 522;
pub(crate) const A_MOD_DRAGONRIDING_MAX_DECELERATION: EffectAuraType = 524;
pub(crate) const A_DISPLAY_PROFESSION_EQUIPMENT: EffectAuraType = 525;
pub(crate) const A_ALLOW_BLOCKING_SPELLS: EffectAuraType = 528;
pub(crate) const A_MOD_SPELL_BLOCK_CHANCE: EffectAuraType = 529;
pub(crate) const A_MOD_AUTO_ATTACK_DAMAGE_PCT: EffectAuraType = 530;
pub(crate) const A_MOD_GUARDIAN_DAMAGE_DONE: EffectAuraType = 531;
pub(crate) const A_DISABLE_NAVIGATION: EffectAuraType = 533;
pub(crate) const A_IGNORE_SPELL_CREATURE_TYPE_REQUIREMENTS: EffectAuraType = 536;
pub(crate) const A_MOD_DAMAGE_FROM_CASTER_SPELLS_LABEL: EffectAuraType = 537;
pub(crate) const A_MOD_FAKE_INEBRIATION_MOVEMENT_ONLY: EffectAuraType = 538;
pub(crate) const A_ALLOW_MOUNT_IN_COMBAT: EffectAuraType = 539;
pub(crate) const A_MOD_SUPPORT_STAT: EffectAuraType = 540;
pub(crate) const A_MOD_REQUIRED_MOUNT_CAPABILITY_FLAGS: EffectAuraType = 541;
pub(crate) const A_TRIGGER_SPELL_ON_STACK_AMOUNT: EffectAuraType = 542;
pub(crate) const A_SET_CANT_SWIM: EffectAuraType = 545;
pub(crate) const A_MOD_CRIT_PERCENT_VERSUS: EffectAuraType = 547;
pub(crate) const A_MOD_RUNE_REGEN_SPEED: EffectAuraType = 548;
pub(crate) const A_EXTRA_ATTACKS: EffectAuraType = 551;
pub(crate) const A_MOD_SPELL_CRIT_CHANCE_SCHOOL: EffectAuraType = 552;
pub(crate) const A_MOD_POWER_COST_SCHOOL2: EffectAuraType = 553;
pub(crate) const A_MOD_MELEE_DAMAGE_TAKEN2: EffectAuraType = 556;
pub(crate) const A_MOD_RANGED_HASTE_QUIVER: EffectAuraType = 557;
pub(crate) const A_MOD_RESISTANCE_EXCLUSIVE: EffectAuraType = 558;
pub(crate) const A_MOD_PET_TALENT_POINTS: EffectAuraType = 559;
pub(crate) const A_RETAIN_COMBO_POINTS: EffectAuraType = 560;
pub(crate) const A_MOD_SHIELD_BLOCKVALUE_PCT2: EffectAuraType = 561;
pub(crate) const A_SPLIT_DAMAGE_FLAT: EffectAuraType = 562;
pub(crate) const A_PET_DAMAGE_MULTI2: EffectAuraType = 563;
pub(crate) const A_MOD_SHIELD_BLOCKVALUE: EffectAuraType = 564;
pub(crate) const A_SPELL_AURA_MOD_AOE_AVOIDANCE: EffectAuraType = 565;
pub(crate) const A_MELEE_ATTACK_POWER_ATTACKER_BONUS2: EffectAuraType = 566;
pub(crate) const A_MOD_ATTACKER_SPELL_CRIT_CHANCE: EffectAuraType = 569;
pub(crate) const A_MOD_RESISTANCE_OF_STAT_PERCENT: EffectAuraType = 571;
pub(crate) const A_MOD_CRITICAL_THREAT: EffectAuraType = 572;
pub(crate) const A_MOD_ATTACKER_RANGED_CRIT_CHANCE: EffectAuraType = 573;
pub(crate) const A_MOD_TARGET_ABILITY_ABSORB_SCHOOL: EffectAuraType = 574;
pub(crate) const A_MOD_ATTACKER_MELEE_CRIT_DAMAGE: EffectAuraType = 577;
pub(crate) const A_MOD_ATTACKER_RANGED_CRIT_DAMAGE: EffectAuraType = 578;
pub(crate) const A_MOD_SCHOOL_CRIT_DMG_TAKEN: EffectAuraType = 579;
pub(crate) const A_580: EffectAuraType = 580;
pub(crate) const A_MOD_RATING_FROM_STAT: EffectAuraType = 583;
pub(crate) const A_RAID_PROC_FROM_CHARGE: EffectAuraType = 585;
pub(crate) const A_MOD_DISPEL_RESIST: EffectAuraType = 588;
pub(crate) const A_MOD_SPELL_DAMAGE_OF_ATTACK_POWER: EffectAuraType = 589;
pub(crate) const A_MOD_SPELL_HEALING_OF_ATTACK_POWER: EffectAuraType = 590;
pub(crate) const A_MOD_SCALE_3: EffectAuraType = 591;
pub(crate) const A_MOD_COMBAT_RESULT_CHANCE2: EffectAuraType = 593;
pub(crate) const A_MOD_TARGET_RESIST_BY_SPELL_CLASS2: EffectAuraType = 594;
pub(crate) const A_598: EffectAuraType = 598;
pub(crate) const A_MOD_IGNORE_TARGET_RESIST2: EffectAuraType = 599;
pub(crate) const A_SCHOOL_MASK_DAMAGE_FROM_CASTER: EffectAuraType = 600;
pub(crate) const A_IGNORE_MELEE_RESET: EffectAuraType = 601;
pub(crate) const A_MOD_HONOR_GAIN_PCT2: EffectAuraType = 604;
pub(crate) const A_MOD_BASE_HEALTH_PCT2: EffectAuraType = 606;
pub(crate) const A_MOD_ATTACK_POWER_OF_ARMOR: EffectAuraType = 607;
pub(crate) const A_ABILITY_PERIODIC_CRIT: EffectAuraType = 608;
pub(crate) const A_MOD_RANGED_HASTE_3: EffectAuraType = 615;
pub(crate) const A_MOD_BLIND: EffectAuraType = 619;
pub(crate) const A_MOD_VENDOR_ITEMS_PRICES2: EffectAuraType = 620;
pub(crate) const A_INCREASE_SKILL_GAIN_CHANCE: EffectAuraType = 621;
pub(crate) const A_MOD_GATHERING_ITEMS_GAINED_PERCENT: EffectAuraType = 623;
pub(crate) const A_MOD_DAMAGE_FROM_MANA: EffectAuraType = 624;
pub(crate) const A_635: EffectAuraType = 635;
pub(crate) const A_MOD_EXPLORATION_EXPERIENCE: EffectAuraType = 637;
pub(crate) const A_MOD_CRITICAL_BLOCK_AMOUNT: EffectAuraType = 638;
pub(crate) const A_MOD_DAMAGE_DONE_TO_CASTER_FROM_SCHOOL: EffectAuraType = 639;
pub(crate) const A_MOD_RANGED_ATTACK_SPEED_FLAT: EffectAuraType = 643;
pub(crate) const A_MOD_FLAT_PVP_MULTIPLIER: EffectAuraType = 646;
pub(crate) const A_MOD_PCT_PVP_MULTIPLIER: EffectAuraType = 647;
pub(crate) const A_MOD_FLAT_LABEL_PVP_MULTIPLIER: EffectAuraType = 648;
pub(crate) const A_MOD_PCT_LABEL_PVP_MULTIPLIER: EffectAuraType = 649;

// ImplicitTarget, Go dbcenums/targets.go
pub(crate) const TARGET_NONE: ImplicitTarget = 0;
pub(crate) const TARGET_UNIT_CASTER: ImplicitTarget = 1;
pub(crate) const TARGET_UNIT_NEARBY_ENEMY: ImplicitTarget = 2;
pub(crate) const TARGET_UNIT_NEARBY_ALLY: ImplicitTarget = 3;
pub(crate) const TARGET_UNIT_NEARBY_PARTY: ImplicitTarget = 4;
pub(crate) const TARGET_UNIT_PET: ImplicitTarget = 5;
pub(crate) const TARGET_UNIT_TARGET_ENEMY: ImplicitTarget = 6;
pub(crate) const TARGET_UNIT_SRC_AREA_ENTRY: ImplicitTarget = 7;
pub(crate) const TARGET_UNIT_DEST_AREA_ENTRY: ImplicitTarget = 8;
pub(crate) const TARGET_DEST_HOME: ImplicitTarget = 9;
pub(crate) const TARGET_UNIT_SRC_AREA_UNK_11: ImplicitTarget = 11;
pub(crate) const TARGET_UNIT_SRC_AREA_ENEMY: ImplicitTarget = 15;
pub(crate) const TARGET_UNIT_DEST_AREA_ENEMY: ImplicitTarget = 16;
pub(crate) const TARGET_DEST_DB: ImplicitTarget = 17;
pub(crate) const TARGET_DEST_CASTER: ImplicitTarget = 18;
pub(crate) const TARGET_UNIT_CASTER_AREA_PARTY: ImplicitTarget = 20;
pub(crate) const TARGET_UNIT_TARGET_ALLY: ImplicitTarget = 21;
pub(crate) const TARGET_SRC_CASTER: ImplicitTarget = 22;
pub(crate) const TARGET_GAMEOBJECT_TARGET: ImplicitTarget = 23;
pub(crate) const TARGET_UNIT_CONE_ENEMY_24: ImplicitTarget = 24;
pub(crate) const TARGET_UNIT_TARGET_ANY: ImplicitTarget = 25;
pub(crate) const TARGET_GAMEOBJECT_ITEM_TARGET: ImplicitTarget = 26;
pub(crate) const TARGET_UNIT_MASTER: ImplicitTarget = 27;
pub(crate) const TARGET_DEST_DYNOBJ_ENEMY: ImplicitTarget = 28;
pub(crate) const TARGET_DEST_DYNOBJ_ALLY: ImplicitTarget = 29;
pub(crate) const TARGET_UNIT_SRC_AREA_ALLY: ImplicitTarget = 30;
pub(crate) const TARGET_UNIT_DEST_AREA_ALLY: ImplicitTarget = 31;
pub(crate) const TARGET_DEST_CASTER_SUMMON: ImplicitTarget = 32;
pub(crate) const TARGET_UNIT_SRC_AREA_PARTY: ImplicitTarget = 33;
pub(crate) const TARGET_UNIT_DEST_AREA_PARTY: ImplicitTarget = 34;
pub(crate) const TARGET_UNIT_TARGET_PARTY: ImplicitTarget = 35;
pub(crate) const TARGET_DEST_CASTER_UNK_36: ImplicitTarget = 36;
pub(crate) const TARGET_UNIT_LASTTARGET_AREA_PARTY: ImplicitTarget = 37;
pub(crate) const TARGET_UNIT_NEARBY_ENTRY: ImplicitTarget = 38;
pub(crate) const TARGET_DEST_CASTER_FISHING: ImplicitTarget = 39;
pub(crate) const TARGET_GAMEOBJECT_NEARBY_ENTRY: ImplicitTarget = 40;
pub(crate) const TARGET_DEST_CASTER_FRONT_RIGHT: ImplicitTarget = 41;
pub(crate) const TARGET_DEST_CASTER_BACK_RIGHT: ImplicitTarget = 42;
pub(crate) const TARGET_DEST_CASTER_BACK_LEFT: ImplicitTarget = 43;
pub(crate) const TARGET_DEST_CASTER_FRONT_LEFT: ImplicitTarget = 44;
pub(crate) const TARGET_UNIT_TARGET_CHAINHEAL_ALLY: ImplicitTarget = 45;
pub(crate) const TARGET_DEST_NEARBY_ENTRY: ImplicitTarget = 46;
pub(crate) const TARGET_DEST_CASTER_FRONT: ImplicitTarget = 47;
pub(crate) const TARGET_DEST_CASTER_BACK: ImplicitTarget = 48;
pub(crate) const TARGET_DEST_CASTER_RIGHT: ImplicitTarget = 49;
pub(crate) const TARGET_DEST_CASTER_LEFT: ImplicitTarget = 50;
pub(crate) const TARGET_GAMEOBJECT_SRC_AREA: ImplicitTarget = 51;
pub(crate) const TARGET_GAMEOBJECT_DEST_AREA: ImplicitTarget = 52;
pub(crate) const TARGET_DEST_TARGET_ENEMY: ImplicitTarget = 53;
pub(crate) const TARGET_UNIT_CONE_180_DEG_ENEMY: ImplicitTarget = 54;
pub(crate) const TARGET_DEST_CASTER_FRONT_LEAP: ImplicitTarget = 55;
pub(crate) const TARGET_UNIT_CASTER_AREA_RAID: ImplicitTarget = 56;
pub(crate) const TARGET_UNIT_TARGET_RAID: ImplicitTarget = 57;
pub(crate) const TARGET_UNIT_NEARBY_RAID: ImplicitTarget = 58;
pub(crate) const TARGET_UNIT_CONE_ALLY: ImplicitTarget = 59;
pub(crate) const TARGET_UNIT_CONE_ENTRY: ImplicitTarget = 60;
pub(crate) const TARGET_UNIT_TARGET_AREA_RAID_CLASS: ImplicitTarget = 61;
pub(crate) const TARGET_DEST_CASTER_GROUND: ImplicitTarget = 62;
pub(crate) const TARGET_DEST_TARGET_ANY: ImplicitTarget = 63;
pub(crate) const TARGET_DEST_TARGET_FRONT: ImplicitTarget = 64;
pub(crate) const TARGET_DEST_TARGET_BACK: ImplicitTarget = 65;
pub(crate) const TARGET_DEST_TARGET_RIGHT: ImplicitTarget = 66;
pub(crate) const TARGET_DEST_TARGET_LEFT: ImplicitTarget = 67;
pub(crate) const TARGET_DEST_TARGET_FRONT_RIGHT: ImplicitTarget = 68;
pub(crate) const TARGET_DEST_TARGET_BACK_RIGHT: ImplicitTarget = 69;
pub(crate) const TARGET_DEST_TARGET_BACK_LEFT: ImplicitTarget = 70;
pub(crate) const TARGET_DEST_TARGET_FRONT_LEFT: ImplicitTarget = 71;
pub(crate) const TARGET_DEST_CASTER_RANDOM: ImplicitTarget = 72;
pub(crate) const TARGET_DEST_CASTER_RADIUS: ImplicitTarget = 73;
pub(crate) const TARGET_DEST_TARGET_RANDOM: ImplicitTarget = 74;
pub(crate) const TARGET_DEST_TARGET_RADIUS: ImplicitTarget = 75;
pub(crate) const TARGET_DEST_CHANNEL_TARGET: ImplicitTarget = 76;
pub(crate) const TARGET_UNIT_CHANNEL_TARGET: ImplicitTarget = 77;
pub(crate) const TARGET_DEST_DEST_FRONT: ImplicitTarget = 78;
pub(crate) const TARGET_DEST_DEST_BACK: ImplicitTarget = 79;
pub(crate) const TARGET_DEST_DEST_RIGHT: ImplicitTarget = 80;
pub(crate) const TARGET_DEST_DEST_LEFT: ImplicitTarget = 81;
pub(crate) const TARGET_DEST_DEST_FRONT_RIGHT: ImplicitTarget = 82;
pub(crate) const TARGET_DEST_DEST_BACK_RIGHT: ImplicitTarget = 83;
pub(crate) const TARGET_DEST_DEST_BACK_LEFT: ImplicitTarget = 84;
pub(crate) const TARGET_DEST_DEST_FRONT_LEFT: ImplicitTarget = 85;
pub(crate) const TARGET_DEST_DEST_RANDOM: ImplicitTarget = 86;
pub(crate) const TARGET_DEST_DEST: ImplicitTarget = 87;
pub(crate) const TARGET_DEST_DYNOBJ_NONE: ImplicitTarget = 88;
pub(crate) const TARGET_DEST_TRAJ: ImplicitTarget = 89;
pub(crate) const TARGET_UNIT_TARGET_MINIPET: ImplicitTarget = 90;
pub(crate) const TARGET_DEST_DEST_RADIUS: ImplicitTarget = 91;
pub(crate) const TARGET_UNIT_SUMMONER: ImplicitTarget = 92;
pub(crate) const TARGET_CORPSE_SRC_AREA_ENEMY: ImplicitTarget = 93;
pub(crate) const TARGET_UNIT_VEHICLE: ImplicitTarget = 94;
pub(crate) const TARGET_UNIT_TARGET_PASSENGER: ImplicitTarget = 95;
pub(crate) const TARGET_UNIT_PASSENGER_0: ImplicitTarget = 96;
pub(crate) const TARGET_UNIT_PASSENGER_1: ImplicitTarget = 97;
pub(crate) const TARGET_UNIT_PASSENGER_2: ImplicitTarget = 98;
pub(crate) const TARGET_UNIT_PASSENGER_3: ImplicitTarget = 99;
pub(crate) const TARGET_UNIT_PASSENGER_4: ImplicitTarget = 100;
pub(crate) const TARGET_UNIT_PASSENGER_5: ImplicitTarget = 101;
pub(crate) const TARGET_UNIT_PASSENGER_6: ImplicitTarget = 102;
pub(crate) const TARGET_UNIT_PASSENGER_7: ImplicitTarget = 103;
pub(crate) const TARGET_UNIT_CONE_CASTER_TO_DEST_ENEMY: ImplicitTarget = 104;
pub(crate) const TARGET_UNIT_CASTER_AND_PASSENGERS: ImplicitTarget = 105;
pub(crate) const TARGET_DEST_NEARBY_DB: ImplicitTarget = 106;
pub(crate) const TARGET_DEST_NEARBY_ENTRY_2: ImplicitTarget = 107;
pub(crate) const TARGET_GAMEOBJECT_CONE_CASTER_TO_DEST_ENEMY: ImplicitTarget = 108;
pub(crate) const TARGET_GAMEOBJECT_CONE_CASTER_TO_DEST_ALLY: ImplicitTarget = 109;
pub(crate) const TARGET_UNIT_CONE_CASTER_TO_DEST_ENTRY: ImplicitTarget = 110;
pub(crate) const TARGET_UNIT_SRC_AREA_FURTHEST_ENEMY: ImplicitTarget = 115;
pub(crate) const TARGET_UNIT_AND_DEST_LAST_ENEMY: ImplicitTarget = 116;
pub(crate) const TARGET_UNIT_TARGET_ALLY_OR_RAID: ImplicitTarget = 118;
pub(crate) const TARGET_CORPSE_SRC_AREA_RAID: ImplicitTarget = 119;
pub(crate) const TARGET_UNIT_CASTER_AND_SUMMONS: ImplicitTarget = 120;
pub(crate) const TARGET_CORPSE_TARGET_ALLY: ImplicitTarget = 121;
pub(crate) const TARGET_UNIT_AREA_THREAT_LIST: ImplicitTarget = 122;
pub(crate) const TARGET_UNIT_AREA_TAP_LIST: ImplicitTarget = 123;
pub(crate) const TARGET_UNIT_TARGET_TAP_LIST: ImplicitTarget = 124;
pub(crate) const TARGET_DEST_CASTER_GROUND_2: ImplicitTarget = 125;
pub(crate) const TARGET_UNIT_CASTER_AREA_ENEMY_CLUMP: ImplicitTarget = 126;
pub(crate) const TARGET_DEST_CASTER_ENEMY_CLUMP_CENTROID: ImplicitTarget = 127;
pub(crate) const TARGET_UNIT_RECT_CASTER_ALLY: ImplicitTarget = 128;
pub(crate) const TARGET_UNIT_RECT_CASTER_ENEMY: ImplicitTarget = 129;
pub(crate) const TARGET_UNIT_RECT_CASTER: ImplicitTarget = 130;
pub(crate) const TARGET_DEST_SUMMONER: ImplicitTarget = 131;
pub(crate) const TARGET_DEST_TARGET_ALLY: ImplicitTarget = 132;
pub(crate) const TARGET_UNIT_LINE_CASTER_TO_DEST_ALLY: ImplicitTarget = 133;
pub(crate) const TARGET_UNIT_LINE_CASTER_TO_DEST_ENEMY: ImplicitTarget = 134;
pub(crate) const TARGET_UNIT_LINE_CASTER_TO_DEST: ImplicitTarget = 135;
pub(crate) const TARGET_UNIT_CONE_CASTER_TO_DEST_ALLY: ImplicitTarget = 136;
pub(crate) const TARGET_DEST_CASTER_MOVEMENT_DIRECTION: ImplicitTarget = 137;
pub(crate) const TARGET_DEST_DEST_GROUND: ImplicitTarget = 138;
pub(crate) const TARGET_DEST_CASTER_CLUMP_CENTROID: ImplicitTarget = 140;
pub(crate) const TARGET_DEST_NEARBY_ENTRY_OR_DB: ImplicitTarget = 142;
pub(crate) const TARGET_DEST_DEST_TARGET_TOWARDS_CASTER: ImplicitTarget = 148;
pub(crate) const TARGET_UNIT_OWN_CRITTER: ImplicitTarget = 150;
pub(crate) const TARGET_UNK_151: ImplicitTarget = 151;
pub(crate) const TARGET_153: ImplicitTarget = 153;

// Mechanic, Go dbcenums/mechanics.go
pub(crate) const MECHANIC_NONE: Mechanic = 0;
pub(crate) const MECHANIC_CHARM: Mechanic = 1;
pub(crate) const MECHANIC_DISORIENTED: Mechanic = 2;
pub(crate) const MECHANIC_DISARM: Mechanic = 3;
pub(crate) const MECHANIC_DISTRACT: Mechanic = 4;
pub(crate) const MECHANIC_FEAR: Mechanic = 5;
pub(crate) const MECHANIC_GRIP: Mechanic = 6;
pub(crate) const MECHANIC_ROOT: Mechanic = 7;
pub(crate) const MECHANIC_SLOW_ATTACK: Mechanic = 8;
pub(crate) const MECHANIC_SILENCE: Mechanic = 9;
pub(crate) const MECHANIC_SLEEP: Mechanic = 10;
pub(crate) const MECHANIC_SNARE: Mechanic = 11;
pub(crate) const MECHANIC_STUN: Mechanic = 12;
pub(crate) const MECHANIC_FREEZE: Mechanic = 13;
pub(crate) const MECHANIC_KNOCKOUT: Mechanic = 14;
pub(crate) const MECHANIC_BLEED: Mechanic = 15;
pub(crate) const MECHANIC_BANDAGE: Mechanic = 16;
pub(crate) const MECHANIC_POLYMORPH: Mechanic = 17;
pub(crate) const MECHANIC_BANISH: Mechanic = 18;
pub(crate) const MECHANIC_SHIELD: Mechanic = 19;
pub(crate) const MECHANIC_SHACKLE: Mechanic = 20;
pub(crate) const MECHANIC_MOUNT: Mechanic = 21;
pub(crate) const MECHANIC_INFECTED: Mechanic = 22;
pub(crate) const MECHANIC_TURN: Mechanic = 23;
pub(crate) const MECHANIC_HORROR: Mechanic = 24;
pub(crate) const MECHANIC_INVULNERABILITY: Mechanic = 25;
pub(crate) const MECHANIC_INTERRUPT: Mechanic = 26;
pub(crate) const MECHANIC_DAZE: Mechanic = 27;
pub(crate) const MECHANIC_DISCOVERY: Mechanic = 28;
pub(crate) const MECHANIC_IMMUNE_SHIELD: Mechanic = 29;
pub(crate) const MECHANIC_SAPPED: Mechanic = 30;
pub(crate) const MECHANIC_ENRAGED: Mechanic = 31;
pub(crate) const MECHANIC_WOUNDED: Mechanic = 32;
pub(crate) const MECHANIC_INFECTED_2: Mechanic = 33;
pub(crate) const MECHANIC_INFECTED_3: Mechanic = 34;
pub(crate) const MECHANIC_INFECTED_4: Mechanic = 35;
pub(crate) const MECHANIC_TAUNTED: Mechanic = 36;

// PowerType, Go dbcenums/powers.go
pub(crate) const POWER_HEALTH: PowerType = -2;
pub(crate) const POWER_MANA: PowerType = 0;
pub(crate) const POWER_RAGE: PowerType = 1;
pub(crate) const POWER_FOCUS: PowerType = 2;
pub(crate) const POWER_ENERGY: PowerType = 3;
pub(crate) const POWER_COMBO_POINTS: PowerType = 4;

// SpellModOp, Go dbcenums/spellmods.go
/// damage and healing done - Fire Power, Piercing Ice, Contagion
pub(crate) const SPELLMOD_DAMAGE: SpellModOp = 0;
/// aura duration - Permafrost, Improved Gouge, Brutal Impact
pub(crate) const SPELLMOD_DURATION: SpellModOp = 1;
/// threat generated - Subtlety, Improved Drain Soul
pub(crate) const SPELLMOD_THREAT: SpellModOp = 2;
/// the first effect's value - Arcane Potency, Improved Concentration Aura
pub(crate) const SPELLMOD_EFFECT1: SpellModOp = 3;
/// aura charges - Improved Shield Block, Improved Holy Shield
pub(crate) const SPELLMOD_CHARGES: SpellModOp = 4;
/// range - Arctic Reach, Flame Throwing, Grim Reach
pub(crate) const SPELLMOD_RANGE: SpellModOp = 5;
/// area radius - Arctic Reach, Holy Reach, Booming Voice
pub(crate) const SPELLMOD_RADIUS: SpellModOp = 6;
/// critical strike chance - Arcane Impact, Improved Flamestrike, Incineration
pub(crate) const SPELLMOD_CRITICAL_CHANCE: SpellModOp = 7;
/// every effect's value - Frost Warding, Magic Attunement, Demonic Aegis
pub(crate) const SPELLMOD_ALL_EFFECTS: SpellModOp = 8;
/// pushback taken while casting - Burning Soul, Fel Concentration, Intensity
pub(crate) const SPELLMOD_NOT_LOSE_CASTING_TIME: SpellModOp = 9;
/// cast time - Improved Fireball, Improved Frostbolt
pub(crate) const SPELLMOD_CASTING_TIME: SpellModOp = 10;
/// cooldown - Improved Fire Blast, Improved Frost Nova, Ice Floes
pub(crate) const SPELLMOD_COOLDOWN: SpellModOp = 11;
/// the second effect's value - Malediction, Mana Feed
pub(crate) const SPELLMOD_EFFECT2: SpellModOp = 12;
pub(crate) const SPELLMOD_IGNORE_ARMOR: SpellModOp = 13;
/// power cost - Frost Channeling, Cataclysm
pub(crate) const SPELLMOD_COST: SpellModOp = 14;
/// critical strike damage bonus - Ice Shards, Ruin, Vengeance
pub(crate) const SPELLMOD_CRIT_DAMAGE_BONUS: SpellModOp = 15;
/// chance to hit - Arcane Focus, Elemental Precision, Suppression
pub(crate) const SPELLMOD_RESIST_MISS_CHANCE: SpellModOp = 16;
/// chain targets
pub(crate) const SPELLMOD_JUMP_TARGETS: SpellModOp = 17;
/// Improved Poisons, Improved Nature's Grasp
pub(crate) const SPELLMOD_CHANCE_OF_SUCCESS: SpellModOp = 18;
/// Improved Fire Totems
pub(crate) const SPELLMOD_ACTIVATION_TIME: SpellModOp = 19;
pub(crate) const SPELLMOD_DAMAGE_MULTIPLIER: SpellModOp = 20;
/// global cooldown - Improved Slam
pub(crate) const SPELLMOD_GLOBAL_COOLDOWN: SpellModOp = 21;
/// periodic damage and healing - Emberstorm, Contagion, Fire Power
pub(crate) const SPELLMOD_DOT: SpellModOp = 22;
/// the third effect's value - Improved Faerie Fire, Savage Fury
pub(crate) const SPELLMOD_EFFECT3: SpellModOp = 23;
/// spell power coefficient - Empowered Arcane Missiles / Fireball / Frostbolt / Corruption
pub(crate) const SPELLMOD_BONUS_MULTIPLIER: SpellModOp = 24;
pub(crate) const SPELLMOD_TRIGGER_DAMAGE: SpellModOp = 25;
/// procs per minute
pub(crate) const SPELLMOD_PROC_PER_MINUTE: SpellModOp = 26;
/// Improved Mana Shield
pub(crate) const SPELLMOD_VALUE_MULTIPLIER: SpellModOp = 27;
/// chance to resist a dispel - Vile Poisons, Sanctified Seals
pub(crate) const SPELLMOD_RESIST_DISPEL_CHANCE: SpellModOp = 28;
pub(crate) const SPELLMOD_CRIT_DAMAGE_BONUS_2: SpellModOp = 29;
pub(crate) const SPELLMOD_SPELL_COST_REFUND_ON_FAIL: SpellModOp = 30;
pub(crate) const SPELLMOD_DOSES: SpellModOp = 31;
/// the fourth effect's value
pub(crate) const SPELLMOD_EFFECT4: SpellModOp = 32;
/// the fifth effect's value
pub(crate) const SPELLMOD_EFFECT5: SpellModOp = 33;
pub(crate) const SPELLMOD_COST2: SpellModOp = 34;
pub(crate) const SPELLMOD_JUMP_DISTANCE: SpellModOp = 35;
pub(crate) const SPELLMOD_AREATRIGGER_MAX_SUMMONS: SpellModOp = 36;
/// maximum aura stacks
pub(crate) const SPELLMOD_MAX_AURA_STACKS: SpellModOp = 37;
/// internal cooldown between procs
pub(crate) const SPELLMOD_PROC_COOLDOWN: SpellModOp = 38;
pub(crate) const SPELLMOD_COST3: SpellModOp = 39;
/// maximum targets
pub(crate) const SPELLMOD_MAX_TARGETS: SpellModOp = 40;

// ShapeshiftForm, Go dbcenums/forms_auto_gen.go
pub(crate) const FORM_CAT_FORM: ShapeshiftForm = 1;
pub(crate) const FORM_TREE_FORM: ShapeshiftForm = 2;
pub(crate) const FORM_TRAVEL_FORM: ShapeshiftForm = 3;
pub(crate) const FORM_AQUATIC_FORM: ShapeshiftForm = 4;
pub(crate) const FORM_BEAR_FORM: ShapeshiftForm = 5;
pub(crate) const FORM_AMBIENT: ShapeshiftForm = 6;
pub(crate) const FORM_GHOUL: ShapeshiftForm = 7;
pub(crate) const FORM_DIRE_BEAR_FORM: ShapeshiftForm = 8;
pub(crate) const FORM_GHOST_WOLF: ShapeshiftForm = 16;
pub(crate) const FORM_BATTLE_STANCE: ShapeshiftForm = 17;
pub(crate) const FORM_DEFENSIVE_STANCE: ShapeshiftForm = 18;
pub(crate) const FORM_BERSERKER_STANCE: ShapeshiftForm = 19;
pub(crate) const FORM_METAMORPHOSIS: ShapeshiftForm = 22;
pub(crate) const FORM_SHADOWFORM: ShapeshiftForm = 28;
pub(crate) const FORM_STEALTH: ShapeshiftForm = 30;
pub(crate) const FORM_MOONKIN_FORM: ShapeshiftForm = 31;
pub(crate) const FORM_SPIRIT_OF_REDEMPTION: ShapeshiftForm = 32;

// Spell attribute bits, Go dbcenums/attributes.go
/// Spell attribute flags, named after the Attributes column they live in: ATTR_EX_3 is a flag
/// in Attributes[3]. Read them through the Spell helpers rather than indexing Attributes.
/// The spell is never cast: a stance's passive, a talent that only modifies other spells.
pub(crate) const ATTR_PASSIVE: u32 = 0x40;
/// The spell cannot be cast while the caster is in any shapeshift form.
pub(crate) const ATTR_NOT_SHAPESHIFTED: u32 = 0x10000;
/// The two bits the client marks a channel with. Arcane Missiles and Blizzard carry the first,
/// Evocation and Tranquility only the second, so a channel check has to read both.
pub(crate) const ATTR_EX_1_IS_CHANNELLED: u32 = 0x4;
pub(crate) const ATTR_EX_1_IS_SELF_CHANNELLED: u32 = 0x40;
/// The server refunds 80% of the power cost when the spell misses: every rage special that
/// costs rage up front, Heroic Strike and Rend among them. Cleave and Whirlwind lack it.
pub(crate) const ATTR_EX_1_DISCOUNT_POWER_ON_MISS: u32 = 0x8000000;
pub(crate) const ATTR_EX_2_CANT_CRIT: u32 = 0x20000000;
/// The spell is castable while shapeshifted even though its form bars casting: the exception to
/// ATTR_NOT_SHAPESHIFTED.
pub(crate) const ATTR_EX_2_CASTABLE_IN_CASTER_FORM: u32 = 0x80000;
/// On a triggered spell: aura listeners treat its hits like a normal ability hit. Seal of
/// Command damage, every Judgement, Stormstrike's bonus hits and Sweeping Strikes carry it.
pub(crate) const ATTR_EX_3_NOT_A_PROC: u32 = 0x200;
pub(crate) const ATTR_EX_3_CAN_PROC_FROM_PROCS: u32 = 0x4000000;
/// Weapon procs (Player::CastItemCombatSpell) ignore hits of this spell, as do auras marked
/// ATTR_EX_6_AURA_IS_WEAPON_PROC. In TBC that is the Seal of Blood, Righteousness and Martyr
/// damage spells plus Gouge, Sap, Scatter Shot and Maim.
pub(crate) const ATTR_EX_4_SUPPRESS_WEAPON_PROCS: u32 = 0x800000;
/// An aura proc that honours ATTR_EX_4_SUPPRESS_WEAPON_PROCS anyway: Black Bow of the Betrayer
/// and the Sunwell melee neck.
pub(crate) const ATTR_EX_6_AURA_IS_WEAPON_PROC: u32 = 0x80;
/// A periodic effect whose ticks roll a critical strike: Rend, Corruption, Rupture and the
/// other bleeds and DoTs the client marks, 225 spells in this build.
pub(crate) const ATTR_EX_8_PERIODIC_CAN_CRIT: u32 = 0x200;
pub(crate) const ATTR_EX_11_SCALES_WITH_ITEM_LEVEL: u32 = 0x4;
pub(crate) const ATTR_EX_12_ONLY_PROC_FROM_CLASS_ABILITIES: u32 = 0x80000000;
/// The remaining named attribute bits.
pub(crate) const ATTR_RANGED_ABILITY: u32 = 0x2;
pub(crate) const ATTR_ABILITY: u32 = 0x10;
pub(crate) const ATTR_TRADESKILL_ABILITY: u32 = 0x20;
pub(crate) const ATTR_HIDDEN: u32 = 0x80;
pub(crate) const ATTR_REQ_STEALTH: u32 = 0x20000;
pub(crate) const ATTR_CANCEL_AUTO_ATTACK: u32 = 0x100000;
pub(crate) const ATTR_NO_D_P_B: u32 = 0x200000;
pub(crate) const ATTR_NO_COMBAT: u32 = 0x400000;
pub(crate) const ATTR_NO_CANCEL: u32 = 0x80000000;
pub(crate) const ATTR_EX_1_NO_STEALTH_BREAK: u32 = 0x20;
pub(crate) const ATTR_EX_1_MELEE_COMBAT_START: u32 = 0x200;
pub(crate) const ATTR_EX_1_NO_THREAT: u32 = 0x400;
pub(crate) const ATTR_EX_1_DONT_DISPLAY_IN_AURA_BAR: u32 = 0x10000000;
pub(crate) const ATTR_EX_2_FOOD_AURA: u32 = 0x80000000;
pub(crate) const ATTR_EX_3_REQ_MAIN_HAND: u32 = 0x400;
pub(crate) const ATTR_EX_3_SUPPRESS_CASTER_PROCS: u32 = 0x10000;
pub(crate) const ATTR_EX_3_SUPPRESS_TARGET_PROCS: u32 = 0x20000;
pub(crate) const ATTR_EX_3_ALWAYS_HIT: u32 = 0x40000;
pub(crate) const ATTR_EX_3_REQ_OFF_HAND: u32 = 0x1000000;
pub(crate) const ATTR_EX_3_TREAT_AS_PERIODIC: u32 = 0x2000000;
pub(crate) const ATTR_EX_4_DISABLE_TARGET_MULT: u32 = 0x100;
pub(crate) const ATTR_EX_5_TICK_ON_APPLICATION: u32 = 0x200;
pub(crate) const ATTR_EX_5_DOT_HASTED: u32 = 0x2000;
pub(crate) const ATTR_EX_5_TREAT_AS_AREA_EFFECT: u32 = 0x8000;
pub(crate) const ATTR_EX_5_REQ_LINE_OF_SIGHT: u32 = 0x4000000;
pub(crate) const ATTR_EX_6_IGNORE_FOR_MOD_TIME_RATE: u32 = 0x10;
pub(crate) const ATTR_EX_6_DISABLE_PLAYER_MULT: u32 = 0x20000000;
pub(crate) const ATTR_EX_7_NO_DODGE: u32 = 0x800000;
pub(crate) const ATTR_EX_7_NO_PARRY: u32 = 0x1000000;
pub(crate) const ATTR_EX_7_NO_MISS: u32 = 0x2000000;
pub(crate) const ATTR_EX_7_CAN_PROC_FROM_SUPPRESSED_TGT: u32 = 0x40000000;
pub(crate) const ATTR_EX_8_NO_BLOCK: u32 = 0x1;
pub(crate) const ATTR_EX_8_DURATION_HASTED: u32 = 0x20000;
pub(crate) const ATTR_EX_8_REQUIRES_EQUIPPED_ARMOR_TYPE: u32 = 0x100000;
pub(crate) const ATTR_EX_8_DOT_HASTED_MELEE: u32 = 0x400000;
pub(crate) const ATTR_EX_8_MASTERY_AFFECTS_POINTS: u32 = 0x20000000;
pub(crate) const ATTR_EX_9_FIXED_TRAVEL_TIME: u32 = 0x10;
pub(crate) const ATTR_EX_9_DISABLE_PLAYER_HEALING_MULT: u32 = 0x1000000;
pub(crate) const ATTR_EX_10_DISABLE_TARGET_POSITIVE_MULT: u32 = 0x2;
pub(crate) const ATTR_EX_10_TARGET_SPECIFIC_COOLDOWN: u32 = 0x400;
pub(crate) const ATTR_EX_10_ROLLING_PERIODIC: u32 = 0x4000;
/// requires CAN_PROC_FROM_SUPPRESSED on driver
pub(crate) const ATTR_EX_12_ENABLE_PROCS_FROM_SUPPRESSED: u32 = 0x1;
/// requires ENABLE_PROCS_FROM_SUPPRESSED on action
pub(crate) const ATTR_EX_12_CAN_PROC_FROM_SUPPRESSED: u32 = 0x2;
pub(crate) const ATTR_EX_13_ALLOW_CLASS_ABILITY_PROCS: u32 = 0x1;
pub(crate) const ATTR_EX_13_REFRESH_EXTENDS_DURATION: u32 = 0x100000;
pub(crate) const ATTR_EX_15_AURA_DOES_NOT_REFRESH: u32 = 0x200;
pub(crate) const ATTR_EX_15_ASYNCHRONOUS_STACKING_AURA: u32 = 0x400;
pub(crate) const ATTR_EX_15_IMPORTANT_SPELL: u32 = 0x800;
pub(crate) const ATTR_EX_15_IS_EXTERNAL_DEFENSIVE: u32 = 0x80000;
pub(crate) const ATTR_EX_16_IS_BIG_DEFENSIVE: u32 = 0x1;
/// Attributes index each ATTR_EX_ flag above belongs to.
pub(crate) const ATTR_INDEX_BASE: usize = 0;
pub(crate) const ATTR_INDEX_EX_1: usize = 1;
pub(crate) const ATTR_INDEX_EX_2: usize = 2;
pub(crate) const ATTR_INDEX_EX_3: usize = 3;
pub(crate) const ATTR_INDEX_EX_4: usize = 4;
pub(crate) const ATTR_INDEX_EX_6: usize = 6;
pub(crate) const ATTR_INDEX_EX_8: usize = 8;
pub(crate) const ATTR_INDEX_EX_11: usize = 11;
pub(crate) const ATTR_INDEX_EX_12: usize = 12;

// Interrupt flags, Go dbcenums/interrupts.go
/// SpellInterrupts.InterruptFlags bits.
/// Damage taken while casting pushes the cast back. Frostbolt, Fireball, Greater Heal and Slam
/// carry it; the bombs state 0x5 without it, and the grenades and dynamite have no row at all.
pub(crate) const SPELL_INTERRUPT_FLAG_PUSHBACK: u32 = 0x2;

// Proc flags, Go dbcenums/proc_flags.go
/// Named bits of SpellAuraOptions.ProcTypeMask word 0, under TrinityCore's names for them.
pub(crate) const PROC_FLAG_NONE: u32 = 0;
/// 00 Heartbeat
pub(crate) const PROC_FLAG_HEARTBEAT: u32 = 0x00000001;
/// 01 Kill target (in most cases need XP/Honor reward)
pub(crate) const PROC_FLAG_KILL: u32 = 0x00000002;
/// 02 Done melee auto attack
pub(crate) const PROC_FLAG_DEAL_MELEE_SWING: u32 = 0x00000004;
/// 03 Taken melee auto attack
pub(crate) const PROC_FLAG_TAKE_MELEE_SWING: u32 = 0x00000008;
/// 04 Done attack by Spell that has dmg class melee
pub(crate) const PROC_FLAG_DEAL_MELEE_ABILITY: u32 = 0x00000010;
/// 05 Taken attack by Spell that has dmg class melee
pub(crate) const PROC_FLAG_TAKE_MELEE_ABILITY: u32 = 0x00000020;
/// 06 Done ranged auto attack
pub(crate) const PROC_FLAG_DEAL_RANGED_ATTACK: u32 = 0x00000040;
/// 07 Taken ranged auto attack
pub(crate) const PROC_FLAG_TAKE_RANGED_ATTACK: u32 = 0x00000080;
/// 08 Done attack by Spell that has dmg class ranged
pub(crate) const PROC_FLAG_DEAL_RANGED_ABILITY: u32 = 0x00000100;
/// 09 Taken attack by Spell that has dmg class ranged
pub(crate) const PROC_FLAG_TAKE_RANGED_ABILITY: u32 = 0x00000200;
/// 10 Done positive spell that has dmg class none
pub(crate) const PROC_FLAG_DEAL_HELPFUL_ABILITY: u32 = 0x00000400;
/// 11 Taken positive spell that has dmg class none
pub(crate) const PROC_FLAG_TAKE_HELPFUL_ABILITY: u32 = 0x00000800;
/// 12 Done negative spell that has dmg class none
pub(crate) const PROC_FLAG_DEAL_HARMFUL_ABILITY: u32 = 0x00001000;
/// 13 Taken negative spell that has dmg class none
pub(crate) const PROC_FLAG_TAKE_HARMFUL_ABILITY: u32 = 0x00002000;
/// 14 Done positive spell that has dmg class magic
pub(crate) const PROC_FLAG_DEAL_HELPFUL_SPELL: u32 = 0x00004000;
/// 15 Taken positive spell that has dmg class magic
pub(crate) const PROC_FLAG_TAKE_HELPFUL_SPELL: u32 = 0x00008000;
/// 16 Done negative spell that has dmg class magic
pub(crate) const PROC_FLAG_DEAL_HARMFUL_SPELL: u32 = 0x00010000;
/// 17 Taken negative spell that has dmg class magic
pub(crate) const PROC_FLAG_TAKE_HARMFUL_SPELL: u32 = 0x00020000;
/// 18 Successful do periodic (damage)
pub(crate) const PROC_FLAG_DEAL_HARMFUL_PERIODIC: u32 = 0x00040000;
/// 19 Taken spell periodic (damage)
pub(crate) const PROC_FLAG_TAKE_HARMFUL_PERIODIC: u32 = 0x00080000;
/// 20 Taken any damage
pub(crate) const PROC_FLAG_TAKE_ANY_DAMAGE: u32 = 0x00100000;
/// 21
pub(crate) const PROC_FLAG_DEAL_HELPFUL_PERIODIC: u32 = 0x00200000;
/// 22 Done main-hand melee attacks (spell and autoattack)
pub(crate) const PROC_FLAG_MAIN_HAND_WEAPON_SWING: u32 = 0x00400000;
/// 23 Done off-hand melee attacks (spell and autoattack)
pub(crate) const PROC_FLAG_OFF_HAND_WEAPON_SWING: u32 = 0x00800000;
/// 24 The caster died
pub(crate) const PROC_FLAG_DEATH: u32 = 0x01000000;
/// 25 The caster jumped
pub(crate) const PROC_FLAG_JUMP: u32 = 0x02000000;
/// 26 Proc clone spell
pub(crate) const PROC_FLAG_PROC_CLONE_SPELL: u32 = 0x04000000;
/// 27 The caster entered combat
pub(crate) const PROC_FLAG_ENTER_COMBAT: u32 = 0x08000000;
/// 28 The encounter started
pub(crate) const PROC_FLAG_ENCOUNTER_START: u32 = 0x10000000;
/// 29 A cast ended, however it ended
pub(crate) const PROC_FLAG_CAST_ENDED: u32 = 0x20000000;
/// 30 The caster looted
pub(crate) const PROC_FLAG_LOOTED: u32 = 0x40000000;
/// 31 Taken helpful periodic
pub(crate) const PROC_FLAG_TAKE_HELPFUL_PERIODIC: u32 = 0x80000000;
pub(crate) const PROC_FLAG_ANY_DIRECT_TAKEN: u32 = PROC_FLAG_TAKE_MELEE_SWING
    | PROC_FLAG_TAKE_MELEE_ABILITY
    | PROC_FLAG_TAKE_RANGED_ABILITY
    | PROC_FLAG_TAKE_HARMFUL_ABILITY
    | PROC_FLAG_TAKE_RANGED_ATTACK
    | PROC_FLAG_TAKE_HELPFUL_SPELL
    | PROC_FLAG_TAKE_HELPFUL_ABILITY
    | PROC_FLAG_TAKE_ANY_DAMAGE
    | PROC_FLAG_TAKE_HARMFUL_SPELL;
pub(crate) const PROC_FLAG_ANY_DIRECT_DEALT: u32 = PROC_FLAG_DEAL_MELEE_SWING
    | PROC_FLAG_DEAL_MELEE_ABILITY
    | PROC_FLAG_DEAL_RANGED_ATTACK
    | PROC_FLAG_DEAL_RANGED_ABILITY
    | PROC_FLAG_DEAL_HARMFUL_ABILITY
    | PROC_FLAG_DEAL_HARMFUL_SPELL;
pub(crate) const PROC_FLAG_ANY_HEAL: u32 =
    PROC_FLAG_DEAL_HELPFUL_PERIODIC | PROC_FLAG_DEAL_HELPFUL_ABILITY | PROC_FLAG_DEAL_HELPFUL_SPELL;
/// Named bits of ProcTypeMask word 1, under TrinityCore's names. The sim models only
/// PROC_FLAG_2_MAIN_HAND_ONLY, which carries no TrinityCore name: Bloodthrill and the main-hand
/// weapon procs that state it hear main-hand melee attacks alone.
/// 32 Kill or assist in killing the target
pub(crate) const PROC_FLAG_2_TARGET_DIES: u32 = 0x00000001;
/// 33 Knockback
pub(crate) const PROC_FLAG_2_KNOCKBACK: u32 = 0x00000002;
/// 34 Cast successful
pub(crate) const PROC_FLAG_2_CAST_SUCCESSFUL: u32 = 0x00000004;
/// 36 Successful dispel
pub(crate) const PROC_FLAG_2_SUCCESSFUL_DISPEL: u32 = 0x00000010;
/// 37 Main-hand melee attacks only
pub(crate) const PROC_FLAG_2_MAIN_HAND_ONLY: u32 = 0x00000020;
/// 38 Do emote
pub(crate) const PROC_FLAG_2_DO_EMOTE: u32 = 0x00000040;

// RPPM modifier types, Go dbcenums/rppm.go
/// 1
pub(crate) const RPPM_MODIFIER_HASTE: i32 = 1;
pub(crate) const RPPM_MODIFIER_CRIT: i32 = 2;
pub(crate) const RPPM_MODIFIER_CLASS: i32 = 3;
pub(crate) const RPPM_MODIFIER_SPEC: i32 = 4;
pub(crate) const RPPM_MODIFIER_RACE: i32 = 5;
pub(crate) const RPPM_MODIFIER_ILEVEL: i32 = 6;
pub(crate) const RPPM_MODIFIER_UNK_ADJUST: i32 = 7;
pub(crate) const RPPM_MODIFIER_AURA: i32 = 8;

#[cfg(test)]
mod tests {
    use super::*;

    /// Go `TestClientValues`: the client's numbers, which the rows are written against.
    #[test]
    fn client_values() {
        assert_eq!(E_SCHOOL_DAMAGE, 2);
        assert_eq!(A_PROC_TRIGGER_SPELL, 42);
        assert_eq!(ATTR_EX_8_PERIODIC_CAN_CRIT, 0x200);
        assert_eq!(PROC_FLAG_DEAL_HARMFUL_SPELL, 0x10000);
    }

    #[test]
    fn spot_values_follow_go() {
        assert_eq!(E_APPLY_AURA, 6);
        assert_eq!(E_HEAL, 10);
        assert_eq!(E_ENERGIZE, 30);
        assert_eq!(A_PERIODIC_DAMAGE, 3);
        assert_eq!(A_PERIODIC_ENERGIZE, 24);
        assert_eq!(TARGET_UNIT_CASTER, 1);
        assert_eq!(TARGET_UNIT_TARGET_ENEMY, 6);
        assert_eq!(MECHANIC_BLEED, 15);
        assert_eq!(POWER_HEALTH, -2);
        assert_eq!(POWER_RAGE, 1);
        assert_eq!(SPELLMOD_DOT, 22);
        assert_eq!(SPELLMOD_MAX_TARGETS, 40);
        assert_eq!(FORM_MOONKIN_FORM, 31);
        assert_eq!(ATTR_NOT_SHAPESHIFTED, 0x10000);
        assert_eq!(ATTR_INDEX_EX_12, 12);
        assert_eq!(SPELL_INTERRUPT_FLAG_PUSHBACK, 0x2);
        assert_eq!(RPPM_MODIFIER_HASTE, 1);
        assert_eq!(RPPM_MODIFIER_AURA, 8);
        assert_eq!(PROC_FLAG_2_MAIN_HAND_ONLY, 0x20);
    }

    #[test]
    fn composite_proc_flags_or_their_parts() {
        assert_eq!(
            PROC_FLAG_ANY_HEAL,
            PROC_FLAG_DEAL_HELPFUL_PERIODIC
                | PROC_FLAG_DEAL_HELPFUL_ABILITY
                | PROC_FLAG_DEAL_HELPFUL_SPELL
        );
        assert_ne!(PROC_FLAG_ANY_DIRECT_DEALT & PROC_FLAG_DEAL_MELEE_SWING, 0);
        assert_ne!(PROC_FLAG_ANY_DIRECT_TAKEN & PROC_FLAG_TAKE_HARMFUL_SPELL, 0);
    }

    #[test]
    fn powers_are_stated_in_tenths_for_rage_only() {
        assert!(power_in_tenths(POWER_RAGE));
        for power in [
            POWER_HEALTH,
            POWER_MANA,
            POWER_FOCUS,
            POWER_ENERGY,
            POWER_COMBO_POINTS,
        ] {
            assert!(!power_in_tenths(power));
        }
    }

    #[test]
    fn proc_trigger_auras() {
        assert!(is_proc_trigger(A_PROC_TRIGGER_SPELL));
        assert!(is_proc_trigger(A_PROC_TRIGGER_SPELL_WITH_VALUE));
        assert!(!is_proc_trigger(A_PERIODIC_TRIGGER_SPELL));
    }

    #[test]
    fn form_masks_name_bit_id_minus_one() {
        assert_eq!(form_mask(0), 0);
        assert_eq!(form_mask(FORM_CAT_FORM), 1);
        assert_eq!(form_mask(FORM_TREE_FORM), 2);
        assert_eq!(form_mask(FORM_MOONKIN_FORM), 0x4000_0000);
        // A form past the mask's 64 bits shifts out, as Go's shift does.
        assert_eq!(form_mask(65), 0);
        // Cat Form is not a stance; Battle Stance is.
        assert!(!form_is_stance(FORM_CAT_FORM));
        assert!(form_is_stance(FORM_BATTLE_STANCE));
        assert!(!form_is_stance(0));
    }
}
