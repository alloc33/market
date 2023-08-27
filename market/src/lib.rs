pub mod api;
pub mod config;
pub mod middleware;
pub mod model;
pub mod objects;

use std::{sync::Arc, time::Duration};

use api::*;
use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
    Router,
};
use config::AppConfig;
use sqlx::{postgres::PgConnectOptions, Error as SqlxError, PgPool};
use tower::ServiceBuilder;

pub struct App {
    pub db: PgPool,
    pub config: AppConfig,
}

pub async fn build_state(config: AppConfig) -> Result<App, SqlxError> {
    let opts = config.database_url.parse::<PgConnectOptions>()?;

    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(None)
        .min_connections(1)
        .connect_with(opts)
        .await?;

    match sqlx::migrate!("./migrations").run(&pool).await {
        Ok(_) => tracing::info!("successfully run db migrations"),
        Err(err) => {
            tracing::error!("failed to run db migrations, error: {:?}", err);
            std::process::exit(1);
        }
    }

    let app = App { db: pool, config };
    Ok(app)
}

pub fn build_routes(app_state: Arc<App>) -> Router {
    Router::new()
        .route("/alert", post(api::alert::process_alert))
        .route("/alert", get(api::alert::get_alerts))
        .layer(
            ServiceBuilder::new()
                .layer(from_fn_with_state(app_state.clone(), middleware::auth))
                .layer(from_fn(middleware::log_request))
                .layer(from_fn(middleware::log_response)),
        )
        .with_state(Arc::clone(&app_state))
}
