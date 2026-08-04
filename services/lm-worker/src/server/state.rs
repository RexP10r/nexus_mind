use std::sync::Arc;

use crate::config::Config;
use crate::db::DbLayer;
use crate::traits::agent::Agent;
use crate::traits::llm::LlmProvider;

#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<dyn Agent>,
    pub llm: Arc<dyn LlmProvider>,
    pub db: Arc<DbLayer>,
    pub config: Config,
}
