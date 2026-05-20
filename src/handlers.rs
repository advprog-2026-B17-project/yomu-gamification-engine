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

fn extract_header_user_id(req: &HttpRequest) -> Option<Uuid> {
    req.headers()
        .get("X-User-Id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AchievementRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub milestone: i32,
    pub icon_url: Option<String>,
    pub unlocked: bool,
    pub unlocked_at: Option<chrono::DateTime<chrono::Utc>>,
    pub visible: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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
#[serde(rename_all = "camelCase")]
pub struct ClanRow {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub total_score: f64,
    pub leader_id: String,
    pub leader_name: String,
    pub member_count: i64,
    pub my_role: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ClanLeaderboardEntry {
    pub clan_id: String,
    pub clan_name: String,
    pub tier: String,
    pub total_score: f64,
    pub member_count: i64,
    pub multiplier: f64,
    pub effective_score: f64,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
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
    pub created_at: chrono::DateTime<chrono::Utc>,
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
        SELECT a.id::text as id, a.name, a.description, a.milestone, a.icon_url,
               true as unlocked, ua.unlocked_at, ua.is_visible as visible
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
        SELECT dm.id::text as id, dm.title, dm.description, dm.target_type, dm.target_count, dm.xp_reward,
               COALESCE(um.progress, 0) as progress,
               COALESCE(um.claimed, false) as claimed,
               COALESCE(um.date, CURRENT_DATE) as date
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

pub async fn get_clans(
    pool: web::Data<PgPool>,
    req: HttpRequest,
) -> HttpResponse {
    let user_id = extract_header_user_id(&req);

    match sqlx::query_as::<_, ClanRow>(
        r#"
        SELECT c.id::text as id, c.name, c.tier,
               (COALESCE(c.total_score, 0) * COALESCE(EXP(SUM(LN(b.multiplier)) FILTER (WHERE b.expires_at IS NULL)), 1.0))::float8 as total_score,
               c.leader_id::text as leader_id,
               COALESCE(u.display_name, u.username, 'Unknown') as leader_name,
               COUNT(DISTINCT cm.id)::int8 as member_count,
               MAX(CASE WHEN cm.user_id = $1 THEN cm.role ELSE NULL END) as my_role
        FROM gamification.clans c
        LEFT JOIN auth.users u ON c.leader_id = u.id
        LEFT JOIN gamification.clan_members cm ON c.id = cm.clan_id
        LEFT JOIN gamification.buffs b ON c.id = b.clan_id
        GROUP BY c.id, c.name, c.tier, c.total_score, c.leader_id, u.display_name, u.username
        ORDER BY total_score DESC, c.name ASC
        "#
    )
    .bind(user_id)
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(clans) => HttpResponse::Ok().json(clans),
        Err(e) => {
            tracing::error!("Failed to fetch clans: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_my_clan(
    pool: web::Data<PgPool>,
    req: HttpRequest,
) -> HttpResponse {
    let user_id = match extract_header_user_id(&req) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Missing user ID"})),
    };

    match sqlx::query_as::<_, ClanRow>(
        r#"
        SELECT c.id::text as id, c.name, c.tier, COALESCE(c.total_score, 0)::float8 as total_score,
               c.leader_id::text as leader_id,
               COALESCE(u.display_name, u.username, 'Unknown') as leader_name,
               COUNT(all_members.id)::int8 as member_count,
               member.role as my_role
        FROM gamification.clan_members member
        JOIN gamification.clans c ON member.clan_id = c.id
        LEFT JOIN auth.users u ON c.leader_id = u.id
        LEFT JOIN gamification.clan_members all_members ON c.id = all_members.clan_id
        WHERE member.user_id = $1
        GROUP BY c.id, c.name, c.tier, c.total_score, c.leader_id, u.display_name, u.username, member.role
        LIMIT 1
        "#
    )
    .bind(user_id)
    .fetch_optional(pool.get_ref())
    .await
    {
        Ok(Some(clan)) => HttpResponse::Ok().json(clan),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({"error": "Clan not found"})),
        Err(e) => {
            tracing::error!("Failed to fetch user's clan: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({"error": "Internal error"}))
        }
    }
}

pub async fn get_global_clan_leaderboard(
    pool: web::Data<PgPool>,
) -> HttpResponse {
    match sqlx::query_as::<_, ClanLeaderboardEntry>(
        r#"
        SELECT c.id::text as clan_id, c.name as clan_name, c.tier,
               COALESCE(c.total_score, 0)::float8 as total_score,
               COUNT(DISTINCT cm.id)::int8 as member_count,
               COALESCE(EXP(SUM(LN(b.multiplier)) FILTER (WHERE b.expires_at IS NULL)), 1.0)::float8 as multiplier,
               (COALESCE(c.total_score, 0) * COALESCE(EXP(SUM(LN(b.multiplier)) FILTER (WHERE b.expires_at IS NULL)), 1.0))::float8 as effective_score
        FROM gamification.clans c
        LEFT JOIN gamification.clan_members cm ON c.id = cm.clan_id
        LEFT JOIN gamification.buffs b ON c.id = b.clan_id
        GROUP BY c.id, c.name, c.tier, c.total_score
        ORDER BY effective_score DESC, c.name ASC
        "#
    )
    .fetch_all(pool.get_ref())
    .await
    {
        Ok(leaderboard) => HttpResponse::Ok().json(leaderboard),
        Err(e) => {
            tracing::error!("Failed to fetch global clan leaderboard: {}", e);
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
        SELECT u.id::text as user_id, u.display_name, u.username,
               COALESCE(SUM(cr.score), 0) as total_score,
               COUNT(cr.id) as readings_completed,
               COALESCE(AVG(cr.accuracy), 0)::float8 as avg_accuracy
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
        SELECT id::text as id, user_id::text as user_id, notification_type, title, message, is_read, created_at
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
    req: HttpRequest,
) -> HttpResponse {
    let user_id = match extract_header_user_id(&req) {
        Some(id) => id,
        None => return HttpResponse::Unauthorized().json(serde_json::json!({"error": "Missing user ID"})),
    };
    let notification_id = match Uuid::parse_str(&path.into_inner()) {
        Ok(id) => id,
        Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({"error": "Invalid notification ID"})),
    };

    match sqlx::query(
        "UPDATE gamification.notifications SET is_read = true WHERE id = $1 AND user_id = $2"
    )
    .bind(notification_id)
    .bind(user_id)
    .execute(pool.get_ref())
    .await
    {
        Ok(result) if result.rows_affected() > 0 => HttpResponse::Ok().json(serde_json::json!({"status": "ok"})),
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({"error": "Notification not found"})),
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
