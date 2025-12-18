use anyhow::Result;
use directories::ProjectDirs;
use futures::TryStreamExt;
use sqlx::Row;
use sqlx::SqlitePool;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;

use crate::card::Card;
use crate::fsrs::Performance;
use crate::fsrs::ReviewStatus;
use crate::fsrs::ReviewedPerformance;
use crate::fsrs::update_performance;

#[derive(Debug, Default)]
pub struct CardStats {
    pub total_cards_in_db: i64,
    pub num_cards: i64,
    pub new_cards: i64,
    pub reviewed_cards: i64,
    pub due_cards: i64,
    pub overdue_cards: i64,
    pub upcoming_week: Vec<UpcomingCount>,
    pub upcoming_month: i64,
}

#[derive(Debug, Clone)]
pub struct UpcomingCount {
    pub day: String,
    pub count: i64,
}

pub struct DB {
    pool: SqlitePool,
}

impl DB {
    pub async fn new() -> Result<Self> {
        let proj_dirs = ProjectDirs::from("", "", "repeat")
            .ok_or_else(|| anyhow!("Could not determine project directory"))?;
        let data_dir = proj_dirs.data_dir();
        std::fs::create_dir_all(data_dir)
            .map_err(|e| anyhow!("Failed to create data directory: {}", e))?;

        let db_path: PathBuf = data_dir.join("cards.db");
        let options =
            SqliteConnectOptions::from_str(&db_path.to_string_lossy())?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        let table_exists = probe_schema_exists(&pool).await;
        if let Ok(false) = table_exists {
            sqlx::query(include_str!("schema.sql"))
                .execute(&pool)
                .await?;
        }

        Ok(Self { pool })
    }

    pub async fn add_card(&self, card: &Card) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            r#"
        INSERT or ignore INTO cards (
            card_hash,
            added_at,
            last_reviewed_at,
            stability,
            difficulty,
            interval_raw,
            interval_days,
            due_date,
            review_count
        )
        VALUES (?, ?, NULL, NULL, NULL, NULL, 0, NULL, 0)
        "#,
        )
        .bind(&card.card_hash)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn add_cards_batch(&self, cards: &[Card]) -> Result<()> {
        let mut tx = self.pool.begin().await?;

        let now = chrono::Utc::now().to_rfc3339();

        for card in cards {
            sqlx::query(
                r#"
            INSERT or ignore INTO cards (
                card_hash,
                added_at,
                last_reviewed_at,
                stability,
                difficulty,
                interval_raw,
                interval_days,
                due_date,
                review_count
            )
            VALUES (?, ?, NULL, NULL, NULL, NULL, 0, NULL, 0)
            "#,
            )
            .bind(&card.card_hash)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    pub async fn card_exists(&self, card: &Card) -> Result<bool> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(1) FROM cards WHERE card_hash = ?")
            .bind(&card.card_hash)
            .fetch_one(&self.pool)
            .await?;
        Ok(count > 0)
    }

    pub async fn update_card_performance(
        &self,
        card: &Card,
        review_status: ReviewStatus,
    ) -> Result<bool> {
        let current_performance = self.get_card_performance(card).await?;
        let now = chrono::Utc::now();
        let new_performance = update_performance(current_performance, review_status, now);
        let card_hash = card.card_hash.clone();

        let result = sqlx::query(
            r#"
            UPDATE cards
            SET
                last_reviewed_at = ?,
                stability = ?,
                difficulty = ?,
                interval_raw = ?,
                interval_days = ?,
                due_date = ?,
                review_count = ?
            WHERE card_hash = ?
            "#,
        )
        .bind(new_performance.last_reviewed_at)
        .bind(new_performance.stability)
        .bind(new_performance.difficulty)
        .bind(new_performance.interval_raw)
        .bind(new_performance.interval_days as i64)
        .bind(new_performance.due_date)
        .bind(new_performance.review_count as i64)
        .bind(card_hash)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_card_performance(&self, card: &Card) -> Result<Performance> {
        let card_hash = card.card_hash.clone();
        let sql = "SELECT added_at, last_reviewed_at, stability, difficulty, interval_raw, interval_days, due_date, review_count 
           FROM cards
           WHERE card_hash = ?;";

        let row = sqlx::query(sql)
            .bind(card_hash)
            .fetch_one(&self.pool)
            .await?;

        let review_count: i64 = row.get("review_count");
        if review_count == 0 {
            return Ok(Performance::default());
        }
        let reviewed = ReviewedPerformance {
            last_reviewed_at: row.get("last_reviewed_at"),
            stability: row.get("stability"),
            difficulty: row.get("difficulty"),
            interval_raw: row.get("interval_raw"),
            interval_days: row.get::<i64, _>("interval_days") as usize,
            due_date: row.get("due_date"),
            review_count: review_count as usize,
        };

        Ok(Performance::Reviewed(reviewed))
    }

    pub async fn due_today(
        &self,
        card_hashes: HashMap<String, Card>,
        card_limit: Option<usize>,
    ) -> Result<Vec<Card>> {
        let now = chrono::Utc::now().to_rfc3339();

        let sql = "SELECT card_hash 
           FROM cards
           WHERE due_date <= ? OR due_date IS NULL;";
        let mut rows = sqlx::query(sql).bind(now).fetch(&self.pool);
        let mut cards = Vec::new();
        while let Some(row) = rows.try_next().await? {
            let card_hash: String = row.get("card_hash");
            if !card_hashes.contains_key(&card_hash) {
                continue;
            }

            if let Some(card) = card_hashes.get(&card_hash) {
                cards.push(card.clone());
            }

            if let Some(card_limit) = card_limit
                && cards.len() >= card_limit
            {
                break;
            }
        }

        Ok(cards)
    }

    pub async fn collection_stats(&self, card_hashes: &HashMap<String, Card>) -> Result<CardStats> {
        let now = chrono::Utc::now();
        let week_horizon = now + chrono::Duration::days(7);
        let month_horizon = now + chrono::Duration::days(30);

        let mut stats = CardStats::default();
        stats.num_cards = card_hashes.len() as i64;
        let mut upcoming_week_counts: BTreeMap<String, i64> = BTreeMap::new();

        let mut rows = sqlx::query(
            r#"
            SELECT card_hash, review_count, due_date
            FROM cards
            "#,
        )
        .fetch(&self.pool);

        while let Some(row) = rows.try_next().await? {
            let card_hash: String = row.get("card_hash");
            stats.total_cards_in_db += 1;
            if !card_hashes.contains_key(&card_hash) {
                continue;
            }

            let review_count: i64 = row.get("review_count");
            if review_count == 0 {
                stats.new_cards += 1;
            } else {
                stats.reviewed_cards += 1;
            }

            let due_date = row
                .try_get::<Option<String>, _>("due_date")?
                .and_then(|due| chrono::DateTime::parse_from_rfc3339(&due).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            match due_date {
                None => {
                    stats.due_cards += 1;
                }
                Some(due_date) => {
                    if due_date <= now {
                        stats.due_cards += 1;
                        if due_date < now {
                            stats.overdue_cards += 1;
                        }
                    } else {
                        if due_date <= week_horizon {
                            let day = due_date.format("%Y-%m-%d").to_string();
                            *upcoming_week_counts.entry(day).or_insert(0) += 1;
                        }

                        if due_date <= month_horizon {
                            stats.upcoming_month += 1;
                        }
                    }
                }
            }
        }

        stats.upcoming_week = upcoming_week_counts
            .into_iter()
            .map(|(day, count)| UpcomingCount { day, count })
            .collect();

        Ok(stats)
    }
}

async fn probe_schema_exists(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let sql = "select count(*) from sqlite_master where type='table' AND name=?;";

    let count: (i64,) = sqlx::query_as(sql).bind("cards").fetch_one(pool).await?;
    Ok(count.0 > 0)
}
