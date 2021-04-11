#![allow(clippy::upper_case_acronyms)]

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_value::Value as SerdeValue;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WeaponSTDamage {
    None,
    Thr,
    ThrLeveled,
    Sw,
    SwLeveled,
}
impl Default for WeaponSTDamage {
    fn default() -> Self {
        Self::None
    }
}

fn default_armor_divisor() -> f64 {
    1.0
}
fn is_default_armor_divisor(armor_divisor: &f64) -> bool {
    (*armor_divisor - default_armor_divisor()).abs() < f64::EPSILON
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WeaponDamage {
    // TODO: Enum
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(
        default,
        rename = "st",
        skip_serializing_if = "serde_skip::is_default"
    )]
    pub strength: WeaponSTDamage,
    // TODO: Parse base Dice
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub base: Option<String>,
    #[serde(
        default = "default_armor_divisor",
        skip_serializing_if = "is_default_armor_divisor"
    )]
    pub armor_divisor: f64,

    // TODO: Parse fragmentation Dice
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub fragmentation: Option<String>,
    #[serde(
        default = "default_armor_divisor",
        skip_serializing_if = "is_default_armor_divisor"
    )]
    pub fragmentation_armor_divisor: f64,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub fragmentation_type: Option<String>,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub modifier_per_die: i64,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

// NOTE: Skill Defaults as stored in skills also include level, adjusted_level
// and points values.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum SkillDefault {
    ST {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    DX {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    IQ {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    HT {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Will {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Per {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Skill {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Parry {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Block {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: String,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
    Base10 {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        modifier: i64,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MeleeWeapon {
    pub damage: WeaponDamage,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub strength: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub usage: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub reach: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub parry: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub block: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub defaults: Vec<SkillDefault>,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub struct RangedWeapon {
    pub damage: WeaponDamage,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub strength: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub usage: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub accuracy: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub range: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub rate_of_fire: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub shots: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub bulk: String,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub recoil: String,

    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub defaults: Vec<SkillDefault>,

    #[serde(flatten)]
    pub extra: HashMap<String, SerdeValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Weapon {
    MeleeWeapon(MeleeWeapon),
    RangedWeapon(RangedWeapon),
}
