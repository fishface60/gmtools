pub mod advantage;
pub mod character;
pub mod control_roll;
pub mod date_format;
pub mod feature;
pub mod print_settings;
pub mod version_serdes;

use serde::{Deserialize, Serialize};

pub use crate::character::Character;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FileKind {
    Character(Character),
    #[serde(other)]
    Unknown,
}
