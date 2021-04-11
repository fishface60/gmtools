#![allow(clippy::upper_case_acronyms)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LeveledIntegerAmount {
    pub amount: i64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub per_level: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LeveledDoubleAmount {
    pub amount: f64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub per_level: bool,
}
