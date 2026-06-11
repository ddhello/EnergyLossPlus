use anyhow::Context;
use aws_sdk_dynamodb::types::AttributeValue;
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
