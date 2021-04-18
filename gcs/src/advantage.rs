use std::collections::HashMap;

use serde::{
    de::{Error as DeserializeError, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_value::ValueDeserializer;

use crate::advantage_modifier::AdvantageModifierKind;
use crate::bonus::Bonuses;
use crate::control_roll::{ControlRoll, ControlRollAdjust};
use crate::feature::Feature;
use crate::list_row::RowIdFragment;
use crate::version_serdes::{
    VersionDeserializeWrapper, VersionSerializeWrapper,
};
use crate::weapon::Weapon;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AdvantageV1 {
    #[serde(flatten)]
    pub id: RowIdFragment,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub round_down: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub allow_half_levels: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub disabled: bool,
    pub name: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub mental: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub physical: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub social: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub exotic: bool,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub supernatural: bool,

    // Levels are stored as a string so that half-levels can be x.5
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub levels: Option<String>,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub base_points: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub points_per_level: i64,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub weapons: Vec<Weapon>,
    #[serde(
        default,
        rename = "cr",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub control_roll: ControlRoll,
    #[serde(
        default,
        rename = "cr_adj",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub control_roll_adjust: ControlRollAdjust,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub modifiers: Vec<AdvantageModifierKind>,
    #[serde(
        default,
        rename = "userdesc",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub user_description: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub reference: String,

    // TODO: prereqs
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub features: Vec<Feature>,
    //[{type: attribute_bonus, amount: -1, per_level: true, attribute:
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub notes: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub categories: Vec<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_value::Value>,
}
impl AdvantageV1 {
    pub fn bonuses(&self) -> Bonuses {
        if self.disabled {
            return Default::default();
        }
        let levels: f64 = match self.levels {
            None => 0f64,
            Some(ref s) => {
                if self.allow_half_levels {
                    s.parse::<f64>().expect("real string")
                } else {
                    s.parse::<i64>().expect("integer string") as f64
                }
            }
        };
        self.features.iter().map(|f| f.bonuses(levels)).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Advantage {
    V1(AdvantageV1),
}
impl Advantage {
    fn bonuses(&self) -> Bonuses {
        match self {
            Advantage::V1(ref advantage) => advantage.bonuses(),
        }
    }
}
impl<'de> Deserialize<'de> for Advantage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => AdvantageV1::deserialize(ValueDeserializer::<D::Error>::new(
                data.inner,
            ))
            .map(Advantage::V1),
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for Advantage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Advantage::V1(ref advantage_v1) => VersionSerializeWrapper {
                version: 1,
                inner: advantage_v1,
            }
            .serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AdvantageContainerV1 {
    #[serde(flatten)]
    pub id: RowIdFragment,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub disabled: bool,
    // TODO: Container type
    pub name: String,

    #[serde(
        default,
        rename = "cr",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub control_roll: ControlRoll,
    #[serde(
        default,
        rename = "cr_adj",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub control_roll_adjust: ControlRollAdjust,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub modifiers: Vec<AdvantageModifierKind>,
    #[serde(
        default,
        rename = "userdesc",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub user_description: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub reference: String,

    // TODO: prereqs
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub notes: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub categories: Vec<String>,

    // TODO: When hashing omit this
    pub open: bool,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub children: Vec<AdvantageKind>,

    #[serde(flatten)]
    pub extra: HashMap<String, serde_value::Value>,
}
impl AdvantageContainerV1 {
    fn bonuses(&self) -> Bonuses {
        if self.disabled {
            return Default::default();
        }
        self.children.iter().map(AdvantageKind::bonuses).sum()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AdvantageContainer {
    V1(AdvantageContainerV1),
}
impl AdvantageContainer {
    fn bonuses(&self) -> Bonuses {
        match self {
            AdvantageContainer::V1(ref container) => container.bonuses(),
        }
    }
}
impl<'de> Deserialize<'de> for AdvantageContainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => AdvantageContainerV1::deserialize(
                ValueDeserializer::<D::Error>::new(data.inner),
            )
            .map(AdvantageContainer::V1),
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for AdvantageContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            AdvantageContainer::V1(ref advantage_container_v1) => {
                VersionSerializeWrapper {
                    version: 1,
                    inner: advantage_container_v1,
                }
                .serialize(serializer)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AdvantageKind {
    Advantage(Advantage),
    AdvantageContainer(AdvantageContainer),
    #[serde(other)]
    Unknown,
}

impl AdvantageKind {
    pub fn bonuses(&self) -> Bonuses {
        match self {
            AdvantageKind::Advantage(ref advantage) => advantage.bonuses(),
            AdvantageKind::AdvantageContainer(ref container) => {
                container.bonuses()
            }
            _ => Default::default(),
        }
    }
}
