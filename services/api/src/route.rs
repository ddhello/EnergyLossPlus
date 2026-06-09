use crate::auth::{create_session, ChallengeRecord, PasskeyService, Session, UserAccount};
use crate::dynamo::DynamoStore;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::Client;
use chrono::NaiveDate;
use energy_core::{
    recommend_goal, ExerciseEntry, FoodEntry, GoalRecommendation, ProfileInput, WeightEntry,
};
use lambda_http::{Body, Error, Request, Response};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {
    store: DynamoStore,
    passkeys: PasskeyService,
}

impl AppState {
    pub async fn from_env() -> anyhow::Result<Self> {
        let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
        let client = Client::new(&config);
        let table = std::env::var("TABLE_NAME").unwrap_or_else(|_| "EnergyLossPlus".to_string());
        Ok(Self {
            store: DynamoStore::new(client, table),
            passkeys: PasskeyService::from_env()?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegisterStartRequest {
    nickname: String,
    device_name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginStartRequest {
    nickname: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinishRequest {
    challenge_id: String,
    credential: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    session: Option<Session>,
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    foods: Vec<FoodEntry>,
    exercises: Vec<ExerciseEntry>,
    weights: Vec<WeightEntry>,
    sync_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateFoodRequest {
    date: NaiveDate,
    meal: String,
    name: String,
    calories: u16,
    protein_g: f32,
    carbs_g: f32,
    fat_g: f32,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateExerciseRequest {
    date: NaiveDate,
    name: String,
    calories_burned: u16,
    duration_minutes: u16,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateWeightRequest {
    date: NaiveDate,
    weight_kg: f32,
    note: Option<String>,
}

pub async fn handler(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let method = event.method().as_str();
    let path = event.uri().path();
    match (method, path) {
        ("OPTIONS", _) => empty_response(204),
        ("POST", "/auth/register/start") => register_start(event, state).await,
        ("POST", "/auth/register/finish") => register_finish(event, state).await,
        ("POST", "/auth/login/start") => login_start(event, state).await,
        ("POST", "/auth/login/finish") => login_finish(event, state).await,
        ("GET", "/snapshot") => snapshot(event, state).await,
        ("PUT", "/goal") => update_goal(event, state).await,
        ("POST", "/foods") => create_food(event, state).await,
        ("POST", "/exercises") => create_exercise(event, state).await,
        ("POST", "/weights") => create_weight(event, state).await,
        ("PUT", _) if path_param(path, "/foods/").is_some() => update_food(event, state).await,
        ("DELETE", _) if path_param(path, "/foods/").is_some() => delete_food(event, state).await,
        ("PUT", _) if path_param(path, "/exercises/").is_some() => {
            update_exercise(event, state).await
        }
        ("DELETE", _) if path_param(path, "/exercises/").is_some() => {
            delete_exercise(event, state).await
        }
        ("PUT", _) if path_param(path, "/weights/").is_some() => update_weight(event, state).await,
        ("DELETE", _) if path_param(path, "/weights/").is_some() => {
            delete_weight(event, state).await
        }
        _ => json_response(404, &serde_json::json!({ "error": "not found" })),
    }
}

async fn register_start(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: RegisterStartRequest = json_body(&event)?;
    if input.nickname.trim().is_empty() || input.device_name.trim().is_empty() {
        return json_response(
            400,
            &serde_json::json!({ "error": "nickname and deviceName are required" }),
        );
    }
    if state
        .store
        .get_json::<String>(&nickname_pk(&input.nickname), "user")
        .await?
        .is_some()
    {
        return json_response(
            409,
            &serde_json::json!({ "error": "nickname already registered" }),
        );
    }

    let (record, challenge) =
        state
            .passkeys
            .start_registration(input.nickname, input.device_name, &[])?;
    state
        .store
        .put_json("challenge", &record.challenge_id, &record)
        .await?;
    json_response(200, &challenge)
}

async fn register_finish(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: FinishRequest = json_body(&event)?;
    let Some(record): Option<ChallengeRecord> = state
        .store
        .get_json("challenge", &input.challenge_id)
        .await?
    else {
        return json_response(400, &serde_json::json!({ "error": "unknown challenge" }));
    };
    let account = match state
        .passkeys
        .finish_registration(input.credential, record.clone())
    {
        Ok(account) => account,
        Err(error) => {
            return json_response(400, &serde_json::json!({ "error": error.to_string() }))
        }
    };
    let session = create_session(&account);

    state
        .store
        .put_json(&user_pk(&account.user_id.to_string()), "account", &account)
        .await?;
    state
        .store
        .put_json(
            &nickname_pk(&account.nickname),
            "user",
            &account.user_id.to_string(),
        )
        .await?;
    state
        .store
        .put_json("token", &session.token, &session)
        .await?;
    state
        .store
        .delete("challenge", &record.challenge_id)
        .await?;

    json_response(200, &session)
}

async fn login_start(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: LoginStartRequest = json_body(&event)?;
    let Some(user_id): Option<String> = state
        .store
        .get_json(&nickname_pk(&input.nickname), "user")
        .await?
    else {
        return json_response(401, &serde_json::json!({ "error": "unknown passkey user" }));
    };
    let Some(account): Option<UserAccount> =
        state.store.get_json(&user_pk(&user_id), "account").await?
    else {
        return json_response(401, &serde_json::json!({ "error": "unknown passkey user" }));
    };
    let (record, challenge) = state.passkeys.start_login(&account)?;
    state
        .store
        .put_json("challenge", &record.challenge_id, &record)
        .await?;
    json_response(200, &challenge)
}

async fn login_finish(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: FinishRequest = json_body(&event)?;
    let Some(record): Option<ChallengeRecord> = state
        .store
        .get_json("challenge", &input.challenge_id)
        .await?
    else {
        return json_response(400, &serde_json::json!({ "error": "unknown challenge" }));
    };
    let Some(account): Option<UserAccount> = state
        .store
        .get_json(&user_pk(&record.user_id.to_string()), "account")
        .await?
    else {
        return json_response(401, &serde_json::json!({ "error": "unknown passkey user" }));
    };
    let account = match state
        .passkeys
        .finish_login(input.credential, record.clone(), account)
    {
        Ok(account) => account,
        Err(error) => {
            return json_response(400, &serde_json::json!({ "error": error.to_string() }))
        }
    };
    let session = create_session(&account);

    state
        .store
        .put_json(&user_pk(&account.user_id.to_string()), "account", &account)
        .await?;
    state
        .store
        .put_json("token", &session.token, &session)
        .await?;
    state
        .store
        .delete("challenge", &record.challenge_id)
        .await?;

    json_response(200, &session)
}

async fn snapshot(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let payload = load_snapshot(&state, &session).await?;
    json_response(200, &payload)
}

async fn update_goal(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let profile: ProfileInput = json_body(&event)?;
    let recommendation = match recommend_goal(&profile) {
        Ok(recommendation) => recommendation,
        Err(error) => {
            return json_response(400, &serde_json::json!({ "error": error.to_string() }))
        }
    };
    let mut payload = load_snapshot(&state, &session).await?;
    payload.profile = profile;
    payload.recommendation = Some(recommendation);
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn create_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateFoodRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    payload.foods.push(FoodEntry {
        id: uuid::Uuid::new_v4(),
        user_id: session.user_id.clone(),
        date: input.date,
        meal: input.meal,
        name: input.name,
        calories: input.calories,
        protein_g: input.protein_g,
        carbs_g: input.carbs_g,
        fat_g: input.fat_g,
        note: input.note,
    });
    save_snapshot(&state, &session, &payload).await?;
    json_response(201, &payload)
}

async fn create_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateExerciseRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    payload.exercises.push(ExerciseEntry {
        id: uuid::Uuid::new_v4(),
        user_id: session.user_id.clone(),
        date: input.date,
        name: input.name,
        calories_burned: input.calories_burned,
        duration_minutes: input.duration_minutes,
        note: input.note,
    });
    save_snapshot(&state, &session, &payload).await?;
    json_response(201, &payload)
}

async fn create_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateWeightRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    payload.weights.push(WeightEntry {
        id: uuid::Uuid::new_v4(),
        user_id: session.user_id.clone(),
        date: input.date,
        weight_kg: input.weight_kg,
        note: input.note,
    });
    save_snapshot(&state, &session, &payload).await?;
    json_response(201, &payload)
}

async fn update_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/foods/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateFoodRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    let Some(entry) = payload
        .foods
        .iter_mut()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(404, &serde_json::json!({ "error": "food entry not found" }));
    };
    *entry = FoodEntry {
        id: entry.id,
        user_id: session.user_id.clone(),
        date: input.date,
        meal: input.meal,
        name: input.name,
        calories: input.calories,
        protein_g: input.protein_g,
        carbs_g: input.carbs_g,
        fat_g: input.fat_g,
        note: input.note,
    };
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn delete_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/foods/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let mut payload = load_snapshot(&state, &session).await?;
    let before = payload.foods.len();
    payload.foods.retain(|entry| entry.id.to_string() != id);
    if payload.foods.len() == before {
        return json_response(404, &serde_json::json!({ "error": "food entry not found" }));
    }
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn update_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/exercises/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateExerciseRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    let Some(entry) = payload
        .exercises
        .iter_mut()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "exercise entry not found" }),
        );
    };
    *entry = ExerciseEntry {
        id: entry.id,
        user_id: session.user_id.clone(),
        date: input.date,
        name: input.name,
        calories_burned: input.calories_burned,
        duration_minutes: input.duration_minutes,
        note: input.note,
    };
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn delete_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/exercises/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let mut payload = load_snapshot(&state, &session).await?;
    let before = payload.exercises.len();
    payload.exercises.retain(|entry| entry.id.to_string() != id);
    if payload.exercises.len() == before {
        return json_response(
            404,
            &serde_json::json!({ "error": "exercise entry not found" }),
        );
    }
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn update_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/weights/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let input: CreateWeightRequest = json_body(&event)?;
    let mut payload = load_snapshot(&state, &session).await?;
    let Some(entry) = payload
        .weights
        .iter_mut()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "weight entry not found" }),
        );
    };
    *entry = WeightEntry {
        id: entry.id,
        user_id: session.user_id.clone(),
        date: input.date,
        weight_kg: input.weight_kg,
        note: input.note,
    };
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn delete_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/weights/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let Some(session) = authenticated_session(&event, &state).await? else {
        return json_response(401, &serde_json::json!({ "error": "invalid bearer token" }));
    };
    let mut payload = load_snapshot(&state, &session).await?;
    let before = payload.weights.len();
    payload.weights.retain(|entry| entry.id.to_string() != id);
    if payload.weights.len() == before {
        return json_response(
            404,
            &serde_json::json!({ "error": "weight entry not found" }),
        );
    }
    save_snapshot(&state, &session, &payload).await?;
    json_response(200, &payload)
}

async fn authenticated_session(
    event: &Request,
    state: &AppState,
) -> Result<Option<Session>, Error> {
    let Some(token) = bearer_token(event) else {
        return Ok(None);
    };
    let Some(session): Option<Session> = state.store.get_json("token", token).await? else {
        return Ok(None);
    };
    if session_is_expired(&session) {
        state.store.delete("token", token).await?;
        return Ok(None);
    }
    Ok(Some(session))
}

async fn load_snapshot(state: &AppState, session: &Session) -> Result<Snapshot, Error> {
    if let Some(snapshot) = state
        .store
        .get_json::<Snapshot>(&user_pk(&session.user_id), "snapshot")
        .await?
    {
        return Ok(Snapshot {
            session: Some(session.clone()),
            sync_status: "online".to_string(),
            ..snapshot
        });
    }
    Ok(default_snapshot(session))
}

async fn save_snapshot(
    state: &AppState,
    session: &Session,
    snapshot: &Snapshot,
) -> Result<(), Error> {
    state
        .store
        .put_json(&user_pk(&session.user_id), "snapshot", snapshot)
        .await?;
    Ok(())
}

fn default_snapshot(session: &Session) -> Snapshot {
    let profile = ProfileInput {
        sex: energy_core::Sex::Male,
        age_years: 34,
        height_cm: 178.0,
        weight_kg: 82.0,
        activity_level: energy_core::ActivityLevel::Moderate,
        goal_kind: energy_core::GoalKind::Lose,
    };
    Snapshot {
        session: Some(session.clone()),
        recommendation: recommend_goal(&profile).ok(),
        profile,
        foods: vec![],
        exercises: vec![],
        weights: vec![],
        sync_status: "online".to_string(),
    }
}

fn json_body<T>(event: &Request) -> Result<T, Error>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = match event.body() {
        Body::Text(text) => text.as_bytes().to_vec(),
        Body::Binary(bytes) => bytes.clone(),
        Body::Empty => Vec::new(),
    };
    Ok(serde_json::from_slice(&bytes)?)
}

fn bearer_token(event: &Request) -> Option<&str> {
    event
        .headers()
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
}

fn json_response<T>(status: u16, value: &T) -> Result<Response<Body>, Error>
where
    T: Serialize,
{
    Ok(Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .header("access-control-allow-origin", cors_origin())
        .header("access-control-allow-headers", "Content-Type, Authorization")
        .header(
            "access-control-allow-methods",
            "OPTIONS, GET, POST, PUT, DELETE",
        )
        .body(serde_json::to_string(value)?.into())?)
}

fn empty_response(status: u16) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("access-control-allow-origin", cors_origin())
        .header("access-control-allow-headers", "Content-Type, Authorization")
        .header(
            "access-control-allow-methods",
            "OPTIONS, GET, POST, PUT, DELETE",
        )
        .body(Body::Empty)?)
}

fn cors_origin() -> String {
    std::env::var("WEBAUTHN_ORIGIN").unwrap_or_else(|_| "http://localhost:1420".to_string())
}

fn nickname_pk(nickname: &str) -> String {
    format!("nickname#{}", nickname.trim().to_lowercase())
}

fn user_pk(user_id: &str) -> String {
    format!("user#{user_id}")
}

fn session_is_expired(session: &Session) -> bool {
    let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(&session.expires_at) else {
        return true;
    };
    expires_at < chrono::Utc::now()
}

fn path_param<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let value = path.strip_prefix(prefix)?;
    if value.is_empty() || value.contains('/') {
        return None;
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn session_with_expiry(expires_at: String) -> Session {
        Session {
            token: "token".to_string(),
            user_id: "user-id".to_string(),
            nickname: "tester".to_string(),
            device_name: "desktop".to_string(),
            expires_at,
        }
    }

    #[test]
    fn extracts_single_segment_path_params() {
        assert_eq!(path_param("/foods/abc", "/foods/"), Some("abc"));
        assert_eq!(path_param("/foods/", "/foods/"), None);
        assert_eq!(path_param("/foods/abc/def", "/foods/"), None);
        assert_eq!(path_param("/exercises/abc", "/foods/"), None);
    }

    #[test]
    fn treats_expired_or_invalid_sessions_as_expired() {
        let active = session_with_expiry((Utc::now() + Duration::minutes(5)).to_rfc3339());
        let expired = session_with_expiry((Utc::now() - Duration::minutes(5)).to_rfc3339());
        let invalid = session_with_expiry("not-a-date".to_string());

        assert!(!session_is_expired(&active));
        assert!(session_is_expired(&expired));
        assert!(session_is_expired(&invalid));
    }
}
