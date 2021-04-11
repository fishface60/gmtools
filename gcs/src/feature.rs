#![allow(clippy::upper_case_acronyms)]

use serde::{Deserialize, Serialize};

use crate::attribute_bonus::AttributeBonus;
use crate::bonus::LeveledIntegerAmount;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "compare")]
pub enum StringCriteria {
    Any,
    Is {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    IsNot {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    Contains {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    DoesNotContain {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    StartsWith {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    DoesNotStartWith {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    EndsWith {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
    DoesNotEndWith {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: String,
    },
}
impl Default for StringCriteria {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "compare")]
pub enum IntegerCriteria {
    Is {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: i64,
    },
    AtLeast {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: i64,
    },
    AtMost {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        qualifier: i64,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DRBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    // TODO: Location enum?
    pub location: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReactionBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    pub situation: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ConditionalModifier {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    pub situation: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "selection_type")]
pub enum SkillSelectionType {
    ThisWeapon,
    WeaponsWithName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        category: StringCriteria,
    },
    SkillsWithName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        category: StringCriteria,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SkillLevelBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(flatten)]
    pub selection_type: SkillSelectionType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SkillPointBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(flatten)]
    pub selection_type: SkillSelectionType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "match")]
pub enum SpellSelectionType {
    AllColleges,
    CollegeName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
    },
    PowerSourceName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
    },
    SpellName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
    },
}
impl Default for SpellSelectionType {
    fn default() -> Self {
        Self::AllColleges
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpellLevelBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(flatten)]
    pub selection_type: SpellSelectionType,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub category: StringCriteria,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpellPointBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(flatten)]
    pub selection_type: SpellSelectionType,
    #[serde(default, skip_serializing_if = "serde_skip::is_default")]
    pub category: StringCriteria,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "selection_type")]
pub enum WeaponSelectionType {
    ThisWeapon,
    WeaponsWithName {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        category: StringCriteria,
    },
    WeaponsWithRequiredSkill {
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        name: StringCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        specialization: StringCriteria,
        level: IntegerCriteria,
        #[serde(default, skip_serializing_if = "serde_skip::is_default")]
        category: StringCriteria,
    },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WeaponDamageBonus {
    #[serde(flatten)]
    pub amount: LeveledIntegerAmount,
    #[serde(flatten)]
    pub selection_type: WeaponSelectionType,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "lowercase", tag = "attribute")]
pub enum ReduceAttributeCost {
    ST { percentage: i64 },
    DX { percentage: i64 },
    IQ { percentage: i64 },
    HT { percentage: i64 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ReduceContainedWeight {
    // TODO: Serialise as either a percentage or a weight value
    pub reduction: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Feature {
    AttributeBonus(AttributeBonus),
    #[serde(rename = "dr_bonus")]
    DRBonus(DRBonus),
    ReactionBonus(ReactionBonus),
    ConditionalModifier(ConditionalModifier),
    #[serde(rename = "skill_bonus")]
    SkillLevelBonus(SkillLevelBonus),
    SkillPointBonus(SkillPointBonus),
    #[serde(rename = "spell_bonus")]
    SpellLevelBonus(SpellLevelBonus),
    SpellPointBonus(SpellPointBonus),
    #[serde(rename = "weapon_bonus")]
    WeaponDamageBonus(WeaponDamageBonus),
    #[serde(rename = "cost_reduction")]
    ReduceAttributeCost(ReduceAttributeCost),
    #[serde(rename = "contained_weight_reduction")]
    ReduceContainedWeight(ReduceContainedWeight),
}
