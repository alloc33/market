pub mod api;
pub mod app_config;
pub mod events;
pub mod middleware;
pub mod strategy_manager;
pub mod trade_executor;

use std::{sync::Arc, time::Duration};

use api::*;
use app_config::AppConfig;
use axum::{
    middleware::{from_fn, from_fn_with_state},
    routing::post,
    Router,
};
use events::Event;
use sqlx::{postgres::PgConnectOptions, Error as SqlxError, PgPool};
use tokio::sync::mpsc::UnboundedSender;
use tower::ServiceBuilder;

pub struct App {
    pub db: PgPool,
    pub event_sender: UnboundedSender<Event>,
    pub config: AppConfig,
}

pub async fn build_state(
    config: AppConfig,
    event_sender: UnboundedSender<Event>,
) -> Result<App, SqlxError> {
    let opts = config.database.url.parse::<PgConnectOptions>()?;

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

    let app = App {
        db: pool,
        event_sender,
        config,
    };

    Ok(app)
}

pub fn build_routes(app_state: Arc<App>) -> Router {
    let mut router = Router::new();
    #[cfg(feature = "webhooks")]
    {
        router = router.route("/webhook/alert", post(api::webhook_receiver::receive_alert));
    }

    router = router.route("/status", post(|| async { "OK" })).layer(
        ServiceBuilder::new()
            .layer(from_fn_with_state(app_state.clone(), middleware::auth))
            .layer(from_fn(middleware::log_request))
            .layer(from_fn(middleware::log_response)),
    );

    router.with_state(Arc::clone(&app_state))
}
