//! 本文件验证 Memory Protocol 的鉴权、来源限制及 HTTP 写入检索闭环。

use axum::{
    body::Body,
    http::{Request, StatusCode, header},
};
use http_body_util::BodyExt;
use nexus_core::MemoryStore;
use nexus_protocol::{CapabilityGrant, ProtocolState, Scope, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-capability-token";

/// 验证带正确 scope 的客户端能写入并通过混合检索找到同一条记忆。
#[tokio::test]
async fn creates_and_searches_memory_over_http() {
    let app = test_router(
        [Scope::MemoryWrite, Scope::Search],
        Some("external:test".into()),
    );
    let create_response = app
        .clone()
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:test",
                "kind": "note",
                "title": "发布计划",
                "content": "Release owner is Alice and launch is Friday.",
                "content_format": "markdown",
                "tags": ["meeting"]
            }),
        ))
        .await
        .expect("协议路由应返回响应");
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let create_body = response_json(create_response).await;
    let search_response = app
        .oneshot(json_request(
            "/v1/search",
            json!({"text": "Alice", "mode": "hybrid", "limit": 10}),
        ))
        .await
        .expect("协议路由应返回响应");
    assert_eq!(search_response.status(), StatusCode::OK);
    let search_body = response_json(search_response).await;
    assert_eq!(search_body["hits"][0]["memory_id"], create_body["id"]);
}

/// 验证没有 Bearer token 的写入请求会被拒绝。
#[tokio::test]
async fn rejects_unauthenticated_write() {
    let app = test_router([Scope::MemoryWrite], Some("external:test".into()));
    let request = Request::builder()
        .method("POST")
        .uri("/v1/memories")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "source": "external:test",
                "kind": "note",
                "content": "unauthenticated request",
                "content_format": "plain"
            })
            .to_string(),
        ))
        .expect("应能构造测试请求");
    let response = app.oneshot(request).await.expect("协议路由应返回响应");
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

/// 验证客户端不能越权写入授权范围之外的来源。
#[tokio::test]
async fn rejects_write_to_ungranted_source() {
    let app = test_router([Scope::MemoryWrite], Some("external:test".into()));
    let response = app
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:other",
                "kind": "note",
                "content": "unauthorized source",
                "content_format": "plain"
            }),
        ))
        .await
        .expect("协议路由应返回响应");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

/// 创建使用内存数据库和测试 capability grant 的协议路由。
fn test_router(
    scopes: impl IntoIterator<Item = Scope>,
    writable_source: Option<String>,
) -> axum::Router {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    router(ProtocolState::new(
        store,
        CapabilityGrant::new(TOKEN, scopes, writable_source),
    ))
}

/// 构造携带测试 Bearer token 的 JSON POST 请求。
fn json_request(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::from(body.to_string()))
        .expect("应能构造测试请求")
}

/// 将协议响应体解析为 JSON 以便断言契约字段。
async fn response_json(response: axum::response::Response) -> Value {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("应能读取响应体")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("响应体应为合法 JSON")
}
