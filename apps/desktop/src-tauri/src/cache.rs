use anyhow::Context;
use energy_core::{ExerciseEntry, FoodEntry, GoalRecommendation, ProfileInput, WeightEntry};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub token: String,
    pub user_id: String,
    pub nickname: String,
    pub device_name: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CachedSnapshot {
    pub session: Option<Session>,
    pub profile: ProfileInput,
    pub recommendation: Option<GoalRecommendation>,
    #[serde(default)]
    pub daily_calorie_target: Option<u16>,
    pub foods: Vec<FoodEntry>,
    pub exercises: Vec<ExerciseEntry>,
    pub weights: Vec<WeightEntry>,
    pub sync_status: String,
}

pub struct Cache {
    connection: Connection,
}

impl Cache {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let connection = Connection::open(path).context("failed to open local cache")?;
        let cache = Self { connection };
        cache.migrate()?;
        Ok(cache)
    }

    pub fn load_snapshot(&self) -> anyhow::Result<CachedSnapshot> {
        let profile = self
            .get_json::<ProfileInput>("profile")?
            .unwrap_or_else(default_profile);
        let recommendation = self.get_json::<GoalRecommendation>("recommendation")?;
        let daily_calorie_target = self.get_json::<u16>("daily_calorie_target")?;
        let foods = self
            .get_json::<Vec<FoodEntry>>("foods")?
            .unwrap_or_default();
        let exercises = self
            .get_json::<Vec<ExerciseEntry>>("exercises")?
            .unwrap_or_default();
        let weights = self
            .get_json::<Vec<WeightEntry>>("weights")?
            .unwrap_or_default();
        let session = self.get_json::<Session>("session")?;

        Ok(CachedSnapshot {
            session,
            profile,
            recommendation,
            daily_calorie_target,
            foods,
            exercises,
            weights,
            sync_status: "cached".to_string(),
        })
    }

    pub fn save_snapshot(&self, snapshot: &CachedSnapshot) -> anyhow::Result<()> {
        self.set_json("profile", &snapshot.profile)?;
        self.set_json("recommendation", &snapshot.recommendation)?;
        self.set_json("daily_calorie_target", &snapshot.daily_calorie_target)?;
        self.set_json("foods", &snapshot.foods)?;
        self.set_json("exercises", &snapshot.exercises)?;
        self.set_json("weights", &snapshot.weights)?;
        if let Some(session) = &snapshot.session {
            self.save_session(session)?;
        }
        Ok(())
    }

    pub fn save_session(&self, session: &Session) -> anyhow::Result<()> {
        self.set_json("session", session)
    }

    pub fn clear_session(&self) -> anyhow::Result<()> {
        self.connection
            .execute("delete from kv where key = 'session'", [])
            .context("failed to clear session")?;
        Ok(())
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.connection
            .execute(
                "create table if not exists kv (
                    key text primary key not null,
                    value text not null,
                    updated_at text not null default current_timestamp
                )",
                [],
            )
            .context("failed to migrate local cache")?;
        Ok(())
    }

    fn get_json<T>(&self, key: &str) -> anyhow::Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut statement = self
            .connection
            .prepare("select value from kv where key = ?1")
            .context("failed to prepare cache query")?;
        let mut rows = statement
            .query(params![key])
            .context("failed to query cache")?;
        if let Some(row) = rows.next().context("failed to read cache row")? {
            let value: String = row.get(0).context("failed to read cache value")?;
            Ok(Some(
                serde_json::from_str(&value).context("failed to parse cache value")?,
            ))
        } else {
            Ok(None)
        }
    }

    fn set_json<T>(&self, key: &str, value: &T) -> anyhow::Result<()>
    where
        T: Serialize,
    {
        let value = serde_json::to_string(value).context("failed to serialize cache value")?;
        self.connection
            .execute(
                "insert into kv(key, value, updated_at)
                 values(?1, ?2, current_timestamp)
                 on conflict(key) do update set value = excluded.value, updated_at = current_timestamp",
                params![key, value],
            )
            .context("failed to write cache value")?;
        Ok(())
    }
}

fn default_profile() -> ProfileInput {
    ProfileInput {
        sex: energy_core::Sex::Male,
        age_years: 34,
        height_cm: 178.0,
        weight_kg: 82.0,
        activity_level: energy_core::ActivityLevel::Moderate,
        goal_kind: energy_core::GoalKind::Lose,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn saves_and_loads_session() {
        let dir = tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache.sqlite")).unwrap();
        let session = Session {
            token: "token".into(),
            user_id: "user".into(),
            nickname: "tester".into(),
            device_name: "desktop".into(),
            expires_at: "2026-06-08T00:00:00Z".into(),
        };

        cache.save_session(&session).unwrap();
        let snapshot = cache.load_snapshot().unwrap();
        assert_eq!(snapshot.session, Some(session));
    }
}
