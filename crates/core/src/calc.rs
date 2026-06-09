use crate::model::{
    ActivityLevel, DailySummary, ExerciseEntry, FoodEntry, GoalKind, GoalRecommendation,
    MacroTargets, ProfileInput, Sex,
};
use chrono::NaiveDate;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum CalculationError {
    #[error("age must be between 13 and 120")]
    InvalidAge,
    #[error("height must be between 100cm and 250cm")]
    InvalidHeight,
    #[error("weight must be between 30kg and 300kg")]
    InvalidWeight,
}

pub fn bmr(input: &ProfileInput) -> Result<u16, CalculationError> {
    validate_profile(input)?;
    let sex_adjustment = match input.sex {
        Sex::Male => 5.0,
        Sex::Female => -161.0,
    };
    let value = 10.0 * input.weight_kg + 6.25 * input.height_cm - 5.0 * input.age_years as f32
        + sex_adjustment;
    Ok(value.round().max(0.0) as u16)
}

pub fn tdee(input: &ProfileInput) -> Result<u16, CalculationError> {
    let multiplier = match input.activity_level {
        ActivityLevel::Sedentary => 1.2,
        ActivityLevel::Light => 1.375,
        ActivityLevel::Moderate => 1.55,
        ActivityLevel::Active => 1.725,
        ActivityLevel::VeryActive => 1.9,
    };
    Ok((bmr(input)? as f32 * multiplier).round() as u16)
}

pub fn recommend_goal(input: &ProfileInput) -> Result<GoalRecommendation, CalculationError> {
    let bmr_value = bmr(input)?;
    let tdee_value = tdee(input)?;
    let calorie_delta: i32 = match input.goal_kind {
        GoalKind::Lose => -400,
        GoalKind::Maintain => 0,
        GoalKind::Gain => 250,
    };
    let daily_calorie_target = (tdee_value as i32 + calorie_delta).max(1200) as u16;
    let macros = macro_targets(daily_calorie_target, input.weight_kg, input.goal_kind);

    Ok(GoalRecommendation {
        bmr: bmr_value,
        tdee: tdee_value,
        daily_calorie_target,
        macros,
    })
}

pub fn summarize_day(
    date: NaiveDate,
    foods: &[FoodEntry],
    exercises: &[ExerciseEntry],
) -> DailySummary {
    let mut summary = DailySummary {
        date,
        ..DailySummary::default()
    };

    for food in foods.iter().filter(|entry| entry.date == date) {
        summary.calories_in = summary.calories_in.saturating_add(food.calories);
        summary.protein_g += food.protein_g;
        summary.carbs_g += food.carbs_g;
        summary.fat_g += food.fat_g;
    }

    for exercise in exercises.iter().filter(|entry| entry.date == date) {
        summary.calories_burned = summary
            .calories_burned
            .saturating_add(exercise.calories_burned);
    }

    summary.net_calories = summary.calories_in as i32 - summary.calories_burned as i32;
    summary
}

fn macro_targets(calories: u16, weight_kg: f32, goal: GoalKind) -> MacroTargets {
    let protein_per_kg = match goal {
        GoalKind::Lose => 2.0,
        GoalKind::Maintain => 1.6,
        GoalKind::Gain => 1.8,
    };
    let protein_g = (weight_kg * protein_per_kg).round() as u16;
    let fat_calories = calories as f32 * 0.28;
    let fat_g = (fat_calories / 9.0).round() as u16;
    let protein_calories = protein_g as f32 * 4.0;
    let remaining = (calories as f32 - protein_calories - fat_calories).max(0.0);
    let carbs_g = (remaining / 4.0).round() as u16;

    MacroTargets {
        protein_g,
        carbs_g,
        fat_g,
    }
}

fn validate_profile(input: &ProfileInput) -> Result<(), CalculationError> {
    if !(13..=120).contains(&input.age_years) {
        return Err(CalculationError::InvalidAge);
    }
    if !(100.0..=250.0).contains(&input.height_cm) {
        return Err(CalculationError::InvalidHeight);
    }
    if !(30.0..=300.0).contains(&input.weight_kg) {
        return Err(CalculationError::InvalidWeight);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ActivityLevel, GoalKind, Sex};

    fn profile(goal_kind: GoalKind) -> ProfileInput {
        ProfileInput {
            sex: Sex::Male,
            age_years: 34,
            height_cm: 178.0,
            weight_kg: 82.0,
            activity_level: ActivityLevel::Moderate,
            goal_kind,
        }
    }

    #[test]
    fn calculates_bmr_and_tdee() {
        let input = profile(GoalKind::Maintain);
        assert_eq!(bmr(&input).unwrap(), 1768);
        assert_eq!(tdee(&input).unwrap(), 2740);
    }

    #[test]
    fn calorie_target_changes_by_goal() {
        let lose = recommend_goal(&profile(GoalKind::Lose)).unwrap();
        let maintain = recommend_goal(&profile(GoalKind::Maintain)).unwrap();
        let gain = recommend_goal(&profile(GoalKind::Gain)).unwrap();

        assert!(lose.daily_calorie_target < maintain.daily_calorie_target);
        assert!(gain.daily_calorie_target > maintain.daily_calorie_target);
        assert!(lose.macros.protein_g > maintain.macros.protein_g);
    }

    #[test]
    fn rejects_unrealistic_profile() {
        let mut input = profile(GoalKind::Maintain);
        input.age_years = 8;
        assert_eq!(bmr(&input), Err(CalculationError::InvalidAge));
    }
}
