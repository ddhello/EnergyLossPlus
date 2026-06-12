use anyhow::Context;
use aws_sdk_dynamodb::types::{AttributeValue, Delete, Put, TransactWriteItem};
use aws_sdk_dynamodb::Client;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;

#[derive(Clone)]
pub struct DynamoStore {
    client: Client,
    table_name: String,
}

impl DynamoStore {
    pub fn new(client: Client, table_name: String) -> Self {
        Self { client, table_name }
    }

    pub async fn put_json<T>(&self, pk: &str, sk: &str, value: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let json = serde_json::to_string(value).context("failed to encode item")?;
        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("pk", AttributeValue::S(pk.to_string()))
            .item("sk", AttributeValue::S(sk.to_string()))
            .item("json", AttributeValue::S(json))
            .send()
            .await
            .context("failed to put item")?;
        Ok(())
    }

    pub async fn put_json_with_ttl<T>(
        &self,
        pk: &str,
        sk: &str,
        value: &T,
        expires_at_epoch: i64,
    ) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let json = serde_json::to_string(value).context("failed to encode item")?;
        self.client
            .put_item()
            .table_name(&self.table_name)
            .item("pk", AttributeValue::S(pk.to_string()))
            .item("sk", AttributeValue::S(sk.to_string()))
            .item("json", AttributeValue::S(json))
            .item(
                "expiresAtEpoch",
                AttributeValue::N(expires_at_epoch.to_string()),
            )
            .send()
            .await
            .context("failed to put item with ttl")?;
        Ok(())
    }

    pub async fn get_json<T>(&self, pk: &str, sk: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let output = self
            .client
            .get_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(pk.to_string()))
            .key("sk", AttributeValue::S(sk.to_string()))
            .send()
            .await
            .context("failed to get item")?;

        let Some(item) = output.item else {
            return Ok(None);
        };
        decode_item(item)
    }

    pub async fn delete(&self, pk: &str, sk: &str) -> anyhow::Result<()> {
        self.client
            .delete_item()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(pk.to_string()))
            .key("sk", AttributeValue::S(sk.to_string()))
            .send()
            .await
            .context("failed to delete item")?;
        Ok(())
    }

    pub async fn query_json_prefix<T>(&self, pk: &str, sk_prefix: &str) -> anyhow::Result<Vec<T>>
    where
        T: DeserializeOwned,
    {
        let mut values = Vec::new();
        let mut last_evaluated_key = None;
        loop {
            let output = self
                .client
                .query()
                .table_name(&self.table_name)
                .key_condition_expression("pk = :pk and begins_with(sk, :prefix)")
                .expression_attribute_values(":pk", AttributeValue::S(pk.to_string()))
                .expression_attribute_values(":prefix", AttributeValue::S(sk_prefix.to_string()))
                .set_exclusive_start_key(last_evaluated_key)
                .send()
                .await
                .context("failed to query items")?;
            for item in output.items.unwrap_or_default() {
                if let Some(value) = decode_item(item)? {
                    values.push(value);
                }
            }
            last_evaluated_key = output.last_evaluated_key;
            if last_evaluated_key.as_ref().is_none_or(HashMap::is_empty) {
                return Ok(values);
            }
        }
    }

    pub async fn move_json<T>(
        &self,
        pk: &str,
        old_sk: &str,
        new_sk: &str,
        value: &T,
    ) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        if old_sk == new_sk {
            return self.put_json(pk, new_sk, value).await;
        }
        let json = serde_json::to_string(value).context("failed to encode item")?;
        let delete = Delete::builder()
            .table_name(&self.table_name)
            .key("pk", AttributeValue::S(pk.to_string()))
            .key("sk", AttributeValue::S(old_sk.to_string()))
            .build()
            .context("failed to build transaction delete")?;
        let put = Put::builder()
            .table_name(&self.table_name)
            .item("pk", AttributeValue::S(pk.to_string()))
            .item("sk", AttributeValue::S(new_sk.to_string()))
            .item("json", AttributeValue::S(json))
            .build()
            .context("failed to build transaction put")?;
        self.client
            .transact_write_items()
            .transact_items(TransactWriteItem::builder().delete(delete).build())
            .transact_items(TransactWriteItem::builder().put(put).build())
            .send()
            .await
            .context("failed to move item")?;
        Ok(())
    }
}

fn decode_item<T>(item: HashMap<String, AttributeValue>) -> anyhow::Result<Option<T>>
where
    T: DeserializeOwned,
{
    let Some(AttributeValue::S(json)) = item.get("json") else {
        return Ok(None);
    };
    Ok(Some(
        serde_json::from_str(json).context("failed to decode item")?,
    ))
}
