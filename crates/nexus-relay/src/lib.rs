//! 本文件实现只持久化签名密文与不可逆元数据的 Nexus 零知识同步中继。

use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path as AxumPath, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use nexus_sync::{
    DeviceIdentity, EncryptedSyncEnvelope, OperationKind, SealedPairingKey, SyncError,
    verify_device_signature,
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use uuid::Uuid;

const SNAPSHOT_VERSION: u8 = 1;
const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_PULL_LIMIT: usize = 1_000;
const PAIRING_TTL_MILLIS: i64 = 10 * 60 * 1_000;
const PROOF_MAX_SKEW_MILLIS: i64 = 5 * 60 * 1_000;

/// 表示中继认证、设备签名、密文协议或持久化失败。
#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// Bearer 访问令牌缺失或无效。
    #[error("中继访问令牌无效")]
    Unauthorized,
    /// 设备不存在、已撤销或无权执行当前操作。
    #[error("设备无权执行当前操作")]
    Forbidden,
    /// 请求字段、游标或协议状态无效。
    #[error("中继请求无效: {0}")]
    InvalidRequest(String),
    /// 请求的设备、操作或配对会话不存在。
    #[error("中继资源不存在")]
    NotFound,
    /// 相同设备序号或操作标识对应了不同内容。
    #[error("同步操作与中继已有状态冲突")]
    Conflict,
    /// 中继状态文件读写失败。
    #[error("中继持久化失败: {0}")]
    Io(#[from] io::Error),
    /// JSON 状态编解码失败。
    #[error("中继状态编解码失败: {0}")]
    Serialization(#[from] serde_json::Error),
    /// E2E 信封或设备签名无效。
    #[error(transparent)]
    Sync(#[from] SyncError),
    /// 中继共享状态不可用。
    #[error("中继共享状态不可用")]
    StateUnavailable,
}

impl IntoResponse for RelayError {
    /// 将中继错误转换为稳定 JSON HTTP 响应。
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::InvalidRequest(_) | Self::Sync(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::Io(_) | Self::Serialization(_) | Self::StateUnavailable => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

/// 持有账户访问令牌哈希和可选磁盘快照的零知识中继状态。
#[derive(Clone)]
pub struct RelayState {
    access_token_hash: [u8; 32],
    snapshot_path: Option<PathBuf>,
    snapshot: Arc<Mutex<RelaySnapshot>>,
}

impl RelayState {
    /// 创建仅用于测试或临时自托管的内存中继。
    pub fn in_memory(access_token: impl AsRef<str>) -> Result<Self, RelayError> {
        Self::new(None, access_token.as_ref())
    }

    /// 打开可持久化中继；状态文件只包含设备公钥、签名密文和不可逆同步元数据。
    pub fn open(path: impl AsRef<Path>, access_token: impl AsRef<str>) -> Result<Self, RelayError> {
        Self::new(Some(path.as_ref().to_path_buf()), access_token.as_ref())
    }

    /// 校验高熵访问令牌并载入现有中继快照。
    fn new(snapshot_path: Option<PathBuf>, access_token: &str) -> Result<Self, RelayError> {
        if access_token.trim().len() < 32 {
            return Err(RelayError::InvalidRequest(
                "中继访问令牌至少需要 32 个字符".into(),
            ));
        }
        let snapshot = match snapshot_path.as_deref() {
            Some(path) if path.exists() => {
                let snapshot: RelaySnapshot = serde_json::from_slice(&fs::read(path)?)?;
                if snapshot.version != SNAPSHOT_VERSION {
                    return Err(RelayError::InvalidRequest(
                        "中继状态文件版本不受支持".into(),
                    ));
                }
                snapshot
            }
            _ => RelaySnapshot::default(),
        };
        Ok(Self {
            access_token_hash: *blake3::hash(access_token.trim().as_bytes()).as_bytes(),
            snapshot_path,
            snapshot: Arc::new(Mutex::new(snapshot)),
        })
    }

    /// 使用常量时间哈希比较验证账户访问令牌。
    fn authorize(&self, headers: &HeaderMap) -> Result<(), RelayError> {
        let token = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or(RelayError::Unauthorized)?;
        let candidate = *blake3::hash(token.as_bytes()).as_bytes();
        if self.access_token_hash.ct_eq(&candidate).into() {
            Ok(())
        } else {
            Err(RelayError::Unauthorized)
        }
    }

    /// 在持锁期间修改快照，并仅在修改成功后原子持久化。
    fn mutate<T>(
        &self,
        operation: impl FnOnce(&mut RelaySnapshot) -> Result<T, RelayError>,
    ) -> Result<T, RelayError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| RelayError::StateUnavailable)?;
        snapshot.remove_expired_pairings(unix_millis());
        let previous = snapshot.clone();
        let result = operation(&mut snapshot)?;
        if let Err(error) = self.persist(&snapshot) {
            *snapshot = previous;
            return Err(error);
        }
        Ok(result)
    }

    /// 只读访问当前快照，并在读取前清理过期配对会话。
    fn inspect<T>(
        &self,
        operation: impl FnOnce(&RelaySnapshot) -> Result<T, RelayError>,
    ) -> Result<T, RelayError> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| RelayError::StateUnavailable)?;
        snapshot.remove_expired_pairings(unix_millis());
        operation(&snapshot)
    }

    /// 将完整快照写入同目录临时文件后替换；Windows 不支持覆盖 rename 时保留备份回滚。
    fn persist(&self, snapshot: &RelaySnapshot) -> Result<(), RelayError> {
        let Some(path) = &self.snapshot_path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec(snapshot)?)?;
        match fs::rename(&temporary, path) {
            Ok(()) => {}
            Err(_error) if path.exists() => {
                let backup = path.with_extension("bak");
                if backup.exists() {
                    fs::remove_file(&backup)?;
                }
                fs::rename(path, &backup)?;
                if let Err(replace_error) = fs::rename(&temporary, path) {
                    let _ = fs::rename(&backup, path);
                    return Err(RelayError::Io(replace_error));
                }
                fs::remove_file(backup)?;
            }
            Err(error) => return Err(RelayError::Io(error)),
        }
        Ok(())
    }
}

/// 构造零知识中继 HTTP 路由；生产部署必须由 TLS 终止层只暴露 HTTPS。
pub fn router(state: RelayState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/sync/capabilities", get(capabilities))
        .route("/v1/sync/devices/bootstrap", post(bootstrap_device))
        .route("/v1/sync/devices/recover", post(recover_device))
        .route("/v1/sync/devices", get(list_devices))
        .route("/v1/sync/devices/{device_id}", delete(revoke_device))
        .route("/v1/sync/changes", post(push_change).get(pull_changes))
        .route("/v1/sync/ack", post(acknowledge_changes))
        .route("/v1/sync/pairings", post(create_pairing))
        .route("/v1/sync/pairings/{session_id}", get(get_pairing_status))
        .route(
            "/v1/sync/pairings/{session_id}/request",
            post(request_pairing),
        )
        .route(
            "/v1/sync/pairings/{session_id}/approve",
            post(approve_pairing),
        )
        .route(
            "/v1/sync/pairings/{session_id}/package",
            post(fetch_pairing_package),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

/// 返回不包含账户信息的存活状态。
async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok"}))
}

/// 返回中继支持的零知识协议能力。
async fn capabilities(
    State(state): State<RelayState>,
    headers: HeaderMap,
) -> Result<Json<RelayCapabilities>, RelayError> {
    state.authorize(&headers)?;
    Ok(Json(RelayCapabilities {
        protocol: "nexus-sync-v1",
        zero_knowledge: true,
        encryption: "xchacha20poly1305",
        device_signatures: "ed25519",
        recovery_phrase: "bip39-24",
        max_body_bytes: MAX_BODY_BYTES,
    }))
}

/// 在空工作区登记首台设备；已有设备后只能通过配对流程加入。
async fn bootstrap_device(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(request): Json<BootstrapDeviceRequest>,
) -> Result<(StatusCode, Json<RelayDevice>), RelayError> {
    state.authorize(&headers)?;
    validate_workspace_id(&request.workspace_id)?;
    validate_device_fields(&request.device_id, &request.name, &request.public_key)?;
    verify_device_signature(
        &request.public_key,
        bootstrap_message(&request).as_bytes(),
        &request.signature,
    )?;
    verify_device_signature(
        &request.recovery_public_key,
        recovery_registration_message(&request).as_bytes(),
        &request.recovery_signature,
    )?;
    let device = state.mutate(|snapshot| {
        if snapshot.workspaces.contains_key(&request.workspace_id) {
            return Err(RelayError::Conflict);
        }
        snapshot.workspaces.insert(
            request.workspace_id.clone(),
            RelayWorkspace {
                workspace_id: request.workspace_id.clone(),
                recovery_public_key: request.recovery_public_key,
                created_at: unix_millis(),
            },
        );
        let device = RelayDevice {
            workspace_id: request.workspace_id,
            device_id: request.device_id,
            name: request.name,
            public_key: request.public_key,
            created_at: unix_millis(),
            last_seen_at: unix_millis(),
            revoked_at: None,
            last_sequence: 0,
            acknowledged_cursor: 0,
        };
        snapshot.devices.insert(
            device_map_key(&device.workspace_id, &device.device_id),
            device.clone(),
        );
        Ok(device)
    })?;
    Ok((StatusCode::CREATED, Json(device)))
}

/// 使用恢复短语派生签名登记新设备；根密钥和恢复短语始终不上传中继。
async fn recover_device(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(request): Json<RecoverDeviceRequest>,
) -> Result<(StatusCode, Json<RelayDevice>), RelayError> {
    state.authorize(&headers)?;
    validate_workspace_id(&request.workspace_id)?;
    validate_device_fields(&request.device_id, &request.name, &request.public_key)?;
    verify_device_signature(
        &request.public_key,
        recover_device_message(&request).as_bytes(),
        &request.device_signature,
    )?;
    let device = state.mutate(|snapshot| {
        let workspace = snapshot
            .workspaces
            .get(&request.workspace_id)
            .ok_or(RelayError::NotFound)?;
        verify_device_signature(
            &workspace.recovery_public_key,
            recover_device_message(&request).as_bytes(),
            &request.recovery_signature,
        )?;
        let map_key = device_map_key(&request.workspace_id, &request.device_id);
        if snapshot.devices.contains_key(&map_key) {
            return Err(RelayError::Conflict);
        }
        let device = RelayDevice {
            workspace_id: request.workspace_id,
            device_id: request.device_id,
            name: request.name,
            public_key: request.public_key,
            created_at: unix_millis(),
            last_seen_at: unix_millis(),
            revoked_at: None,
            last_sequence: 0,
            acknowledged_cursor: 0,
        };
        snapshot.devices.insert(map_key, device.clone());
        Ok(device)
    })?;
    Ok((StatusCode::CREATED, Json(device)))
}

/// 返回工作区内设备，必须由未撤销设备签名当前读取请求。
async fn list_devices(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Query(request): Query<SignedWorkspaceQuery>,
) -> Result<Json<Vec<RelayDevice>>, RelayError> {
    state.authorize(&headers)?;
    state.inspect(|snapshot| {
        let proof = request.proof();
        snapshot.verify_proof(
            &request.workspace_id,
            &proof,
            &format!("devices:list:{}", request.workspace_id),
        )?;
        let mut devices = snapshot
            .devices
            .values()
            .filter(|device| device.workspace_id == request.workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        devices.sort_by_key(|device| device.created_at);
        Ok(Json(devices))
    })
}

/// 撤销目标设备；至少保留一台未撤销设备，避免工作区失去管理入口。
async fn revoke_device(
    State(state): State<RelayState>,
    headers: HeaderMap,
    AxumPath(target_device_id): AxumPath<String>,
    Json(request): Json<RevokeDeviceRequest>,
) -> Result<Json<RelayDevice>, RelayError> {
    state.authorize(&headers)?;
    state.mutate(|snapshot| {
        snapshot.verify_proof(
            &request.workspace_id,
            &request.proof,
            &format!("device:revoke:{}:{target_device_id}", request.workspace_id),
        )?;
        let active_count = snapshot
            .devices
            .values()
            .filter(|device| {
                device.workspace_id == request.workspace_id && device.revoked_at.is_none()
            })
            .count();
        let key = device_map_key(&request.workspace_id, &target_device_id);
        let target = snapshot.devices.get_mut(&key).ok_or(RelayError::NotFound)?;
        if target.revoked_at.is_none() && active_count <= 1 {
            return Err(RelayError::InvalidRequest(
                "不能撤销工作区最后一台有效设备".into(),
            ));
        }
        target.revoked_at.get_or_insert_with(unix_millis);
        Ok(Json(target.clone()))
    })
}

/// 验证设备签名并存储密文操作；墓碑会立即删除同实体的既有密文。
async fn push_change(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(envelope): Json<EncryptedSyncEnvelope>,
) -> Result<(StatusCode, Json<PushChangeResponse>), RelayError> {
    state.authorize(&headers)?;
    envelope.validate()?;
    let response = state.mutate(|snapshot| snapshot.push_envelope(envelope))?;
    Ok((StatusCode::CREATED, Json(response)))
}

/// 按服务器游标拉取密文增量，不接触或解析操作明文。
async fn pull_changes(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Query(request): Query<PullChangesRequest>,
) -> Result<Json<PullChangesResponse>, RelayError> {
    state.authorize(&headers)?;
    if request.limit == 0 || request.limit > MAX_PULL_LIMIT {
        return Err(RelayError::InvalidRequest(format!(
            "单次拉取数量必须为 1 到 {MAX_PULL_LIMIT}"
        )));
    }
    state.inspect(|snapshot| {
        let proof = request.proof();
        snapshot.verify_proof(
            &request.workspace_id,
            &proof,
            &format!(
                "changes:pull:{}:{}:{}",
                request.workspace_id, request.after, request.limit
            ),
        )?;
        let changes = snapshot
            .changes
            .range(request.after.saturating_add(1)..)
            .filter(|(_, change)| change.envelope.workspace_id == request.workspace_id)
            .take(request.limit)
            .map(|(_, change)| change.clone())
            .collect::<Vec<_>>();
        let next_cursor = changes.last().map_or(request.after, |change| change.cursor);
        Ok(Json(PullChangesResponse {
            changes,
            next_cursor,
            has_more: snapshot
                .changes
                .range(next_cursor.saturating_add(1)..)
                .any(|(_, change)| change.envelope.workspace_id == request.workspace_id),
        }))
    })
}

/// 记录设备已应用游标，并在全部有效设备确认墓碑后删除墓碑密文。
async fn acknowledge_changes(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(request): Json<AcknowledgeRequest>,
) -> Result<Json<AcknowledgeResponse>, RelayError> {
    state.authorize(&headers)?;
    state.mutate(|snapshot| {
        snapshot.verify_proof(
            &request.workspace_id,
            &request.proof,
            &format!("changes:ack:{}:{}", request.workspace_id, request.cursor),
        )?;
        if request.cursor > snapshot.cursor {
            return Err(RelayError::InvalidRequest(
                "确认游标超过中继最新游标".into(),
            ));
        }
        let device_key = device_map_key(&request.workspace_id, &request.proof.device_id);
        let acknowledged_cursor = {
            let device = snapshot
                .devices
                .get_mut(&device_key)
                .ok_or(RelayError::Forbidden)?;
            device.acknowledged_cursor = device.acknowledged_cursor.max(request.cursor);
            device.last_seen_at = unix_millis();
            device.acknowledged_cursor
        };
        let removed_tombstones = snapshot.compact_acknowledged_tombstones(&request.workspace_id);
        Ok(Json(AcknowledgeResponse {
            acknowledged_cursor,
            removed_tombstones,
        }))
    })
}

/// 创建一次性配对会话；二维码中的秘密不包含在该请求中。
async fn create_pairing(
    State(state): State<RelayState>,
    headers: HeaderMap,
    Json(request): Json<CreatePairingRequest>,
) -> Result<(StatusCode, Json<PairingSessionResponse>), RelayError> {
    state.authorize(&headers)?;
    state.mutate(|snapshot| {
        snapshot.verify_proof(
            &request.workspace_id,
            &request.proof,
            &format!(
                "pairing:create:{}:{}",
                request.workspace_id, request.session_id
            ),
        )?;
        if snapshot.pairings.contains_key(&request.session_id) {
            return Err(RelayError::Conflict);
        }
        let expires_at = unix_millis() + PAIRING_TTL_MILLIS;
        snapshot.pairings.insert(
            request.session_id,
            PairingSession {
                session_id: request.session_id,
                workspace_id: request.workspace_id,
                created_by: request.proof.device_id,
                created_at: unix_millis(),
                expires_at,
                pending_device: None,
                sealed_key: None,
                consumed_at: None,
            },
        );
        Ok((
            StatusCode::CREATED,
            Json(PairingSessionResponse {
                session_id: request.session_id,
                expires_at,
            }),
        ))
    })
}

/// 返回一次配对会话的待批准设备元数据，不返回或解密任何根密钥材料。
async fn get_pairing_status(
    State(state): State<RelayState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<Uuid>,
    Query(request): Query<PairingStatusQuery>,
) -> Result<Json<PairingStatusResponse>, RelayError> {
    state.authorize(&headers)?;
    state.inspect(|snapshot| {
        let proof = request.proof();
        snapshot.verify_proof(
            &request.workspace_id,
            &proof,
            &format!("pairing:status:{}:{session_id}", request.workspace_id),
        )?;
        let session = snapshot
            .pairings
            .get(&session_id)
            .filter(|session| session.workspace_id == request.workspace_id)
            .ok_or(RelayError::NotFound)?;
        Ok(Json(PairingStatusResponse {
            session_id,
            workspace_id: session.workspace_id.clone(),
            expires_at: session.expires_at,
            pending_device: session.pending_device.clone(),
            approved: session.sealed_key.is_some(),
            consumed: session.consumed_at.is_some(),
        }))
    })
}

/// 新设备提交公钥和自签名加入请求，中继仍未获得二维码秘密或根密钥。
async fn request_pairing(
    State(state): State<RelayState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<PairingDeviceRequest>,
) -> Result<StatusCode, RelayError> {
    state.authorize(&headers)?;
    validate_device_fields(&request.device_id, &request.name, &request.public_key)?;
    verify_device_signature(
        &request.public_key,
        pairing_request_message(session_id, &request).as_bytes(),
        &request.signature,
    )?;
    state.mutate(|snapshot| {
        let session = snapshot
            .pairings
            .get_mut(&session_id)
            .ok_or(RelayError::NotFound)?;
        if session.consumed_at.is_some() || session.pending_device.is_some() {
            return Err(RelayError::Conflict);
        }
        session.pending_device = Some(PendingDevice {
            device_id: request.device_id,
            name: request.name,
            public_key: request.public_key,
            requested_at: unix_millis(),
        });
        Ok(StatusCode::ACCEPTED)
    })
}

/// 已连接设备批准新设备，并上传仅二维码秘密可解密的根密钥包。
async fn approve_pairing(
    State(state): State<RelayState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<ApprovePairingRequest>,
) -> Result<Json<RelayDevice>, RelayError> {
    state.authorize(&headers)?;
    request.sealed_key.validate()?;
    state.mutate(|snapshot| {
        let session = snapshot
            .pairings
            .get(&session_id)
            .ok_or(RelayError::NotFound)?;
        let pending = session
            .pending_device
            .as_ref()
            .ok_or_else(|| RelayError::InvalidRequest("尚无新设备等待批准".into()))?;
        if session.workspace_id != request.sealed_key.workspace_id
            || pending.device_id != request.sealed_key.target_device_id
            || request.sealed_key.session_id != session_id
        {
            return Err(RelayError::InvalidRequest("配对密钥包与会话不匹配".into()));
        }
        snapshot.verify_proof(
            &session.workspace_id,
            &request.proof,
            &format!(
                "pairing:approve:{session_id}:{}:{}",
                pending.device_id,
                blake3::hash(request.sealed_key.ciphertext.as_bytes()).to_hex()
            ),
        )?;
        let device = RelayDevice {
            workspace_id: session.workspace_id.clone(),
            device_id: pending.device_id.clone(),
            name: pending.name.clone(),
            public_key: pending.public_key.clone(),
            created_at: unix_millis(),
            last_seen_at: unix_millis(),
            revoked_at: None,
            last_sequence: 0,
            acknowledged_cursor: 0,
        };
        snapshot.devices.insert(
            device_map_key(&device.workspace_id, &device.device_id),
            device.clone(),
        );
        let session = snapshot
            .pairings
            .get_mut(&session_id)
            .ok_or(RelayError::NotFound)?;
        session.sealed_key = Some(request.sealed_key);
        Ok(Json(device))
    })
}

/// 新设备用自己的私钥证明身份并取回不可解密的配对密钥包；会话随后立即消费。
async fn fetch_pairing_package(
    State(state): State<RelayState>,
    headers: HeaderMap,
    AxumPath(session_id): AxumPath<Uuid>,
    Json(request): Json<FetchPairingPackageRequest>,
) -> Result<Json<SealedPairingKey>, RelayError> {
    state.authorize(&headers)?;
    state.mutate(|snapshot| {
        let session = snapshot
            .pairings
            .get_mut(&session_id)
            .ok_or(RelayError::NotFound)?;
        let pending = session
            .pending_device
            .as_ref()
            .ok_or(RelayError::Forbidden)?;
        if pending.device_id != request.device_id {
            return Err(RelayError::Forbidden);
        }
        verify_device_signature(
            &pending.public_key,
            format!("pairing:fetch:{session_id}:{}", request.device_id).as_bytes(),
            &request.signature,
        )?;
        let sealed = session.sealed_key.clone().ok_or(RelayError::NotFound)?;
        // 同一待配对设备可在会话有效期内幂等重取，避免本机 Keystore 写入失败后永久丢失配对包。
        session.consumed_at.get_or_insert_with(unix_millis);
        Ok(Json(sealed))
    })
}

/// 中继能力响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RelayCapabilities {
    protocol: &'static str,
    zero_knowledge: bool,
    encryption: &'static str,
    device_signatures: &'static str,
    recovery_phrase: &'static str,
    max_body_bytes: usize,
}

/// 表示中继登记的设备公钥与同步进度。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayDevice {
    /// 不可逆工作区标识。
    pub workspace_id: String,
    /// 稳定设备标识。
    pub device_id: String,
    /// 用户可识别设备名称。
    pub name: String,
    /// URL-safe Base64 编码的 Ed25519 公钥。
    pub public_key: String,
    /// Unix 毫秒创建时间。
    pub created_at: i64,
    /// Unix 毫秒最近活动时间。
    pub last_seen_at: i64,
    /// Unix 毫秒撤销时间；未撤销时为空。
    pub revoked_at: Option<i64>,
    /// 中继接受的当前设备最大连续序号。
    pub last_sequence: u64,
    /// 当前设备已应用的服务器游标。
    pub acknowledged_cursor: u64,
}

/// 表示通用设备签名证明。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceProof {
    /// 发起请求的设备标识。
    pub device_id: String,
    /// Unix 毫秒签名时间。
    pub timestamp: i64,
    /// 调用方生成的随机重放区分值。
    pub nonce: String,
    /// 对端点稳定消息的 Ed25519 签名。
    pub signature: String,
}

/// 首台设备登记请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDeviceRequest {
    /// 从同步根密钥派生的工作区标识。
    pub workspace_id: String,
    /// 稳定设备标识。
    pub device_id: String,
    /// 用户可识别设备名称。
    pub name: String,
    /// Ed25519 公钥。
    pub public_key: String,
    /// 从同步根密钥域分离派生的恢复签名公钥。
    pub recovery_public_key: String,
    /// 设备对登记消息的自签名。
    pub signature: String,
    /// 恢复签名身份对工作区登记消息的签名。
    pub recovery_signature: String,
}

/// 使用恢复短语登记新设备的请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoverDeviceRequest {
    /// 恢复短语对应的工作区标识。
    pub workspace_id: String,
    /// 新设备稳定标识。
    pub device_id: String,
    /// 用户可识别设备名称。
    pub name: String,
    /// 新设备 Ed25519 公钥。
    pub public_key: String,
    /// 新设备对恢复登记消息的签名。
    pub device_signature: String,
    /// 根密钥派生恢复身份对同一消息的签名。
    pub recovery_signature: String,
}

/// 设备列表查询。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedWorkspaceQuery {
    workspace_id: String,
    device_id: String,
    timestamp: i64,
    nonce: String,
    signature: String,
}

impl SignedWorkspaceQuery {
    /// 将扁平查询字段恢复为统一设备证明。
    fn proof(&self) -> DeviceProof {
        DeviceProof {
            device_id: self.device_id.clone(),
            timestamp: self.timestamp,
            nonce: self.nonce.clone(),
            signature: self.signature.clone(),
        }
    }
}

/// 配对状态读取查询。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingStatusQuery {
    workspace_id: String,
    device_id: String,
    timestamp: i64,
    nonce: String,
    signature: String,
}

impl PairingStatusQuery {
    /// 将扁平查询字段恢复为统一设备证明。
    fn proof(&self) -> DeviceProof {
        DeviceProof {
            device_id: self.device_id.clone(),
            timestamp: self.timestamp,
            nonce: self.nonce.clone(),
            signature: self.signature.clone(),
        }
    }
}

/// 设备撤销请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeDeviceRequest {
    /// 目标工作区。
    pub workspace_id: String,
    /// 当前有效设备签名证明。
    pub proof: DeviceProof,
}

/// 带服务器游标的密文信封。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEnvelope {
    /// 中继分配的单调游标。
    pub cursor: u64,
    /// 设备签名的 E2E 密文信封。
    pub envelope: EncryptedSyncEnvelope,
    /// 墓碑写入时生成的删除回执哈希。
    pub deletion_receipt: Option<String>,
}

/// 上传密文操作响应。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PushChangeResponse {
    /// 中继分配或幂等复用的游标。
    pub cursor: u64,
    /// 相同操作此前是否已经存在。
    pub duplicate: bool,
    /// 墓碑操作删除的旧密文数量。
    pub removed_ciphertexts: usize,
    /// 删除回执哈希；非墓碑操作为空。
    pub deletion_receipt: Option<String>,
}

/// 密文增量拉取查询。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullChangesRequest {
    workspace_id: String,
    #[serde(default)]
    after: u64,
    #[serde(default = "default_pull_limit")]
    limit: usize,
    device_id: String,
    timestamp: i64,
    nonce: String,
    signature: String,
}

impl PullChangesRequest {
    /// 将扁平查询字段恢复为统一设备证明。
    fn proof(&self) -> DeviceProof {
        DeviceProof {
            device_id: self.device_id.clone(),
            timestamp: self.timestamp,
            nonce: self.nonce.clone(),
            signature: self.signature.clone(),
        }
    }
}

/// 密文增量拉取响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PullChangesResponse {
    changes: Vec<StoredEnvelope>,
    next_cursor: u64,
    has_more: bool,
}

/// 游标确认请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgeRequest {
    /// 目标工作区。
    pub workspace_id: String,
    /// 已完整应用的服务器游标。
    pub cursor: u64,
    /// 当前有效设备签名证明。
    pub proof: DeviceProof,
}

/// 游标确认响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcknowledgeResponse {
    acknowledged_cursor: u64,
    removed_tombstones: usize,
}

/// 创建配对会话请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePairingRequest {
    /// 二维码中的会话标识，不包含一次性秘密。
    pub session_id: Uuid,
    /// 目标工作区。
    pub workspace_id: String,
    /// 当前有效设备签名证明。
    pub proof: DeviceProof,
}

/// 配对会话创建响应。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingSessionResponse {
    session_id: Uuid,
    expires_at: i64,
}

/// 配对会话当前状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingStatusResponse {
    session_id: Uuid,
    workspace_id: String,
    expires_at: i64,
    pending_device: Option<PendingDevice>,
    approved: bool,
    consumed: bool,
}

/// 新设备配对申请。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingDeviceRequest {
    /// 新设备稳定标识。
    pub device_id: String,
    /// 用户可识别设备名称。
    pub name: String,
    /// 新设备 Ed25519 公钥。
    pub public_key: String,
    /// 新设备对加入消息的自签名。
    pub signature: String,
}

/// 已连接设备批准配对请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovePairingRequest {
    /// 只可由二维码一次性秘密解密的根密钥包。
    pub sealed_key: SealedPairingKey,
    /// 当前有效设备签名证明。
    pub proof: DeviceProof,
}

/// 新设备取回配对包请求。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchPairingPackageRequest {
    /// 新设备稳定标识。
    pub device_id: String,
    /// 新设备对取回消息的签名。
    pub signature: String,
}

/// 表示配对会话中等待现有设备批准的新设备公开信息。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingDevice {
    /// 待批准设备标识。
    pub device_id: String,
    /// 待批准设备展示名称。
    pub name: String,
    /// 待批准设备 Ed25519 公钥。
    pub public_key: String,
    /// Unix 毫秒申请时间。
    pub requested_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PairingSession {
    session_id: Uuid,
    workspace_id: String,
    created_by: String,
    created_at: i64,
    expires_at: i64,
    pending_device: Option<PendingDevice>,
    sealed_key: Option<SealedPairingKey>,
    consumed_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RelaySnapshot {
    version: u8,
    cursor: u64,
    #[serde(default)]
    workspaces: HashMap<String, RelayWorkspace>,
    devices: HashMap<String, RelayDevice>,
    changes: BTreeMap<u64, StoredEnvelope>,
    pairings: HashMap<Uuid, PairingSession>,
}

impl Default for RelaySnapshot {
    /// 创建空的版本化中继状态。
    fn default() -> Self {
        Self {
            version: SNAPSHOT_VERSION,
            cursor: 0,
            workspaces: HashMap::new(),
            devices: HashMap::new(),
            changes: BTreeMap::new(),
            pairings: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RelayWorkspace {
    workspace_id: String,
    recovery_public_key: String,
    created_at: i64,
}

impl RelaySnapshot {
    /// 验证设备存在、未撤销、时间窗口合理且签名覆盖当前动作。
    fn verify_proof(
        &self,
        workspace_id: &str,
        proof: &DeviceProof,
        action: &str,
    ) -> Result<(), RelayError> {
        validate_workspace_id(workspace_id)?;
        if unix_millis().abs_diff(proof.timestamp) > PROOF_MAX_SKEW_MILLIS as u64 {
            return Err(RelayError::Forbidden);
        }
        if proof.nonce.trim().len() < 16 || proof.nonce.len() > 128 {
            return Err(RelayError::InvalidRequest("设备证明 nonce 长度无效".into()));
        }
        let device = self
            .devices
            .get(&device_map_key(workspace_id, &proof.device_id))
            .filter(|device| device.revoked_at.is_none())
            .ok_or(RelayError::Forbidden)?;
        let message = format!("{action}:{}:{}", proof.timestamp, proof.nonce);
        verify_device_signature(&device.public_key, message.as_bytes(), &proof.signature)?;
        Ok(())
    }

    /// 幂等写入信封并维护设备连续序号、服务器游标和墓碑删除回执。
    fn push_envelope(
        &mut self,
        envelope: EncryptedSyncEnvelope,
    ) -> Result<PushChangeResponse, RelayError> {
        let device_key = device_map_key(&envelope.workspace_id, &envelope.device_id);
        let device = self
            .devices
            .get(&device_key)
            .filter(|device| device.revoked_at.is_none())
            .ok_or(RelayError::Forbidden)?;
        envelope.verify_signature(&device.public_key)?;
        if let Some(existing) = self
            .changes
            .values()
            .find(|change| change.envelope.operation_id == envelope.operation_id)
        {
            if existing.envelope == envelope {
                return Ok(PushChangeResponse {
                    cursor: existing.cursor,
                    duplicate: true,
                    removed_ciphertexts: 0,
                    deletion_receipt: existing.deletion_receipt.clone(),
                });
            }
            return Err(RelayError::Conflict);
        }
        if envelope.device_sequence != device.last_sequence.saturating_add(1) {
            return Err(RelayError::Conflict);
        }

        let removed_operation_ids = if envelope.kind == OperationKind::Tombstone {
            let matching = self
                .changes
                .iter()
                .filter(|(_, change)| {
                    change.envelope.workspace_id == envelope.workspace_id
                        && change.envelope.entity_key == envelope.entity_key
                })
                .map(|(cursor, change)| (*cursor, change.envelope.operation_id))
                .collect::<Vec<_>>();
            for (cursor, _) in &matching {
                self.changes.remove(cursor);
            }
            matching
                .into_iter()
                .map(|(_, operation_id)| operation_id)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        self.cursor = self
            .cursor
            .checked_add(1)
            .ok_or_else(|| RelayError::InvalidRequest("中继游标已溢出".into()))?;
        let cursor = self.cursor;
        let deletion_receipt = if envelope.kind == OperationKind::Tombstone {
            Some(deletion_receipt_hash(
                &envelope,
                cursor,
                &removed_operation_ids,
            ))
        } else {
            None
        };
        self.changes.insert(
            cursor,
            StoredEnvelope {
                cursor,
                envelope: envelope.clone(),
                deletion_receipt: deletion_receipt.clone(),
            },
        );
        let device = self
            .devices
            .get_mut(&device_key)
            .ok_or(RelayError::Forbidden)?;
        device.last_sequence = envelope.device_sequence;
        device.last_seen_at = unix_millis();
        Ok(PushChangeResponse {
            cursor,
            duplicate: false,
            removed_ciphertexts: removed_operation_ids.len(),
            deletion_receipt,
        })
    }

    /// 删除全部有效设备都已确认的墓碑，使中继不再保留该实体任何密文。
    fn compact_acknowledged_tombstones(&mut self, workspace_id: &str) -> usize {
        let minimum_ack = self
            .devices
            .values()
            .filter(|device| device.workspace_id == workspace_id && device.revoked_at.is_none())
            .map(|device| device.acknowledged_cursor)
            .min()
            .unwrap_or(0);
        let tombstones = self
            .changes
            .iter()
            .filter(|(cursor, change)| {
                **cursor <= minimum_ack
                    && change.envelope.workspace_id == workspace_id
                    && change.envelope.kind == OperationKind::Tombstone
            })
            .map(|(cursor, _)| *cursor)
            .collect::<Vec<_>>();
        for cursor in &tombstones {
            self.changes.remove(cursor);
        }
        tombstones.len()
    }

    /// 清除过期或已消费一小时以上的配对会话。
    fn remove_expired_pairings(&mut self, now: i64) {
        self.pairings.retain(|_, pairing| {
            pairing.expires_at >= now
                && pairing
                    .consumed_at
                    .is_none_or(|consumed_at| now - consumed_at < 60 * 60 * 1_000)
        });
    }
}

/// 返回 bootstrap 自签名覆盖的稳定消息。
#[must_use]
pub fn bootstrap_message(request: &BootstrapDeviceRequest) -> String {
    format!(
        "device:bootstrap:{}:{}:{}:{}:{}",
        request.workspace_id,
        request.device_id,
        request.name,
        request.public_key,
        request.recovery_public_key
    )
}

/// 返回首台设备登记时恢复签名覆盖的稳定消息。
#[must_use]
pub fn recovery_registration_message(request: &BootstrapDeviceRequest) -> String {
    format!(
        "workspace:recovery:{}:{}",
        request.workspace_id, request.recovery_public_key
    )
}

/// 返回恢复短语登记新设备时两种身份共同签名的稳定消息。
#[must_use]
pub fn recover_device_message(request: &RecoverDeviceRequest) -> String {
    format!(
        "device:recover:{}:{}:{}:{}",
        request.workspace_id, request.device_id, request.name, request.public_key
    )
}

/// 返回新设备配对申请自签名覆盖的稳定消息。
#[must_use]
pub fn pairing_request_message(session_id: Uuid, request: &PairingDeviceRequest) -> String {
    format!(
        "pairing:request:{session_id}:{}:{}:{}",
        request.device_id, request.name, request.public_key
    )
}

/// 返回通用设备证明需要签名的稳定消息。
#[must_use]
pub fn proof_message(action: &str, timestamp: i64, nonce: &str) -> String {
    format!("{action}:{timestamp}:{nonce}")
}

/// 返回确认配对动作字符串，调用方再附加时间、nonce 并签名。
#[must_use]
pub fn approve_pairing_action(
    session_id: Uuid,
    target_device_id: &str,
    sealed_key: &SealedPairingKey,
) -> String {
    format!(
        "pairing:approve:{session_id}:{target_device_id}:{}",
        blake3::hash(sealed_key.ciphertext.as_bytes()).to_hex()
    )
}

/// 使用工作区和设备标识构造无歧义内存映射键。
fn device_map_key(workspace_id: &str, device_id: &str) -> String {
    format!("{workspace_id}\0{device_id}")
}

/// 校验工作区标识是 BLAKE3 派生的 32 字符小写十六进制值。
fn validate_workspace_id(workspace_id: &str) -> Result<(), RelayError> {
    if workspace_id.len() != 32
        || !workspace_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(RelayError::InvalidRequest("工作区标识格式无效".into()));
    }
    Ok(())
}

/// 校验设备展示名称、稳定标识和 Ed25519 公钥编码。
fn validate_device_fields(device_id: &str, name: &str, public_key: &str) -> Result<(), RelayError> {
    if device_id.trim().is_empty()
        || device_id.len() > 128
        || device_id.chars().any(char::is_control)
        || name.trim().is_empty()
        || name.chars().count() > 80
        || name.chars().any(char::is_control)
        || public_key.len() < 40
        || public_key.len() > 64
    {
        return Err(RelayError::InvalidRequest("设备登记字段格式无效".into()));
    }
    Ok(())
}

/// 计算包含墓碑、游标和已删除操作集合的稳定回执哈希。
fn deletion_receipt_hash(
    envelope: &EncryptedSyncEnvelope,
    cursor: u64,
    removed_operation_ids: &[Uuid],
) -> String {
    let mut ids = removed_operation_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>();
    ids.sort();
    blake3::hash(
        format!(
            "nexus-delete-v1:{}:{}:{}:{}:{}",
            envelope.workspace_id,
            envelope.entity_key,
            envelope.operation_id,
            cursor,
            ids.join(",")
        )
        .as_bytes(),
    )
    .to_hex()
    .to_string()
}

/// 返回密文增量单页默认数量。
const fn default_pull_limit() -> usize {
    200
}

/// 返回中继时间戳使用的 Unix 毫秒值。
fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

/// 使用设备身份创建当前动作的签名证明，供客户端和契约测试复用。
pub fn create_device_proof(identity: &DeviceIdentity, action: &str, nonce: &str) -> DeviceProof {
    let timestamp = unix_millis();
    DeviceProof {
        device_id: identity.device_id().into(),
        timestamp,
        nonce: nonce.into(),
        signature: identity.sign(proof_message(action, timestamp, nonce).as_bytes()),
    }
}
