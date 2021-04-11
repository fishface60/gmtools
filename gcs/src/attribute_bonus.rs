#![allow(clippy::upper_case_acronyms)]

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_value::{Value as SerdeValue, ValueDeserializer};

use crate::bonus::{LeveledDoubleAmount, LeveledIntegerAmount};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum STLimitation {
    None,
    LiftingOnly,
    StrikingOnly,
}
impl Default for STLimitation {
    fn default() -> Self {
        Self::None
    }
}
impl Default for &STLimitation {
    fn default() -> Self {
        &STLimitation::None
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct StrengthAmount {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub limit: STLimitation,
}
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeBonus {
    Strength(StrengthAmount),
    Dexterity(LeveledIntegerAmount),
    Intelligence(LeveledIntegerAmount),
    Health(LeveledIntegerAmount),
    Will(LeveledIntegerAmount),
    FrightCheck(LeveledIntegerAmount),
    Perception(LeveledIntegerAmount),
    Vision(LeveledIntegerAmount),
    Hearing(LeveledIntegerAmount),
    TasteSmell(LeveledIntegerAmount),
    Touch(LeveledIntegerAmount),
    Dodge(LeveledIntegerAmount),
    Parry(LeveledIntegerAmount),
    Block(LeveledIntegerAmount),
    Speed(LeveledDoubleAmount),
    Move(LeveledIntegerAmount),
    FatiguePoints(LeveledIntegerAmount),
    HitPoints(LeveledIntegerAmount),
    SizeModifier(LeveledIntegerAmount),
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum AttributeTag {
    #[serde(rename = "st")]
    Strength,
    #[serde(rename = "dx")]
    Dexterity,
    #[serde(rename = "iq")]
    Intelligence,
    #[serde(rename = "ht")]
    Health,
    Will,
    FrightCheck,
    Perception,
    Vision,
    Hearing,
    TasteSmell,
    Touch,
    Dodge,
    Parry,
    Block,
    Speed,
    Move,
    #[serde(rename = "fp")]
    FatiguePoints,
    #[serde(rename = "hp")]
    HitPoints,
    #[serde(rename = "sm")]
    SizeModifier,
}
impl<'de> Deserialize<'de> for AttributeBonus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct AttributeWrapper {
            attribute: AttributeTag,
            #[serde(flatten)]
            inner: SerdeValue,
        }
        let data = AttributeWrapper::deserialize(deserializer)?;
        let value_deserializer = ValueDeserializer::<D::Error>::new(data.inner);
        match data.attribute {
            AttributeTag::Strength => {
                StrengthAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Strength)
            }
            AttributeTag::Dexterity => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Dexterity)
            }
            AttributeTag::Intelligence => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Intelligence)
            }
            AttributeTag::Health => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Health)
            }
            AttributeTag::Will => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Will)
            }
            AttributeTag::FrightCheck => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::FrightCheck)
            }
            AttributeTag::Perception => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Perception)
            }
            AttributeTag::Vision => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Vision)
            }
            AttributeTag::Hearing => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Hearing)
            }
            AttributeTag::TasteSmell => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::TasteSmell)
            }
            AttributeTag::Touch => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Touch)
            }
            AttributeTag::Dodge => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Dodge)
            }
            AttributeTag::Parry => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Parry)
            }
            AttributeTag::Block => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Block)
            }
            AttributeTag::Speed => {
                LeveledDoubleAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Speed)
            }
            AttributeTag::Move => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::Move)
            }
            AttributeTag::FatiguePoints => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::FatiguePoints)
            }
            AttributeTag::HitPoints => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::HitPoints)
            }
            AttributeTag::SizeModifier => {
                LeveledIntegerAmount::deserialize(value_deserializer)
                    .map(AttributeBonus::SizeModifier)
            }
        }
    }
}
impl Serialize for AttributeBonus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct SerializeWrapper<A, L = ()>
        where
            L: Default + PartialEq,
        {
            #[serde(flatten)]
            amount: A,
            attribute: AttributeTag,
            #[serde(default, skip_serializing_if = "serde_skip::is_default")]
            limit: L,
        }
        match *self {
            AttributeBonus::Strength(StrengthAmount {
                ref amount,
                ref limit,
            }) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Strength,
                limit,
            }
            .serialize(serializer),
            AttributeBonus::Dexterity(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Dexterity,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Intelligence(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Intelligence,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Health(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Health,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Will(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Will,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::FrightCheck(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::FrightCheck,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Perception(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Perception,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Vision(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Vision,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Hearing(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Hearing,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::TasteSmell(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::TasteSmell,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Touch(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Touch,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Dodge(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Dodge,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Parry(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Parry,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Block(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Block,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Speed(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Speed,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::Move(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::Move,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::FatiguePoints(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::FatiguePoints,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::HitPoints(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::HitPoints,
                limit: (),
            }
            .serialize(serializer),
            AttributeBonus::SizeModifier(ref amount) => SerializeWrapper {
                amount,
                attribute: AttributeTag::SizeModifier,
                limit: (),
            }
            .serialize(serializer),
        }
    }
}
