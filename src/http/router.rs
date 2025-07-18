use axum::{Extension, Router, http::StatusCode, response::IntoResponse, routing::get};
use std::sync::Arc;
use tokio::sync::RwLock;

async fn metrics_get(Extension(data): Extension<Arc<RwLock<String>>>) -> impl IntoResponse {
    let buffer = data.read().await;
    if buffer.len() == 0 {
        (StatusCode::NO_CONTENT, "".to_string())
    } else {
        (StatusCode::OK, buffer.to_string())
    }
}

pub fn get_router(recv: Arc<RwLock<String>>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_get))
        .layer(Extension(recv))
}
