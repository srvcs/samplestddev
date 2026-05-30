use axum::body::Body;
use axum::extract::Json as JsonExtract;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_samplestddev::{api::Deps, health, router, telemetry};
use tower::ServiceExt;

const DEAD_URL: &str = "http://127.0.0.1:1";

async fn serve(app: AxumRouter) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// Mock `srvcs-samplevariance` that ACTUALLY COMPUTES the sample variance
/// (sum of squared deviations from the mean, divided by `n - 1`) of the
/// `values` array and returns `{"values", "result": <f64>}`.
async fn spawn_computing_samplevariance() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|JsonExtract(req): JsonExtract<Value>| async move {
            let nums: Vec<f64> = req["values"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_f64).collect())
                .unwrap_or_default();
            let n = nums.len() as f64;
            let mean = nums.iter().sum::<f64>() / n;
            let ss: f64 = nums.iter().map(|x| (x - mean) * (x - mean)).sum();
            let result = ss / (n - 1.0);
            Json(json!({ "values": req["values"], "result": result }))
        }),
    );
    serve(app).await
}

/// Mock `srvcs-sqrt` that ACTUALLY COMPUTES `value.sqrt()` as an `f64`.
async fn spawn_computing_sqrt() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|JsonExtract(req): JsonExtract<Value>| async move {
            let value = req["value"].as_f64().unwrap_or(0.0);
            Json(json!({ "value": value, "result": value.sqrt() }))
        }),
    );
    serve(app).await
}

/// Mock that always answers with a fixed status + body (used to simulate a
/// `422` rejection forwarded from a dependency).
async fn spawn_fixed(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    serve(app).await
}

fn app(samplevariance_url: &str, sqrt_url: &str) -> axum::Router {
    router(
        telemetry::metrics_handle_for_tests(),
        Deps {
            samplevariance_url: samplevariance_url.to_string(),
            sqrt_url: sqrt_url.to_string(),
        },
    )
}

async fn eval(samplevariance_url: &str, sqrt_url: &str, values: Value) -> (StatusCode, Value) {
    let res = app(samplevariance_url, sqrt_url)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(json!({ "values": values }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn status_of(uri: &str) -> StatusCode {
    app(DEAD_URL, DEAD_URL)
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

fn approx(got: &Value, expected: f64) -> bool {
    got.as_f64().map(|x| (x - expected).abs() < 1e-9) == Some(true)
}

// --- Standard endpoints ---

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

// --- Correctness cases, exercised against REAL computing dependencies ---

#[tokio::test]
async fn stddev_of_one_to_five() {
    let var = spawn_computing_samplevariance().await;
    let sqrt = spawn_computing_sqrt().await;
    // samplevariance([1,2,3,4,5]) = 2.5; sqrt(2.5) = 1.5811388300841898
    let (status, body) = eval(&var, &sqrt, json!([1, 2, 3, 4, 5])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        approx(&body["result"], 1.5811388300841898),
        "got {:?}",
        body["result"]
    );
    assert_eq!(body["values"], json!([1, 2, 3, 4, 5]));
}

#[tokio::test]
async fn stddev_of_identical_values_is_zero() {
    let var = spawn_computing_samplevariance().await;
    let sqrt = spawn_computing_sqrt().await;
    // variance of identical values is 0; sqrt(0) = 0
    let (status, body) = eval(&var, &sqrt, json!([4, 4, 4, 4])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(approx(&body["result"], 0.0), "got {:?}", body["result"]);
}

#[tokio::test]
async fn stddev_of_pair() {
    let var = spawn_computing_samplevariance().await;
    let sqrt = spawn_computing_sqrt().await;
    // samplevariance([2, 5]) = ((1.5^2)+(1.5^2)) / 1 = 4.5; sqrt(4.5) = 2.1213203435596424
    let (status, body) = eval(&var, &sqrt, json!([2, 5])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        approx(&body["result"], 2.1213203435596424),
        "got {:?}",
        body["result"]
    );
}

#[tokio::test]
async fn stddev_with_floats() {
    let var = spawn_computing_samplevariance().await;
    let sqrt = spawn_computing_sqrt().await;
    // samplevariance([1.5, 2.5, 3.5]) = 1.0; sqrt(1.0) = 1.0
    let (status, body) = eval(&var, &sqrt, json!([1.5, 2.5, 3.5])).await;
    assert_eq!(status, StatusCode::OK);
    assert!(approx(&body["result"], 1.0), "got {:?}", body["result"]);
}

// --- Error / edge cases ---

#[tokio::test]
async fn forwards_422_from_samplevariance() {
    let var = spawn_fixed(
        StatusCode::UNPROCESSABLE_ENTITY,
        json!({ "error": "sample variance requires at least two values" }),
    )
    .await;
    let sqrt = spawn_computing_sqrt().await;
    let (status, body) = eval(&var, &sqrt, json!([1])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        body["error"],
        "sample variance requires at least two values"
    );
}

#[tokio::test]
async fn degrades_when_samplevariance_unreachable() {
    let sqrt = spawn_computing_sqrt().await;
    let (status, body) = eval(DEAD_URL, &sqrt, json!([1, 2, 3])).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-samplevariance");
}

#[tokio::test]
async fn degrades_when_sqrt_unreachable() {
    let var = spawn_computing_samplevariance().await;
    let (status, body) = eval(&var, DEAD_URL, json!([1, 2, 3])).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-sqrt");
}
