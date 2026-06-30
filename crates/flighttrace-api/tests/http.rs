//! HTTP integration tests that exercise the full router against an
//! in-memory SQLite pool via `tower::ServiceExt::oneshot`.

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use flighttrace_api::{AppState, build};
use flighttrace_storage::connect_in_memory;

async fn test_router() -> Router {
    let pool = connect_in_memory().await.expect("pool");
    let state = AppState::new(pool);
    build(state)
}

async fn json_response(router: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let response = router.clone().oneshot(req).await.expect("oneshot succeeds");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("collect body")
        .to_bytes();
    let value: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("parse JSON body")
    };
    (status, value)
}

#[tokio::test]
async fn health_returns_ok() {
    let router = test_router().await;
    let req = Request::builder()
        .method(Method::GET)
        .uri("/health")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
}

#[tokio::test]
async fn demo_run_then_list_includes_it() {
    let router = test_router().await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/demo/run")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, create_req).await;
    assert_eq!(status, StatusCode::CREATED);
    let id = body["id"].as_str().expect("id is a string").to_string();

    let list_req = Request::builder()
        .method(Method::GET)
        .uri("/api/runs")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, list_req).await;
    assert_eq!(status, StatusCode::OK);
    let runs = body.as_array().expect("runs is an array");
    assert!(
        runs.iter().any(|r| r["id"].as_str() == Some(id.as_str())),
        "expected list to include just-created run id {id}"
    );
}

#[tokio::test]
async fn failure_report_reports_takeoff_timeout() {
    let router = test_router().await;

    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/demo/run")
        .body(Body::empty())
        .unwrap();
    let (_, body) = json_response(&router, create_req).await;
    let id = body["id"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{id}/failure-report"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["reason"], "TAKEOFF_TIMEOUT");
    assert!(!body["evidence"].as_array().unwrap().is_empty());
    assert!(body["likelyCause"].as_str().unwrap().contains("altitude"));
    assert_eq!(body["summary"], "Mission failed: TAKEOFF_TIMEOUT");
}

#[tokio::test]
async fn events_and_assertions_endpoints_return_arrays() {
    let router = test_router().await;
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/demo/run")
        .body(Body::empty())
        .unwrap();
    let (_, body) = json_response(&router, create_req).await;
    let id = body["id"].as_str().unwrap().to_string();

    let events_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{id}/events"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, events_req).await;
    assert_eq!(status, StatusCode::OK);
    let events = body.as_array().expect("events array");
    assert!(!events.is_empty());
    // camelCase field check.
    assert!(events[0]["runId"].is_string());

    let asserts_req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{id}/assertions"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, asserts_req).await;
    assert_eq!(status, StatusCode::OK);
    let assertions = body.as_array().expect("assertions array");
    assert!(!assertions.is_empty());
    let has_fail = assertions
        .iter()
        .any(|a| a["outcome"].as_str() == Some("FAIL"));
    assert!(has_fail, "expected at least one FAIL assertion");
}

#[tokio::test]
async fn telemetry_endpoint_returns_samples() {
    let router = test_router().await;
    let create_req = Request::builder()
        .method(Method::POST)
        .uri("/api/demo/run")
        .body(Body::empty())
        .unwrap();
    let (_, body) = json_response(&router, create_req).await;
    let id = body["id"].as_str().unwrap().to_string();

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{id}/telemetry"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    let samples = body.as_array().expect("telemetry array");
    assert!(!samples.is_empty(), "demo telemetry should be non-empty");
    let first = &samples[0];
    assert!(first["runId"].is_string());
    assert!(first["altitudeM"].is_number());
    assert!(first["verticalSpeedMps"].is_number());
    assert!(first["mode"].is_string());
    assert!(first["armed"].is_boolean());
    assert!(first["batteryRemainingPct"].is_number());
}

#[tokio::test]
async fn unknown_run_returns_404_with_error_envelope() {
    let router = test_router().await;
    let bogus = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{bogus}"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
    assert!(body["error"]["message"].is_string());
}

#[tokio::test]
async fn unknown_run_failure_report_returns_404() {
    let router = test_router().await;
    let bogus = uuid::Uuid::new_v4();

    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/runs/{bogus}/failure-report"))
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn list_supports_limit_and_offset() {
    let router = test_router().await;
    for _ in 0..3 {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/api/demo/run")
            .body(Body::empty())
            .unwrap();
        let (status, _) = json_response(&router, req).await;
        assert_eq!(status, StatusCode::CREATED);
    }

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/runs?limit=2&offset=0")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 2);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/api/runs?limit=2&offset=2")
        .body(Body::empty())
        .unwrap();
    let (status, body) = json_response(&router, req).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
}
