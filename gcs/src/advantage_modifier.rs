use std::collections::HashMap;

use serde::{
    de::{Error as DeserializeError, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_value::{Value as SerdeValue, ValueDeserializer};

use crate::feature::Feature;
use crate::list_row::RowIdFragment;
use crate::version_serdes::{
    VersionDeserializeWrapper, VersionSerializeWrapper,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "cost_type")]
pub enum AdvantageModifierCost {
    // TODO: enum for affects
    Percentage { cost: i64, affects: String },
    Points { cost: i64, affects: String },
    Multiplier { cost: f64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AdvantageModifierV1 {
    #[serde(flatten)]
    pub id: RowIdFragment,

    // Modifier section
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub disabled: bool,
    pub name: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub reference: String,

    // AdvantageModifier section
    #[serde(flatten)]
    pub cost: AdvantageModifierCost,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub levels: i64,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub features: Vec<Feature>,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub notes: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AdvantageModifierContainerV1 {
    #[serde(flatten)]
    pub id: RowIdFragment,

    // Modifier section
    pub name: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub reference: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub notes: String,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AdvantageModifier {
    V1(AdvantageModifierV1),
}
impl<'de> Deserialize<'de> for AdvantageModifier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => AdvantageModifierV1::deserialize(
                ValueDeserializer::<D::Error>::new(data.inner),
            )
            .map(AdvantageModifier::V1),
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for AdvantageModifier {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            AdvantageModifier::V1(ref modifier_v1) => VersionSerializeWrapper {
                version: 1,
                inner: modifier_v1,
            }
            .serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum AdvantageModifierContainer {
    V1(AdvantageModifierContainerV1),
}
impl<'de> Deserialize<'de> for AdvantageModifierContainer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => {
                AdvantageModifierContainerV1::deserialize(ValueDeserializer::<
                    D::Error,
                >::new(
                    data.inner
                ))
                .map(AdvantageModifierContainer::V1)
            }
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for AdvantageModifierContainer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            AdvantageModifierContainer::V1(ref modifier_v1) => {
                VersionSerializeWrapper {
                    version: 1,
                    inner: modifier_v1,
                }
                .serialize(serializer)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum AdvantageModifierKind {
    Modifier(AdvantageModifier),
    ModifierContainer(AdvantageModifierContainer),
}
