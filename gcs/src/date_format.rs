use chrono::naive::NaiveDateTime;
use serde::{Deserialize, Deserializer, Serializer};

const FORMAT: &str = "%b %-d, %Y, %-I:%M %p";

pub fn serialize<S>(
    date: &NaiveDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = format!("{}", date.format(FORMAT));
    serializer.serialize_str(&s)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<NaiveDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    NaiveDateTime::parse_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
}
