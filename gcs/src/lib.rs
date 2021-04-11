pub mod advantage;
pub mod character;
pub mod control_roll;
pub mod date_format;
pub mod feature;
pub mod print_settings;
pub mod settings;
pub mod version_serdes;
pub mod weapon;

use serde::{Deserialize, Serialize};

pub use crate::character::Character;

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum FileKind {
    Character(Character),
    #[serde(other)]
    Unknown,
}

pub fn to_json(file: &FileKind) -> Result<Vec<u8>, serde_json::Error> {
    let mut contents: Vec<u8> = Vec::with_capacity(128);
    let fmt = serde_json::ser::PrettyFormatter::with_indent(b"\t");
    let mut ser = serde_json::Serializer::with_formatter(&mut contents, fmt);
    file.serialize(&mut ser)?;
    Ok(contents)
}
