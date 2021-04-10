use serde::{Deserialize, Serialize};

fn default_number_up() -> u64 {
    1
}
fn is_default_number_up(number_up: &u64) -> bool {
    *number_up == default_number_up()
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PrintSettings {
    // TODO: length units enum
    pub units: String,
    // TODO: Page orientation enum
    pub orientation: String,
    pub width: f64,
    pub height: f64,
    pub top_margin: f64,
    pub left_margin: f64,
    pub bottom_margin: f64,
    pub right_margin: f64,
    // TODO: Ink Chromacity enum
    pub ink_chromaticity: String,
    // TODO: page print sides enum
    pub sides: String,
    #[serde(
        default = "default_number_up",
        skip_serializing_if = "is_default_number_up"
    )]
    pub number_up: u64,
    // TODO: Quality enum
    pub quality: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub resolution: Option<String>,
}
