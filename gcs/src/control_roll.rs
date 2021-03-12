use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub enum ControlRoll {
    NotApplicable,
    Rarely,
    FairlyOften,
    QuiteOften,
    AlmostAlways,
    NoneRequired,
}
impl From<&ControlRoll> for u64 {
    fn from(value: &ControlRoll) -> Self {
        match value {
            ControlRoll::NotApplicable => 0,
            ControlRoll::Rarely => 6,
            ControlRoll::FairlyOften => 9,
            ControlRoll::QuiteOften => 12,
            ControlRoll::AlmostAlways => 15,
            ControlRoll::NoneRequired => u64::MAX,
        }
    }
}
impl From<u64> for ControlRoll {
    fn from(value: u64) -> Self {
        match value {
            0 => ControlRoll::NotApplicable,
            6 => ControlRoll::Rarely,
            9 => ControlRoll::FairlyOften,
            12 => ControlRoll::QuiteOften,
            15 => ControlRoll::AlmostAlways,
            _ => ControlRoll::NoneRequired,
        }
    }
}
impl Default for ControlRoll {
    fn default() -> Self {
        ControlRoll::NoneRequired
    }
}
impl<'de> serde::Deserialize<'de> for ControlRoll {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(u64::deserialize(deserializer)?.into())
    }
}
impl serde::Serialize for ControlRoll {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        u64::from(self).serialize(serializer)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRollAdjust {
    None,
    ActionPenalty,
    ReactionPenalty,
    FrightCheckPenalty,
    FrightCheckBonus,
    MinorCostOfLivingIncrease,
    MajorCostOfLivingIncrease,
}
impl Default for ControlRollAdjust {
    fn default() -> Self {
        ControlRollAdjust::None
    }
}
