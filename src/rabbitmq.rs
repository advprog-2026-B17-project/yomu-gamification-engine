use futures_util::stream::StreamExt;
use lapin::{
    options::*, types::FieldTable, Channel, ExchangeKind,
    Connection, ConnectionProperties, Consumer,
};
use sqlx::PgPool;
use std::sync::Arc;

use crate::models::EventEnvelope;
use crate::services;

pub async fn connect(rabbitmq_url: &str) -> Result<(Channel, Consumer), lapin::Error> {
    let conn = Connection::connect(rabbitmq_url, ConnectionProperties::default()).await?;
    let channel = conn.create_channel().await?;

    channel
        .queue_declare(
            "yomu-gamification-queue",
            QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    channel
        .exchange_declare(
            "yomu.events",
            ExchangeKind::Topic,
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            "yomu-gamification-queue",
            "yomu.events",
            "quiz.completed",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            "yomu-gamification-queue",
            "yomu.events",
            "mission.progress",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            "yomu-gamification-queue",
            "yomu.events",
            "season.ended",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            "yomu-gamification-queue",
            "yomu.events",
            "reading.completed",
            QueueBindOptions::default(),
            FieldTable::default(),
        )
        .await?;

    let consumer = channel
        .basic_consume(
            "yomu-gamification-queue",
            "gamification-consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    Ok((channel, consumer))
}

pub async fn consume_events(mut consumer: Consumer, pool: PgPool) {
    let pool = Arc::new(pool);
    tracing::info!("Starting to consume events from RabbitMQ (push model)");

    while let Some(delivery_result) = consumer.next().await {
        match delivery_result {
            Ok(delivery) => {
                tracing::info!("Got delivery from queue, size: {}", delivery.data.len());

                match serde_json::from_slice::<EventEnvelope>(&delivery.data) {
                    Ok(event) => {
                        tracing::info!("Received event: {} | payload: {:?}", event.event_type, event.payload);

                        let pool_clone = pool.clone();
                        match event.event_type.as_str() {
                            "quiz.completed" => services::handle_quiz_completed(&event, &pool_clone).await,
                            "reading.completed" => services::handle_reading_completed(&event, &pool_clone).await,
                            "mission.progress" => services::handle_mission_progress(&event, &pool_clone).await,
                            "season.ended" => services::handle_season_ended(&event, &pool_clone).await,
                            _ => tracing::warn!("Unknown event type: {}", event.event_type),
                        }

                        if let Err(e) = delivery.ack(BasicAckOptions::default()).await {
                            tracing::error!("Failed to ack message: {}", e);
                        }
                    }
                    Err(e) => {
                        let payload_str = String::from_utf8_lossy(&delivery.data);
                        tracing::error!("Failed to parse message JSON: {}. Payload: {}", e, payload_str);
                        let _ = delivery.nack(BasicNackOptions { multiple: false, requeue: false }).await;
                    }
                }
            }
            Err(e) => {
                tracing::error!("Error getting message from stream: {}", e);
            }
        }
    }
}