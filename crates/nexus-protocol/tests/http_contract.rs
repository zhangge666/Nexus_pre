//! 本文件验证 Memory Protocol 的鉴权、来源限制及 HTTP 写入检索闭环。

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use nexus_core::MemoryStore;
use nexus_protocol::{CapabilityGrant, ProtocolState, Scope, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-capability-token";

/// 验证管理员可以通过协议管理记忆关联、集合和集合成员。
#[tokio::test]
async fn manages_links_and_collections_over_http() {
    let app = test_router([Scope::Admin], Some("external:test".into()));
    let first = create_protocol_memory(&app, "first linked memory").await;
    let second = create_protocol_memory(&app, "second linked memory").await;

    let linked = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/links",
            Some(json!({
                "from_id": first,
                "to_id": second,
                "relation": "references"
            })),
        ))
        .await
        .expect("创建关联请求应返回响应");
    assert_eq!(linked.status(), StatusCode::CREATED);
    let listed_links = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            &format!("/v1/links?memory_id={first}"),
            None,
        ))
        .await
        .expect("关联列表请求应返回响应");
    assert_eq!(
        response_json(listed_links).await.as_array().unwrap().len(),
        1
    );

    let created_collection = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/collections",
            Some(json!({"name": "项目", "icon": "folder", "sort": 10})),
        ))
        .await
        .expect("创建集合请求应返回响应");
    assert_eq!(created_collection.status(), StatusCode::CREATED);
    let collection_id = response_json(created_collection).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let added = app
        .clone()
        .oneshot(authorized_request(
            Method::PUT,
            &format!("/v1/collections/{collection_id}/memories/{first}"),
            None,
        ))
        .await
        .expect("添加集合成员请求应返回响应");
    assert_eq!(added.status(), StatusCode::NO_CONTENT);
    let members = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            &format!("/v1/collections/{collection_id}/memories"),
            None,
        ))
        .await
        .expect("集合成员请求应返回响应");
    assert_eq!(response_json(members).await[0], first);

    let deleted_link = app
        .clone()
        .oneshot(authorized_request(
            Method::DELETE,
            &format!("/v1/links/{first}/{second}/references"),
            None,
        ))
        .await
        .expect("删除关联请求应返回响应");
    assert_eq!(deleted_link.status(), StatusCode::NO_CONTENT);
}

/// 验证订阅权限能够通过 SSE 收到事务提交后的记忆创建事件。
#[tokio::test]
async fn streams_committed_memory_events_over_sse() {
    let app = test_router(
        [Scope::Subscribe, Scope::MemoryWrite],
        Some("external:test".into()),
    );
    let mut events = app
        .clone()
        .oneshot(authorized_request(Method::GET, "/v1/events", None))
        .await
        .expect("事件订阅请求应返回响应");
    assert_eq!(events.status(), StatusCode::OK);

    let created = app
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:test",
                "kind": "note",
                "content": "SSE event content",
                "content_format": "plain"
            }),
        ))
        .await
        .expect("创建请求应返回响应");
    assert_eq!(created.status(), StatusCode::CREATED);

    let frame = events
        .body_mut()
        .frame()
        .await
        .expect("SSE 应产生事件帧")
        .expect("SSE 事件帧应有效");
    let payload = std::str::from_utf8(frame.data_ref().expect("SSE 帧应包含数据"))
        .expect("SSE 数据应为 UTF-8");
    assert!(payload.contains("event: memory.created"));
    assert!(payload.contains("memory_created"));
}

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

/// 验证完整记忆可以读取、更新、列表过滤并级联删除。
#[tokio::test]
async fn supports_memory_crud_and_list_contract() {
    let app = test_router(
        [Scope::MemoryRead, Scope::MemoryWrite, Scope::MemoryDelete],
        Some("external:test".into()),
    );
    let created = app
        .clone()
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:test",
                "kind": "note",
                "content": "Original protocol content",
                "content_format": "plain",
                "tags": ["protocol"],
                "captured_at": 1710000000000_i64
            }),
        ))
        .await
        .expect("创建请求应返回响应");
    let id = response_json(created).await["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let fetched = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            &format!("/v1/memories/{id}"),
            None,
        ))
        .await
        .expect("读取请求应返回响应");
    assert_eq!(fetched.status(), StatusCode::OK);
    let fetched_body = response_json(fetched).await;
    assert_eq!(fetched_body["source"], "external:test");
    assert_eq!(fetched_body["captured_at"], 1710000000000_i64);

    let updated = app
        .clone()
        .oneshot(authorized_request(
            Method::PATCH,
            &format!("/v1/memories/{id}"),
            Some(json!({"content": "Updated protocol content", "pinned": true})),
        ))
        .await
        .expect("更新请求应返回响应");
    assert_eq!(updated.status(), StatusCode::OK);
    let updated_body = response_json(updated).await;
    assert_eq!(updated_body["content"], "Updated protocol content");
    assert_eq!(updated_body["pinned"], true);

    let listed = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            "/v1/memories?source=external:test&kind=note&tags=protocol&limit=10",
            None,
        ))
        .await
        .expect("列表请求应返回响应");
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(response_json(listed).await["total"], 1);

    let deleted = app
        .clone()
        .oneshot(authorized_request(
            Method::DELETE,
            &format!("/v1/memories/{id}"),
            None,
        ))
        .await
        .expect("删除请求应返回响应");
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    let missing = app
        .oneshot(authorized_request(
            Method::GET,
            &format!("/v1/memories/{id}"),
            None,
        ))
        .await
        .expect("读取请求应返回响应");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);
}

/// 验证检索过滤只返回请求指定来源的命中。
#[tokio::test]
async fn applies_search_source_filters() {
    let app = test_router([Scope::MemoryWrite, Scope::Search], None);
    for source in ["external:first", "external:second"] {
        let response = app
            .clone()
            .oneshot(json_request(
                "/v1/memories",
                json!({
                    "source": source,
                    "kind": "note",
                    "content": "Shared searchable contract phrase",
                    "content_format": "plain"
                }),
            ))
            .await
            .expect("创建请求应返回响应");
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let response = app
        .oneshot(json_request(
            "/v1/search",
            json!({
                "text": "searchable",
                "mode": "hybrid",
                "filters": {"source": ["external:first"]},
                "limit": 10
            }),
        ))
        .await
        .expect("检索请求应返回响应");
    let body = response_json(response).await;
    assert_eq!(body["hits"].as_array().unwrap().len(), 1);
}

/// 验证缺少 `memory:delete` scope 的令牌不能删除记忆。
#[tokio::test]
async fn rejects_delete_without_scope() {
    let app = test_router([Scope::MemoryWrite], Some("external:test".into()));
    let created = app
        .clone()
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:test",
                "kind": "note",
                "content": "Protected memory",
                "content_format": "plain"
            }),
        ))
        .await
        .expect("创建请求应返回响应");
    let id = response_json(created).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = app
        .oneshot(authorized_request(
            Method::DELETE,
            &format!("/v1/memories/{id}"),
            None,
        ))
        .await
        .expect("删除请求应返回响应");
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
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

/// 通过协议创建一条组织关系测试记忆并返回标识。
async fn create_protocol_memory(app: &axum::Router, content: &str) -> String {
    let response = app
        .clone()
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "external:test",
                "kind": "note",
                "content": content,
                "content_format": "plain"
            }),
        ))
        .await
        .expect("组织关系测试记忆应创建成功");
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// 构造携带测试 Bearer token 的 JSON POST 请求。
fn json_request(uri: &str, body: Value) -> Request<Body> {
    authorized_request(Method::POST, uri, Some(body))
}

/// 构造携带测试 Bearer token 的任意 HTTP 请求。
fn authorized_request(method: Method, uri: &str, body: Option<Value>) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {TOKEN}"))
        .body(Body::from(
            body.map_or_else(String::new, |value| value.to_string()),
        ))
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
