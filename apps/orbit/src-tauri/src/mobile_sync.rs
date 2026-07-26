//! 本文件实现 Orbit Android 的 E2E 身份、设备配对、加密本地副本与零知识增量同步客户端。

use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use nexus_platform_mobile::{SecureStorage, SecureStorageExt};
use nexus_sync::{
    DeviceIdentity, EncryptedSyncEnvelope, OperationKind, PairingOffer, PlainSyncOperation,
    SealedPairingKey, SyncKey, VersionVector, VersionedRecord,
};
use qrcode::{QrCode, render::svg};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{Collection, MemoryHit, MemorySummary, mobile_cache::EncryptedCache};

const ROOT_KEY_STORAGE_KEY: &str = "orbit.e2e-root-key";
const DEVICE_ID_STORAGE_KEY: &str = "orbit.e2e-device-id";
const DEVICE_IDENTITY_STORAGE_KEY: &str = "orbit.e2e-device-identity";
const OUTGOING_PAIRING_STORAGE_KEY: &str = "orbit.e2e-outgoing-pairing";
const PENDING_JOIN_STORAGE_KEY: &str = "orbit.e2e-pending-join";
const REPLICA_CACHE_KEY: &str = "E2E:CONTENT-REPLICA";
const REPLICA_VERSION: u8 = 1;
const PULL_LIMIT: usize = 200;

// 前台命令与 WorkManager JNI 入口共享同一副本，完整同步和本地排队必须按因果顺序串行。
static CONTENT_SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// 表示 Android 当前 E2E 工作区和设备身份状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eStatus {
    /// 是否已经在 Keystore 中保存同步根密钥和设备身份。
    pub configured: bool,
    /// 不可逆工作区标识。
    pub workspace_id: Option<String>,
    /// 当前设备标识。
    pub device_id: Option<String>,
    /// 是否存在等待另一台设备批准的加入请求。
    pub pending_join: bool,
    /// 是否存在当前设备创建的配对邀请。
    pub outgoing_pairing: bool,
}

/// 表示 Android 设置页展示的二维码配对邀请。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingOfferResponse {
    /// 配对会话标识。
    pub session_id: String,
    /// 可由另一台设备扫描或粘贴的完整 URI。
    pub pairing_uri: String,
    /// 可直接渲染的 SVG data URL。
    pub qr_data_url: String,
    /// 两台设备人工核对的六位确认码。
    pub verification_code: String,
    /// 中继返回的会话过期时间。
    pub expires_at: i64,
}

/// 表示已连接设备看到的配对申请状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusResponse {
    /// 配对会话标识。
    pub session_id: String,
    /// 会话过期时间。
    pub expires_at: i64,
    /// 等待批准的新设备；尚未申请时为空。
    pub pending_device: Option<SyncDevice>,
    /// 是否已经上传根密钥配对包。
    pub approved: bool,
    /// 新设备是否已经领取配对包。
    pub consumed: bool,
}

/// 表示加入配对请求创建结果。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingJoinResponse {
    /// 新设备生成的稳定标识。
    pub device_id: String,
    /// 与邀请设备核对的六位确认码。
    pub verification_code: String,
    /// 当前加入流程是否仍等待批准。
    pub waiting_for_approval: bool,
}

/// 表示中继登记的设备公开信息。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDevice {
    /// 不可逆工作区标识。
    pub workspace_id: String,
    /// 稳定设备标识。
    pub device_id: String,
    /// 用户可识别设备名称。
    pub name: String,
    /// Ed25519 公钥。
    pub public_key: String,
    /// Unix 毫秒创建时间。
    pub created_at: i64,
    /// Unix 毫秒最近活动时间。
    pub last_seen_at: i64,
    /// Unix 毫秒撤销时间。
    pub revoked_at: Option<i64>,
    /// 当前设备已上传序号。
    pub last_sequence: u64,
    /// 当前设备已确认中继游标。
    pub acknowledged_cursor: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapDeviceRequest {
    workspace_id: String,
    device_id: String,
    name: String,
    public_key: String,
    recovery_public_key: String,
    signature: String,
    recovery_signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoverDeviceRequest {
    workspace_id: String,
    device_id: String,
    name: String,
    public_key: String,
    device_signature: String,
    recovery_signature: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceProof {
    device_id: String,
    timestamp: i64,
    nonce: String,
    signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreatePairingRequest {
    session_id: Uuid,
    workspace_id: String,
    proof: DeviceProof,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedPairingResponse {
    expires_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PairingDeviceRequest {
    device_id: String,
    name: String,
    public_key: String,
    signature: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelayPairingStatus {
    session_id: Uuid,
    expires_at: i64,
    pending_device: Option<PendingDevice>,
    approved: bool,
    consumed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PendingDevice {
    device_id: String,
    name: String,
    public_key: String,
    requested_at: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApprovePairingRequest {
    sealed_key: SealedPairingKey,
    proof: DeviceProof,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FetchPairingPackageRequest {
    device_id: String,
    signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RevokeDeviceRequest {
    workspace_id: String,
    proof: DeviceProof,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PendingJoin {
    pairing_uri: String,
    device_id: String,
    device_name: String,
    identity_pkcs8: String,
}

/// 表示同步密文中可以承载的 Android 内容实体。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "entityType", content = "data", rename_all = "snake_case")]
enum SyncEntity {
    /// 一条完整记忆快照。
    Memory(MemorySummary),
    /// 一个集合及其层级、排序信息。
    Collection(Collection),
    /// 记忆与集合之间的幂等成员关系。
    Membership(CollectionMembership),
}

/// 表示一条集合成员关系；实体标识保证同一关系只保留一个收敛记录。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionMembership {
    collection_id: String,
    memory_id: String,
}

/// 表示 Android 私有目录中的加密 E2E 内容副本与待上传信封。
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalReplica {
    version: u8,
    workspace_id: String,
    cursor: u64,
    local_sequence: u64,
    last_sync_at: Option<i64>,
    records: BTreeMap<String, VersionedRecord<SyncEntity>>,
    pending: Vec<EncryptedSyncEnvelope>,
}

impl LocalReplica {
    /// 为当前根密钥创建一个空副本；工作区变化时绝不复用旧副本内容。
    fn empty(key: &SyncKey) -> Self {
        Self {
            version: REPLICA_VERSION,
            workspace_id: key.workspace_id(),
            cursor: 0,
            local_sequence: 0,
            last_sync_at: None,
            records: BTreeMap::new(),
            pending: Vec::new(),
        }
    }
}

/// 表示一次前台增量同步后可供设置页展示的状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSyncStatus {
    /// 已完整应用并确认的中继游标。
    pub cursor: u64,
    /// 因离线或中继失败仍等待上传的本地操作数量。
    pub pending_changes: usize,
    /// 当前副本保留的并发失败版本数量。
    pub conflict_count: usize,
    /// 最近一次完整上传、拉取并确认成功的 Unix 毫秒时间。
    pub last_sync_at: Option<i64>,
}

/// 表示中继返回的一条带服务器游标的加密信封。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEnvelope {
    cursor: u64,
    envelope: EncryptedSyncEnvelope,
}

/// 表示中继增量拉取响应。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullChangesResponse {
    changes: Vec<StoredEnvelope>,
    next_cursor: u64,
    has_more: bool,
}

/// 表示中继接受或幂等复用上传操作后的响应。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushChangeResponse {
    cursor: u64,
    duplicate: bool,
}

/// 表示设备确认已经完整应用的服务器游标。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcknowledgeRequest {
    workspace_id: String,
    cursor: u64,
    proof: DeviceProof,
}

/// 返回 Android Keystore 中的当前 E2E 配置摘要。
pub fn status(app: &AppHandle) -> Result<E2eStatus, String> {
    let key = load_sync_key(app)?;
    let identity = load_identity(app)?;
    let configured = key.is_some() && identity.is_some();
    Ok(E2eStatus {
        configured,
        workspace_id: key.as_ref().map(SyncKey::workspace_id),
        device_id: identity.map(|identity| identity.device_id().to_owned()),
        pending_join: load_secret(app, PENDING_JOIN_STORAGE_KEY)?.is_some(),
        outgoing_pairing: load_secret(app, OUTGOING_PAIRING_STORAGE_KEY)?.is_some(),
    })
}

/// 创建首个 E2E 工作区和设备身份，并在中继登记恢复签名公钥。
pub async fn initialize(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    device_name: &str,
) -> Result<E2eStatus, String> {
    validate_device_name(device_name)?;
    validate_relay(client, endpoint, token).await?;
    if load_sync_key(app)?.is_some() {
        return Err("当前设备已经配置端到端同步".into());
    }
    let key = SyncKey::generate();
    let identity = DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
    let recovery_public_key = key
        .recovery_public_key()
        .map_err(|error| error.to_string())?;
    let mut request = BootstrapDeviceRequest {
        workspace_id: key.workspace_id(),
        device_id: identity.device_id().into(),
        name: device_name.trim().into(),
        public_key: identity.public_key().into(),
        recovery_public_key,
        signature: String::new(),
        recovery_signature: String::new(),
    };
    request.signature = identity.sign(bootstrap_message(&request).as_bytes());
    request.recovery_signature = key
        .sign_recovery_claim(recovery_registration_message(&request).as_bytes())
        .map_err(|error| error.to_string())?;
    send_json::<serde_json::Value>(
        client
            .post(relay_url(endpoint, "/v1/sync/devices/bootstrap")?)
            .bearer_auth(token)
            .json(&request),
    )
    .await?;
    persist_current_identity(app, &key, &identity)?;
    status(app)
}

/// 使用 24 词 BIP39 恢复短语登记新设备，短语与根密钥不上传中继。
pub async fn restore(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    phrase: &str,
    device_name: &str,
) -> Result<E2eStatus, String> {
    validate_device_name(device_name)?;
    validate_relay(client, endpoint, token).await?;
    let key = SyncKey::from_recovery_phrase(phrase).map_err(|error| error.to_string())?;
    let identity = DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
    let mut request = RecoverDeviceRequest {
        workspace_id: key.workspace_id(),
        device_id: identity.device_id().into(),
        name: device_name.trim().into(),
        public_key: identity.public_key().into(),
        device_signature: String::new(),
        recovery_signature: String::new(),
    };
    let message = recover_device_message(&request);
    request.device_signature = identity.sign(message.as_bytes());
    request.recovery_signature = key
        .sign_recovery_claim(message.as_bytes())
        .map_err(|error| error.to_string())?;
    send_json::<serde_json::Value>(
        client
            .post(relay_url(endpoint, "/v1/sync/devices/recover")?)
            .bearer_auth(token)
            .json(&request),
    )
    .await?;
    persist_current_identity(app, &key, &identity)?;
    status(app)
}

/// 返回当前根密钥对应的 24 词 BIP39 恢复短语。
pub fn recovery_phrase(app: &AppHandle) -> Result<String, String> {
    load_sync_key(app)?
        .ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?
        .recovery_phrase()
        .map_err(|error| error.to_string())
}

/// 创建中继一次性会话并生成包含高熵秘密的二维码。
pub async fn create_pairing_offer(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<PairingOfferResponse, String> {
    let (key, identity) = current_identity(app)?;
    let offer = PairingOffer::create(&key);
    let action = format!("pairing:create:{}:{}", key.workspace_id(), offer.session_id);
    let request = CreatePairingRequest {
        session_id: offer.session_id,
        workspace_id: key.workspace_id(),
        proof: create_proof(&identity, &action),
    };
    let created: CreatedPairingResponse = send_json(
        client
            .post(relay_url(endpoint, "/v1/sync/pairings")?)
            .bearer_auth(token)
            .json(&request),
    )
    .await?;
    let pairing_uri = offer.to_uri();
    store_secret(app, OUTGOING_PAIRING_STORAGE_KEY, pairing_uri.as_bytes())?;
    Ok(PairingOfferResponse {
        session_id: offer.session_id.to_string(),
        qr_data_url: qr_data_url(&pairing_uri)?,
        pairing_uri,
        verification_code: offer.verification_code(),
        expires_at: created.expires_at,
    })
}

/// 查询当前设备创建的配对邀请是否已有新设备等待批准。
pub async fn pairing_status(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<PairingStatusResponse, String> {
    let (key, identity) = current_identity(app)?;
    let offer = load_outgoing_offer(app)?;
    let action = format!("pairing:status:{}:{}", key.workspace_id(), offer.session_id);
    let proof = create_proof(&identity, &action);
    let response: RelayPairingStatus = send_json(
        client
            .get(relay_url(
                endpoint,
                &format!("/v1/sync/pairings/{}", offer.session_id),
            )?)
            .bearer_auth(token)
            .query(&[
                ("workspaceId", key.workspace_id()),
                ("deviceId", proof.device_id),
                ("timestamp", proof.timestamp.to_string()),
                ("nonce", proof.nonce),
                ("signature", proof.signature),
            ]),
    )
    .await?;
    Ok(PairingStatusResponse {
        session_id: response.session_id.to_string(),
        expires_at: response.expires_at,
        pending_device: response.pending_device.map(|device| SyncDevice {
            workspace_id: key.workspace_id(),
            device_id: device.device_id,
            name: device.name,
            public_key: device.public_key,
            created_at: device.requested_at,
            last_seen_at: device.requested_at,
            revoked_at: None,
            last_sequence: 0,
            acknowledged_cursor: 0,
        }),
        approved: response.approved,
        consumed: response.consumed,
    })
}

/// 批准当前配对会话中的新设备，并上传二维码秘密封装的根密钥包。
pub async fn approve_pairing(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<SyncDevice, String> {
    let (key, identity) = current_identity(app)?;
    let offer = load_outgoing_offer(app)?;
    let status = pairing_status(app, client, endpoint, token).await?;
    let pending = status
        .pending_device
        .ok_or_else(|| "当前配对会话尚无设备等待批准".to_owned())?;
    let sealed_key = offer
        .secret
        .seal_sync_key(&key, &offer, &pending.device_id)
        .map_err(|error| error.to_string())?;
    let action = format!(
        "pairing:approve:{}:{}:{}",
        offer.session_id,
        pending.device_id,
        blake3::hash(sealed_key.ciphertext.as_bytes()).to_hex()
    );
    let device = send_json(
        client
            .post(relay_url(
                endpoint,
                &format!("/v1/sync/pairings/{}/approve", offer.session_id),
            )?)
            .bearer_auth(token)
            .json(&ApprovePairingRequest {
                sealed_key,
                proof: create_proof(&identity, &action),
            }),
    )
    .await?;
    delete_secret(app, OUTGOING_PAIRING_STORAGE_KEY)?;
    Ok(device)
}

/// 使用扫描或粘贴的二维码 URI 创建新设备加入请求。
pub async fn request_pairing(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    pairing_uri: &str,
    device_name: &str,
) -> Result<PairingJoinResponse, String> {
    validate_device_name(device_name)?;
    let offer = PairingOffer::from_uri(pairing_uri).map_err(|error| error.to_string())?;
    let identity = DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
    let mut request = PairingDeviceRequest {
        device_id: identity.device_id().into(),
        name: device_name.trim().into(),
        public_key: identity.public_key().into(),
        signature: String::new(),
    };
    request.signature = identity.sign(
        format!(
            "pairing:request:{}:{}:{}:{}",
            offer.session_id, request.device_id, request.name, request.public_key
        )
        .as_bytes(),
    );
    send_empty(
        client
            .post(relay_url(
                endpoint,
                &format!("/v1/sync/pairings/{}/request", offer.session_id),
            )?)
            .bearer_auth(token)
            .json(&request),
    )
    .await?;
    store_json_secret(
        app,
        PENDING_JOIN_STORAGE_KEY,
        &PendingJoin {
            pairing_uri: pairing_uri.trim().into(),
            device_id: identity.device_id().into(),
            device_name: device_name.trim().into(),
            identity_pkcs8: STANDARD.encode(identity.pkcs8_bytes()),
        },
    )?;
    Ok(PairingJoinResponse {
        device_id: identity.device_id().into(),
        verification_code: offer.verification_code(),
        waiting_for_approval: true,
    })
}

/// 新设备签名领取已批准配对包，解封根密钥并写入 Android Keystore。
pub async fn complete_pairing(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<E2eStatus, String> {
    let pending: PendingJoin = load_json_secret(app, PENDING_JOIN_STORAGE_KEY)?
        .ok_or_else(|| "当前设备没有等待完成的配对请求".to_owned())?;
    let offer = PairingOffer::from_uri(&pending.pairing_uri).map_err(|error| error.to_string())?;
    let identity = DeviceIdentity::from_pkcs8(
        pending.device_id.clone(),
        STANDARD
            .decode(&pending.identity_pkcs8)
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let request = FetchPairingPackageRequest {
        device_id: pending.device_id,
        signature: identity.sign(
            format!(
                "pairing:fetch:{}:{}",
                offer.session_id,
                identity.device_id()
            )
            .as_bytes(),
        ),
    };
    let sealed: SealedPairingKey = send_json(
        client
            .post(relay_url(
                endpoint,
                &format!("/v1/sync/pairings/{}/package", offer.session_id),
            )?)
            .bearer_auth(token)
            .json(&request),
    )
    .await?;
    let key = offer
        .secret
        .open_sync_key(&sealed, identity.device_id())
        .map_err(|error| error.to_string())?;
    persist_current_identity(app, &key, &identity)?;
    delete_secret(app, PENDING_JOIN_STORAGE_KEY)?;
    status(app)
}

/// 列出工作区全部有效和已撤销设备。
pub async fn list_devices(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<Vec<SyncDevice>, String> {
    let (key, identity) = current_identity(app)?;
    list_devices_with_identity(&key, &identity, client, endpoint, token).await
}

/// 使用已解锁的同步材料读取设备目录，供前台和 WorkManager 复用同一签名协议。
async fn list_devices_with_identity(
    key: &SyncKey,
    identity: &DeviceIdentity,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<Vec<SyncDevice>, String> {
    let action = format!("devices:list:{}", key.workspace_id());
    let proof = create_proof(identity, &action);
    send_json(
        client
            .get(relay_url(endpoint, "/v1/sync/devices")?)
            .bearer_auth(token)
            .query(&[
                ("workspaceId", key.workspace_id()),
                ("deviceId", proof.device_id),
                ("timestamp", proof.timestamp.to_string()),
                ("nonce", proof.nonce),
                ("signature", proof.signature),
            ]),
    )
    .await
}

/// 撤销工作区内指定设备。
pub async fn revoke_device(
    app: &AppHandle,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    target_device_id: &str,
) -> Result<SyncDevice, String> {
    let (key, identity) = current_identity(app)?;
    if target_device_id == identity.device_id() {
        return Err("不能在当前设备上撤销自己；请先在另一台有效设备上操作".into());
    }
    let action = format!("device:revoke:{}:{target_device_id}", key.workspace_id());
    send_json(
        client
            .delete(relay_url(
                endpoint,
                &format!("/v1/sync/devices/{target_device_id}"),
            )?)
            .bearer_auth(token)
            .json(&RevokeDeviceRequest {
                workspace_id: key.workspace_id(),
                proof: create_proof(&identity, &action),
            }),
    )
    .await
}

/// 上传本机待处理操作、拉取远端密文、确定性合并并确认最新游标。
pub async fn sync_content(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<ContentSyncStatus, String> {
    let _sync_guard = CONTENT_SYNC_LOCK.lock().await;
    let (key, identity) = current_identity(app)?;
    sync_content_with_identity(cache, client, endpoint, token, &key, &identity).await
}

/// 使用 WorkManager 从 Keystore 临时解锁的材料执行同步，不依赖 Activity、WebView 或 Tauri IPC。
pub async fn sync_content_with_material(
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    key: &SyncKey,
    identity: &DeviceIdentity,
) -> Result<ContentSyncStatus, String> {
    let _sync_guard = CONTENT_SYNC_LOCK.lock().await;
    sync_content_with_identity(cache, client, endpoint, token, key, identity).await
}

/// 在已持有内容同步锁时完成上传、拉取、合并与游标确认。
async fn sync_content_with_identity(
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    key: &SyncKey,
    identity: &DeviceIdentity,
) -> Result<ContentSyncStatus, String> {
    let devices = list_devices_with_identity(key, identity, client, endpoint, token).await?;
    let public_keys = devices
        .iter()
        .map(|device| (device.device_id.clone(), device.public_key.clone()))
        .collect::<HashMap<_, _>>();
    let mut replica = load_replica(cache, key)?;
    if let Some(current) = devices
        .iter()
        .find(|device| device.device_id == identity.device_id())
    {
        replica.local_sequence = replica.local_sequence.max(current.last_sequence);
    }

    // 每成功上传一条就立即落盘移除，进程中断后可从第一条未确认信封继续幂等重试。
    while let Some(envelope) = replica.pending.first().cloned() {
        let response: PushChangeResponse = send_json(
            client
                .post(relay_url(endpoint, "/v1/sync/changes")?)
                .bearer_auth(token)
                .json(&envelope),
        )
        .await?;
        if response.cursor == 0 {
            return Err("E2E 中继返回了无效上传游标".into());
        }
        let _was_duplicate = response.duplicate;
        replica.pending.remove(0);
        persist_replica(cache, &replica)?;
    }

    loop {
        let action = format!(
            "changes:pull:{}:{}:{}",
            key.workspace_id(),
            replica.cursor,
            PULL_LIMIT
        );
        let proof = create_proof(identity, &action);
        let response: PullChangesResponse = send_json(
            client
                .get(relay_url(endpoint, "/v1/sync/changes")?)
                .bearer_auth(token)
                .query(&[
                    ("workspaceId", key.workspace_id()),
                    ("after", replica.cursor.to_string()),
                    ("limit", PULL_LIMIT.to_string()),
                    ("deviceId", proof.device_id),
                    ("timestamp", proof.timestamp.to_string()),
                    ("nonce", proof.nonce),
                    ("signature", proof.signature),
                ]),
        )
        .await?;
        for stored in response.changes {
            let public_key = public_keys
                .get(&stored.envelope.device_id)
                .ok_or_else(|| "同步信封来源设备未登记，已拒绝解密".to_owned())?;
            let operation = key
                .decrypt_operation(&stored.envelope, public_key)
                .map_err(|error| error.to_string())?;
            apply_operation(&mut replica, operation)?;
            replica.cursor = replica.cursor.max(stored.cursor);
        }
        replica.cursor = replica.cursor.max(response.next_cursor);
        persist_replica(cache, &replica)?;
        if !response.has_more {
            break;
        }
    }

    if replica.cursor > 0 {
        let action = format!("changes:ack:{}:{}", key.workspace_id(), replica.cursor);
        let _: serde_json::Value = send_json(
            client
                .post(relay_url(endpoint, "/v1/sync/ack")?)
                .bearer_auth(token)
                .json(&AcknowledgeRequest {
                    workspace_id: key.workspace_id(),
                    cursor: replica.cursor,
                    proof: create_proof(identity, &action),
                }),
        )
        .await?;
    }
    replica.last_sync_at = Some(unix_millis());
    persist_replica(cache, &replica)?;
    Ok(replica_status(&replica))
}

/// 返回本机加密副本的同步进度，不发起网络请求。
pub fn content_status(
    app: &AppHandle,
    cache: &EncryptedCache,
) -> Result<ContentSyncStatus, String> {
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    Ok(replica_status(&load_replica(cache, &key)?))
}

/// 从加密副本列出可见记忆；联网时先执行一次增量同步，离线时保留本地结果。
pub async fn list_memories(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    source: Option<&str>,
) -> Result<Vec<MemorySummary>, String> {
    refresh_replica_best_effort(app, cache, client, endpoint, token).await?;
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    let replica = load_replica(cache, &key)?;
    let mut memories = replica
        .records
        .values()
        .filter_map(memory_from_record)
        .filter(|memory| source.is_none_or(|source| memory.source == source))
        .collect::<Vec<_>>();
    memories.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    Ok(memories)
}

/// 从加密副本读取一条完整记忆。
pub async fn get_memory(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    id: &str,
) -> Result<MemorySummary, String> {
    refresh_replica_best_effort(app, cache, client, endpoint, token).await?;
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    load_replica(cache, &key)?
        .records
        .get(&memory_entity_id(id))
        .and_then(memory_from_record)
        .ok_or_else(|| "记忆不存在或已经删除".to_owned())
}

/// 在加密副本创建记忆并将签名密文加入可靠上传队列。
pub async fn create_memory(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    content: String,
) -> Result<MemorySummary, String> {
    let content = content.trim();
    if content.is_empty() {
        return Err("记忆正文不能为空".into());
    }
    let now = unix_millis();
    let memory = MemorySummary {
        id: Uuid::now_v7().to_string(),
        source: "orbit".into(),
        kind: "note".into(),
        title: content.lines().next().map(str::to_owned),
        content: content.into(),
        content_format: "markdown".into(),
        tags: Vec::new(),
        pinned: false,
        archived: false,
        created_at: now,
        updated_at: now,
        captured_at: Some(now),
        links: Vec::new(),
        conflict_count: 0,
    };
    queue_entity_change(
        app,
        cache,
        client,
        endpoint,
        token,
        memory_entity_id(&memory.id),
        Some(SyncEntity::Memory(memory.clone())),
    )
    .await?;
    Ok(memory)
}

/// 更新加密副本中的记忆，并在版本向量中记录当前设备的新全局序号。
pub async fn update_memory(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    id: &str,
    edit: (Option<String>, String),
) -> Result<MemorySummary, String> {
    let (title, content) = edit;
    let mut memory = get_memory(app, cache, client, endpoint, token, id).await?;
    if content.trim().is_empty() {
        return Err("记忆正文不能为空".into());
    }
    memory.title = title.filter(|value| !value.trim().is_empty());
    memory.content = content;
    memory.updated_at = unix_millis();
    memory.conflict_count = 0;
    queue_entity_change(
        app,
        cache,
        client,
        endpoint,
        token,
        memory_entity_id(id),
        Some(SyncEntity::Memory(memory.clone())),
    )
    .await?;
    get_memory(app, cache, client, endpoint, token, id)
        .await
        .or(Ok(memory))
}

/// 写入记忆墓碑并同步删除所有已知集合成员关系，防止旧内容重新出现。
pub async fn delete_memory(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    id: &str,
) -> Result<(), String> {
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    refresh_replica_best_effort(app, cache, client, endpoint, token).await?;
    let replica = load_replica(cache, &key)?;
    if !replica.records.contains_key(&memory_entity_id(id)) {
        return Err("记忆不存在或已经删除".into());
    }
    let memberships = replica
        .records
        .iter()
        .filter_map(|(entity_id, record)| match record.value.as_ref() {
            Some(SyncEntity::Membership(membership)) if membership.memory_id == id => {
                Some(entity_id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    queue_entity_change(
        app,
        cache,
        client,
        endpoint,
        token,
        memory_entity_id(id),
        None,
    )
    .await?;
    for entity_id in memberships {
        queue_entity_change(app, cache, client, endpoint, token, entity_id, None).await?;
    }
    Ok(())
}

/// 在本地解密内容上执行关键词检索；零知识中继不会接触查询或明文索引。
pub async fn search_memories(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    query: &str,
) -> Result<Vec<MemoryHit>, String> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let memories = list_memories(app, cache, client, endpoint, token, None).await?;
    let mut hits = memories
        .into_iter()
        .filter_map(|memory| {
            let searchable = format!(
                "{}\n{}\n{}",
                memory.title.as_deref().unwrap_or_default(),
                memory.content,
                memory.tags.join(" ")
            )
            .to_lowercase();
            let matched = terms
                .iter()
                .filter(|term| searchable.contains(term.as_str()))
                .count();
            (matched > 0).then(|| MemoryHit {
                memory_id: memory.id.clone(),
                block_id: format!("e2e:{}", memory.id),
                score: matched as f32 / terms.len() as f32,
                snippet: memory.content.chars().take(180).collect(),
            })
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.memory_id.cmp(&right.memory_id))
    });
    Ok(hits)
}

/// 从加密副本列出全部可见集合。
pub async fn list_collections(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<Vec<Collection>, String> {
    refresh_replica_best_effort(app, cache, client, endpoint, token).await?;
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    let mut collections = load_replica(cache, &key)?
        .records
        .values()
        .filter_map(|record| match record.value.as_ref() {
            Some(SyncEntity::Collection(collection)) => Some(collection.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    collections.sort_by(|left, right| {
        left.sort
            .cmp(&right.sort)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(collections)
}

/// 创建一个可跨设备收敛的集合。
pub async fn create_collection(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    name: String,
) -> Result<Collection, String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err("集合名称长度必须为 1 到 120 个字符".into());
    }
    let collection = Collection {
        id: Uuid::now_v7().to_string(),
        name: name.into(),
        icon: None,
        parent_id: None,
        sort: unix_millis(),
    };
    queue_entity_change(
        app,
        cache,
        client,
        endpoint,
        token,
        collection_entity_id(&collection.id),
        Some(SyncEntity::Collection(collection.clone())),
    )
    .await?;
    Ok(collection)
}

/// 将一条记忆幂等加入集合，并以独立实体同步成员关系。
pub async fn add_memory_to_collection(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    collection_id: &str,
    memory_id: &str,
) -> Result<(), String> {
    let collections = list_collections(app, cache, client, endpoint, token).await?;
    if !collections
        .iter()
        .any(|collection| collection.id == collection_id)
    {
        return Err("目标集合不存在或已经删除".into());
    }
    let _ = get_memory(app, cache, client, endpoint, token, memory_id).await?;
    queue_entity_change(
        app,
        cache,
        client,
        endpoint,
        token,
        membership_entity_id(collection_id, memory_id),
        Some(SyncEntity::Membership(CollectionMembership {
            collection_id: collection_id.into(),
            memory_id: memory_id.into(),
        })),
    )
    .await
}

/// 从本地解密副本读取指定集合中的全部可见记忆。
pub async fn list_collection_memories(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    collection_id: &str,
) -> Result<Vec<MemorySummary>, String> {
    refresh_replica_best_effort(app, cache, client, endpoint, token).await?;
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    let replica = load_replica(cache, &key)?;
    let member_ids = replica
        .records
        .values()
        .filter_map(|record| match record.value.as_ref() {
            Some(SyncEntity::Membership(membership))
                if membership.collection_id == collection_id =>
            {
                Some(membership.memory_id.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut memories = member_ids
        .into_iter()
        .filter_map(|memory_id| {
            replica
                .records
                .get(&memory_entity_id(memory_id))
                .and_then(memory_from_record)
        })
        .collect::<Vec<_>>();
    memories.sort_by_key(|memory| Reverse(memory.updated_at));
    Ok(memories)
}

/// 将一个本地实体变更加密、持久化到待上传队列，并尽力立即同步。
async fn queue_entity_change(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
    entity_id: String,
    value: Option<SyncEntity>,
) -> Result<(), String> {
    let _sync_guard = CONTENT_SYNC_LOCK.lock().await;
    let (key, identity) = current_identity(app)?;
    let _ = sync_content_with_identity(cache, client, endpoint, token, &key, &identity).await;
    let mut replica = load_replica(cache, &key)?;
    replica.local_sequence = replica
        .local_sequence
        .checked_add(1)
        .ok_or_else(|| "设备同步序号已溢出".to_owned())?;
    let mut version = replica
        .records
        .get(&entity_id)
        .map_or_else(VersionVector::default, |record| record.version.clone());
    version
        .observe(identity.device_id(), replica.local_sequence)
        .map_err(|error| error.to_string())?;
    let operation = PlainSyncOperation {
        operation_id: Uuid::now_v7(),
        entity_id,
        device_id: identity.device_id().into(),
        device_sequence: replica.local_sequence,
        version,
        kind: if value.is_some() {
            OperationKind::Upsert
        } else {
            OperationKind::Tombstone
        },
        payload: value
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| error.to_string())?,
        created_at: unix_millis(),
    };
    let envelope = key
        .encrypt_operation(&operation, &identity)
        .map_err(|error| error.to_string())?;
    apply_operation(&mut replica, operation)?;
    replica.pending.push(envelope);
    persist_replica(cache, &replica)?;
    let _ = sync_content_with_identity(cache, client, endpoint, token, &key, &identity).await;
    Ok(())
}

/// 将一条已认证明文操作合并进副本，墓碑和并发版本均交给共享收敛规则处理。
fn apply_operation(
    replica: &mut LocalReplica,
    operation: PlainSyncOperation,
) -> Result<(), String> {
    let value = operation
        .payload
        .map(serde_json::from_value::<SyncEntity>)
        .transpose()
        .map_err(|error| format!("同步实体载荷无效：{error}"))?;
    validate_entity_payload(&operation.entity_id, value.as_ref())?;
    let incoming = VersionedRecord {
        value,
        version: operation.version,
        device_id: operation.device_id.clone(),
        modified_at: operation.created_at,
        conflicts: Vec::new(),
    };
    if let Some(existing) = replica.records.remove(&operation.entity_id) {
        replica
            .records
            .insert(operation.entity_id, existing.merge(incoming).record);
    } else {
        replica.records.insert(operation.entity_id, incoming);
    }
    Ok(())
}

/// 校验实体命名空间与解密载荷类型一致，拒绝已签名但结构异常的数据污染本地副本。
fn validate_entity_payload(entity_id: &str, value: Option<&SyncEntity>) -> Result<(), String> {
    let valid = match value {
        Some(SyncEntity::Memory(memory)) => entity_id == memory_entity_id(&memory.id),
        Some(SyncEntity::Collection(collection)) => {
            entity_id == collection_entity_id(&collection.id)
        }
        Some(SyncEntity::Membership(membership)) => {
            entity_id == membership_entity_id(&membership.collection_id, &membership.memory_id)
        }
        None => {
            entity_id.starts_with("memory:")
                || entity_id.starts_with("collection:")
                || entity_id.starts_with("membership:")
        }
    };
    valid
        .then_some(())
        .ok_or_else(|| "同步实体标识与载荷类型不一致".to_owned())
}

/// 尝试联网刷新副本；只要本地身份与加密副本可读，离线不会阻断浏览或写入。
async fn refresh_replica_best_effort(
    app: &AppHandle,
    cache: &EncryptedCache,
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<(), String> {
    let key = load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?;
    let _ = load_identity(app)?.ok_or_else(|| "当前设备签名身份不可用".to_owned())?;
    let _ = load_replica(cache, &key)?;
    let _ = sync_content(app, cache, client, endpoint, token).await;
    Ok(())
}

/// 从加密缓存恢复当前工作区副本；格式损坏必须显式报错，禁止静默丢弃待上传操作。
fn load_replica(cache: &EncryptedCache, key: &SyncKey) -> Result<LocalReplica, String> {
    let Some(encoded) = cache.get(REPLICA_CACHE_KEY)? else {
        return Ok(LocalReplica::empty(key));
    };
    let replica: LocalReplica =
        serde_json::from_str(&encoded).map_err(|error| format!("E2E 本地副本损坏：{error}"))?;
    if replica.version != REPLICA_VERSION {
        return Err("E2E 本地副本版本不受支持".into());
    }
    if replica.workspace_id != key.workspace_id() {
        return Ok(LocalReplica::empty(key));
    }
    Ok(replica)
}

/// 原子更新由 Keystore 托管密钥加密的本地同步副本。
fn persist_replica(cache: &EncryptedCache, replica: &LocalReplica) -> Result<(), String> {
    cache.put(
        REPLICA_CACHE_KEY.into(),
        serde_json::to_string(replica).map_err(|error| error.to_string())?,
    )
}

/// 汇总本地副本的游标、待上传数量与冲突留痕数量。
fn replica_status(replica: &LocalReplica) -> ContentSyncStatus {
    ContentSyncStatus {
        cursor: replica.cursor,
        pending_changes: replica.pending.len(),
        conflict_count: replica
            .records
            .values()
            .map(|record| record.conflicts.len())
            .sum(),
        last_sync_at: replica.last_sync_at,
    }
}

/// 将一条可见记忆记录转换为前端摘要，并附带并发冲突数量。
fn memory_from_record(record: &VersionedRecord<SyncEntity>) -> Option<MemorySummary> {
    let Some(SyncEntity::Memory(memory)) = record.value.as_ref() else {
        return None;
    };
    let mut memory = memory.clone();
    memory.conflict_count = record.conflicts.len();
    Some(memory)
}

/// 构造记忆实体的加密载荷内标识。
fn memory_entity_id(id: &str) -> String {
    format!("memory:{id}")
}

/// 构造集合实体的加密载荷内标识。
fn collection_entity_id(id: &str) -> String {
    format!("collection:{id}")
}

/// 构造集合成员关系的加密载荷内标识。
fn membership_entity_id(collection_id: &str, memory_id: &str) -> String {
    format!("membership:{collection_id}:{memory_id}")
}

/// 清除当前 Android 设备上的 E2E 根密钥、签名身份和未完成配对材料。
pub fn clear_local_identity(app: &AppHandle) -> Result<(), String> {
    let mut errors = Vec::new();
    for key in [
        ROOT_KEY_STORAGE_KEY,
        DEVICE_ID_STORAGE_KEY,
        DEVICE_IDENTITY_STORAGE_KEY,
        OUTGOING_PAIRING_STORAGE_KEY,
        PENDING_JOIN_STORAGE_KEY,
    ] {
        if let Err(error) = delete_secret(app, key) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("；"))
    }
}

/// 验证目标端点声明零知识同步能力。
pub async fn validate_relay(
    client: &reqwest::Client,
    endpoint: &str,
    token: &str,
) -> Result<(), String> {
    let capabilities: serde_json::Value = send_json(
        client
            .get(relay_url(endpoint, "/v1/sync/capabilities")?)
            .bearer_auth(token),
    )
    .await?;
    if capabilities
        .get("zeroKnowledge")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("目标服务未声明零知识同步能力".into());
    }
    Ok(())
}

/// 读取当前根密钥和签名身份。
fn current_identity(app: &AppHandle) -> Result<(SyncKey, DeviceIdentity), String> {
    Ok((
        load_sync_key(app)?.ok_or_else(|| "当前设备尚未配置端到端同步".to_owned())?,
        load_identity(app)?.ok_or_else(|| "当前设备签名身份不可用".to_owned())?,
    ))
}

/// 从 Android Keystore 读取同步根密钥。
fn load_sync_key(app: &AppHandle) -> Result<Option<SyncKey>, String> {
    load_secret(app, ROOT_KEY_STORAGE_KEY)?
        .map(|bytes| {
            bytes
                .try_into()
                .map(SyncKey::from_bytes)
                .map_err(|_| "Android E2E 根密钥长度无效".to_owned())
        })
        .transpose()
}

/// 从 Android Keystore 读取当前设备标识和 PKCS#8 身份。
fn load_identity(app: &AppHandle) -> Result<Option<DeviceIdentity>, String> {
    let Some(device_id) = load_secret(app, DEVICE_ID_STORAGE_KEY)? else {
        return Ok(None);
    };
    let Some(pkcs8) = load_secret(app, DEVICE_IDENTITY_STORAGE_KEY)? else {
        return Ok(None);
    };
    let device_id = String::from_utf8(device_id).map_err(|error| error.to_string())?;
    DeviceIdentity::from_pkcs8(device_id, pkcs8)
        .map(Some)
        .map_err(|error| error.to_string())
}

/// 事务式保存根密钥、设备标识和设备私钥；中途失败时清理已写条目。
fn persist_current_identity(
    app: &AppHandle,
    key: &SyncKey,
    identity: &DeviceIdentity,
) -> Result<(), String> {
    let result = (|| {
        store_secret(app, ROOT_KEY_STORAGE_KEY, &key.to_bytes())?;
        store_secret(app, DEVICE_ID_STORAGE_KEY, identity.device_id().as_bytes())?;
        store_secret(app, DEVICE_IDENTITY_STORAGE_KEY, &identity.pkcs8_bytes())
    })();
    if let Err(error) = result {
        let _ = delete_secret(app, ROOT_KEY_STORAGE_KEY);
        let _ = delete_secret(app, DEVICE_ID_STORAGE_KEY);
        let _ = delete_secret(app, DEVICE_IDENTITY_STORAGE_KEY);
        return Err(error);
    }
    Ok(())
}

/// 读取当前设备创建的配对邀请。
fn load_outgoing_offer(app: &AppHandle) -> Result<PairingOffer, String> {
    let bytes = load_secret(app, OUTGOING_PAIRING_STORAGE_KEY)?
        .ok_or_else(|| "当前设备没有待处理的配对邀请".to_owned())?;
    PairingOffer::from_uri(&String::from_utf8(bytes).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

/// 使用 Android Keystore 插件保存任意敏感字节。
fn store_secret(app: &AppHandle, key: &str, value: &[u8]) -> Result<(), String> {
    app.secure_storage()
        .store(key, value)
        .map_err(|error| error.to_string())
}

/// 使用 Android Keystore 插件读取敏感字节。
fn load_secret(app: &AppHandle, key: &str) -> Result<Option<Vec<u8>>, String> {
    app.secure_storage()
        .load(key)
        .map_err(|error| error.to_string())
}

/// 使用 Android Keystore 插件删除敏感字节。
fn delete_secret(app: &AppHandle, key: &str) -> Result<(), String> {
    app.secure_storage()
        .delete(key)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// 将结构化敏感状态编码后写入 Keystore。
fn store_json_secret<T: Serialize>(app: &AppHandle, key: &str, value: &T) -> Result<(), String> {
    store_secret(
        app,
        key,
        &serde_json::to_vec(value).map_err(|error| error.to_string())?,
    )
}

/// 从 Keystore 读取并解码结构化敏感状态。
fn load_json_secret<T: DeserializeOwned>(app: &AppHandle, key: &str) -> Result<Option<T>, String> {
    load_secret(app, key)?
        .map(|value| serde_json::from_slice(&value).map_err(|error| error.to_string()))
        .transpose()
}

/// 创建包含时间、随机 nonce 和设备签名的中继动作证明。
fn create_proof(identity: &DeviceIdentity, action: &str) -> DeviceProof {
    let timestamp = unix_millis();
    let mut nonce = [0_u8; 18];
    OsRng.fill_bytes(&mut nonce);
    let nonce = URL_SAFE_NO_PAD.encode(nonce);
    DeviceProof {
        device_id: identity.device_id().into(),
        timestamp,
        signature: identity.sign(format!("{action}:{timestamp}:{nonce}").as_bytes()),
        nonce,
    }
}

/// 构造首台设备自签名消息。
fn bootstrap_message(request: &BootstrapDeviceRequest) -> String {
    format!(
        "device:bootstrap:{}:{}:{}:{}:{}",
        request.workspace_id,
        request.device_id,
        request.name,
        request.public_key,
        request.recovery_public_key
    )
}

/// 构造工作区恢复公钥登记消息。
fn recovery_registration_message(request: &BootstrapDeviceRequest) -> String {
    format!(
        "workspace:recovery:{}:{}",
        request.workspace_id, request.recovery_public_key
    )
}

/// 构造恢复短语登记新设备消息。
fn recover_device_message(request: &RecoverDeviceRequest) -> String {
    format!(
        "device:recover:{}:{}:{}:{}",
        request.workspace_id, request.device_id, request.name, request.public_key
    )
}

/// 拼接去除尾斜线后的中继端点与稳定路径。
fn relay_url(endpoint: &str, path: &str) -> Result<String, String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.is_empty() {
        return Err("请先配置 E2E 中继地址".into());
    }
    Ok(format!("{endpoint}{path}"))
}

/// 发送 JSON 请求并解析成功响应，错误时保留中继返回的安全错误信息。
async fn send_json<T: DeserializeOwned>(request: reqwest::RequestBuilder) -> Result<T, String> {
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
            .unwrap_or(body);
        return Err(format!("E2E 中继返回 {status}: {message}"));
    }
    serde_json::from_str(&body).map_err(|error| error.to_string())
}

/// 发送不需要响应正文的中继请求。
async fn send_empty(request: reqwest::RequestBuilder) -> Result<(), String> {
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    if status.is_success() {
        return Ok(());
    }
    let body = response.text().await.unwrap_or_default();
    Err(format!("E2E 中继返回 {status}: {body}"))
}

/// 将配对 URI 生成不依赖外部网络的 SVG 二维码 data URL。
fn qr_data_url(pairing_uri: &str) -> Result<String, String> {
    let svg = QrCode::new(pairing_uri.as_bytes())
        .map_err(|error| error.to_string())?
        .render::<svg::Color>()
        .min_dimensions(224, 224)
        .dark_color(svg::Color("#111318"))
        .light_color(svg::Color("#ffffff"))
        .build();
    Ok(format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg)
    ))
}

/// 校验设备展示名称，避免把控制字符写入中继审计元数据。
fn validate_device_name(device_name: &str) -> Result<(), String> {
    let name = device_name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("设备名称长度必须为 1 到 80 个字符且不能包含控制字符".into());
    }
    Ok(())
}

/// 生成不含用户信息的 Android 设备标识。
fn new_device_id() -> String {
    format!("android-{}", Uuid::now_v7().simple())
}

/// 返回签名证明使用的 Unix 毫秒时间。
fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}
