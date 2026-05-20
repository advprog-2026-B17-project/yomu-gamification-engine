mod config;
mod db;
mod handlers;
mod models;
mod rabbitmq;
mod services;

use actix_web::{dev::Service, web, App, HttpResponse, HttpServer, middleware};
use futures_util::future::{ready, Either, FutureExt};
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

    let gateway_shared_secret = config.gateway_shared_secret.clone();

    tracing::info!("Starting yomu-gamification-engine on :{}", config.server_port);

    HttpServer::new(move || {
        let gateway_shared_secret = gateway_shared_secret.clone();

        App::new()
            .wrap(middleware::Logger::default())
            .wrap_fn(move |req, srv| {
                let gateway_shared_secret = gateway_shared_secret.clone();
                let protected = req.path().starts_with("/api/");
                let authorized = gateway_shared_secret.as_deref().map_or(true, |secret| {
                    req.headers()
                        .get("X-Gateway-Secret")
                        .and_then(|value| value.to_str().ok())
                        .map_or(false, |value| value == secret)
                });

                if protected && !authorized {
                    return Either::Left(ready(Ok(
                        req.into_response(HttpResponse::Forbidden().finish())
                            .map_into_right_body(),
                    )));
                }

                Either::Right(srv.call(req).map(|res| res.map(|res| res.map_into_left_body())))
            })
            .app_data(web::Data::new(pool.clone()))
            .route("/health", web::get().to(handlers::health))
            .route("/api/achievements/{user_id}", web::get().to(handlers::get_user_achievements))
            .route("/api/missions/{user_id}", web::get().to(handlers::get_user_missions))
            .route("/api/clans", web::get().to(handlers::get_clans))
            .route("/api/clans/me", web::get().to(handlers::get_my_clan))
            .route("/api/clans/leaderboard", web::get().to(handlers::get_global_clan_leaderboard))
            .route("/api/clans/{clan_id}/leaderboard", web::get().to(handlers::get_clan_leaderboard))
            .route("/api/notifications/{user_id}", web::get().to(handlers::get_user_notifications))
            .route("/api/notifications/{user_id}/unread-count", web::get().to(handlers::get_unread_notification_count))
            .route("/api/notifications/read/{notification_id}", web::put().to(handlers::mark_notification_read))
    })
    .bind(format!("0.0.0.0:{}", config.server_port))?
    .run()
    .await
}
