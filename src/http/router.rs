use std::sync::Arc;

use axum::{Extension, Router, http::StatusCode, response::IntoResponse, routing::get};
use prometheus_client::encoding::text::encode;
use prometheus_client::registry::Registry;
use tokio::time::Instant;

use crate::constants::UnitMap;
use crate::metrics::UnitMetrics;

async fn metrics_get(
    Extension(units): Extension<UnitMap<'_>>,
    Extension(mut recorder): Extension<UnitMetrics>,
    Extension(registry): Extension<Arc<Registry>>,
) -> impl IntoResponse {
    recorder.new_batch();
    let start = Instant::now();
    for unit in units.read().await.values() {
        if let Err(err) = recorder.record_unit(unit).await {
            tracing::error!("Failed to record unit: {}", err);
        }
    }
    // {
    // let unit_names: Vec<String> = units.read().await.keys().cloned().collect();
    // let mut js = JoinSet::new();
    // for unit_name in unit_names {
    //     let mut recorder = recorder.clone();
    //     let units_local = units.clone();
    //     js.spawn(async move {
    //         if let Some(unit) = units_local.read().await.get(&unit_name) {
    //             recorder.record_unit(unit).await
    //         } else {
    //             Ok(())
    //         }
    //     });
    // }

    // while let Some(Ok(result)) = js.join_next().await {
    //     if let Err(err) = result {
    //         tracing::error!("Failed to record unit: {}", err);
    //     }
    // }
    // }
    let end = Instant::now();
    recorder.record_scrape(end - start);
    let mut buffer = String::new();
    if let Err(err) = encode(&mut buffer, &registry) {
        tracing::error!("Failed to encode registry: {}", err);
    }

    (StatusCode::OK, buffer.to_string())
}

pub fn get_router(units: UnitMap<'static>, recorder: UnitMetrics, registry: Registry) -> Router {
    Router::new()
        .route("/metrics", get(metrics_get))
        .layer(Extension(units))
        .layer(Extension(recorder))
        .layer(Extension(Arc::new(registry)))
}
