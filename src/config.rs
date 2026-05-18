use config::{Config, File};

#[derive(Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub rabbitmq_url: String,
    pub server_port: u16,
}

pub fn load() -> AppConfig {
    // Build config with file first, then env vars override
    let config = Config::builder()
        .add_source(File::with_name("config").required(false))
        .add_source(config::Environment::default().prefix("APP_"))
        .build()
        .expect("Failed to load config");

    // Use env vars directly as they take precedence
    AppConfig {
        database_url: std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/gamification".into()),
        rabbitmq_url: std::env::var("CLOUDAMQP_URL")
            .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".into()),
        server_port: std::env::var("APP_PORT")
            .unwrap_or_else(|_| "8081".into())
            .parse()
            .unwrap_or(8081),
    }
}