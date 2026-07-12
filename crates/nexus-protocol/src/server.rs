//! 本文件实现仅监听回环地址的 Memory Protocol v1 HTTP 路由与错误响应。

use std::{convert::Infallible, io, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{get, post},
};
use nexus_core::{
    CoreError, HashEmbedder, IngestInput, Ingestor, ListQuery, MemoryFilters, MemoryKind,
    MemoryPatch, MemorySource, MemoryStore, SearchMode, SearchQuery,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{
    CapabilityGrant, Scope,
    dto::{
        CapabilitiesResponse, CreateMemoryRequest, CreateMemoryResponse, ListMemoriesRequest,
        ListMemoriesResponse, MemoryResponse, SearchHitResponse, SearchRequest, SearchResponse,
        UpdateMemoryRequest,
    },
    protocol_version,
};

const IMPLEMENTED_CAPABILITIES: &[&str] = &[
    "memory:create",
    "memory:read",
    "memory:update",
    "memory:delete",
    "memory:list",
    "search",
    "events:subscribe",
    "capabilities",
];

/// 持有本地协议服务共享的存储、嵌入器和客户端授权。
#[derive(Clone)]
pub struct ProtocolState {
    store: Arc<MemoryStore>,
    embedder: Arc<HashEmbedder>,
    grant: Arc<CapabilityGrant>,
}

impl ProtocolState {
    /// 使用工作库和单个客户端授权创建本地服务状态。
    #[must_use]
    pub fn new(store: MemoryStore, grant: CapabilityGrant) -> Self {
        Self {
            store: Arc::new(store),
            embedder: Arc::new(HashEmbedder::default()),
            grant: Arc::new(grant),
        }
    }
}

/// 表示协议认证、授权、输入或核心操作错误。
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    /// 请求没有携带有效 Bearer 令牌。
    #[error("未通过身份验证")]
    Unauthorized,
    /// 令牌没有当前操作所需能力或来源权限。
    #[error("当前令牌无权执行此操作")]
    Forbidden,
    /// JSON 字段值不符合协议约束。
    #[error("请求无效: {0}")]
    InvalidRequest(String),
    /// 核心存储或检索操作失败。
    #[error(transparent)]
    Core(#[from] CoreError),
    /// 本地服务不能绑定到非回环接口。
    #[error("本地服务只允许监听回环地址")]
    NonLoopbackAddress,
    /// HTTP 服务运行失败。
    #[error(transparent)]
    Io(#[from] io::Error),
}

impl IntoResponse for ProtocolError {
    /// 将内部错误转换为稳定的 JSON HTTP 响应。
    fn into_response(self) -> Response {
        let status = match &self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::InvalidRequest(_) | Self::Core(CoreError::InvalidInput(_)) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            Self::NonLoopbackAddress => StatusCode::BAD_REQUEST,
            Self::Core(CoreError::NotFound(_)) => StatusCode::NOT_FOUND,
            Self::Core(_) | Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(ErrorResponse {
                error: self.to_string(),
            }),
        )
            .into_response()
    }
}

/// 构造包含 Memory Protocol v1 路径的 Axum 路由。
pub fn router(state: ProtocolState) -> Router {
    Router::new()
        .route("/v1/capabilities", get(capabilities))
        .route("/v1/memories", post(create_memory).get(list_memories))
        .route(
            "/v1/memories/{id}",
            get(get_memory).patch(update_memory).delete(delete_memory),
        )
        .route("/v1/search", post(search))
        .route("/v1/events", get(events))
        .with_state(state)
}

/// 校验订阅权限并把核心提交事件持续转换为 SSE 消息。
async fn events(
    State(state): State<ProtocolState>,
    Query(request): Query<EventSubscriptionRequest>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ProtocolError> {
    authorize(&headers, &state.grant, Scope::Subscribe)?;
    let subscription = state.store.subscribe()?;
    let requested_types = split_csv(request.types.as_deref());
    let source_restriction = state.grant.source_restriction().map(str::to_owned);
    let (sender, receiver) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);
    tokio::task::spawn_blocking(move || {
        while let Some(event) = subscription.recv() {
            let source = match &event {
                nexus_core::CoreEvent::MemoryCreated { source, .. }
                | nexus_core::CoreEvent::MemoryUpdated { source, .. }
                | nexus_core::CoreEvent::MemoryDeleted { source, .. } => source,
            };
            if source_restriction
                .as_ref()
                .is_some_and(|allowed| allowed != source)
            {
                continue;
            }
            let event_name = match &event {
                nexus_core::CoreEvent::MemoryCreated { .. } => "memory.created",
                nexus_core::CoreEvent::MemoryUpdated { .. } => "memory.updated",
                nexus_core::CoreEvent::MemoryDeleted { .. } => "memory.deleted",
            };
            if !requested_types.is_empty()
                && !requested_types.iter().any(|value| value == event_name)
            {
                continue;
            }
            let Ok(data) = serde_json::to_string(&event) else {
                continue;
            };
            if sender
                .blocking_send(Ok(Event::default().event(event_name).data(data)))
                .is_err()
            {
                break;
            }
        }
    });
    Ok(Sse::new(ReceiverStream::new(receiver)).keep_alive(KeepAlive::default()))
}

/// 表示事件订阅可选的逗号分隔事件类型过滤器。
#[derive(Debug, Default, Deserialize)]
struct EventSubscriptionRequest {
    /// 例如 `memory.created,memory.updated`；省略时订阅全部事件。
    types: Option<String>,
}

/// 校验读取权限并返回一条完整记忆。
async fn get_memory(
    State(state): State<ProtocolState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<MemoryResponse>, ProtocolError> {
    authorize(&headers, &state.grant, Scope::MemoryRead)?;
    let memory = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !state.grant.allows_source(&memory.source.as_storage_value()) {
        return Err(ProtocolError::Forbidden);
    }
    Ok(Json(memory.into()))
}

/// 校验读取权限并返回经过来源、类别、标签和时间过滤的记忆页。
async fn list_memories(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Query(request): Query<ListMemoriesRequest>,
) -> Result<Json<ListMemoriesResponse>, ProtocolError> {
    authorize(&headers, &state.grant, Scope::MemoryRead)?;
    let filters = list_filters(&request, &state.grant)?;
    let page = state.store.list(&ListQuery {
        filters,
        limit: request.limit,
        offset: request.offset,
    })?;
    Ok(Json(ListMemoriesResponse {
        items: page.items.into_iter().map(MemoryResponse::from).collect(),
        total: page.total,
        next_offset: page.next_offset,
    }))
}

/// 校验写入权限和来源范围，并应用字段级记忆更新。
async fn update_memory(
    State(state): State<ProtocolState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<UpdateMemoryRequest>,
) -> Result<Json<MemoryResponse>, ProtocolError> {
    authorize(&headers, &state.grant, Scope::MemoryWrite)?;
    let existing = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !state
        .grant
        .allows_source(&existing.source.as_storage_value())
    {
        return Err(ProtocolError::Forbidden);
    }
    let memory = state.store.update(
        &id,
        MemoryPatch {
            title: request.title.map(Some),
            content: request.content,
            content_format: request.content_format,
            tags: request.tags,
            pinned: request.pinned,
            archived: request.archived,
            captured_at: request.captured_at.map(Some),
            meta: request.meta,
        },
        state.embedder.as_ref(),
    )?;
    Ok(Json(memory.into()))
}

/// 校验删除权限和来源范围，并执行级联删除。
async fn delete_memory(
    State(state): State<ProtocolState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, ProtocolError> {
    authorize(&headers, &state.grant, Scope::MemoryDelete)?;
    let existing = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !state
        .grant
        .allows_source(&existing.source.as_storage_value())
    {
        return Err(ProtocolError::Forbidden);
    }
    state.store.delete(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 在已绑定的回环监听器上运行本地协议服务。
pub async fn serve(listener: TcpListener, state: ProtocolState) -> Result<(), ProtocolError> {
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(ProtocolError::NonLoopbackAddress);
    }
    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// 返回协议版本、已实现能力与全部授权域。
async fn capabilities() -> Json<CapabilitiesResponse> {
    Json(CapabilitiesResponse {
        version: protocol_version(),
        capabilities: IMPLEMENTED_CAPABILITIES,
        scopes: [
            Scope::MemoryRead,
            Scope::MemoryWrite,
            Scope::MemoryDelete,
            Scope::Search,
            Scope::Subscribe,
            Scope::Review,
            Scope::Admin,
        ]
        .into_iter()
        .map(Scope::as_str)
        .collect(),
    })
}

/// 校验写入权限并通过 nexus-core 统一管线创建记忆。
async fn create_memory(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Json(request): Json<CreateMemoryRequest>,
) -> Result<(StatusCode, Json<CreateMemoryResponse>), ProtocolError> {
    authorize(&headers, &state.grant, Scope::MemoryWrite)?;
    if !state.grant.allows_source(&request.source) {
        return Err(ProtocolError::Forbidden);
    }
    let source = parse_source(&request.source)?;
    let memory = Ingestor::new(&state.store, state.embedder.as_ref()).ingest(IngestInput {
        source,
        kind: request.kind,
        title: request.title,
        content: request.content,
        content_format: request.content_format,
        tags: request.tags,
        captured_at: request.captured_at,
        device_id: request.device_id.unwrap_or_else(|| "protocol-local".into()),
        meta: request.meta,
    })?;
    Ok((
        StatusCode::CREATED,
        Json(CreateMemoryResponse {
            id: memory.id,
            created_at: memory.created_at,
        }),
    ))
}

/// 校验检索权限并将协议请求映射到 nexus-core 混合检索。
async fn search(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Json(request): Json<SearchRequest>,
) -> Result<Json<SearchResponse>, ProtocolError> {
    authorize(&headers, &state.grant, Scope::Search)?;
    let mode = match request.mode.as_str() {
        "semantic" => SearchMode::Semantic,
        "keyword" => SearchMode::Keyword,
        "hybrid" => SearchMode::Hybrid,
        value => {
            return Err(ProtocolError::InvalidRequest(format!(
                "未知检索模式: {value}"
            )));
        }
    };
    let mut filters = MemoryFilters {
        sources: request.filters.source,
        kinds: request.filters.kind,
        tags: request.filters.tags,
        created_from: request.filters.created_from,
        created_to: request.filters.created_to,
    };
    apply_source_restriction(&mut filters.sources, &state.grant)?;
    let hits = state.store.search(
        &SearchQuery {
            text: request.text,
            mode,
            filters,
            limit: request.limit.min(100),
        },
        state.embedder.as_ref(),
    )?;
    Ok(Json(SearchResponse {
        hits: hits
            .into_iter()
            .map(|hit| SearchHitResponse {
                memory_id: hit.memory_id,
                block_id: hit.block_id,
                score: hit.score,
                snippet: hit.snippet,
            })
            .collect(),
    }))
}

/// 将列表查询字符串转换为核心过滤条件，并强制应用令牌来源限制。
fn list_filters(
    request: &ListMemoriesRequest,
    grant: &CapabilityGrant,
) -> Result<MemoryFilters, ProtocolError> {
    let mut sources = split_csv(request.source.as_deref());
    apply_source_restriction(&mut sources, grant)?;
    let kinds = split_csv(request.kind.as_deref())
        .into_iter()
        .map(|value| parse_kind(&value))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MemoryFilters {
        sources,
        kinds,
        tags: split_csv(request.tags.as_deref()),
        created_from: request.created_from,
        created_to: request.created_to,
    })
}

/// 将 capability token 的来源限制合并到请求过滤器，拒绝显式越权来源。
fn apply_source_restriction(
    sources: &mut Vec<String>,
    grant: &CapabilityGrant,
) -> Result<(), ProtocolError> {
    if let Some(allowed) = grant.source_restriction() {
        if !sources.is_empty() && sources.iter().any(|source| source != allowed) {
            return Err(ProtocolError::Forbidden);
        }
        sources.clear();
        sources.push(allowed.to_owned());
    }
    Ok(())
}

/// 拆分逗号分隔的查询参数并去除空白项。
fn split_csv(value: Option<&str>) -> Vec<String> {
    value
        .into_iter()
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 将查询字符串中的类别转换为统一记忆枚举。
fn parse_kind(value: &str) -> Result<MemoryKind, ProtocolError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| ProtocolError::InvalidRequest(format!("未知记忆类别: {value}")))
}

/// 从 Authorization 请求头提取令牌并校验目标能力域。
fn authorize(
    headers: &HeaderMap,
    grant: &CapabilityGrant,
    scope: Scope,
) -> Result<(), ProtocolError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ProtocolError::Unauthorized)?;
    if !grant.accepts_token(token) {
        return Err(ProtocolError::Unauthorized);
    }
    if !grant.allows(scope) {
        return Err(ProtocolError::Forbidden);
    }
    Ok(())
}

/// 将协议 source 字符串转换为统一数据模型来源。
fn parse_source(source: &str) -> Result<MemorySource, ProtocolError> {
    match source {
        "echo" => Ok(MemorySource::Echo),
        "muse" => Ok(MemorySource::Muse),
        "quill" => Ok(MemorySource::Quill),
        "orbit" => Ok(MemorySource::Orbit),
        value if value.starts_with("external:") && value.len() > "external:".len() => {
            Ok(MemorySource::External(value["external:".len()..].into()))
        }
        value => Err(ProtocolError::InvalidRequest(format!(
            "未知记忆来源: {value}"
        ))),
    }
}

/// 表示所有协议错误共用的 JSON 响应结构。
#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}
