//! 本文件实现仅监听回环地址的 Memory Protocol v1 HTTP 路由与错误响应。

use std::{
    collections::HashMap,
    convert::Infallible,
    io,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

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
    Collection, CollectionPatch, CoreError, HashEmbedder, IngestInput, Ingestor, Link,
    LinkRelation, ListQuery, MemoryFilters, MemoryKind, MemoryPatch, MemorySource, MemoryStore,
    SearchMode, SearchQuery,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use crate::{
    CapabilityGrant, Scope,
    dto::{
        CapabilitiesResponse, ConnectedAppResponse, CreateCollectionRequest, CreateLinkRequest,
        CreateMemoryRequest, CreateMemoryResponse, ListLinksRequest, ListMemoriesRequest,
        ListMemoriesResponse, MemoryResponse, RegisterConnectionRequest,
        RegisterConnectionResponse, SearchHitResponse, SearchRequest, SearchResponse,
        UpdateCollectionRequest, UpdateMemoryRequest,
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
    "links:manage",
    "collections:manage",
    "connections:manage",
    "capabilities",
];

/// 保存一条可撤销的本地应用授权及其连接元数据。
#[derive(Clone)]
struct RegisteredConnection {
    token_id: Uuid,
    app_id: String,
    name: String,
    source: String,
    grant: CapabilityGrant,
    last_active_at: i64,
}

/// 持有本地协议服务共享的存储、嵌入器和客户端授权。
#[derive(Clone)]
pub struct ProtocolState {
    store: Arc<MemoryStore>,
    embedder: Arc<HashEmbedder>,
    admin_grant: Arc<CapabilityGrant>,
    connections: Arc<RwLock<HashMap<Uuid, RegisteredConnection>>>,
}

impl ProtocolState {
    /// 使用工作库和单个客户端授权创建本地服务状态。
    #[must_use]
    pub fn new(store: MemoryStore, grant: CapabilityGrant) -> Self {
        Self::from_shared(Arc::new(store), Arc::new(HashEmbedder::default()), grant)
    }

    /// 使用共享工作库与嵌入器创建服务状态，供本地持有者同时服务 IPC 和 HTTP。
    #[must_use]
    pub fn from_shared(
        store: Arc<MemoryStore>,
        embedder: Arc<HashEmbedder>,
        grant: CapabilityGrant,
    ) -> Self {
        Self {
            store,
            embedder,
            admin_grant: Arc::new(grant),
            connections: Arc::new(RwLock::new(HashMap::new())),
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
        .route(
            "/v1/connections",
            post(register_connection).get(list_connections),
        )
        .route(
            "/v1/connections/{token_id}",
            axum::routing::delete(revoke_connection),
        )
        .route("/v1/links", post(create_link).get(list_links))
        .route(
            "/v1/links/{from_id}/{to_id}/{relation}",
            axum::routing::delete(delete_link),
        )
        .route(
            "/v1/collections",
            post(create_collection).get(list_collections),
        )
        .route(
            "/v1/collections/{id}",
            get(get_collection)
                .patch(update_collection)
                .delete(delete_collection),
        )
        .route(
            "/v1/collections/{collection_id}/memories/{memory_id}",
            axum::routing::put(add_collection_memory).delete(remove_collection_memory),
        )
        .route(
            "/v1/collections/{id}/memories",
            get(list_collection_memories),
        )
        .with_state(state)
}

/// 校验持有者凭据并为 M3 Muse 签发仅可写入 `source=muse` 的令牌。
async fn register_connection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Json(request): Json<RegisterConnectionRequest>,
) -> Result<(StatusCode, Json<RegisterConnectionResponse>), ProtocolError> {
    authorize_admin(&headers, &state)?;
    if request.app_id != "com.nexus.muse"
        || request.name.trim() != "Muse"
        || request.source != "muse"
        || request.scopes != [Scope::MemoryWrite.as_str()]
    {
        return Err(ProtocolError::InvalidRequest(
            "M3 仅允许 Muse 申请 source=muse 的 memory:write 能力".into(),
        ));
    }

    let mut connections = state
        .connections
        .write()
        .map_err(|_| ProtocolError::InvalidRequest("连接授权状态不可用".into()))?;
    if let Some(existing) = connections
        .values_mut()
        .find(|connection| connection.app_id == request.app_id)
    {
        existing.last_active_at = unix_millis();
        return Ok((
            StatusCode::OK,
            Json(RegisterConnectionResponse {
                token_id: existing.token_id,
                token: existing.grant.token_value().to_owned(),
                scopes: vec![Scope::MemoryWrite.as_str().into()],
                source: existing.source.clone(),
            }),
        ));
    }

    let token_id = Uuid::new_v4();
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    connections.insert(
        token_id,
        RegisteredConnection {
            token_id,
            app_id: request.app_id,
            name: request.name,
            source: request.source.clone(),
            grant: CapabilityGrant::new(
                token.clone(),
                [Scope::MemoryWrite],
                Some(request.source.clone()),
            ),
            last_active_at: unix_millis(),
        },
    );
    Ok((
        StatusCode::CREATED,
        Json(RegisterConnectionResponse {
            token_id,
            token,
            scopes: vec![Scope::MemoryWrite.as_str().into()],
            source: request.source,
        }),
    ))
}

/// 校验管理权限并返回已登记应用及其来源记忆数量。
async fn list_connections(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ConnectedAppResponse>>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    let connections = state
        .connections
        .read()
        .map_err(|_| ProtocolError::InvalidRequest("连接授权状态不可用".into()))?;
    let mut response = Vec::with_capacity(connections.len());
    for connection in connections.values() {
        let page = state.store.list(&ListQuery {
            filters: MemoryFilters {
                sources: vec![connection.source.clone()],
                ..MemoryFilters::default()
            },
            limit: 1,
            offset: 0,
        })?;
        response.push(ConnectedAppResponse {
            id: connection.app_id.clone(),
            name: connection.name.clone(),
            source: connection.source.clone(),
            scopes: connection
                .grant
                .scopes()
                .map(|scope| scope.as_str().to_owned())
                .collect(),
            last_active_at: connection.last_active_at,
            memories_count: page.total,
            token_id: connection.token_id,
        });
    }
    response.sort_by_key(|connection| std::cmp::Reverse(connection.last_active_at));
    Ok(Json(response))
}

/// 校验管理权限并立即撤销指定本地应用令牌。
async fn revoke_connection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path(token_id): Path<Uuid>,
) -> Result<StatusCode, ProtocolError> {
    authorize_admin(&headers, &state)?;
    let removed = state
        .connections
        .write()
        .map_err(|_| ProtocolError::InvalidRequest("连接授权状态不可用".into()))?
        .remove(&token_id);
    if removed.is_none() {
        return Err(ProtocolError::InvalidRequest("连接不存在或已被撤销".into()));
    }
    Ok(StatusCode::NO_CONTENT)
}

/// 校验管理权限并创建记忆关联。
async fn create_link(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Json(request): Json<CreateLinkRequest>,
) -> Result<(StatusCode, Json<Link>), ProtocolError> {
    authorize_admin(&headers, &state)?;
    let link = state.store.create_link(
        request.from_id,
        request.to_id,
        request.relation,
        request.created_by,
    )?;
    Ok((StatusCode::CREATED, Json(link)))
}

/// 校验管理权限并返回指定记忆参与的关联。
async fn list_links(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Query(request): Query<ListLinksRequest>,
) -> Result<Json<Vec<Link>>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    Ok(Json(state.store.list_links(request.memory_id)?))
}

/// 校验管理权限并删除指定关联。
async fn delete_link(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path((from_id, to_id, relation)): Path<(Uuid, Uuid, String)>,
) -> Result<StatusCode, ProtocolError> {
    authorize_admin(&headers, &state)?;
    state
        .store
        .delete_link(from_id, to_id, parse_relation(&relation)?)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 校验管理权限并创建集合。
async fn create_collection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Json(request): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<Collection>), ProtocolError> {
    authorize_admin(&headers, &state)?;
    let collection = state.store.create_collection(
        request.name,
        request.icon,
        request.parent_id,
        request.sort,
    )?;
    Ok((StatusCode::CREATED, Json(collection)))
}

/// 校验管理权限并返回全部集合。
async fn list_collections(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Collection>>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    Ok(Json(state.store.list_collections()?))
}

/// 校验管理权限并读取单个集合。
async fn get_collection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Collection>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    Ok(Json(
        state
            .store
            .get_collection(id)?
            .ok_or(CoreError::NotFound(id))?,
    ))
}

/// 校验管理权限并更新集合字段或层级。
async fn update_collection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateCollectionRequest>,
) -> Result<Json<Collection>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    let icon = request
        .clear_icon
        .then_some(None)
        .or(request.icon.map(Some));
    let parent_id = request
        .move_to_root
        .then_some(None)
        .or(request.parent_id.map(Some));
    Ok(Json(state.store.update_collection(
        id,
        CollectionPatch {
            name: request.name,
            icon,
            parent_id,
            sort: request.sort,
        },
    )?))
}

/// 校验管理权限并删除集合。
async fn delete_collection(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ProtocolError> {
    authorize_admin(&headers, &state)?;
    state.store.delete_collection(id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 校验管理权限并幂等地把记忆加入集合。
async fn add_collection_memory(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path((collection_id, memory_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ProtocolError> {
    authorize_admin(&headers, &state)?;
    state
        .store
        .add_memory_to_collection(collection_id, memory_id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 校验管理权限并从集合移除记忆。
async fn remove_collection_memory(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path((collection_id, memory_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ProtocolError> {
    authorize_admin(&headers, &state)?;
    state
        .store
        .remove_memory_from_collection(collection_id, memory_id)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 校验管理权限并返回集合成员标识。
async fn list_collection_memories(
    State(state): State<ProtocolState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Uuid>>, ProtocolError> {
    authorize_admin(&headers, &state)?;
    Ok(Json(state.store.list_collection_memory_ids(id)?))
}

/// 校验订阅权限并把核心提交事件持续转换为 SSE 消息。
async fn events(
    State(state): State<ProtocolState>,
    Query(request): Query<EventSubscriptionRequest>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ProtocolError> {
    let grant = authorize(&headers, &state, Scope::Subscribe)?;
    let subscription = state.store.subscribe()?;
    let requested_types = split_csv(request.types.as_deref());
    let source_restriction = grant.source_restriction().map(str::to_owned);
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
    let grant = authorize(&headers, &state, Scope::MemoryRead)?;
    let memory = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !grant.allows_source(&memory.source.as_storage_value()) {
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
    let grant = authorize(&headers, &state, Scope::MemoryRead)?;
    let filters = list_filters(&request, &grant)?;
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
    let grant = authorize(&headers, &state, Scope::MemoryWrite)?;
    let existing = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !grant.allows_source(&existing.source.as_storage_value()) {
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
    let grant = authorize(&headers, &state, Scope::MemoryDelete)?;
    let existing = state.store.get(&id)?.ok_or(CoreError::NotFound(id))?;
    if !grant.allows_source(&existing.source.as_storage_value()) {
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

/// 在回环监听器上运行服务，并在关闭信号完成后优雅停止。
pub async fn serve_with_shutdown<F>(
    listener: TcpListener,
    state: ProtocolState,
    shutdown: F,
) -> Result<(), ProtocolError>
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    if !listener.local_addr()?.ip().is_loopback() {
        return Err(ProtocolError::NonLoopbackAddress);
    }
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await?;
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
    let grant = authorize(&headers, &state, Scope::MemoryWrite)?;
    if !grant.allows_source(&request.source) {
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
    let grant = authorize(&headers, &state, Scope::Search)?;
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
    apply_source_restriction(&mut filters.sources, &grant)?;
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

/// 将路径中的关系类型转换为统一关联枚举。
fn parse_relation(value: &str) -> Result<LinkRelation, ProtocolError> {
    serde_json::from_str(&format!("\"{value}\""))
        .map_err(|_| ProtocolError::InvalidRequest(format!("未知关联类型: {value}")))
}

/// 从 Authorization 请求头提取令牌并校验目标能力域，返回匹配的授权快照。
fn authorize(
    headers: &HeaderMap,
    state: &ProtocolState,
    scope: Scope,
) -> Result<CapabilityGrant, ProtocolError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ProtocolError::Unauthorized)?;
    if state.admin_grant.accepts_token(token) {
        if !state.admin_grant.allows(scope) {
            return Err(ProtocolError::Forbidden);
        }
        return Ok((*state.admin_grant).clone());
    }

    let mut connections = state
        .connections
        .write()
        .map_err(|_| ProtocolError::Unauthorized)?;
    let connection = connections
        .values_mut()
        .find(|connection| connection.grant.accepts_token(token))
        .ok_or(ProtocolError::Unauthorized)?;
    if !connection.grant.allows(scope) {
        return Err(ProtocolError::Forbidden);
    }
    connection.last_active_at = unix_millis();
    Ok(connection.grant.clone())
}

/// 仅接受 Orbit 持有者管理令牌，避免普通连接自行登记或撤销其他应用。
fn authorize_admin(headers: &HeaderMap, state: &ProtocolState) -> Result<(), ProtocolError> {
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .ok_or(ProtocolError::Unauthorized)?;
    if !state.admin_grant.accepts_token(token) || !state.admin_grant.allows(Scope::Admin) {
        return Err(ProtocolError::Unauthorized);
    }
    Ok(())
}

/// 返回本地连接审计使用的 Unix 毫秒时间。
fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
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
