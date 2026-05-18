use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Achievement {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub milestone: i32,
    pub achievement_type: String,
    pub icon_url: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserAchievement {
    pub id: Uuid,
    pub user_id: Uuid,
    pub achievement_id: Uuid,
    pub unlocked_at: DateTime<Utc>,
    pub is_visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DailyMission {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub target_type: String,
    pub target_count: i32,
    pub xp_reward: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct UserMission {
    pub id: Uuid,
    pub user_id: Uuid,
    pub mission_id: Uuid,
    pub progress: i32,
    pub claimed: bool,
    pub date: chrono::NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Clan {
    pub id: Uuid,
    pub name: String,
    pub tier: String,
    pub total_score: f64,
    pub leader_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ClanMember {
    pub id: Uuid,
    pub clan_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub joined_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buff {
    pub id: Uuid,
    pub clan_id: Uuid,
    pub buff_type: String,
    pub multiplier: f64,
    pub activated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizCompletedPayload {
    pub user_id: String,
    pub reading_id: String,
    pub score: i32,
    pub accuracy: f64,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingCompletedPayload {
    pub user_id: String,
    pub reading_id: String,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub is_read: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Season {
    pub id: Uuid,
    pub name: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonRankingEntry {
    pub clan_id: String,
    pub clan_name: String,
    pub total_score: f64,
    pub new_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeasonEndedPayload {
    pub season_id: String,
    pub rankings: Vec<SeasonRankingEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissionProgressPayload {
    pub user_id: String,
    pub mission_id: String,
    pub progress: i32,
    pub target: i32,
    pub xp_reward: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    #[serde(rename = "eventId")]
    pub event_id: String,
    #[serde(rename = "eventType")]
    pub event_type: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}
