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

#[derive(Debug, thiserror::Error)]
#[error("invalid bearer token")]
struct InvalidBearerToken;

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppFinishRequest {
    challenge_id: String,
    credential: serde_json::Value,
    state: String,
    code_challenge: String,
    callback_origin: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppExchangeRequest {
    code: String,
    state: String,
    code_verifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppAuthorizationCode {
    session: Session,
    state: String,
    code_challenge: String,
    expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    session: Option<Session>,
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    #[serde(default)]
    daily_calorie_target: Option<u16>,
    foods: Vec<FoodEntry>,
    exercises: Vec<ExerciseEntry>,
    weights: Vec<WeightEntry>,
    sync_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserState {
    storage_version: u8,
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    #[serde(default)]
    daily_calorie_target: Option<u16>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Bootstrap {
    session: Session,
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    daily_calorie_target: Option<u16>,
    sync_status: String,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiaryMonth {
    foods: Vec<FoodEntry>,
    exercises: Vec<ExerciseEntry>,
    weights: Vec<WeightEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "record", rename_all = "camelCase")]
enum DiaryRecord {
    Food(FoodEntry),
    Exercise(ExerciseEntry),
    Weight(WeightEntry),
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
struct UpdateDailyTargetRequest {
    daily_calorie_target: u16,
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
    let response = match (method, path) {
        ("OPTIONS", _) => empty_response(204),
        ("GET", "/auth/app") => html_response(200, app_auth_html()),
        ("POST", "/auth/app/register/finish") => app_register_finish(event, state).await,
        ("POST", "/auth/app/login/finish") => app_login_finish(event, state).await,
        ("POST", "/auth/app/exchange") => app_exchange(event, state).await,
        ("POST", "/auth/register/start") => register_start(event, state).await,
        ("POST", "/auth/register/finish") => register_finish(event, state).await,
        ("POST", "/auth/login/start") => login_start(event, state).await,
        ("POST", "/auth/login/finish") => login_finish(event, state).await,
        ("GET", "/snapshot") => snapshot(event, state).await,
        ("PUT", "/goal") => update_goal(event, state).await,
        ("PUT", "/daily-target") => update_daily_target(event, state).await,
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
        ("GET", "/v2/bootstrap") => bootstrap(event, state).await,
        ("GET", "/v2/diary") => diary_month(event, state).await,
        ("PUT", "/v2/goal") => update_goal_v2(event, state).await,
        ("PUT", "/v2/daily-target") => update_daily_target_v2(event, state).await,
        ("POST", "/v2/foods") => create_food_v2(event, state).await,
        ("POST", "/v2/exercises") => create_exercise_v2(event, state).await,
        ("POST", "/v2/weights") => create_weight_v2(event, state).await,
        ("PUT", _) if dated_path_params(path, "/v2/foods/").is_some() => {
            update_food_v2(event, state).await
        }
        ("DELETE", _) if dated_path_params(path, "/v2/foods/").is_some() => {
            delete_food_v2(event, state).await
        }
        ("PUT", _) if dated_path_params(path, "/v2/exercises/").is_some() => {
            update_exercise_v2(event, state).await
        }
        ("DELETE", _) if dated_path_params(path, "/v2/exercises/").is_some() => {
            delete_exercise_v2(event, state).await
        }
        ("PUT", _) if dated_path_params(path, "/v2/weights/").is_some() => {
            update_weight_v2(event, state).await
        }
        ("DELETE", _) if dated_path_params(path, "/v2/weights/").is_some() => {
            delete_weight_v2(event, state).await
        }
        _ => json_response(404, &serde_json::json!({ "error": "not found" })),
    };
    match response {
        Err(error) if error.downcast_ref::<InvalidBearerToken>().is_some() => {
            json_response(401, &serde_json::json!({ "error": "invalid bearer token" }))
        }
        other => other,
    }
}

async fn app_register_finish(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: AppFinishRequest = json_body(&event)?;
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
        .delete("challenge", &record.challenge_id)
        .await?;
    issue_app_code(
        &state,
        create_session(&account),
        input.state,
        input.code_challenge,
        input.callback_origin,
    )
    .await
}

async fn app_login_finish(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: AppFinishRequest = json_body(&event)?;
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
    state
        .store
        .put_json(&user_pk(&account.user_id.to_string()), "account", &account)
        .await?;
    state
        .store
        .delete("challenge", &record.challenge_id)
        .await?;
    issue_app_code(
        &state,
        create_session(&account),
        input.state,
        input.code_challenge,
        input.callback_origin,
    )
    .await
}

async fn issue_app_code(
    state: &AppState,
    session: Session,
    callback_state: String,
    code_challenge: String,
    callback_origin: Option<String>,
) -> Result<Response<Body>, Error> {
    if callback_state.len() < 16
        || callback_state.len() > 256
        || code_challenge.len() < 43
        || code_challenge.len() > 128
    {
        return json_response(400, &serde_json::json!({ "error": "invalid state" }));
    }
    let code = uuid::Uuid::new_v4().to_string();
    let record = AppAuthorizationCode {
        session,
        state: callback_state.clone(),
        code_challenge,
        expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
    };
    state
        .store
        .put_json_with_ttl(
            "app-code",
            &code,
            &record,
            expiration_epoch(&record.expires_at)?,
        )
        .await?;
    let callback_url = match callback_origin {
        Some(origin) if web_origin_is_allowed(&origin) => {
            format!("{origin}/?code={code}&state={callback_state}")
        }
        _ => format!("energylossplus://auth/callback?code={code}&state={callback_state}"),
    };
    json_response(
        200,
        &serde_json::json!({
            "callbackUrl": callback_url
        }),
    )
}

async fn app_exchange(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let input: AppExchangeRequest = json_body(&event)?;
    let Some(record): Option<AppAuthorizationCode> =
        state.store.get_json("app-code", &input.code).await?
    else {
        return json_response(
            400,
            &serde_json::json!({ "error": "unknown authorization code" }),
        );
    };
    state.store.delete("app-code", &input.code).await?;
    if record.state != input.state
        || authorization_code_is_expired(&record)
        || record.code_challenge != pkce_challenge(&input.code_verifier)
    {
        return json_response(
            400,
            &serde_json::json!({ "error": "invalid or expired authorization code" }),
        );
    }
    state
        .store
        .put_json_with_ttl(
            "token",
            &record.session.token,
            &record.session,
            expiration_epoch(&record.session.expires_at)?,
        )
        .await?;
    json_response(200, &record.session)
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
        .put_json_with_ttl(
            "challenge",
            &record.challenge_id,
            &record,
            expiration_epoch(&record.expires_at)?,
        )
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
        .put_json_with_ttl(
            "token",
            &session.token,
            &session,
            expiration_epoch(&session.expires_at)?,
        )
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
        .put_json_with_ttl(
            "challenge",
            &record.challenge_id,
            &record,
            expiration_epoch(&record.expires_at)?,
        )
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
        .put_json_with_ttl(
            "token",
            &session.token,
            &session,
            expiration_epoch(&session.expires_at)?,
        )
        .await?;
    state
        .store
        .delete("challenge", &record.challenge_id)
        .await?;

    json_response(200, &session)
}

async fn snapshot(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn update_goal(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    let profile: ProfileInput = json_body(&event)?;
    let recommendation = match recommend_goal(&profile) {
        Ok(recommendation) => recommendation,
        Err(error) => {
            return json_response(400, &serde_json::json!({ "error": error.to_string() }))
        }
    };
    let mut user_state = ensure_user_v2(&state, &session).await?;
    user_state.profile = profile;
    user_state.recommendation = Some(recommendation);
    save_user_state(&state, &session, &user_state).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn update_daily_target(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    let input: UpdateDailyTargetRequest = json_body(&event)?;
    if !valid_daily_target(input.daily_calorie_target) {
        return json_response(
            400,
            &serde_json::json!({ "error": "dailyCalorieTarget must be between 500 and 6000" }),
        );
    }
    let mut user_state = ensure_user_v2(&state, &session).await?;
    user_state.daily_calorie_target = Some(input.daily_calorie_target);
    save_user_state(&state, &session, &user_state).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn create_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    create_food_v2(event.clone(), state.clone()).await?;
    let session = require_session(&event, &state).await?;
    json_response(201, &load_snapshot(&state, &session).await?)
}

async fn create_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    create_exercise_v2(event.clone(), state.clone()).await?;
    let session = require_session(&event, &state).await?;
    json_response(201, &load_snapshot(&state, &session).await?)
}

async fn create_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    create_weight_v2(event.clone(), state.clone()).await?;
    let session = require_session(&event, &state).await?;
    json_response(201, &load_snapshot(&state, &session).await?)
}

async fn update_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/foods/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let input: CreateFoodRequest = json_body(&event)?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(old) = snapshot
        .foods
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(404, &serde_json::json!({ "error": "food entry not found" }));
    };
    let entry = FoodEntry {
        id: old.id,
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
    move_record(&state, &session, old.date, &DiaryRecord::Food(entry)).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn delete_food(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/foods/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(entry) = snapshot
        .foods
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(404, &serde_json::json!({ "error": "food entry not found" }));
    };
    delete_record(&state, &session, entry.date, "food", id).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn update_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/exercises/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let input: CreateExerciseRequest = json_body(&event)?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(old) = snapshot
        .exercises
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "exercise entry not found" }),
        );
    };
    let entry = ExerciseEntry {
        id: old.id,
        user_id: session.user_id.clone(),
        date: input.date,
        name: input.name,
        calories_burned: input.calories_burned,
        duration_minutes: input.duration_minutes,
        note: input.note,
    };
    move_record(&state, &session, old.date, &DiaryRecord::Exercise(entry)).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn delete_exercise(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/exercises/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(entry) = snapshot
        .exercises
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "exercise entry not found" }),
        );
    };
    delete_record(&state, &session, entry.date, "exercise", id).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn update_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/weights/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let input: CreateWeightRequest = json_body(&event)?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(old) = snapshot
        .weights
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "weight entry not found" }),
        );
    };
    let entry = WeightEntry {
        id: old.id,
        user_id: session.user_id.clone(),
        date: input.date,
        weight_kg: input.weight_kg,
        note: input.note,
    };
    move_record(&state, &session, old.date, &DiaryRecord::Weight(entry)).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn delete_weight(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some(id) = path_param(event.uri().path(), "/weights/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    let snapshot = load_snapshot(&state, &session).await?;
    let Some(entry) = snapshot
        .weights
        .iter()
        .find(|entry| entry.id.to_string() == id)
    else {
        return json_response(
            404,
            &serde_json::json!({ "error": "weight entry not found" }),
        );
    };
    delete_record(&state, &session, entry.date, "weight", id).await?;
    json_response(200, &load_snapshot(&state, &session).await?)
}

async fn bootstrap(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    let user_state = ensure_user_v2(&state, &session).await?;
    json_response(200, &bootstrap_payload(session, user_state))
}

async fn diary_month(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let Some(month) = query_param(&event, "month").filter(|month| valid_month(month)) else {
        return json_response(
            400,
            &serde_json::json!({ "error": "month must use YYYY-MM" }),
        );
    };
    json_response(
        200,
        &load_diary(&state, &session, &format!("diary#{month}")).await?,
    )
}

async fn update_goal_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    let profile: ProfileInput = json_body(&event)?;
    let recommendation = match recommend_goal(&profile) {
        Ok(value) => value,
        Err(error) => {
            return json_response(400, &serde_json::json!({ "error": error.to_string() }))
        }
    };
    let mut user_state = ensure_user_v2(&state, &session).await?;
    user_state.profile = profile;
    user_state.recommendation = Some(recommendation);
    save_user_state(&state, &session, &user_state).await?;
    json_response(200, &bootstrap_payload(session, user_state))
}

async fn update_daily_target_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    let input: UpdateDailyTargetRequest = json_body(&event)?;
    if !valid_daily_target(input.daily_calorie_target) {
        return json_response(
            400,
            &serde_json::json!({ "error": "dailyCalorieTarget must be between 500 and 6000" }),
        );
    }
    let mut user_state = ensure_user_v2(&state, &session).await?;
    user_state.daily_calorie_target = Some(input.daily_calorie_target);
    save_user_state(&state, &session, &user_state).await?;
    json_response(200, &bootstrap_payload(session, user_state))
}

async fn create_food_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateFoodRequest = json_body(&event)?;
    let entry = FoodEntry {
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
    };
    put_record(&state, &session, &DiaryRecord::Food(entry.clone())).await?;
    json_response(201, &entry)
}

async fn create_exercise_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateExerciseRequest = json_body(&event)?;
    let entry = ExerciseEntry {
        id: uuid::Uuid::new_v4(),
        user_id: session.user_id.clone(),
        date: input.date,
        name: input.name,
        calories_burned: input.calories_burned,
        duration_minutes: input.duration_minutes,
        note: input.note,
    };
    put_record(&state, &session, &DiaryRecord::Exercise(entry.clone())).await?;
    json_response(201, &entry)
}

async fn create_weight_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateWeightRequest = json_body(&event)?;
    let entry = WeightEntry {
        id: uuid::Uuid::new_v4(),
        user_id: session.user_id.clone(),
        date: input.date,
        weight_kg: input.weight_kg,
        note: input.note,
    };
    put_record(&state, &session, &DiaryRecord::Weight(entry.clone())).await?;
    json_response(201, &entry)
}

async fn update_food_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some((old_date, id)) = dated_path_params(event.uri().path(), "/v2/foods/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateFoodRequest = json_body(&event)?;
    let id = parse_uuid(id)?;
    if !record_exists(&state, &session, old_date, "food", &id.to_string()).await? {
        return json_response(404, &serde_json::json!({ "error": "food entry not found" }));
    }
    let entry = FoodEntry {
        id,
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
    move_record(
        &state,
        &session,
        old_date,
        &DiaryRecord::Food(entry.clone()),
    )
    .await?;
    json_response(200, &entry)
}

async fn update_exercise_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some((old_date, id)) = dated_path_params(event.uri().path(), "/v2/exercises/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateExerciseRequest = json_body(&event)?;
    let id = parse_uuid(id)?;
    if !record_exists(&state, &session, old_date, "exercise", &id.to_string()).await? {
        return json_response(
            404,
            &serde_json::json!({ "error": "exercise entry not found" }),
        );
    }
    let entry = ExerciseEntry {
        id,
        user_id: session.user_id.clone(),
        date: input.date,
        name: input.name,
        calories_burned: input.calories_burned,
        duration_minutes: input.duration_minutes,
        note: input.note,
    };
    move_record(
        &state,
        &session,
        old_date,
        &DiaryRecord::Exercise(entry.clone()),
    )
    .await?;
    json_response(200, &entry)
}

async fn update_weight_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    let Some((old_date, id)) = dated_path_params(event.uri().path(), "/v2/weights/") else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    let input: CreateWeightRequest = json_body(&event)?;
    let id = parse_uuid(id)?;
    if !record_exists(&state, &session, old_date, "weight", &id.to_string()).await? {
        return json_response(
            404,
            &serde_json::json!({ "error": "weight entry not found" }),
        );
    }
    let entry = WeightEntry {
        id,
        user_id: session.user_id.clone(),
        date: input.date,
        weight_kg: input.weight_kg,
        note: input.note,
    };
    move_record(
        &state,
        &session,
        old_date,
        &DiaryRecord::Weight(entry.clone()),
    )
    .await?;
    json_response(200, &entry)
}

async fn delete_food_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    delete_record_v2(event, state, "/v2/foods/", "food").await
}

async fn delete_exercise_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    delete_record_v2(event, state, "/v2/exercises/", "exercise").await
}

async fn delete_weight_v2(event: Request, state: AppState) -> Result<Response<Body>, Error> {
    delete_record_v2(event, state, "/v2/weights/", "weight").await
}

async fn delete_record_v2(
    event: Request,
    state: AppState,
    prefix: &str,
    kind: &str,
) -> Result<Response<Body>, Error> {
    let Some((date, id)) = dated_path_params(event.uri().path(), prefix) else {
        return json_response(404, &serde_json::json!({ "error": "not found" }));
    };
    let session = require_session(&event, &state).await?;
    ensure_user_v2(&state, &session).await?;
    if !record_exists(&state, &session, date, kind, id).await? {
        return json_response(
            404,
            &serde_json::json!({ "error": format!("{kind} entry not found") }),
        );
    }
    delete_record(&state, &session, date, kind, id).await?;
    empty_response(204)
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
    let user_state = ensure_user_v2(state, session).await?;
    let diary = load_diary(state, session, "diary#").await?;
    Ok(Snapshot {
        session: Some(session.clone()),
        profile: user_state.profile,
        recommendation: user_state.recommendation,
        daily_calorie_target: user_state.daily_calorie_target,
        foods: diary.foods,
        exercises: diary.exercises,
        weights: diary.weights,
        sync_status: "online".to_string(),
    })
}

async fn ensure_user_v2(state: &AppState, session: &Session) -> Result<UserState, Error> {
    let pk = user_pk(&session.user_id);
    if let Some(user_state) = state.store.get_json::<UserState>(&pk, "state").await? {
        return Ok(user_state);
    }
    if let Some(snapshot) = state.store.get_json::<Snapshot>(&pk, "snapshot").await? {
        for entry in &snapshot.foods {
            put_record(state, session, &DiaryRecord::Food(entry.clone())).await?;
        }
        for entry in &snapshot.exercises {
            put_record(state, session, &DiaryRecord::Exercise(entry.clone())).await?;
        }
        for entry in &snapshot.weights {
            put_record(state, session, &DiaryRecord::Weight(entry.clone())).await?;
        }
        let user_state = UserState {
            storage_version: 2,
            profile: snapshot.profile,
            recommendation: snapshot.recommendation,
            daily_calorie_target: snapshot.daily_calorie_target,
        };
        save_user_state(state, session, &user_state).await?;
        return Ok(user_state);
    }
    let user_state = default_user_state();
    save_user_state(state, session, &user_state).await?;
    Ok(user_state)
}

async fn save_user_state(
    state: &AppState,
    session: &Session,
    user_state: &UserState,
) -> Result<(), Error> {
    state
        .store
        .put_json(&user_pk(&session.user_id), "state", user_state)
        .await?;
    Ok(())
}

fn default_user_state() -> UserState {
    let profile = ProfileInput {
        sex: energy_core::Sex::Male,
        age_years: 34,
        height_cm: 178.0,
        weight_kg: 82.0,
        activity_level: energy_core::ActivityLevel::Moderate,
        goal_kind: energy_core::GoalKind::Lose,
    };
    UserState {
        storage_version: 2,
        recommendation: recommend_goal(&profile).ok(),
        daily_calorie_target: None,
        profile,
    }
}

fn bootstrap_payload(session: Session, state: UserState) -> Bootstrap {
    Bootstrap {
        session,
        profile: state.profile,
        recommendation: state.recommendation,
        daily_calorie_target: state.daily_calorie_target,
        sync_status: "online".to_string(),
    }
}

async fn load_diary(
    state: &AppState,
    session: &Session,
    prefix: &str,
) -> Result<DiaryMonth, Error> {
    let records = state
        .store
        .query_json_prefix::<DiaryRecord>(&user_pk(&session.user_id), prefix)
        .await?;
    Ok(records
        .into_iter()
        .fold(DiaryMonth::default(), |mut diary, record| {
            match record {
                DiaryRecord::Food(entry) => diary.foods.push(entry),
                DiaryRecord::Exercise(entry) => diary.exercises.push(entry),
                DiaryRecord::Weight(entry) => diary.weights.push(entry),
            }
            diary
        }))
}

async fn put_record(
    state: &AppState,
    session: &Session,
    record: &DiaryRecord,
) -> Result<(), Error> {
    state
        .store
        .put_json(&user_pk(&session.user_id), &record_sk(record), record)
        .await?;
    Ok(())
}

async fn move_record(
    state: &AppState,
    session: &Session,
    old_date: NaiveDate,
    record: &DiaryRecord,
) -> Result<(), Error> {
    let (kind, id, _) = record_parts(record);
    state
        .store
        .move_json(
            &user_pk(&session.user_id),
            &diary_sk(old_date, kind, &id),
            &record_sk(record),
            record,
        )
        .await?;
    Ok(())
}

async fn delete_record(
    state: &AppState,
    session: &Session,
    date: NaiveDate,
    kind: &str,
    id: &str,
) -> Result<(), Error> {
    state
        .store
        .delete(&user_pk(&session.user_id), &diary_sk(date, kind, id))
        .await?;
    Ok(())
}

async fn record_exists(
    state: &AppState,
    session: &Session,
    date: NaiveDate,
    kind: &str,
    id: &str,
) -> Result<bool, Error> {
    let exists = state
        .store
        .get_json::<DiaryRecord>(&user_pk(&session.user_id), &diary_sk(date, kind, id))
        .await?;
    Ok(exists.is_some())
}

fn record_sk(record: &DiaryRecord) -> String {
    let (kind, id, date) = record_parts(record);
    diary_sk(date, kind, &id)
}

fn record_parts(record: &DiaryRecord) -> (&'static str, String, NaiveDate) {
    match record {
        DiaryRecord::Food(entry) => ("food", entry.id.to_string(), entry.date),
        DiaryRecord::Exercise(entry) => ("exercise", entry.id.to_string(), entry.date),
        DiaryRecord::Weight(entry) => ("weight", entry.id.to_string(), entry.date),
    }
}

fn diary_sk(date: NaiveDate, kind: &str, id: &str) -> String {
    format!("diary#{date}#{kind}#{id}")
}

fn valid_month(month: &str) -> bool {
    month.len() == 7
        && month.as_bytes().get(4) == Some(&b'-')
        && NaiveDate::parse_from_str(&format!("{month}-01"), "%Y-%m-%d").is_ok()
}

fn query_param<'a>(event: &'a Request, name: &str) -> Option<&'a str> {
    event.uri().query()?.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then_some(value)
    })
}

fn dated_path_params<'a>(path: &'a str, prefix: &str) -> Option<(NaiveDate, &'a str)> {
    let rest = path.strip_prefix(prefix)?;
    let (date, id) = rest.split_once('/')?;
    if id.is_empty() || id.contains('/') {
        return None;
    }
    Some((NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()?, id))
}

fn parse_uuid(id: &str) -> Result<uuid::Uuid, Error> {
    Ok(uuid::Uuid::parse_str(id)?)
}

fn valid_daily_target(target: u16) -> bool {
    (500..=6000).contains(&target)
}

async fn require_session(event: &Request, state: &AppState) -> Result<Session, Error> {
    authenticated_session(event, state)
        .await?
        .ok_or_else(|| Box::new(InvalidBearerToken) as Error)
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
        .header(
            "access-control-allow-headers",
            "Content-Type, Authorization",
        )
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
        .header(
            "access-control-allow-headers",
            "Content-Type, Authorization",
        )
        .header(
            "access-control-allow-methods",
            "OPTIONS, GET, POST, PUT, DELETE",
        )
        .body(Body::Empty)?)
}

fn html_response(status: u16, html: String) -> Result<Response<Body>, Error> {
    Ok(Response::builder()
        .status(status)
        .header("content-type", "text/html; charset=utf-8")
        .body(html.into())?)
}

fn cors_origin() -> String {
    std::env::var("WEBAUTHN_ORIGIN").unwrap_or_else(|_| "http://localhost:1420".to_string())
}

fn web_origin_is_allowed(origin: &str) -> bool {
    std::env::var("WEB_ORIGINS")
        .unwrap_or_default()
        .split(',')
        .any(|allowed| !allowed.is_empty() && allowed == origin)
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

fn authorization_code_is_expired(code: &AppAuthorizationCode) -> bool {
    let Ok(expires_at) = chrono::DateTime::parse_from_rfc3339(&code.expires_at) else {
        return true;
    };
    expires_at < chrono::Utc::now()
}

fn expiration_epoch(expires_at: &str) -> Result<i64, Error> {
    Ok(chrono::DateTime::parse_from_rfc3339(expires_at)?.timestamp())
}

fn pkce_challenge(verifier: &str) -> String {
    use base64::Engine;
    use sha2::{Digest, Sha256};

    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn app_auth_html() -> String {
    include_str!("app-auth.html").to_string()
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
    fn validates_months_and_dated_record_paths() {
        assert!(valid_month("2026-06"));
        assert!(!valid_month("2026-13"));
        assert!(!valid_month("2026-6"));
        let (date, id) = dated_path_params(
            "/v2/foods/2026-06-12/550e8400-e29b-41d4-a716-446655440000",
            "/v2/foods/",
        )
        .unwrap();
        assert_eq!(date.to_string(), "2026-06-12");
        assert_eq!(id, "550e8400-e29b-41d4-a716-446655440000");
        assert!(dated_path_params("/v2/foods/2026-06-12/id/extra", "/v2/foods/").is_none());
    }

    #[test]
    fn creates_queryable_diary_sort_keys() {
        let date = NaiveDate::from_ymd_opt(2026, 6, 12).unwrap();
        assert_eq!(diary_sk(date, "food", "id"), "diary#2026-06-12#food#id");
        assert!(diary_sk(date, "food", "id").starts_with("diary#2026-06"));
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

    #[test]
    fn treats_expired_authorization_codes_as_expired() {
        let session = session_with_expiry((Utc::now() + Duration::hours(1)).to_rfc3339());
        let active = AppAuthorizationCode {
            session: session.clone(),
            state: "state".to_string(),
            code_challenge: "challenge".to_string(),
            expires_at: (Utc::now() + Duration::minutes(5)).to_rfc3339(),
        };
        let expired = AppAuthorizationCode {
            session,
            state: "state".to_string(),
            code_challenge: "challenge".to_string(),
            expires_at: (Utc::now() - Duration::minutes(5)).to_rfc3339(),
        };

        assert!(!authorization_code_is_expired(&active));
        assert!(authorization_code_is_expired(&expired));
    }

    #[test]
    fn creates_standard_pkce_challenges() {
        assert_eq!(
            pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn external_auth_page_uses_codes_instead_of_tokens() {
        let html = app_auth_html();
        assert!(html.contains("codeChallenge"));
        assert!(html.contains("/auth/app/"));
        assert!(html.contains("excludeCredentials"));
        assert!(!html.contains("callback?token="));
    }

    #[test]
    fn decodes_camel_case_app_exchange_requests() {
        let request: AppExchangeRequest = serde_json::from_value(serde_json::json!({
            "code": "code",
            "state": "state",
            "codeVerifier": "verifier"
        }))
        .unwrap();

        assert_eq!(request.code_verifier, "verifier");
    }

    #[test]
    fn allows_only_configured_web_callback_origins() {
        std::env::set_var(
            "WEB_ORIGINS",
            "https://energylossplus.erasereat.workers.dev,https://energy.114522.xyz",
        );

        assert!(web_origin_is_allowed("https://energy.114522.xyz"));
        assert!(!web_origin_is_allowed("https://evil.example"));
    }
}
