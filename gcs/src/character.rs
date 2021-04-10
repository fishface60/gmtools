use std::collections::HashMap;

use chrono::naive::NaiveDateTime;
use serde::{
    de::{Error as DeserializeError, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_value::{Value as SerdeValue, ValueDeserializer};
use uuid::Uuid;

use crate::advantage::AdvantageKind;
use crate::date_format;
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
    pub id: Uuid,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub based_on_id: Option<Uuid>,
    // Is a base64-encoded SHA3-256 hash of the object
    // as stored as unindented compact JSON,
    // omitting the print settings and third party data
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub based_on_hash: Option<String>,

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
    pub fp_adj: u64,
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

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub print_settings: Option<PrintSettings>,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub third_party: HashMap<String, SerdeValue>,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

impl CharacterV1 {
    pub fn get_hit_points(&self) -> (i64, u64) {
        use crate::advantage::*;
        use crate::feature::*;
        fn get_advantage_bonuses(advantage: &AdvantageKind) -> (i64, i64) {
            match advantage {
                AdvantageKind::Advantage(Advantage::V1(ref advantage)) => {
                    if advantage.disabled {
                        return (0, 0);
                    }
                    let levels: f64 = match advantage.levels {
                        None => 0f64,
                        Some(ref s) => {
                            if advantage.allow_half_levels {
                                s.parse::<f64>().expect("real string")
                            } else {
                                s.parse::<i64>().expect("integer string") as f64
                            }
                        }
                    };

                    advantage
                        .features
                        .iter()
                        .map(|f| match f {
                            Feature::AttributeBonus(
                                AttributeBonus::Strength(ref amount),
                            ) => {
                                match amount.limit {
                                    STLimitation::None => (),
                                    _ => return (0, 0),
                                }
                                (
                                    ((amount.amount.amount as f64)
                                        * if amount.amount.per_level {
                                            levels
                                        } else {
                                            1f64
                                        })
                                        as i64,
                                    0,
                                )
                            }
                            Feature::AttributeBonus(
                                AttributeBonus::HitPoints(ref amount),
                            ) => (
                                0,
                                ((amount.amount as f64)
                                    * if amount.per_level {
                                        levels
                                    } else {
                                        1f64
                                    }) as i64,
                            ),
                            _ => (0, 0),
                        })
                        .fold((0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1))
                }
                AdvantageKind::AdvantageContainer(AdvantageContainer::V1(
                    ref advantage_container,
                )) => {
                    if advantage_container.disabled {
                        return (0, 0);
                    }
                    advantage_container
                        .children
                        .iter()
                        .map(get_advantage_bonuses)
                        .fold((0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1))
                }
                _ => panic!("Don't know how to get bonuses from unknown kind"),
            }
        }
        let (st_bonus, hp_bonus) = self
            .advantages
            .iter()
            .map(get_advantage_bonuses)
            .fold((0, 0), |acc, x| (acc.0 + x.0, acc.1 + x.1));
        let max: u64 =
            ((self.strength as i64) + self.hp_adj + st_bonus + hp_bonus) as u64;
        let cur = (max as i64) - (self.hp_damage as i64);
        (cur, max)
    }
    pub fn set_hit_points(&mut self, new: i64) {
        let (_, max) = self.get_hit_points();
        self.hp_damage = (max as i64 - new) as u64;
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
