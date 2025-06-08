use std::sync::Arc;

use crate::service::SystemdExporter;
use axum::{http::StatusCode, response::IntoResponse, routing::get, Extension, Router};

pub async fn metrics_get(Extension(svc): Extension<Arc<SystemdExporter>>) -> impl IntoResponse {
    StatusCode::NO_CONTENT
}

pub fn get_router(service: Arc<SystemdExporter>) -> Router {
    Router::new()
        .route("/metrics", get(metrics_get))
        .layer(Extension(service))
}
