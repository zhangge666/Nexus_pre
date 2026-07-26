//! 本文件验证 Memory Protocol 的鉴权、来源限制及 HTTP 写入检索闭环。

use std::sync::{Arc, Mutex};

use axum::{
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use http_body_util::BodyExt;
use nexus_ai::{Completion, CompletionFuture, CompletionRequest, CompletionResponse};
use nexus_core::{HashEmbedder, MemoryStore};
use nexus_protocol::{CapabilityGrant, ProtocolState, Scope, router};
use serde_json::{Value, json};
use tower::ServiceExt;

const TOKEN: &str = "test-capability-token";

/// 保存测试 Completion 实际收到的最小上下文。
struct RecordingCompletion {
    request: Arc<Mutex<Option<CompletionRequest>>>,
}

impl Completion for RecordingCompletion {
    /// 记录请求并返回固定回答，避免测试访问网络。
    fn complete<'a>(&'a self, request: CompletionRequest) -> CompletionFuture<'a> {
        Box::pin(async move {
            *self.request.lock().expect("记录锁应可用") = Some(request);
            Ok(CompletionResponse {
                text: "仅依据已筛选片段回答 [1]".into(),
                provider: "recording".into(),
            })
        })
    }

    fn provider_name(&self) -> &str {
        "recording"
    }

    fn sends_data_remote(&self) -> bool {
        true
    }
}

/// 验证卡片创建、AI 生成、到期队列、统计、评分和 review.due SSE 协议闭环。
#[tokio::test]
async fn manages_cards_and_reviews_over_http() {
    let app = test_router([Scope::Admin], None);
    let source = create_protocol_memory(&app, "FSRS 使用 stability 和 difficulty 调度复习。").await;
    let mut events = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            "/v1/events?types=review.due",
            None,
        ))
        .await
        .expect("复习事件订阅应返回响应");

    let created = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/cards",
            Some(json!({
                "card_front": "FSRS 的核心参数是什么？",
                "card_back": "stability 与 difficulty",
                "source_memory_id": source,
                "deck": "学习",
                "tags": ["fsrs"]
            })),
        ))
        .await
        .expect("手动卡片请求应返回响应");
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = response_json(created).await;
    let card_id = created["memory_id"].as_str().expect("应返回卡片标识");
    assert_eq!(created["state"], "new");
    assert_eq!(created["source_memory_id"], source);

    let notified = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/reviews/notify-due",
            None,
        ))
        .await
        .expect("到期扫描应返回响应");
    assert_eq!(response_json(notified).await["notified"], 1);
    let event_frame = events
        .body_mut()
        .frame()
        .await
        .expect("应收到复习到期事件")
        .expect("复习事件帧应有效");
    let event_payload = std::str::from_utf8(event_frame.data_ref().expect("事件应包含数据"))
        .expect("事件应为 UTF-8");
    assert!(event_payload.contains("event: review.due"));

    let due = app
        .clone()
        .oneshot(authorized_request(Method::GET, "/v1/reviews/due", None))
        .await
        .expect("复习队列应返回响应");
    assert_eq!(response_json(due).await.as_array().unwrap().len(), 1);
    let graded = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            &format!("/v1/reviews/{card_id}/grade"),
            Some(json!({"rating": "good"})),
        ))
        .await
        .expect("评分请求应返回响应");
    assert_eq!(graded.status(), StatusCode::OK);
    assert_eq!(response_json(graded).await["new_state"], "review");

    let generated = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/cards/generate",
            Some(json!({
                "source_memory_id": source,
                "max_cards": 2,
                "deck": "AI 生成"
            })),
        ))
        .await
        .expect("AI 卡片生成请求应返回响应");
    assert_eq!(generated.status(), StatusCode::CREATED);
    let generated = response_json(generated).await;
    assert_eq!(generated.as_array().unwrap().len(), 1);
    assert_eq!(generated[0]["source_memory_id"], source);

    let stats = app
        .oneshot(authorized_request(Method::GET, "/v1/reviews/stats", None))
        .await
        .expect("复习统计应返回响应");
    assert_eq!(response_json(stats).await["total_cards"], 2);
}

/// 验证 RAG 返回块级引用、Provider 元数据，并只向 Completion 发送截断后的命中片段。
#[tokio::test]
async fn asks_with_citations_and_minimized_context() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let state = ProtocolState::new(store, CapabilityGrant::new(TOKEN, [Scope::Admin], None));
    let recorded = Arc::new(Mutex::new(None));
    state
        .set_completion(Arc::new(RecordingCompletion {
            request: Arc::clone(&recorded),
        }))
        .expect("测试 Provider 应设置成功");
    let app = router(state);
    let long_content = format!(
        "数据最小化护栏要求只发送检索片段。{}",
        "隐私上下文".repeat(400)
    );
    let source = create_protocol_memory(&app, &long_content).await;

    let asked = app
        .oneshot(authorized_request(
            Method::POST,
            "/v1/ask",
            Some(json!({"question": "数据最小化护栏要求什么？"})),
        ))
        .await
        .expect("问答请求应返回响应");
    assert_eq!(asked.status(), StatusCode::OK);
    let asked = response_json(asked).await;
    assert_eq!(asked["answer"], "仅依据已筛选片段回答 [1]");
    assert_eq!(asked["provider"], "recording");
    assert_eq!(asked["sends_data_remote"], true);
    assert_eq!(asked["citations"][0]["memory_id"], source);
    assert!(asked["citations"][0]["block_id"].is_string());
    assert!(asked["citations"][0]["source_kind"].is_string());

    let request = recorded
        .lock()
        .expect("记录锁应可用")
        .clone()
        .expect("Completion 应收到请求");
    assert!(!request.context.is_empty());
    assert!(request.context.len() <= 6);
    assert!(
        request
            .context
            .iter()
            .all(|item| item.text.chars().count() <= 1_201)
    );
    assert!(request.context[0].text.len() < long_content.len());
}

/// 验证 `/v1/ask/stream` 先发送元数据，再逐段发送回答，并以 done 事件结束。
#[tokio::test]
async fn streams_ask_with_citations_and_provider_metadata() {
    let app = test_router([Scope::Admin], None);
    let source = create_protocol_memory(&app, "流式问答只使用本地命中的必要记忆片段。").await;
    let response = app
        .oneshot(authorized_request(
            Method::POST,
            "/v1/ask/stream",
            Some(json!({"question": "流式问答发送什么内容？"})),
        ))
        .await
        .expect("流式问答请求应返回响应");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        Some("text/event-stream")
    );
    let payload = response
        .into_body()
        .collect()
        .await
        .expect("流式响应应完整返回")
        .to_bytes();
    let payload = std::str::from_utf8(&payload).expect("SSE 应为 UTF-8");
    assert!(payload.contains("event: meta"));
    assert!(payload.contains("event: delta"));
    assert!(payload.contains("event: done"));
    assert!(payload.contains("\"provider\":\"local\""));
    assert!(payload.contains(&source));
}

/// 验证问答集合范围在 Completion 调用前完成过滤，空范围不会向远程 Provider 发送数据。
#[tokio::test]
async fn narrows_ask_to_collection_before_completion() {
    let store = MemoryStore::open_in_memory().expect("应能创建内存工作库");
    let state = ProtocolState::new(store, CapabilityGrant::new(TOKEN, [Scope::Admin], None));
    let recorded = Arc::new(Mutex::new(None));
    state
        .set_completion(Arc::new(RecordingCompletion {
            request: Arc::clone(&recorded),
        }))
        .expect("测试 Provider 应设置成功");
    let app = router(state);
    create_protocol_memory(&app, "集合外部的量子定价备忘").await;
    let collection = app
        .clone()
        .oneshot(authorized_request(
            Method::POST,
            "/v1/collections",
            Some(json!({"name": "空范围"})),
        ))
        .await
        .expect("集合创建应返回响应");
    let collection_id = response_json(collection).await["id"]
        .as_str()
        .expect("应返回集合标识")
        .to_owned();

    let asked = app
        .oneshot(authorized_request(
            Method::POST,
            "/v1/ask",
            Some(json!({
                "question": "量子定价是什么？",
                "scope": {"collection": collection_id}
            })),
        ))
        .await
        .expect("范围问答应返回响应");
    let asked = response_json(asked).await;
    assert_eq!(asked["citations"], json!([]));
    assert_eq!(asked["sent_context_count"], 0);
    assert_eq!(asked["sends_data_remote"], false);
    assert!(recorded.lock().expect("记录锁应可用").is_none());
}

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

/// 验证 Muse 能登记最小写入授权、写入统一来源，并在 Orbit 撤销后立即失效。
#[tokio::test]
async fn registers_lists_and_revokes_muse_connection() {
    let app = test_router([Scope::Admin], None);
    let registered = app
        .clone()
        .oneshot(json_request(
            "/v1/connections",
            json!({
                "app_id": "com.nexus.muse",
                "name": "Muse",
                "source": "muse",
                "scopes": ["memory:write"]
            }),
        ))
        .await
        .expect("Muse 登记请求应返回响应");
    assert_eq!(registered.status(), StatusCode::CREATED);
    let registered = response_json(registered).await;
    let token = registered["token"].as_str().expect("应签发客户端令牌");
    let token_id = registered["tokenId"].as_str().expect("应返回令牌标识");
    assert_eq!(registered["scopes"], json!(["memory:write"]));
    assert_eq!(registered["source"], "muse");

    let mut events = app
        .clone()
        .oneshot(authorized_request(
            Method::GET,
            "/v1/events?types=memory.created",
            None,
        ))
        .await
        .expect("Orbit 事件订阅应返回响应");
    assert_eq!(events.status(), StatusCode::OK);

    let created = app
        .clone()
        .oneshot(request_with_token(
            Method::POST,
            "/v1/memories",
            token,
            Some(json!({
                "source": "muse",
                "kind": "idea",
                "content": "M3 Muse 跨进程写入验证",
                "content_format": "plain",
                "meta": {"capture_method": "text"}
            })),
        ))
        .await
        .expect("Muse 写入请求应返回响应");
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = response_json(created).await;

    let event_frame = events
        .body_mut()
        .frame()
        .await
        .expect("Orbit 应即时收到 Muse 创建事件")
        .expect("Muse 创建事件帧应有效");
    let event_payload =
        std::str::from_utf8(event_frame.data_ref().expect("Muse 创建事件帧应包含数据"))
            .expect("Muse 创建事件应为 UTF-8");
    assert!(event_payload.contains("event: memory.created"));
    assert!(event_payload.contains("\"source\":\"muse\""));

    let searched = app
        .clone()
        .oneshot(json_request(
            "/v1/search",
            json!({"text": "跨进程写入验证", "mode": "hybrid", "limit": 10}),
        ))
        .await
        .expect("Orbit 检索应返回响应");
    assert_eq!(searched.status(), StatusCode::OK);
    let searched = response_json(searched).await;
    assert_eq!(searched["hits"][0]["memory_id"], created["id"]);

    let listed = app
        .clone()
        .oneshot(authorized_request(Method::GET, "/v1/connections", None))
        .await
        .expect("连接列表请求应返回响应");
    let listed = response_json(listed).await;
    assert_eq!(listed[0]["source"], "muse");
    assert_eq!(listed[0]["memoriesCount"], 1);

    let revoked = app
        .clone()
        .oneshot(authorized_request(
            Method::DELETE,
            &format!("/v1/connections/{token_id}"),
            None,
        ))
        .await
        .expect("撤销请求应返回响应");
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);

    let retried = app
        .oneshot(request_with_token(
            Method::POST,
            "/v1/memories",
            token,
            Some(json!({
                "source": "muse",
                "kind": "idea",
                "content": "撤销后不可写入",
                "content_format": "plain"
            })),
        ))
        .await
        .expect("撤销后的请求仍应返回响应");
    assert_eq!(retried.status(), StatusCode::UNAUTHORIZED);
}

/// 验证第三方集成可获得显式选择的权限、读取整库，并且写入仍被限定到自身来源。
#[tokio::test]
async fn registers_external_connection_with_scoped_data_flow() {
    let app = test_router([Scope::Admin], None);
    let existing = app
        .clone()
        .oneshot(json_request(
            "/v1/memories",
            json!({
                "source": "orbit",
                "kind": "note",
                "content": "M6 external integration searchable memory",
                "content_format": "plain"
            }),
        ))
        .await
        .expect("管理端应能预置记忆");
    assert_eq!(existing.status(), StatusCode::CREATED);

    let registered = app
        .clone()
        .oneshot(json_request(
            "/v1/connections",
            json!({
                "app_id": "mcp",
                "name": "Nexus MCP",
                "source": "external:mcp",
                "scopes": ["memory:read", "memory:write", "search"]
            }),
        ))
        .await
        .expect("第三方连接登记应返回响应");
    assert_eq!(registered.status(), StatusCode::CREATED);
    let registered = response_json(registered).await;
    let token = registered["token"].as_str().expect("应签发第三方令牌");
    assert_eq!(
        registered["scopes"],
        json!(["memory:read", "memory:write", "search"])
    );

    let searched = app
        .clone()
        .oneshot(request_with_token(
            Method::POST,
            "/v1/search",
            token,
            Some(json!({
                "text": "external integration searchable",
                "mode": "hybrid",
                "limit": 10
            })),
        ))
        .await
        .expect("第三方检索应返回响应");
    assert_eq!(searched.status(), StatusCode::OK);
    assert_eq!(
        response_json(searched).await["hits"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let created = app
        .clone()
        .oneshot(request_with_token(
            Method::POST,
            "/v1/memories",
            token,
            Some(json!({
                "source": "external:mcp",
                "kind": "note",
                "content": "MCP owned memory",
                "content_format": "plain"
            })),
        ))
        .await
        .expect("第三方自身来源写入应返回响应");
    assert_eq!(created.status(), StatusCode::CREATED);

    let escaped = app
        .clone()
        .oneshot(request_with_token(
            Method::POST,
            "/v1/memories",
            token,
            Some(json!({
                "source": "external:other",
                "kind": "note",
                "content": "越权来源",
                "content_format": "plain"
            })),
        ))
        .await
        .expect("越权写入应返回响应");
    assert_eq!(escaped.status(), StatusCode::FORBIDDEN);

    let listed = app
        .oneshot(authorized_request(Method::GET, "/v1/connections", None))
        .await
        .expect("连接审计列表应返回响应");
    let listed = response_json(listed).await;
    assert_eq!(listed[0]["readCount"], 1);
    assert_eq!(listed[0]["writeCount"], 2);
    assert_eq!(listed[0]["lastScope"], "memory:write");
    assert_eq!(listed[0]["sendsDataRemote"], false);
}

/// 验证第三方连接不能通过登记接口获得管理或复习能力。
#[tokio::test]
async fn rejects_privileged_external_scopes() {
    for scope in ["admin", "review"] {
        let app = test_router([Scope::Admin], None);
        let response = app
            .oneshot(json_request(
                "/v1/connections",
                json!({
                    "app_id": "unsafe-app",
                    "name": "Unsafe App",
                    "source": "external:unsafe-app",
                    "scopes": [scope]
                }),
            ))
            .await
            .expect("越权登记应返回响应");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

/// 验证第三方令牌只以摘要持久化，并在服务重启后继续可用和可撤销。
#[tokio::test]
async fn persists_external_connections_without_plaintext_token() {
    let directory = tempfile::tempdir().expect("应创建临时连接目录");
    let path = directory.path().join("connections.json");
    let state = ProtocolState::from_shared_with_connection_store(
        Arc::new(MemoryStore::open_in_memory().expect("应创建内存工作库")),
        Arc::new(HashEmbedder::default()),
        CapabilityGrant::new(TOKEN, [Scope::Admin], None),
        path.clone(),
    )
    .expect("应创建带持久化的协议状态");
    let app = router(state);
    let registered = app
        .oneshot(json_request(
            "/v1/connections",
            json!({
                "app_id": "persistent-sdk",
                "name": "Persistent SDK",
                "source": "external:persistent-sdk",
                "scopes": ["memory:read", "search"]
            }),
        ))
        .await
        .expect("持久化授权应返回响应");
    let registered = response_json(registered).await;
    let token = registered["token"].as_str().unwrap().to_owned();
    let persisted = std::fs::read_to_string(&path).expect("应写入连接存储");
    assert!(!persisted.contains(&token));
    assert!(persisted.contains("token_digest"));

    let restored = ProtocolState::from_shared_with_connection_store(
        Arc::new(MemoryStore::open_in_memory().expect("应创建重启后的内存工作库")),
        Arc::new(HashEmbedder::default()),
        CapabilityGrant::new(TOKEN, [Scope::Admin], None),
        path.clone(),
    )
    .expect("应恢复连接存储");
    let app = router(restored);
    let searched = app
        .clone()
        .oneshot(request_with_token(
            Method::POST,
            "/v1/search",
            &token,
            Some(json!({"text": "restart", "mode": "hybrid", "limit": 10})),
        ))
        .await
        .expect("恢复后的令牌应返回响应");
    assert_eq!(searched.status(), StatusCode::OK);
    let listed = app
        .oneshot(authorized_request(Method::GET, "/v1/connections", None))
        .await
        .expect("恢复后的连接应可管理");
    assert_eq!(response_json(listed).await[0]["id"], "persistent-sdk");
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
    request_with_token(method, uri, TOKEN, body)
}

/// 构造携带指定 Bearer token 的协议请求。
fn request_with_token(
    method: Method,
    uri: &str,
    token: &str,
    body: Option<Value>,
) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
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
