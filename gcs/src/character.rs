use std::collections::HashMap;

use chrono::naive::NaiveDateTime;
use serde::{
    de::{Error as DeserializeError, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_value::{Value as SerdeValue, ValueDeserializer};

use crate::advantage::AdvantageKind;
use crate::bonus::Bonuses;
use crate::date_format;
use crate::list_row::RowIdFragment;
use crate::print_settings::PrintSettings;
use crate::settings::Settings;
use crate::version_serdes::{
    VersionDeserializeWrapper, VersionSerializeWrapper,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CharacterProfile {
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub player_name: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub name: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub title: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub age: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub birthday: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub eyes: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub hair: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub skin: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub handedness: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub height: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub weight: String,
    #[serde(
        default,
        rename = "SM",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub size_modifier: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub gender: String,
    // TODO: Probably an enum
    pub body_type: String,
    // Note: Interesting that this isn't a number
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub tech_level: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub religion: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CharacterV1 {
    #[serde(flatten)]
    pub id: RowIdFragment,

    pub settings: Settings,

    #[serde(with = "date_format")]
    pub created_date: NaiveDateTime,
    #[serde(with = "date_format")]
    pub modified_date: NaiveDateTime,

    pub profile: CharacterProfile,

    #[serde(
        default,
        rename = "HP_adj",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub hp_adj: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub hp_damage: u64,

    #[serde(
        default,
        rename = "FP_adj",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub fp_adj: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub fp_damage: u64,

    pub total_points: i64,

    #[serde(rename = "ST")]
    pub strength: u64,
    #[serde(rename = "DX")]
    pub dexterity: u64,
    #[serde(rename = "IQ")]
    pub intelligence: u64,
    #[serde(rename = "HT")]
    pub health: u64,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub will_adj: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub per_adj: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub speed_adj: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub move_adj: i64,

    // "Models"
    // TODO: abstract models into flattened structure for use in templates
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub advantages: Vec<AdvantageKind>,

    // TODO: Omit print settings and third_party for hashing
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub print_settings: Option<PrintSettings>,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub third_party: HashMap<String, SerdeValue>,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

impl CharacterV1 {
    pub fn bonuses(&self) -> Bonuses {
        self.advantages.iter().map(AdvantageKind::bonuses).sum()
    }
    pub fn stats(&self) -> (i64, u64, i64, u64) {
        let bonuses = self.bonuses();
        let max_hp: u64 = ((self.strength as i64)
            + self.hp_adj
            + bonuses.strength
            + bonuses.hit_points) as u64;
        let max_fp: u64 = ((self.health as i64)
            + self.fp_adj
            + bonuses.health
            + bonuses.fatigue_points) as u64;
        let cur_hp: i64 = (max_hp as i64) - (self.hp_damage as i64);
        let cur_fp: i64 = (max_fp as i64) - (self.fp_damage as i64);
        (cur_hp, max_hp, cur_fp, max_fp)
    }
    pub fn set_hit_points(&mut self, new: i64) {
        let (_, max, _, _) = self.stats();
        self.hp_damage = (max as i64 - new) as u64;
    }
    pub fn set_fatigue_points(&mut self, new: i64) {
        let (_, _, _, max) = self.stats();
        self.fp_damage = (max as i64 - new) as u64;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Character {
    V1(CharacterV1),
}
impl<'de> Deserialize<'de> for Character {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => CharacterV1::deserialize(ValueDeserializer::<D::Error>::new(
                data.inner,
            ))
            .map(Character::V1),
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for Character {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Character::V1(ref character_v1) => VersionSerializeWrapper {
                version: 1,
                inner: character_v1,
            }
            .serialize(serializer),
        }
    }
}
