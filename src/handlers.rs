use actix_web::{web, HttpRequest, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Extract user_id from X-User-Id header first, fallback to path parameter
fn extract_user_id(req: &HttpRequest, path_user_id: &str) -> Option<Uuid> {
    // Try X-User-Id header first (from gateway)
    req.headers()
        .get("X-User-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        // Fallback to path parameter
        .or_else(|| Uuid::parse_str(path_user_id).ok())
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct AchievementRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub milestone: i32,
    pub icon_url: Option<String>,
    pub unlocked_at: Option<chrono::NaiveDateTime>,
    pub is_visible: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct MissionRow {
    pub id: String,
    pub title: String,
    pub description: String,
    pub target_type: String,
    pub target_count: i32,
    pub xp_reward: i32,
    pub progress: Option<i32>,
    pub claimed: Option<bool>,
    pub date: Option<chrono::NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct LeaderboardEntry {
    pub user_id: String,
    pub display_name: String,
    pub username: String,
    pub total_score: i64,
    pub readings_completed: i64,
    pub avg_accuracy: f64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct NotificationRow {
    pub id: String,
    pub user_id: String,
    pub notification_type: String,
    pub title: String,
    pub message: String,
    pub is_read: bool,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({"status": "ok"}))
}

pub async fn get_user_achievements(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    req: HttpRequest,
) -> HttpResponse {
    let path_user_id = path.into_inner();
    let user_id = match extract_user_id(&req, &path_user_id) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid or missing user ID"})),
    };

    match sqlx::query_as::<_, AchievementRow>(
        r#"
        SELECT a.id, a.name, a.description, a.milestone, a.icon_url, ua.unlocked_at, ua.is_visible
        FROM gamification.achievements a
        JOIN gamification.user_achievements ua ON a.id = ua.achievement_id
        WHERE ua.user_id = $1
        ORDER BY ua.unlocked_at DESC
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(achievements) => HttpResponse::Ok().json(achievements),
        Err(e) => {
            tracing::error!("Failed to fetch achievements: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_user_missions(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    req: HttpRequest,
) -> HttpResponse {
    let path_user_id = path.into_inner();
    let user_id = match extract_user_id(&req, &path_user_id) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid or missing user ID"})),
    };

    match sqlx::query_as::<_, MissionRow>(
        r#"
        SELECT dm.id, dm.title, dm.description, dm.target_type, dm.target_count, dm.xp_reward,
               um.progress, um.claimed, um.date
        FROM gamification.daily_missions dm
        LEFT JOIN gamification.user_missions um ON dm.id = um.mission_id AND um.user_id = $1 AND um.date = CURRENT_DATE
        WHERE dm.is_active = true
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(missions) => HttpResponse::Ok().json(missions),
        Err(e) => {
            tracing::error!("Failed to fetch missions: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_clan_leaderboard(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> HttpResponse {
    let clan_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid clan ID"})),
    };

    match sqlx::query_as::<_, LeaderboardEntry>(
        r#"
        SELECT u.id as user_id, u.display_name, u.username,
               COALESCE(SUM(cr.score), 0) as total_score,
               COUNT(cr.id) as readings_completed,
               COALESCE(AVG(cr.accuracy), 0) as avg_accuracy
        FROM gamification.clan_members cm
        JOIN auth.users u ON cm.user_id = u.id
        LEFT JOIN quiz.completed_readings cr ON u.id = cr.user_id
        WHERE cm.clan_id = $1
        GROUP BY u.id, u.display_name, u.username
        ORDER BY total_score DESC
        "#
    )
    .bind(clan_id)
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(leaderboard) => HttpResponse::Ok().json(leaderboard),
        Err(e) => {
            tracing::error!("Failed to fetch leaderboard: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_user_notifications(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    req: HttpRequest,
) -> HttpResponse {
    let path_user_id = path.into_inner();
    let user_id = match extract_user_id(&req, &path_user_id) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid or missing user ID"})),
    };

    match sqlx::query_as::<_, NotificationRow>(
        r#"
        SELECT id, user_id, notification_type, title, message, is_read, created_at
        FROM gamification.notifications
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(notifications) => HttpResponse::Ok().json(notifications),
        Err(e) => {
            tracing::error!("Failed to fetch notifications: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn mark_notification_read(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
) -> HttpResponse {
    let notification_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid notification ID"})),
    };

    match sqlx::query(
        "UPDATE gamification.notifications SET is_read = true WHERE id = $1"
    )
    .bind(notification_id)
    .execute(pool.get_ref())
    .await
    {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok"})),
        Err(e) => {
            tracing::error!("Failed to mark notification as read: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_unread_notification_count(
    pool: web::Data<PgPool>,
    path: web::Path<String>,
    req: HttpRequest,
) -> HttpResponse {
    let path_user_id = path.into_inner();
    let user_id = match extract_user_id(&req, &path_user_id) {
        Some(id) => id,
        None => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid or missing user ID"})),
    };

    match sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM gamification.notifications WHERE user_id = $1 AND is_read = false"
    )
    .bind(user_id)
    .fetch_one(pool.get_ref())
    .await
    {
        Ok(count) => HttpResponse::Ok().json(serde_json::json!({"count": count})),
        Err(e) => {
            tracing::error!("Failed to count notifications: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}