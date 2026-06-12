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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    pub session: Session,
    pub profile: ProfileInput,
    pub recommendation: Option<GoalRecommendation>,
    pub daily_calorie_target: Option<u16>,
    pub sync_status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiaryMonth {
    pub foods: Vec<FoodEntry>,
    pub exercises: Vec<ExerciseEntry>,
    pub weights: Vec<WeightEntry>,
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

    pub fn load_snapshot(&self, month: &str) -> anyhow::Result<CachedSnapshot> {
        let profile = self
            .get_json::<ProfileInput>("profile")?
            .unwrap_or_else(default_profile);
        let recommendation = self.get_json::<GoalRecommendation>("recommendation")?;
        let daily_calorie_target = self.get_json::<u16>("daily_calorie_target")?;
        let diary = self.load_diary_month(month)?;
        let session = self.get_json::<Session>("session")?;

        Ok(CachedSnapshot {
            session,
            profile,
            recommendation,
            daily_calorie_target,
            foods: diary.foods,
            exercises: diary.exercises,
            weights: diary.weights,
            sync_status: "cached".to_string(),
        })
    }

    pub fn save_bootstrap(&self, bootstrap: &Bootstrap) -> anyhow::Result<()> {
        self.set_json("profile", &bootstrap.profile)?;
        self.set_json("recommendation", &bootstrap.recommendation)?;
        self.set_json("daily_calorie_target", &bootstrap.daily_calorie_target)?;
        self.save_session(&bootstrap.session)
    }

    pub fn load_diary_month(&self, month: &str) -> anyhow::Result<DiaryMonth> {
        let prefix = format!("{month}-%");
        let mut statement = self
            .connection
            .prepare("select kind, value from diary where date like ?1 order by date, id")
            .context("failed to prepare diary query")?;
        let mut rows = statement
            .query(params![prefix])
            .context("failed to query diary")?;
        let mut diary = DiaryMonth::default();
        while let Some(row) = rows.next().context("failed to read diary row")? {
            let kind: String = row.get(0)?;
            let value: String = row.get(1)?;
            match kind.as_str() {
                "food" => diary.foods.push(serde_json::from_str(&value)?),
                "exercise" => diary.exercises.push(serde_json::from_str(&value)?),
                "weight" => diary.weights.push(serde_json::from_str(&value)?),
                _ => {}
            }
        }
        Ok(diary)
    }

    pub fn replace_diary_month(&mut self, month: &str, diary: &DiaryMonth) -> anyhow::Result<()> {
        let transaction = self
            .connection
            .transaction()
            .context("failed to start diary transaction")?;
        transaction.execute(
            "delete from diary where date like ?1",
            params![format!("{month}-%")],
        )?;
        for entry in &diary.foods {
            upsert_diary(
                &transaction,
                "food",
                &entry.id.to_string(),
                &entry.date.to_string(),
                entry,
            )?;
        }
        for entry in &diary.exercises {
            upsert_diary(
                &transaction,
                "exercise",
                &entry.id.to_string(),
                &entry.date.to_string(),
                entry,
            )?;
        }
        for entry in &diary.weights {
            upsert_diary(
                &transaction,
                "weight",
                &entry.id.to_string(),
                &entry.date.to_string(),
                entry,
            )?;
        }
        transaction
            .commit()
            .context("failed to commit diary transaction")
    }

    pub fn upsert_food(&self, entry: &FoodEntry) -> anyhow::Result<()> {
        upsert_diary(
            &self.connection,
            "food",
            &entry.id.to_string(),
            &entry.date.to_string(),
            entry,
        )
    }

    pub fn upsert_exercise(&self, entry: &ExerciseEntry) -> anyhow::Result<()> {
        upsert_diary(
            &self.connection,
            "exercise",
            &entry.id.to_string(),
            &entry.date.to_string(),
            entry,
        )
    }

    pub fn upsert_weight(&self, entry: &WeightEntry) -> anyhow::Result<()> {
        upsert_diary(
            &self.connection,
            "weight",
            &entry.id.to_string(),
            &entry.date.to_string(),
            entry,
        )
    }

    pub fn delete_diary(&self, kind: &str, id: &str) -> anyhow::Result<()> {
        self.connection.execute(
            "delete from diary where kind = ?1 and id = ?2",
            params![kind, id],
        )?;
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
        self.connection
            .execute(
                "create table if not exists diary (
                kind text not null,
                id text not null,
                date text not null,
                value text not null,
                primary key(kind, id)
            )",
                [],
            )
            .context("failed to migrate diary cache")?;
        self.connection
            .execute("create index if not exists diary_date on diary(date)", [])
            .context("failed to create diary date index")?;
        self.migrate_legacy_diary()?;
        Ok(())
    }

    fn migrate_legacy_diary(&self) -> anyhow::Result<()> {
        let count: i64 = self
            .connection
            .query_row("select count(*) from diary", [], |row| row.get(0))?;
        if count != 0 {
            return Ok(());
        }
        for entry in self
            .get_json::<Vec<FoodEntry>>("foods")?
            .unwrap_or_default()
        {
            self.upsert_food(&entry)?;
        }
        for entry in self
            .get_json::<Vec<ExerciseEntry>>("exercises")?
            .unwrap_or_default()
        {
            self.upsert_exercise(&entry)?;
        }
        for entry in self
            .get_json::<Vec<WeightEntry>>("weights")?
            .unwrap_or_default()
        {
            self.upsert_weight(&entry)?;
        }
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

fn upsert_diary<T>(
    connection: &Connection,
    kind: &str,
    id: &str,
    date: &str,
    value: &T,
) -> anyhow::Result<()>
where
    T: Serialize,
{
    let value = serde_json::to_string(value)?;
    connection.execute(
        "insert into diary(kind, id, date, value) values(?1, ?2, ?3, ?4)
         on conflict(kind, id) do update set date = excluded.date, value = excluded.value",
        params![kind, id, date, value],
    )?;
    Ok(())
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
    use chrono::NaiveDate;
    use tempfile::tempdir;
    use uuid::Uuid;

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
        let snapshot = cache.load_snapshot("2026-06").unwrap();
        assert_eq!(snapshot.session, Some(session));
    }

    #[test]
    fn stores_and_filters_independent_diary_records_by_month() {
        let dir = tempdir().unwrap();
        let cache = Cache::open(dir.path().join("cache.sqlite")).unwrap();
        let entry = |date: &str| FoodEntry {
            id: Uuid::new_v4(),
            user_id: "user".into(),
            date: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
            meal: "Lunch".into(),
            name: "Rice".into(),
            calories: 500,
            protein_g: 10.0,
            carbs_g: 80.0,
            fat_g: 5.0,
            note: None,
        };
        cache.upsert_food(&entry("2026-06-12")).unwrap();
        cache.upsert_food(&entry("2026-07-01")).unwrap();

        let june = cache.load_diary_month("2026-06").unwrap();
        assert_eq!(june.foods.len(), 1);
        assert_eq!(june.foods[0].date.to_string(), "2026-06-12");
    }
}
