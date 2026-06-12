use anyhow::Context;
use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;
use energy_core::{ExerciseEntry, FoodEntry, GoalRecommendation, ProfileInput, WeightEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    #[serde(default)]
    daily_calorie_target: Option<u16>,
    foods: Vec<FoodEntry>,
    exercises: Vec<ExerciseEntry>,
    weights: Vec<WeightEntry>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UserState {
    storage_version: u8,
    profile: ProfileInput,
    recommendation: Option<GoalRecommendation>,
    daily_calorie_target: Option<u16>,
}

#[derive(Serialize)]
#[serde(tag = "kind", content = "record", rename_all = "camelCase")]
enum DiaryRecord {
    Food(FoodEntry),
    Exercise(ExerciseEntry),
    Weight(WeightEntry),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let table = argument("--table")
        .or_else(|| std::env::var("TABLE_NAME").ok())
        .context("provide --table or TABLE_NAME")?;
    let client = Client::new(&aws_config::load_defaults(BehaviorVersion::latest()).await);
    let mut last_key = None;
    let mut migrated = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    loop {
        let output = client
            .scan()
            .table_name(&table)
            .set_exclusive_start_key(last_key)
            .send()
            .await?;
        for item in output.items.unwrap_or_default() {
            if item.get("sk").and_then(as_string) != Some("snapshot") {
                continue;
            }
            let Some(pk) = item.get("pk").and_then(as_string) else {
                failed += 1;
                continue;
            };
            if client
                .get_item()
                .table_name(&table)
                .key("pk", AttributeValue::S(pk.to_string()))
                .key("sk", AttributeValue::S("state".to_string()))
                .send()
                .await?
                .item
                .is_some()
            {
                skipped += 1;
                continue;
            }
            let Some(json) = item.get("json").and_then(as_string) else {
                failed += 1;
                continue;
            };
            let snapshot: Snapshot = match serde_json::from_str(json) {
                Ok(value) => value,
                Err(error) => {
                    eprintln!(
                        "{pk}: failed to decode {} byte snapshot: {error}",
                        json.len()
                    );
                    failed += 1;
                    continue;
                }
            };
            let record_count =
                snapshot.foods.len() + snapshot.exercises.len() + snapshot.weights.len();
            if let Err(error) = migrate_user(&client, &table, pk, snapshot).await {
                eprintln!("{pk}: migration failed: {error:#}");
                failed += 1;
            } else {
                println!(
                    "{pk}: migrated {record_count} records from {} bytes",
                    json.len()
                );
                migrated += 1;
            }
        }
        last_key = output.last_evaluated_key;
        if last_key.as_ref().is_none_or(HashMap::is_empty) {
            break;
        }
    }
    println!("migration complete: migrated={migrated}, skipped={skipped}, failed={failed}");
    if failed > 0 {
        anyhow::bail!("{failed} snapshots failed migration");
    }
    Ok(())
}

async fn migrate_user(
    client: &Client,
    table: &str,
    pk: &str,
    snapshot: Snapshot,
) -> anyhow::Result<()> {
    for entry in snapshot.foods {
        put_json(
            client,
            table,
            pk,
            &format!("diary#{}#food#{}", entry.date, entry.id),
            &DiaryRecord::Food(entry),
        )
        .await?;
    }
    for entry in snapshot.exercises {
        put_json(
            client,
            table,
            pk,
            &format!("diary#{}#exercise#{}", entry.date, entry.id),
            &DiaryRecord::Exercise(entry),
        )
        .await?;
    }
    for entry in snapshot.weights {
        put_json(
            client,
            table,
            pk,
            &format!("diary#{}#weight#{}", entry.date, entry.id),
            &DiaryRecord::Weight(entry),
        )
        .await?;
    }
    let state = UserState {
        storage_version: 2,
        profile: snapshot.profile,
        recommendation: snapshot.recommendation,
        daily_calorie_target: snapshot.daily_calorie_target,
    };
    put_json(client, table, pk, "state", &state).await
}

async fn put_json<T: Serialize>(
    client: &Client,
    table: &str,
    pk: &str,
    sk: &str,
    value: &T,
) -> anyhow::Result<()> {
    client
        .put_item()
        .table_name(table)
        .item("pk", AttributeValue::S(pk.to_string()))
        .item("sk", AttributeValue::S(sk.to_string()))
        .item("json", AttributeValue::S(serde_json::to_string(value)?))
        .send()
        .await?;
    Ok(())
}

fn as_string(value: &AttributeValue) -> Option<&str> {
    value.as_s().ok().map(String::as_str)
}

fn argument(name: &str) -> Option<String> {
    let values = std::env::args().collect::<Vec<_>>();
    values
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
}
