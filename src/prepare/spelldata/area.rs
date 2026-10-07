//! Go `spelldata/area.go`: the terrain an area group names, and the bonus a spell takes there.

use super::Spell;

/// `proto.AreaType`, with the numbers of the pinned schema.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AreaType {
    #[default]
    Unknown = 0,
    ForestGrassland = 1,
    Mountainous = 2,
    Snowy = 3,
    Desert = 4,
    Swamp = 5,
    Wasteland = 6,
    Haunted = 7,
    Cavernous = 8,
    Volcanic = 9,
    StrongholdsCities = 10,
}

impl AreaType {
    /// The proto enum value's name, as the request's `area_types` spells it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            AreaType::Unknown => "AreaTypeUnknown",
            AreaType::ForestGrassland => "AreaTypeForestGrassland",
            AreaType::Mountainous => "AreaTypeMountainous",
            AreaType::Snowy => "AreaTypeSnowy",
            AreaType::Desert => "AreaTypeDesert",
            AreaType::Swamp => "AreaTypeSwamp",
            AreaType::Wasteland => "AreaTypeWasteland",
            AreaType::Haunted => "AreaTypeHaunted",
            AreaType::Cavernous => "AreaTypeCavernous",
            AreaType::Volcanic => "AreaTypeVolcanic",
            AreaType::StrongholdsCities => "AreaTypeStrongholdsCities",
        }
    }
}

/// Go `AreaTypeOfGroup`: terrain area groups, each citing a tooltip; any other group is a single
/// zone, which reads as unknown.
pub(crate) fn area_type_of_group(group: i32) -> AreaType {
    match group {
        // "in Forest and Grassland areas", Grovewalker 1306076
        9161 => AreaType::ForestGrassland,
        // "in Mountainous areas", Might of Bedrock 1308764
        9165 => AreaType::Mountainous,
        // "in Snowy areas", Bitter Cold 1308511
        9164 => AreaType::Snowy,
        // "in Desert areas", The Desert Rose 1294646
        9097 => AreaType::Desert,
        // "in Swamp areas", Sporebloom 1306672
        9162 => AreaType::Swamp,
        // "in Wasteland areas", Fallen Grove Walker 1306632
        9163 => AreaType::Wasteland,
        // "in Haunted areas", Phantomspeaker Mask 1307334
        9202 => AreaType::Haunted,
        // "in Cavernous or Underground areas", Royal Seal of Eldre'Thalas 1318480
        9324 => AreaType::Cavernous,
        // "in Strongholds and Cities", Black Horn Necklace 1320332
        9326 => AreaType::StrongholdsCities,
        // Inferred: no spell requires 9203, but it holds the Blackrock zones, MC, BWL and
        // Onyxia's Lair.
        9203 => AreaType::Volcanic,
        _ => AreaType::Unknown,
    }
}

impl Spell {
    pub(crate) fn area_type(&self) -> AreaType {
        area_type_of_group(self.required_areas)
    }

    /// The amount and duration factors in this encounter; 1 and 1 outside the bonus's areas.
    /// `in_area` is Go's `encounter.InArea`. The factors are float32 columns, widened exactly.
    pub(crate) fn area_bonus(&self, in_area: impl Fn(AreaType) -> bool) -> (f64, f64) {
        for &group in &self.area_bonus_groups {
            if in_area(area_type_of_group(group)) {
                return (
                    f64::from(self.area_multiplier),
                    f64::from(self.area_duration_multiplier),
                );
            }
        }
        (1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::{nil, store::must_find};

    /// Go `TestAreaBonusReadsAnyOfItsGroups`.
    #[test]
    fn area_bonus_reads_any_of_its_groups() {
        let bonus = Spell {
            area_bonus_groups: vec![9163, 9202],
            area_multiplier: 2.0,
            area_duration_multiplier: 1.0,
            ..Spell::default()
        };
        let plain = Spell::default();

        let cases: [(&str, &[AreaType], f64, f64); 4] = [
            ("no area", &[], 1.0, 1.0),
            ("another area", &[AreaType::ForestGrassland], 1.0, 1.0),
            (
                "the second group",
                &[AreaType::Snowy, AreaType::Haunted],
                2.0,
                1.0,
            ),
            ("the first group", &[AreaType::Wasteland], 2.0, 1.0),
        ];
        for (name, areas, amount, length) in cases {
            let in_area = |area| areas.contains(&area);
            assert_eq!(bonus.area_bonus(in_area), (amount, length), "{name}");
            assert_eq!(plain.area_bonus(in_area), (1.0, 1.0), "{name}");
        }
        assert_eq!(
            nil().area_bonus(|area| area == AreaType::Volcanic),
            (1.0, 1.0)
        );
    }

    /// Go `TestGeneratedRequiredAreas`: Stolen Power's companion row requires Forest and
    /// Grassland; Staff of Westfall's group is a single zone.
    #[test]
    fn required_areas() {
        let stolen = must_find(1318002);
        assert_eq!(stolen.required_areas, 9161);
        assert_eq!(stolen.area_type(), AreaType::ForestGrassland);
        let plain = must_find(1287561);
        assert_eq!(plain.required_areas, 0);
        assert_eq!(plain.area_type(), AreaType::Unknown);
        let staff = must_find(1292011);
        assert_eq!(staff.required_areas, 9071);
        assert_eq!(staff.area_type(), AreaType::Unknown);
    }

    /// Go `TestGeneratedAreaBonus`: area bonuses come from the override table, not the client.
    #[test]
    fn area_bonuses_come_from_the_overrides() {
        let molten = must_find(1249113);
        assert_eq!(molten.area_bonus_groups, [9203]);
        assert_eq!(
            (molten.area_multiplier, molten.area_duration_multiplier),
            (2.0, 1.0)
        );
        let monkey = must_find(1287571);
        assert_eq!(
            (monkey.area_multiplier, monkey.area_duration_multiplier),
            (2.0, 2.0)
        );
    }

    #[test]
    fn area_numbers_match_the_schema() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../../data/proto-schema.json")).unwrap();
        for entry in schema["enums"]["proto.AreaType"].as_array().unwrap() {
            let area = [
                AreaType::Unknown,
                AreaType::ForestGrassland,
                AreaType::Mountainous,
                AreaType::Snowy,
                AreaType::Desert,
                AreaType::Swamp,
                AreaType::Wasteland,
                AreaType::Haunted,
                AreaType::Cavernous,
                AreaType::Volcanic,
                AreaType::StrongholdsCities,
            ][entry["number"].as_u64().unwrap() as usize];
            assert_eq!(entry["name"], area.name());
            assert_eq!(entry["number"], area as u64);
        }
    }
}
