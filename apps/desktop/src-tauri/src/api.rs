use crate::cache::CachedSnapshot;
use anyhow::Context;
use energy_core::ProfileInput;
use reqwest::header::AUTHORIZATION;
use serde::{de::DeserializeOwned, Serialize};

pub struct ApiClient {
    base_url: String,
    client: reqwest::Client,
}

impl ApiClient {
    pub fn from_env() -> Self {
        let base_url = std::env::var("ENERGY_API_BASE_URL").unwrap_or_else(|_| {
            option_env!("ENERGY_API_BASE_URL")
                .unwrap_or("https://x38dzo14cd.execute-api.ap-northeast-1.amazonaws.com")
                .to_string()
        });
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn snapshot(&self, token: &str) -> anyhow::Result<CachedSnapshot> {
        let url = format!("{}/snapshot", self.base_url);
        let response = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .with_context(|| format!("failed to contact EnergyLossPlus API at {url}"))?;

        if !response.status().is_success() {
            anyhow::bail!("sync failed with {}", response.status());
        }

        response
            .json::<CachedSnapshot>()
            .await
            .context("failed to decode snapshot")
    }

    pub async fn auth_post<T>(&self, path: &str, body: &serde_json::Value) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .with_context(|| format!("failed to contact EnergyLossPlus API at {url}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read EnergyLossPlus API response")?;

        if !status.is_success() {
            anyhow::bail!("Passkey API request failed with {status}: {text}");
        }

        serde_json::from_str(&text).context("failed to decode Passkey API response")
    }

    pub async fn update_goal(
        &self,
        token: &str,
        profile: &ProfileInput,
    ) -> anyhow::Result<CachedSnapshot> {
        self.send_json("PUT", "/goal", token, profile).await
    }

    pub async fn update_daily_target(
        &self,
        token: &str,
        daily_calorie_target: u16,
    ) -> anyhow::Result<CachedSnapshot> {
        self.send_json(
            "PUT",
            "/daily-target",
            token,
            &serde_json::json!({ "dailyCalorieTarget": daily_calorie_target }),
        )
        .await
    }

    pub async fn create_food<T>(&self, token: &str, entry: &T) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("POST", "/foods", token, entry).await
    }

    pub async fn update_food<T>(
        &self,
        token: &str,
        id: &str,
        entry: &T,
    ) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("PUT", &format!("/foods/{id}"), token, entry)
            .await
    }

    pub async fn delete_food(&self, token: &str, id: &str) -> anyhow::Result<CachedSnapshot> {
        self.delete(&format!("/foods/{id}"), token).await
    }

    pub async fn create_exercise<T>(&self, token: &str, entry: &T) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("POST", "/exercises", token, entry).await
    }

    pub async fn update_exercise<T>(
        &self,
        token: &str,
        id: &str,
        entry: &T,
    ) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("PUT", &format!("/exercises/{id}"), token, entry)
            .await
    }

    pub async fn delete_exercise(&self, token: &str, id: &str) -> anyhow::Result<CachedSnapshot> {
        self.delete(&format!("/exercises/{id}"), token).await
    }

    pub async fn create_weight<T>(&self, token: &str, entry: &T) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("POST", "/weights", token, entry).await
    }

    pub async fn update_weight<T>(
        &self,
        token: &str,
        id: &str,
        entry: &T,
    ) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        self.send_json("PUT", &format!("/weights/{id}"), token, entry)
            .await
    }

    pub async fn delete_weight(&self, token: &str, id: &str) -> anyhow::Result<CachedSnapshot> {
        self.delete(&format!("/weights/{id}"), token).await
    }

    async fn send_json<T>(
        &self,
        method: &str,
        path: &str,
        token: &str,
        body: &T,
    ) -> anyhow::Result<CachedSnapshot>
    where
        T: Serialize + ?Sized,
    {
        let request = match method {
            "PUT" => self.client.put(format!("{}{}", self.base_url, path)),
            "POST" => self.client.post(format!("{}{}", self.base_url, path)),
            _ => anyhow::bail!("unsupported API method {method}"),
        };
        let response = request
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .json(body)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to contact EnergyLossPlus API at {}{}",
                    self.base_url, path
                )
            })?;

        if !response.status().is_success() {
            anyhow::bail!("API write failed with {}", response.status());
        }

        response
            .json::<CachedSnapshot>()
            .await
            .context("failed to decode updated snapshot")
    }

    async fn delete(&self, path: &str, token: &str) -> anyhow::Result<CachedSnapshot> {
        let response = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to contact EnergyLossPlus API at {}{}",
                    self.base_url, path
                )
            })?;

        if !response.status().is_success() {
            anyhow::bail!("API delete failed with {}", response.status());
        }

        response
            .json::<CachedSnapshot>()
            .await
            .context("failed to decode updated snapshot")
    }
}
