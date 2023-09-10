pub mod broker;
pub mod trade_error;

use std::sync::Arc;

use apca::{ApiInfo, Client as AlpacaClient};
use config::{Config, ConfigError, File};
use serde::Deserialize;
use thiserror::Error as ThisError;
use tokio::time::{sleep, Duration};
use tracing::{error, info};
use trade_error::TradeError;
use uuid::Uuid;

use self::broker::Broker;
use crate::{
    api::alert::AlertData,
    events::{EventHandler, HandleEventError},
    trade_executor::TradeExecutor,
    App,
};

pub trait TradingClient {
    fn create_order(&self, input: &AlertData) -> Order;
    fn execute_order(&self, order: &Order) -> Result<(), TradeError>;
    fn cancel_order(&self, order: &Order) -> Result<(), TradeError>;
    fn get_order(&self, order: &Order) -> Result<(), TradeError>;
    fn get_orders(&self) -> Result<(), TradeError>;
    fn get_positions(&self) -> Result<(), TradeError>;
    fn get_account(&self) -> Result<(), TradeError>;
}

#[derive(Debug, Deserialize, Clone)]
pub struct Strategy {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
    pub broker: Broker,
    pub max_event_retries: u8,
    pub event_retry_delay: f64,
}

pub struct StrategyManager {
    app_state: Arc<App>,
    strategies: Vec<Strategy>,
    alpaca_client: AlpacaClient,
    trade_executor: TradeExecutor,
}

#[derive(Debug, ThisError)]
pub enum StrategyManagerError {
    #[error(transparent)]
    ConfigError(#[from] ConfigError),
    #[error(transparent)]
    AlpacaClientError(#[from] apca::Error),
    #[error("Unknown strategy - {0}")]
    UnknownStrategy(String),
    #[error("Unknown exchange - {0}")]
    UnknownExchange(String),
}

#[derive(Debug)]
pub struct Order {
    id: Uuid,
}

impl std::fmt::Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ id: {} }}", self.id)
    }
}

impl StrategyManager {
    pub fn new(
        app_state: Arc<App>,
        trade_executor: TradeExecutor,
    ) -> Result<Self, StrategyManagerError> {
        let strategies = Config::builder()
            .add_source(File::with_name("market/strategies.toml"))
            .build()
            .map_err(StrategyManagerError::ConfigError)?
            .try_deserialize::<Vec<Strategy>>()?;

        let alpaca_client = AlpacaClient::new(ApiInfo::from_parts(
            &app_state.config.alpaca.apca_api_base_url,
            &app_state.config.alpaca.apca_api_key_id,
            &app_state.config.alpaca.apca_api_secret_key,
        )?);

        Ok(Self {
            app_state,
            strategies,
            alpaca_client,
            trade_executor,
        })
    }

    fn find_strategy(&self, strategy_id: Uuid) -> Option<&Strategy> {
        self.strategies
            .iter()
            .find(|strategy| strategy.id == strategy_id)
    }
}

#[axum::async_trait]
impl EventHandler for StrategyManager {
    type EventPayload = AlertData;

    async fn handle_event(&self, event: &Self::EventPayload) -> Result<(), HandleEventError> {
        if let Some(strategy) = self.find_strategy(event.strategy_id) {
            let mut retries = 0;

            // let order = strategy.broker.create_order(event);

            // loop {
            //     let trade_result = self.trade_executor.execute_order(&order).await;

            //     if trade_result.is_ok() {
            //         info!("Order successfully executed");
            //         return Ok(());
            //     }

            //     retries += 1;

            //     if retries >= strategy.max_event_retries {
            //         error!("Max event retries reached, giving up.");
            //         return Err(TradeError::MaxRetriesReached(order).into());
            //     }

            //     tokio::time::sleep(Duration::from_secs_f64(strategy.event_retry_delay)).await;
            // }
            Ok(())
        } else {
            Err(StrategyManagerError::UnknownStrategy(event.strategy_id.to_string()).into())
        }
    }
}
