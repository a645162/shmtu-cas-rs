use std::sync::Arc;

use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;

use crate::models::{HealthResponse, OcrRequest, OcrResponse, StatusResponse};
use crate::pool::OcrPool;

pub async fn health_check(State(pool): State<Arc<OcrPool>>) -> Json<HealthResponse> {
    let pending = pool.pending_requests();
    Json(HealthResponse {
        status: if pending < pool.queue_capacity() { "healthy".into() } else { "busy".into() },
        availability_level: pool.availability_level().to_string(),
        reason: None,
        models_loaded: pool.models_loaded(),
        pool_size: pool.pool_size(),
        queue_capacity: pool.queue_capacity(),
        pending_requests: pending,
        model_version: pool.model_version().as_str().to_string(),
        server_name: pool.server_name().map(String::from),
    })
}

pub async fn ocr_base64(
    State(pool): State<Arc<OcrPool>>,
    Json(req): Json<OcrRequest>,
) -> Result<Json<OcrResponse>, (StatusCode, Json<OcrResponse>)> {
    if req.image_base64.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(OcrResponse::error("image_base64 is empty")),
        ));
    }
    let mv = pool.model_version().as_str().to_string();
    match pool.submit_base64(&req.image_base64).await {
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(OcrResponse::error("Queue full")),
        )),
        Some(Ok(r)) => Ok(Json(OcrResponse::success(
            r.expr,
            r.result,
            r.equal_symbol as i32,
            r.operator as i32,
            r.digit1,
            r.digit2,
            mv,
        ))),
        Some(Err(e)) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(OcrResponse::error(e.to_string())),
        )),
    }
}

pub async fn ocr_upload(
    State(pool): State<Arc<OcrPool>>,
    mut multipart: Multipart,
) -> Result<Json<OcrResponse>, (StatusCode, Json<OcrResponse>)> {
    let mut image_data: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        if field.name() == Some("file") || field.name() == Some("image") {
            if let Ok(bytes) = field.bytes().await {
                image_data = Some(bytes.to_vec());
            }
            break;
        }
    }
    let data = match image_data {
        Some(d) => d,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(OcrResponse::error("No file uploaded")),
            ));
        }
    };
    let mv = pool.model_version().as_str().to_string();
    match pool.submit(data).await {
        None => Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(OcrResponse::error("Queue full")),
        )),
        Some(Ok(r)) => Ok(Json(OcrResponse::success(
            r.expr,
            r.result,
            r.equal_symbol as i32,
            r.operator as i32,
            r.digit1,
            r.digit2,
            mv,
        ))),
        Some(Err(e)) => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(OcrResponse::error(e.to_string())),
        )),
    }
}

pub async fn get_status(State(pool): State<Arc<OcrPool>>) -> Json<StatusResponse> {
    let pending = pool.pending_requests();
    Json(StatusResponse {
        status: if pending < pool.queue_capacity() { "healthy".into() } else { "busy".into() },
        availability_level: pool.availability_level().to_string(),
        reason: None,
        models_loaded: pool.models_loaded(),
        pool_size: pool.pool_size(),
        queue_capacity: pool.queue_capacity(),
        pending_requests: pending,
        avg_response_ms: pool.avg_response_ms(),
        total_requests: pool.total_requests(),
        success_count: pool.success_count(),
        failure_count: pool.failure_count(),
        model_version: pool.model_version().as_str().to_string(),
        server_name: pool.server_name().map(String::from),
    })
}
