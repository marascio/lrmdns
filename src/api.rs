use crate::metrics::Metrics;
use axum::{
    extract::State,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct ApiState {
    pub metrics: Arc<Metrics>,
}

pub fn create_router(metrics: Arc<Metrics>) -> Router {
    let state = ApiState { metrics };

    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(get_metrics))
        .with_state(state)
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "lrmdns"
    }))
}

async fn get_metrics(State(state): State<ApiState>) -> impl IntoResponse {
    let snapshot = state.metrics.get_snapshot();

    // Convert query_types HashMap to a JSON-friendly format
    let query_types: std::collections::HashMap<String, u64> = snapshot.query_types
        .iter()
        .map(|(k, v)| (format!("{:?}", k), *v))
        .collect();

    Json(json!({
        "uptime_seconds": snapshot.uptime.as_secs(),
        "queries": {
            "total": snapshot.total_queries,
            "udp": snapshot.udp_queries,
            "tcp": snapshot.tcp_queries,
            "edns": snapshot.edns_queries
        },
        "responses": {
            "noerror": snapshot.noerror_responses,
            "nxdomain": snapshot.nxdomain_responses,
            "servfail": snapshot.servfail_responses,
            "refused": snapshot.refused_responses,
            "formerr": snapshot.formerr_responses
        },
        "query_types": query_types,
        "performance": {
            "avg_latency_us": snapshot.avg_latency_us,
            "min_latency_us": snapshot.min_latency_us,
            "max_latency_us": snapshot.max_latency_us,
            "qps": if snapshot.uptime.as_secs() > 0 {
                snapshot.total_queries / snapshot.uptime.as_secs()
            } else {
                0
            }
        },
        "rate_limited": snapshot.rate_limited,
        "errors": snapshot.errors
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::{Request, StatusCode}};
    use tower::util::ServiceExt;

    #[tokio::test]
    async fn test_health_check() {
        let metrics = Arc::new(Metrics::new());
        let app = create_router(metrics);

        let response = app
            .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_metrics_endpoint() {
        let metrics = Arc::new(Metrics::new());
        let app = create_router(metrics);

        let response = app
            .oneshot(Request::builder().uri("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
