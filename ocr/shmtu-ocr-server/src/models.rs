use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct OcrRequest {
    pub image_base64: String,
}

#[derive(Debug, Serialize)]
pub struct OcrResponse {
    pub success: bool,
    pub expression: Option<String>,
    pub result: Option<i32>,
    pub equal_symbol: Option<i32>,
    pub operator: Option<i32>,
    pub digit1: Option<i32>,
    pub digit2: Option<i32>,
    pub error: Option<String>,
}

impl OcrResponse {
    pub fn success(expr: String, result: i32, equal_symbol: i32, op: i32, d1: i32, d2: i32) -> Self {
        Self { success: true, expression: Some(expr), result: Some(result), equal_symbol: Some(equal_symbol), operator: Some(op), digit1: Some(d1), digit2: Some(d2), error: None }
    }
    pub fn error(msg: impl Into<String>) -> Self {
        Self { success: false, expression: None, result: None, equal_symbol: None, operator: None, digit1: None, digit2: None, error: Some(msg.into()) }
    }
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub availability_level: String,
    pub reason: Option<String>,
    pub models_loaded: bool,
    pub pool_size: usize,
    pub queue_capacity: usize,
    pub pending_requests: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub status: String,
    pub availability_level: String,
    pub reason: Option<String>,
    pub models_loaded: bool,
    pub pool_size: usize,
    pub queue_capacity: usize,
    pub pending_requests: usize,
    pub avg_response_ms: f64,
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
}
