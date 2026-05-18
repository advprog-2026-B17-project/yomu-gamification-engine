mod config;
mod db;
mod handlers;
mod models;
mod rabbitmq;
mod services;

use actix_web::{web, App, HttpServer, middleware};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = config::load();

    let pool = db::create_pool(&config.database_url).await
        .expect("Failed to create database pool");

    let (rabbitmq_channel, consumer) = rabbitmq::connect(&config.rabbitmq_url)
        .await
        .expect("Failed to connect to RabbitMQ");

    tracing::info!("Connected to RabbitMQ, starting consumer");

    let pool_for_consumer = pool.clone();
    tokio::spawn(async move {
        rabbitmq::consume_events(consumer, pool_for_consumer).await;
    });

    tracing::info!("Starting yomu-gamification-engine on :8081");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .app_data(web::Data::new(pool.clone()))
            .route("/health", web::get().to(handlers::health))
            .route("/api/achievements/{user_id}", web::get().to(handlers::get_user_achievements))
            .route("/api/missions/{user_id}", web::get().to(handlers::get_user_missions))
            .route("/api/clans/{clan_id}/leaderboard", web::get().to(handlers::get_clan_leaderboard))
            .route("/api/notifications/{user_id}", web::get().to(handlers::get_user_notifications))
            .route("/api/notifications/{user_id}/unread-count", web::get().to(handlers::get_unread_notification_count))
            .route("/api/notifications/read/{notification_id}", web::put().to(handlers::mark_notification_read))
    })
    .bind(format!("0.0.0.0:{}", config.server_port))?
    .run()
    .await
}
