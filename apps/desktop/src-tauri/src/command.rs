use crate::api::ApiClient;
use crate::cache::{Cache, CachedSnapshot, Session};
use chrono::NaiveDate;
use energy_core::{recommend_goal, GoalRecommendation, ProfileInput};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    Calculation(String),
    #[error("{0}")]
    Cache(String),
    #[error("{0}")]
    Network(String),
}

impl Serialize for CommandError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[tauri::command]
pub fn calculate_goal(profile: ProfileInput) -> Result<GoalRecommendation, CommandError> {
    recommend_goal(&profile).map_err(|error| CommandError::Calculation(error.to_string()))
}

#[tauri::command]
pub fn load_cached_snapshot(app: AppHandle) -> Result<CachedSnapshot, CommandError> {
    cache(&app)?.load_snapshot().map_err(to_cache_error)
}

#[tauri::command]
pub async fn sync_snapshot(app: AppHandle, token: String) -> Result<CachedSnapshot, CommandError> {
    let api = ApiClient::from_env();
    let snapshot = api.snapshot(&token).await.map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub fn save_session(app: AppHandle, session: Session) -> Result<(), CommandError> {
    cache(&app)?.save_session(&session).map_err(to_cache_error)
}

#[tauri::command]
pub fn clear_session(app: AppHandle) -> Result<(), CommandError> {
    cache(&app)?.clear_session().map_err(to_cache_error)
}

#[tauri::command]
pub async fn auth_post(
    path: String,
    body: serde_json::Value,
) -> Result<serde_json::Value, CommandError> {
    const ALLOWED_PATHS: [&str; 5] = [
        "/auth/register/start",
        "/auth/register/finish",
        "/auth/login/start",
        "/auth/login/finish",
        "/auth/app/exchange",
    ];
    if !ALLOWED_PATHS.contains(&path.as_str()) {
        return Err(CommandError::Network(
            "unsupported Passkey API path".to_string(),
        ));
    }

    ApiClient::from_env()
        .auth_post(&path, &body)
        .await
        .map_err(to_network_error)
}

#[tauri::command]
pub async fn update_goal(
    app: AppHandle,
    token: String,
    profile: ProfileInput,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .update_goal(&token, &profile)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn update_daily_target(
    app: AppHandle,
    token: String,
    daily_calorie_target: u16,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .update_daily_target(&token, daily_calorie_target)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn create_food(
    app: AppHandle,
    token: String,
    entry: CreateFoodRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .create_food(&token, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn update_food(
    app: AppHandle,
    token: String,
    id: String,
    entry: CreateFoodRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .update_food(&token, &id, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn delete_food(
    app: AppHandle,
    token: String,
    id: String,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .delete_food(&token, &id)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn create_exercise(
    app: AppHandle,
    token: String,
    entry: CreateExerciseRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .create_exercise(&token, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn update_exercise(
    app: AppHandle,
    token: String,
    id: String,
    entry: CreateExerciseRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .update_exercise(&token, &id, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn delete_exercise(
    app: AppHandle,
    token: String,
    id: String,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .delete_exercise(&token, &id)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn create_weight(
    app: AppHandle,
    token: String,
    entry: CreateWeightRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .create_weight(&token, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn update_weight(
    app: AppHandle,
    token: String,
    id: String,
    entry: CreateWeightRequest,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .update_weight(&token, &id, &entry)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

#[tauri::command]
pub async fn delete_weight(
    app: AppHandle,
    token: String,
    id: String,
) -> Result<CachedSnapshot, CommandError> {
    let snapshot = ApiClient::from_env()
        .delete_weight(&token, &id)
        .await
        .map_err(to_network_error)?;
    cache(&app)?
        .save_snapshot(&snapshot)
        .map_err(to_cache_error)?;
    Ok(snapshot)
}

fn cache(app: &AppHandle) -> Result<Cache, CommandError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|error| CommandError::Cache(error.to_string()))?;
    std::fs::create_dir_all(&dir).map_err(|error| CommandError::Cache(error.to_string()))?;
    Cache::open(dir.join("energy-loss-plus.sqlite")).map_err(to_cache_error)
}

fn to_cache_error(error: anyhow::Error) -> CommandError {
    CommandError::Cache(error.to_string())
}

fn to_network_error(error: anyhow::Error) -> CommandError {
    CommandError::Network(format!("{error:#}"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateFoodRequest {
    pub date: NaiveDate,
    pub meal: String,
    pub name: String,
    pub calories: u16,
    pub protein_g: f32,
    pub carbs_g: f32,
    pub fat_g: f32,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExerciseRequest {
    pub date: NaiveDate,
    pub name: String,
    pub calories_burned: u16,
    pub duration_minutes: u16,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWeightRequest {
    pub date: NaiveDate,
    pub weight_kg: f32,
    pub note: Option<String>,
}
