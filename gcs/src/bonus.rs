#![allow(clippy::upper_case_acronyms)]

use std::collections::HashMap;
use std::iter::Sum;

use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct Bonuses {
    pub strength: i64,
    pub hit_points: i64,
    pub health: i64,
    pub fatigue_points: i64,
}

impl Bonuses {
    pub fn with_strength(strength: i64) -> Self {
        Self {
            strength,
            ..Default::default()
        }
    }
    pub fn with_hit_points(hit_points: i64) -> Self {
        Self {
            hit_points,
            ..Default::default()
        }
    }
    pub fn with_health(health: i64) -> Self {
        Self {
            health,
            ..Default::default()
        }
    }
    pub fn with_fatigue_points(fatigue_points: i64) -> Self {
        Self {
            fatigue_points,
            ..Default::default()
        }
    }

    fn steal(&mut self, mut other: Self) {
        self.strength += other.strength;
        self.hit_points += other.hit_points;
        self.health += other.health;
        self.fatigue_points += other.fatigue_points;
    }
}

impl Sum for Bonuses {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Bonuses::default(), |mut acc, x| {
            acc.steal(x);
            acc
        })
    }
}

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
