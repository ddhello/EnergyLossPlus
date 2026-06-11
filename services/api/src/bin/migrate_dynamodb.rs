use anyhow::{bail, Context};
use aws_config::{BehaviorVersion, Region};
use aws_sdk_dynamodb::types::{AttributeValue, PutRequest, WriteRequest};
use aws_sdk_dynamodb::Client;
use chrono::DateTime;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = arguments()?;
    let source = client(&arguments.source_region).await;
    let destination = client(&arguments.destination_region).await;

    let destination_items = scan_all(&destination, &arguments.destination_table).await?;
    if !destination_items.is_empty() && !arguments.allow_non_empty_destination {
        bail!(
            "destination table contains {} items; use --allow-non-empty-destination only after verifying it is safe",
            destination_items.len()
        );
    }

    let mut source_items = scan_all(&source, &arguments.source_table).await?;
    for item in &mut source_items {
        add_missing_ttl(item);
    }
    println!(
        "Migrating {}/{} -> {}/{}",
        arguments.source_region,
        arguments.source_table,
        arguments.destination_region,
        arguments.destination_table
    );
    for (index, batch) in source_items.chunks(25).enumerate() {
        write_batch(&destination, &arguments.destination_table, batch).await?;
        println!(
            "Copied {} items",
            ((index + 1) * 25).min(source_items.len())
        );
    }

    let final_items = scan_all(&destination, &arguments.destination_table).await?;
    println!(
        "Migration complete. Source count: {}; destination count: {}",
        source_items.len(),
        final_items.len()
    );
    if source_items.len() != final_items.len() {
        bail!("source and destination counts do not match");
    }
    Ok(())
}

async fn client(region: &str) -> Client {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(Region::new(region.to_string()))
        .load()
        .await;
    Client::new(&config)
}

async fn scan_all(
    client: &Client,
    table_name: &str,
) -> anyhow::Result<Vec<HashMap<String, AttributeValue>>> {
    let mut items = Vec::new();
    let mut last_evaluated_key = None;
    loop {
        let output = client
            .scan()
            .table_name(table_name)
            .set_exclusive_start_key(last_evaluated_key)
            .send()
            .await
            .with_context(|| format!("failed to scan {table_name}"))?;
        items.extend(output.items.unwrap_or_default());
        last_evaluated_key = output.last_evaluated_key;
        if last_evaluated_key.as_ref().is_none_or(HashMap::is_empty) {
            return Ok(items);
        }
    }
}

async fn write_batch(
    client: &Client,
    table_name: &str,
    items: &[HashMap<String, AttributeValue>],
) -> anyhow::Result<()> {
    let mut requests = items
        .iter()
        .cloned()
        .map(|item| {
            Ok(WriteRequest::builder()
                .put_request(PutRequest::builder().set_item(Some(item)).build()?)
                .build())
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    while !requests.is_empty() {
        let output = client
            .batch_write_item()
            .request_items(table_name, requests)
            .send()
            .await
            .with_context(|| format!("failed to write batch to {table_name}"))?;
        requests = output
            .unprocessed_items
            .unwrap_or_default()
            .remove(table_name)
            .unwrap_or_default();
    }
    Ok(())
}

fn add_missing_ttl(item: &mut HashMap<String, AttributeValue>) {
    if item.contains_key("expiresAtEpoch") {
        return;
    }
    let Some(AttributeValue::S(pk)) = item.get("pk") else {
        return;
    };
    if !matches!(pk.as_str(), "challenge" | "token" | "app-code") {
        return;
    }
    let Some(AttributeValue::S(json)) = item.get("json") else {
        return;
    };
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(json) else {
        return;
    };
    let Some(expires_at) = payload
        .get("expiresAt")
        .or_else(|| payload.get("expires_at"))
        .and_then(serde_json::Value::as_str)
    else {
        return;
    };
    let Ok(expires_at) = DateTime::parse_from_rfc3339(expires_at) else {
        return;
    };
    item.insert(
        "expiresAtEpoch".to_string(),
        AttributeValue::N(expires_at.timestamp().to_string()),
    );
}

struct Arguments {
    source_region: String,
    destination_region: String,
    source_table: String,
    destination_table: String,
    allow_non_empty_destination: bool,
}

fn arguments() -> anyhow::Result<Arguments> {
    let values = std::env::args().skip(1).collect::<Vec<_>>();
    let value = |name: &str| {
        values
            .windows(2)
            .find(|pair| pair[0] == name)
            .map(|pair| pair[1].clone())
            .with_context(|| format!("missing required argument {name}"))
    };
    Ok(Arguments {
        source_region: value("--source-region")?,
        destination_region: value("--destination-region")?,
        source_table: value("--source-table")?,
        destination_table: value("--destination-table")?,
        allow_non_empty_destination: values
            .iter()
            .any(|value| value == "--allow-non-empty-destination"),
    })
}
