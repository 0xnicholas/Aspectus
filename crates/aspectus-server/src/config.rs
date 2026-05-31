use anyhow::Context;

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub redis_url: String,
    pub db_max_connections: u32,
    pub db_min_connections: u32,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            port: std::env::var("PORT")
                .unwrap_or_else(|_| "3001".into())
                .parse()
                .context("PORT must be a valid u16")?,
            database_url: std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?,
            redis_url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".into()),
            db_max_connections: std::env::var("DB_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "50".into())
                .parse()
                .context("DB_MAX_CONNECTIONS must be a u32")?,
            db_min_connections: std::env::var("DB_MIN_CONNECTIONS")
                .unwrap_or_else(|_| "10".into())
                .parse()
                .context("DB_MIN_CONNECTIONS must be a u32")?,
        })
    }
}
