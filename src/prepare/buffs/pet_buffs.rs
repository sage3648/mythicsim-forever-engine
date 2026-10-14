//! Go sim/core/buffs/pet_buffs.go (fork patch 131): the party auras that reach a player's
//! summoned pet.
//!
//! Forever's party auras are area auras on the party (SpellEffect type 35), and the server
//! applies them to the party's pets unless the spell carries `ATTR_EX_3_ONLY_TARGET_PLAYERS`.
//! The raid-wide buffs and the Greater Blessings carry the flag, so a pet gets none of them.
//! Go leaves out Mana Tide Totem (the player's external cooldown), Retribution Aura (pets take no
//! hits), Blood Pact (health only) and the weapon totems; so does this.

use crate::contracts::request::Message;

use super::super::env::Environment;
use super::super::sim::UnitId;
use super::super::spelldata::must_find;
use super::super::Refusal;
use super::drivers::{
    drive_atiesh_druid, drive_atiesh_mage, drive_atiesh_priest, drive_atiesh_warlock,
};
use super::generated::{
    ATIESH_DRUID, ATIESH_MAGE, ATIESH_PRIEST, ATIESH_WARLOCK, MANA_SPRING_TOTEM, TRUESHOT_AURA,
};
use super::{permanent, tristate, Meta};

type Apply = fn(&mut Environment, UnitId, &Message) -> Result<(), Refusal>;

/// Go `petPartyBuffs`, in its order: the buff's row and how the party's field applies it.
const PET_PARTY_BUFFS: [(&Meta, Apply); 6] = [
    (&MANA_SPRING_TOTEM, |env, pet, party| {
        let effect = party.enum_number("mana_spring_totem");
        if effect != 0 {
            permanent(env, pet, &MANA_SPRING_TOTEM, false, tristate(effect, 0, 5))?;
        }
        Ok(())
    }),
    (&TRUESHOT_AURA, |env, pet, party| {
        if party.bool("trueshot_aura") {
            permanent(env, pet, &TRUESHOT_AURA, false, 0)?;
        }
        Ok(())
    }),
    (&ATIESH_MAGE, |env, pet, party| {
        if party.i32("atiesh_mage") > 0 {
            drive_atiesh_mage(env, pet, party)?;
        }
        Ok(())
    }),
    (&ATIESH_WARLOCK, |env, pet, party| {
        if party.i32("atiesh_warlock") > 0 {
            drive_atiesh_warlock(env, pet, party)?;
        }
        Ok(())
    }),
    (&ATIESH_DRUID, |env, pet, party| {
        if party.i32("atiesh_druid") > 0 {
            drive_atiesh_druid(env, pet, party)?;
        }
        Ok(())
    }),
    (&ATIESH_PRIEST, |env, pet, party| {
        if party.i32("atiesh_priest") > 0 {
            drive_atiesh_priest(env, pet, party)?;
        }
        Ok(())
    }),
];

/// Go `applyPetPartyBuffs`, the `BuffHooks.ApplyPetBuffs` hook `Raid.applyCharacterEffects`
/// calls for each pet that is not a guardian: the buffs above that the party has and whose
/// client row lets the aura reach a pet. Go's `PartyAurasReachPets` switch defaults on and no
/// request sets it.
pub(crate) fn apply_pet_party_buffs(
    env: &mut Environment,
    pet: UnitId,
    party: &Message,
) -> Result<(), Refusal> {
    for (meta, apply) in PET_PARTY_BUFFS {
        if !must_find(meta.spell).only_targets_players() {
            apply(env, pet, party)?;
        }
    }
    Ok(())
}

/// The flags the client rows carry at the pin, as Go's `TestPartyAuraPetFlags` reads them.
#[cfg(test)]
mod tests {
    use super::super::super::spelldata::must_find;

    #[test]
    fn party_aura_pet_flags() {
        for (id, players_only) in [
            (25918, true),  // Greater Blessing of Wisdom
            (23028, true),  // Arcane Brilliance
            (27681, true),  // Prayer of Spirit
            (21850, true),  // Gift of the Wild
            (10293, true),  // Devotion Aura
            (25289, true),  // Battle Shout
            (10494, false), // Mana Spring Totem's aura
            (10301, false), // Retribution Aura
            (20905, false), // Trueshot Aura
        ] {
            assert_eq!(
                must_find(id).only_targets_players(),
                players_only,
                "spell {id}"
            );
        }
    }
}
