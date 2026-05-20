use sqlx::PgPool;
use uuid::Uuid;

use crate::models::{Achievement, QuizCompletedPayload, ReadingCompletedPayload, SeasonEndedPayload, UserMission};

pub async fn handle_reading_completed(event: &crate::models::EventEnvelope, pool: &PgPool) {
    tracing::info!("handle_reading_completed called for event_type: {}", event.event_type);

    let payload_result = serde_json::from_value::<ReadingCompletedPayload>(event.payload.clone());
    tracing::info!("Payload parse result: {:?}", payload_result);

    if let Ok(payload) = payload_result {
        let user_id: Uuid = match Uuid::parse_str(&payload.user_id) {
            Ok(id) => {
                tracing::info!("Parsed user_id: {}", id);
                id
            }
            Err(_) => {
                tracing::warn!("Invalid user_id: {}", payload.user_id);
                return;
            }
        };

        // Update daily mission progress - only for reading target type
        let today = chrono::Utc::now().date_naive();

        // Get all active missions with target_type 'reading' only
        let missions_result = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT dm.id, dm.target_type FROM gamification.daily_missions dm WHERE dm.is_active = true AND dm.target_type = 'reading'"
        )
        .fetch_all(pool)
        .await;

        match missions_result {
            Ok(missions) => {
                tracing::info!("Found {} active reading missions for user {}", missions.len(), user_id);

                for (mission_id, target_type) in missions {
                    tracing::debug!("Checking mission {} (type: {}) for user {}", mission_id, target_type, user_id);

                    // Find user's mission record for this daily mission
                    let um_result = sqlx::query_as::<_, UserMission>(
                        "SELECT * FROM gamification.user_missions WHERE user_id = $1 AND mission_id = $2 AND date = $3 AND claimed = false"
                    )
                    .bind(user_id)
                    .bind(mission_id)
                    .bind(today)
                    .fetch_optional(pool)
                    .await;

                    match um_result {
                        Ok(Some(um)) => {
                            tracing::info!("Found user_mission {} for today, incrementing progress", um.id);
                            if let Err(e) = sqlx::query(
                                "UPDATE gamification.user_missions SET progress = progress + 1 WHERE id = $1"
                            )
                            .bind(um.id)
                            .execute(pool)
                            .await
                            {
                                tracing::error!("Failed to update mission progress: {}", e);
                            } else {
                                tracing::info!("Updated mission {} (type: {}) for user {}", mission_id, target_type, user_id);
                            }
                        }
                        Ok(None) => {
                            tracing::debug!("No user_mission found for mission {} on date {}", mission_id, today);
                        }
                        Err(e) => {
                            tracing::error!("Error fetching user_mission: {}", e);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch missions: {}", e);
            }
        }

        // Check achievements
        check_and_unlock_achievements(user_id, pool).await;
    }
}

pub async fn handle_quiz_completed(event: &crate::models::EventEnvelope, pool: &PgPool) {
    tracing::info!("handle_quiz_completed called for event_type: {}", event.event_type);

    let payload_result = serde_json::from_value::<QuizCompletedPayload>(event.payload.clone());
    tracing::info!("Payload parse result: {:?}", payload_result);

    if let Ok(payload) = payload_result {

        let user_id: Uuid = match Uuid::parse_str(&payload.user_id) {
            Ok(id) => {
                tracing::info!("Parsed user_id: {}", id);
                id
            }
            Err(_) => {
                tracing::warn!("Invalid user_id: {}", payload.user_id);
                return;
            }
        };

        // Update daily mission progress - find user missions with reading/quiz target types
        let today = chrono::Utc::now().date_naive();

        // Get all active missions with target_type 'reading' or 'quiz'
        let missions_result = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT dm.id, dm.target_type FROM gamification.daily_missions dm WHERE dm.is_active = true AND dm.target_type IN ('reading', 'quiz')"
        )
        .fetch_all(pool)
        .await;

        match missions_result {
            Ok(missions) => {
                tracing::info!("Found {} active missions for user {}", missions.len(), user_id);

            for (mission_id, target_type) in missions {
                tracing::debug!("Checking mission {} (type: {}) for user {}", mission_id, target_type, user_id);

                // Find user's mission record for this daily mission
                let um_result = sqlx::query_as::<_, UserMission>(
                    "SELECT * FROM gamification.user_missions WHERE user_id = $1 AND mission_id = $2 AND date = $3 AND claimed = false"
                )
                .bind(user_id)
                .bind(mission_id)
                .bind(today)
                .fetch_optional(pool)
                .await;

                match um_result {
                    Ok(Some(um)) => {
                        tracing::info!("Found user_mission {} for today, incrementing progress", um.id);
                        // Increment progress
                        if let Err(e) = sqlx::query(
                            "UPDATE gamification.user_missions SET progress = progress + 1 WHERE id = $1"
                        )
                        .bind(um.id)
                        .execute(pool)
                        .await
                        {
                            tracing::error!("Failed to update mission progress: {}", e);
                        } else {
                            tracing::info!("Updated mission {} (type: {}) for user {}", mission_id, target_type, user_id);
                        }
                    }
                    Ok(None) => {
                        tracing::debug!("No user_mission found for mission {} on date {}", mission_id, today);
                    }
                    Err(e) => {
                        tracing::error!("Error fetching user_mission: {}", e);
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to fetch missions: {}", e);
        }
    }

        // Check achievements
        check_and_unlock_achievements(user_id, pool).await;

        // Check for "Ahli Kuis" perfect score achievement (score == 100)
        check_and_unlock_quiz_perfect_achievement(user_id, payload.score, pool).await;
    }
}

pub async fn handle_mission_progress(event: &crate::models::EventEnvelope, pool: &PgPool) {
    tracing::info!("Processing mission.progress event");

    // Parse payload with xp_reward field
    let payload = match serde_json::from_value::<crate::models::MissionProgressPayload>(event.payload.clone()) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to parse mission.progress payload: {}", e);
            return;
        }
    };

    // Parse user_id
    let user_id = match uuid::Uuid::parse_str(&payload.user_id) {
        Ok(id) => id,
        Err(_) => {
            tracing::warn!("Invalid user_id in mission.progress: {}", payload.user_id);
            return;
        }
    };

    let xp_reward = payload.xp_reward;

    // Find user's clan_id from clan_members table
    let clan_id: Option<uuid::Uuid> = sqlx::query_scalar(
        "SELECT clan_id FROM gamification.clan_members WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    // If user has clan, update clan total_score only (tier changes at season end)
    if let Some(clan_id) = clan_id {
        // Update score only - tier changes ONLY happen in handle_season_ended
        sqlx::query("UPDATE gamification.clans SET total_score = total_score + $1 WHERE id = $2")
            .bind(xp_reward)
            .bind(clan_id)
            .execute(pool)
            .await
            .ok();

        tracing::info!("Added {} XP to clan {} - tier will update at season end", xp_reward, clan_id);
    } else {
        tracing::info!("User {} has no clan, skipping score update", user_id);
    }
}

pub async fn handle_season_ended(event: &crate::models::EventEnvelope, pool: &PgPool) {
    tracing::info!("Processing season.ended event");

    if let Ok(payload) = serde_json::from_value::<SeasonEndedPayload>(event.payload.clone()) {
        let season_id = match uuid::Uuid::parse_str(&payload.season_id) {
            Ok(id) => id,
            Err(_) => {
                tracing::warn!("Invalid season_id: {}", payload.season_id);
                return;
            }
        };

        // Deactivate the current season
        sqlx::query("UPDATE gamification.seasons SET is_active = false WHERE id = $1")
            .bind(season_id)
            .execute(pool)
            .await
            .ok();

        // Update clan tiers based on final rankings
        for ranking in &payload.rankings {
            let clan_id = match uuid::Uuid::parse_str(&ranking.clan_id) {
                Ok(id) => id,
                Err(_) => {
                    tracing::warn!("Invalid clan_id in ranking: {}", ranking.clan_id);
                    continue;
                }
            };

            sqlx::query("UPDATE gamification.clans SET tier = $1 WHERE id = $2")
                .bind(&ranking.new_tier)
                .bind(clan_id)
                .execute(pool)
                .await
                .ok();

            // Create notification for tier change
            let title = format!("Season Ended: {} promoted to {}", ranking.clan_name, ranking.new_tier);
            let message = format!(
                "Congratulations! Your clan {} has been promoted to {} tier after the season ended!",
                ranking.clan_name, ranking.new_tier
            );

            // Notify all clan members
            if let Ok(members) = sqlx::query_as::<_, (uuid::Uuid,)>(
                "SELECT user_id FROM gamification.clan_members WHERE clan_id = $1"
            )
            .bind(clan_id)
            .fetch_all(pool)
            .await
            {
                for (user_id,) in members {
                    let notification_id = uuid::Uuid::new_v4();
                    sqlx::query(
                        r#"INSERT INTO gamification.notifications (id, user_id, notification_type, title, message, is_read, created_at)
                           VALUES ($1, $2, 'season_ended', $3, $4, false, NOW())"#
                    )
                    .bind(notification_id)
                    .bind(user_id)
                    .bind(&title)
                    .bind(&message)
                    .execute(pool)
                    .await
                    .ok();
                }
            }

            tracing::info!("Updated clan {} to tier {}", ranking.clan_name, ranking.new_tier);
        }

        tracing::info!("Completed season ended processing for season {}", payload.season_id);
    } else {
        tracing::warn!("Failed to parse season.ended payload");
    }
}

async fn check_and_unlock_achievements(user_id: Uuid, pool: &PgPool) {
    tracing::info!("Mengecek achievement untuk user_id: {}", user_id);

    // Count completed readings
    let count: Result<i64, _> = sqlx::query_scalar(
        "SELECT COUNT(*) FROM quiz.completed_readings WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_one(pool)
    .await;

    match count {
        Ok(completed_count) => {
            tracing::info!("User {} punya {} completed readings", user_id, completed_count);

            let achievements = sqlx::query_as::<_, Achievement>(
                "SELECT * FROM gamification.achievements WHERE milestone <= $1 AND achievement_type = 'reading_count'"
            )
            .bind(completed_count as i32)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            tracing::info!("Ditemukan {} achievement potensial untuk di-unlock", achievements.len());

            for achievement in achievements {
                let exists = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM gamification.user_achievements WHERE user_id = $1 AND achievement_id = $2)"
                )
                .bind(user_id)
                .bind(achievement.id)
                .fetch_one(pool)
                .await
                .unwrap_or(false);

                if !exists {
                    if let Err(e) = sqlx::query(
                        "INSERT INTO gamification.user_achievements (id, user_id, achievement_id, unlocked_at, is_visible)
                         VALUES ($1, $2, $3, NOW(), true)
                         ON CONFLICT (user_id, achievement_id) DO NOTHING"
                    )
                    .bind(Uuid::new_v4())
                    .bind(user_id)
                    .bind(achievement.id)
                    .execute(pool)
                    .await
                    {
                        tracing::error!("Gagal insert user_achievement: {}", e);
                    } else {
                        let notification_id = Uuid::new_v4();
                        let notif_title = format!("Achievement Terunlock: {}!", achievement.name);
                        let notif_message = achievement.description.clone().unwrap_or_else(|| "Kamu telah membuka achievement baru".to_string());

                        if let Err(e) = sqlx::query(
                            r#"INSERT INTO gamification.notifications (id, user_id, notification_type, title, message, is_read, created_at)
                               VALUES ($1, $2, 'achievement_unlocked', $3, $4, false, NOW())"#
                        )
                        .bind(notification_id)
                        .bind(user_id)
                        .bind(&notif_title)
                        .bind(&notif_message)
                        .execute(pool)
                        .await
                        {
                            tracing::error!("Gagal insert notification: {}", e);
                        }

                        tracing::info!("✅ Berhasil unlock achievement: {}", achievement.name);
                    }
                } else {
                    tracing::info!("Achievement '{}' sudah pernah di-unlock sebelumnya", achievement.name);
                }
            }
        }
        Err(e) => {
            tracing::error!("❌ Gagal query COUNT dari quiz.completed_readings: {}", e);
        }
    }
}

async fn check_and_unlock_quiz_perfect_achievement(user_id: Uuid, score: i32, pool: &PgPool) {
    // Only check for perfect score achievement if score is 100
    if score != 100 {
        tracing::debug!("Skipping perfect score achievement check - score is {}", score);
        return;
    }

    tracing::info!("Checking Ahli Kuis achievement for user {} with perfect score {}", user_id, score);

    // Find the "Ahli Kuis" achievement - check by name containing "Ahli Kuis" or type "quiz_perfect"
    let achievement_result = sqlx::query_as::<_, Achievement>(
        "SELECT * FROM gamification.achievements WHERE name LIKE '%Ahli Kuis%' OR achievement_type = 'quiz_perfect'"
    )
    .fetch_optional(pool)
    .await;

    match achievement_result {
        Ok(Some(achievement)) => {
            tracing::info!("Found Ahli Kuis achievement: {} (id: {})", achievement.name, achievement.id);

            // Check if already unlocked
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM gamification.user_achievements WHERE user_id = $1 AND achievement_id = $2)"
            )
            .bind(user_id)
            .bind(achievement.id)
            .fetch_one(pool)
            .await
            .unwrap_or(false);

            if !exists {
                if let Err(e) = sqlx::query(
                    "INSERT INTO gamification.user_achievements (id, user_id, achievement_id, unlocked_at, is_visible)
                     VALUES ($1, $2, $3, NOW(), true)
                     ON CONFLICT (user_id, achievement_id) DO NOTHING"
                )
                .bind(Uuid::new_v4())
                .bind(user_id)
                .bind(achievement.id)
                .execute(pool)
                .await
                {
                    tracing::error!("Gagal insert user_achievement for Ahli Kuis: {}", e);
                } else {
                    let notification_id = Uuid::new_v4();
                    let notif_title = format!("Achievement Terunlock: {}!", achievement.name);
                    let notif_message = achievement.description.clone().unwrap_or_else(|| "Skor sempurna! Kamu adalah Ahli Kuis!".to_string());

                    if let Err(e) = sqlx::query(
                        r#"INSERT INTO gamification.notifications (id, user_id, notification_type, title, message, is_read, created_at)
                           VALUES ($1, $2, 'achievement_unlocked', $3, $4, false, NOW())"#
                    )
                    .bind(notification_id)
                    .bind(user_id)
                    .bind(&notif_title)
                    .bind(&notif_message)
                    .execute(pool)
                    .await
                    {
                        tracing::error!("Gagal insert notification for Ahli Kuis: {}", e);
                    }

                    tracing::info!("✅ Berhasil unlock Ahli Kuis achievement for user {}", user_id);
                }
            } else {
                tracing::info!("Achievement 'Ahli Kuis' already unlocked for user {}", user_id);
            }
        }
        Ok(None) => {
            tracing::warn!("Ahli Kuis achievement not found in database");
        }
        Err(e) => {
            tracing::error!("❌ Gagal query Ahli Kuis achievement: {}", e);
        }
    }
}

pub async fn recalculate_clan_buffs(clan_id: Uuid, pool: &PgPool) {
    // Get clan member count
    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM gamification.clan_members WHERE clan_id = $1"
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(1)
    .max(1);

    // Calculate Productivity Buff: >=50% members completed today's missions
    let completed_today: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT um.user_id)
        FROM gamification.user_missions um
        JOIN gamification.clan_members cm ON um.user_id = cm.user_id
        WHERE cm.clan_id = $1 AND um.date = CURRENT_DATE AND um.progress >= 1
        "#
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let productivity_ratio = completed_today as f64 / member_count as f64;
    activate_buff_if_needed(clan_id, "productivity_buff", if productivity_ratio >= 0.5 { 1.2 } else { 1.0 }, pool).await;

    // Calculate Low Accuracy Debuff: avg accuracy <50%
    let avg_accuracy: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(AVG(cr.accuracy), 0)::float8
        FROM quiz.completed_readings cr
        JOIN gamification.clan_members cm ON cr.user_id = cm.user_id
        WHERE cm.clan_id = $1 AND cr.completed_at >= NOW() - INTERVAL '7 days'
        "#
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0.0);

    if avg_accuracy < 0.5 {
        activate_buff_if_needed(clan_id, "low_accuracy_penalty", 0.8, pool).await;
    } else if avg_accuracy >= 0.8 {
        activate_buff_if_needed(clan_id, "consistent_reader_buff", 1.1, pool).await;
    }

    // Calculate Inactive Debuff: <30% members active in last 3 days
    let active_members: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT cr.user_id)
        FROM quiz.completed_readings cr
        JOIN gamification.clan_members cm ON cr.user_id = cm.user_id
        WHERE cm.clan_id = $1 AND cr.completed_at >= NOW() - INTERVAL '3 days'
        "#
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0);

    let activity_ratio = active_members as f64 / member_count as f64;
    if activity_ratio < 0.3 {
        activate_buff_if_needed(clan_id, "inactive_penalty", 0.9, pool).await;
    }

    // Update clan's effective score with multipliers
    let base_score: f64 = sqlx::query_scalar(
        "SELECT COALESCE(total_score, 0)::float8 FROM gamification.clans WHERE id = $1"
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(0.0);

    let multiplier = calculate_effective_multiplier(clan_id, pool).await;
    let effective_score = base_score * multiplier;

    if let Err(e) = sqlx::query("UPDATE gamification.clans SET total_score = $1 WHERE id = $2")
        .bind(effective_score)
        .bind(clan_id)
        .execute(pool)
        .await
    {
        tracing::error!("Gagal update total_score clan {}: {}", clan_id, e);
    }
}

async fn activate_buff_if_needed(clan_id: Uuid, buff_type: &str, multiplier: f64, pool: &PgPool) {
    // Deactivate existing same-type buffs
    sqlx::query(
        "UPDATE gamification.buffs SET expires_at = NOW() WHERE clan_id = $1 AND buff_type = $2 AND expires_at IS NULL"
    )
    .bind(clan_id)
    .bind(buff_type)
    .execute(pool)
    .await
    .ok();

    // Only activate if multiplier != 1.0 (i.e., actually a buff or debuff)
    if (multiplier - 1.0).abs() > 0.001 {
        if let Err(e) = sqlx::query(
            r#"
            INSERT INTO gamification.buffs (id, clan_id, buff_type, multiplier, activated_at, expires_at)
            VALUES ($1, $2, $3, $4, NOW(), NULL)
            "#
        )
        .bind(Uuid::new_v4())
        .bind(clan_id)
        .bind(buff_type)
        .bind(multiplier)
        .execute(pool)
        .await
        {
            tracing::error!("Gagal insert buff {} untuk clan {}: {}", buff_type, clan_id, e);
        }
    }
}

async fn calculate_effective_multiplier(clan_id: Uuid, pool: &PgPool) -> f64 {
    let multiplier: f64 = sqlx::query_scalar(
        "SELECT COALESCE(AVG(multiplier), 1.0)::float8 FROM gamification.buffs WHERE clan_id = $1 AND expires_at IS NULL"
    )
    .bind(clan_id)
    .fetch_one(pool)
    .await
    .unwrap_or(1.0);

    multiplier
}
