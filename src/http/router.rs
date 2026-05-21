use std::sync::Arc;

use axum::{Extension, Router, http::StatusCode, response::IntoResponse, routing::get};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use crate::metrics::UnitMetrics;
use crate::stats::StatsRequest;

async fn metrics_get(
    Extension(tx): Extension<mpsc::Sender<StatsRequest>>,
    Extension(mut recorder): Extension<UnitMetrics>,
    Extension(registry): Extension<Arc<Registry>>,
) -> impl IntoResponse {
    let (resp_tx, resp_rx) = oneshot::channel();
    if let Err(err) = tx.send(StatsRequest { response: resp_tx }).await {
        tracing::error!("Failed to send stats request: {}", err);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Error".to_string(),
        );
    }

    let stats = match resp_rx.await {
        Ok(m) => m,
        Err(err) => {
            tracing::error!("Failed to receive stats: {}", err);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Error".to_string(),
            );
        }
    };

    recorder.new_batch();
    let start = Instant::now();
    for data in stats {
        recorder.record_unit(data);
    }

    let end = Instant::now();
    recorder.record_scrape(end - start);
    let mut buffer = String::new();
    if let Err(err) = encode(&mut buffer, &registry) {
        tracing::error!("Failed to encode registry: {}", err);
    }

    (StatusCode::OK, buffer.to_string())
}

pub fn get_router(
    tx: mpsc::Sender<StatsRequest>,
    recorder: UnitMetrics,
    registry: Registry,
) -> Router {
    Router::new()
        .route("/metrics", get(metrics_get))
        .layer(Extension(tx))
        .layer(Extension(recorder))
        .layer(Extension(Arc::new(registry)))
}
