use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Sex {
    Female,
    Male,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ActivityLevel {
    Sedentary,
    Light,
    Moderate,
    Active,
    VeryActive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum GoalKind {
    Lose,
    Maintain,
    Gain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub sex: Sex,
    pub age_years: u16,
    pub height_cm: f32,
    pub weight_kg: f32,
    pub activity_level: ActivityLevel,
    pub goal_kind: GoalKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MacroTargets {
    pub protein_g: u16,
    pub carbs_g: u16,
    pub fat_g: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GoalRecommendation {
    pub bmr: u16,
    pub tdee: u16,
    pub daily_calorie_target: u16,
    pub macros: MacroTargets,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct UserGoal {
    pub user_id: String,
    pub profile: ProfileInput,
    pub recommendation: GoalRecommendation,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FoodEntry {
    pub id: Uuid,
    pub user_id: String,
    pub date: NaiveDate,
    pub meal: String,
    pub name: String,
    pub calories: u16,
    pub protein_g: f32,
    pub carbs_g: f32,
    pub fat_g: f32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExerciseEntry {
    pub id: Uuid,
    pub user_id: String,
    pub date: NaiveDate,
    pub name: String,
    pub calories_burned: u16,
    pub duration_minutes: u16,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WeightEntry {
    pub id: Uuid,
    pub user_id: String,
    pub date: NaiveDate,
    pub weight_kg: f32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct DailySummary {
    pub date: NaiveDate,
    pub calories_in: u16,
    pub calories_burned: u16,
    pub net_calories: i32,
    pub protein_g: f32,
    pub carbs_g: f32,
    pub fat_g: f32,
}
