//! 本文件实现仅监听回环地址的 Memory Protocol v1 HTTP 路由与错误响应。

use std::{io, sync::Arc};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use nexus_core::{
    CoreError, HashEmbedder, IngestInput, Ingestor, MemorySource, MemoryStore, SearchMode,
    SearchQuery,
};
use serde::Serialize;
use tokio::net::TcpListener;

use crate::{
    CapabilityGrant, Scope,
    dto::{
        CapabilitiesResponse, CreateMemoryRequest, CreateMemoryResponse, SearchHitResponse,
        SearchRequest, SearchResponse,
    },
    protocol_version,
};

const IMPLEMENTED_CAPABILITIES: &[&str] = &["memory:create", "search", "capabilities"];

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
        .route("/v1/memories", post(create_memory))
        .route("/v1/search", post(search))
        .with_state(state)
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
    let hits = state.store.search(
        &SearchQuery {
            text: request.text,
            mode,
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
