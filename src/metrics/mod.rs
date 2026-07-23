
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct IsAliveState{
    pub name: String,
    pub version: String,
    pub env_info: Option<String>,
    pub started: i64,
}

