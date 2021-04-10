use std::collections::HashMap;

use serde::{
    de::{Error as DeserializeError, Unexpected},
    Deserialize, Deserializer, Serialize, Serializer,
};
use serde_value::{Value as SerdeValue, ValueDeserializer};

use crate::version_serdes::{
    VersionDeserializeWrapper, VersionSerializeWrapper,
};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SettingsV1 {
    pub default_length_units: String,
    pub default_weight_units: String,
    pub user_description_display: String,
    pub modifiers_display: String,
    pub notes_display: String,
    pub base_will_on_10: bool,
    pub base_per_on_10: bool,
    pub use_multiplicative_modifiers: bool,
    pub use_modifying_dice_plus_adds: bool,
    pub use_know_your_own_strength: bool,
    pub use_reduced_swing: bool,
    pub use_thrust_equals_swing_minus_2: bool,
    pub use_simple_metric_conversions: bool,
    pub show_college_in_sheet_spells: bool,
    pub show_difficulty: bool,
    pub show_advantage_modifier_adj: bool,
    pub show_equipment_modifier_adj: bool,
    pub show_spell_adj: bool,
    pub use_title_in_footer: bool,
    pub extra_space_around_encumbrance: bool,
    pub block_layout: Vec<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Settings {
    V1(SettingsV1),
}
impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let data = VersionDeserializeWrapper::deserialize(deserializer)?;
        match data.version {
            1 => SettingsV1::deserialize(ValueDeserializer::<D::Error>::new(
                data.inner,
            ))
            .map(Settings::V1),
            value => Err(DeserializeError::invalid_value(
                Unexpected::Unsigned(value),
                &"version number 0 < i <= 1",
            )),
        }
    }
}
impl Serialize for Settings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            Settings::V1(ref settings_v1) => VersionSerializeWrapper {
                version: 1,
                inner: settings_v1,
            }
            .serialize(serializer),
        }
    }
}
