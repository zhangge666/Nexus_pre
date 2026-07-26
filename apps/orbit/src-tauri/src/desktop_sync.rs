//! 本文件实现 Orbit 桌面端 E2E 中继配置、系统凭据库身份、恢复、配对与设备治理。

use std::{
    collections::{BTreeMap, HashMap},
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use keyring::Entry;
use nexus_core::{
    Collection as CoreCollection, CoreEvent, HashEmbedder, ListQuery, Memory, MemoryFilters,
    MemorySource, MemoryStore,
};
use nexus_sync::{
    DeviceIdentity, EncryptedSyncEnvelope, OperationKind, PairingOffer, PlainSyncOperation,
    SealedPairingKey, SyncKey, VersionVector, VersionedRecord,
};
use qrcode::{QrCode, render::svg};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::{Collection, MemorySummary};

const CREDENTIAL_SERVICE: &str = "com.nexus.orbit.sync";
const ACCESS_TOKEN_STORAGE_KEY: &str = "relay-access-token";
const ROOT_KEY_STORAGE_KEY: &str = "e2e-root-key";
const DEVICE_ID_STORAGE_KEY: &str = "e2e-device-id";
const DEVICE_IDENTITY_STORAGE_KEY: &str = "e2e-device-identity";
const OUTGOING_PAIRING_STORAGE_KEY: &str = "e2e-outgoing-pairing";
const PENDING_JOIN_STORAGE_KEY: &str = "e2e-pending-join";
const REPLICA_VERSION: u8 = 1;
const REPLICA_ENVELOPE_VERSION: u8 = 1;
const REPLICA_NONCE_LENGTH: usize = 12;
const PULL_LIMIT: usize = 200;

/// 表示一次桌面增量同步后可供设置页展示的状态。
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

/// 表示桌面端当前 E2E 工作区和设备身份状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct E2eStatus {
    /// 系统凭据库中是否同时存在根密钥和设备签名身份。
    pub configured: bool,
    /// 不可逆工作区标识。
    pub workspace_id: Option<String>,
    /// 当前桌面设备标识。
    pub device_id: Option<String>,
    /// 是否存在等待另一台设备批准的加入请求。
    pub pending_join: bool,
    /// 是否存在当前设备创建的配对邀请。
    pub outgoing_pairing: bool,
}

/// 表示设置页展示的二维码配对邀请。
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
    /// 中继记录的过期时间。
    pub expires_at: i64,
}

/// 表示当前配对会话的中继状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusResponse {
    /// 配对会话标识。
    pub session_id: String,
    /// 会话过期时间。
    pub expires_at: i64,
    /// 尚待批准的新设备。
    pub pending_device: Option<SyncDevice>,
    /// 是否已经上传根密钥配对包。
    pub approved: bool,
    /// 新设备是否已经领取配对包。
    pub consumed: bool,
}

/// 表示当前桌面设备提交加入申请后的状态。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingJoinResponse {
    /// 新设备生成的稳定标识。
    pub device_id: String,
    /// 与邀请设备核对的六位确认码。
    pub verification_code: String,
    /// 是否仍在等待已有设备批准。
    pub waiting_for_approval: bool,
}

/// 表示中继登记的公开设备信息。
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
    /// 当前设备已上传的连续序号。
    pub last_sequence: u64,
    /// 当前设备已确认的中继游标。
    pub acknowledged_cursor: u64,
}

/// 表示 Android 与桌面同步密文共同承载的内容实体。
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

/// 表示一条集合成员关系。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CollectionMembership {
    collection_id: String,
    memory_id: String,
}

/// 表示桌面加密同步元数据、确定性合并状态与可靠待上传队列。
#[derive(Clone, Deserialize, Serialize)]
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
    /// 为当前工作区创建空的同步元数据副本。
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

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplicaEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredEnvelope {
    cursor: u64,
    envelope: EncryptedSyncEnvelope,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullChangesResponse {
    changes: Vec<StoredEnvelope>,
    next_cursor: u64,
    has_more: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PushChangeResponse {
    cursor: u64,
    duplicate: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcknowledgeRequest {
    workspace_id: String,
    cursor: u64,
    proof: DeviceProof,
}

#[derive(Clone)]
struct StoreContext {
    store: Arc<MemoryStore>,
    embedder: Arc<HashEmbedder>,
    replica_path: PathBuf,
}

#[derive(Clone)]
struct RelayConfig {
    endpoint: String,
    token: String,
    enabled: bool,
}

trait SecretStore: Send + Sync {
    /// 从受系统保护的凭据库读取敏感字节。
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String>;
    /// 将敏感字节写入受系统保护的凭据库。
    fn store(&self, key: &str, value: &[u8]) -> Result<(), String>;
    /// 删除受系统保护的凭据条目。
    fn delete(&self, key: &str) -> Result<(), String>;
}

struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        let entry =
            Entry::new(CREDENTIAL_SERVICE, key).map_err(|_| "无法打开桌面系统凭据库".to_owned())?;
        match entry.get_password() {
            Ok(encoded) => STANDARD
                .decode(encoded)
                .map(Some)
                .map_err(|_| "桌面系统凭据内容损坏".to_owned()),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("无法读取桌面系统凭据库".into()),
        }
    }

    fn store(&self, key: &str, value: &[u8]) -> Result<(), String> {
        Entry::new(CREDENTIAL_SERVICE, key)
            .map_err(|_| "无法打开桌面系统凭据库".to_owned())?
            .set_password(&STANDARD.encode(value))
            .map_err(|_| "无法写入桌面系统凭据库".to_owned())
    }

    fn delete(&self, key: &str) -> Result<(), String> {
        let entry =
            Entry::new(CREDENTIAL_SERVICE, key).map_err(|_| "无法打开桌面系统凭据库".to_owned())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("无法删除桌面系统凭据".into()),
        }
    }
}

/// 管理桌面端独立于本地 Memory Protocol 的 E2E Relay 连接和设备身份。
pub struct DesktopSync {
    client: reqwest::Client,
    config: Mutex<RelayConfig>,
    secrets: Arc<dyn SecretStore>,
    store: Mutex<Option<StoreContext>>,
    state_gate: tokio::sync::Mutex<()>,
    network_gate: tokio::sync::Mutex<()>,
    suppressed_events: Mutex<HashMap<String, usize>>,
}

impl DesktopSync {
    /// 从非敏感设置和系统凭据库恢复桌面 E2E 客户端。
    pub fn open(endpoint: String, enabled: bool) -> Self {
        Self::with_secret_store(endpoint, enabled, Arc::new(KeyringSecretStore))
    }

    /// 使用指定安全存储创建客户端，测试可注入不会访问真实系统凭据的实现。
    fn with_secret_store(endpoint: String, enabled: bool, secrets: Arc<dyn SecretStore>) -> Self {
        let token = secrets
            .load(ACCESS_TOKEN_STORAGE_KEY)
            .ok()
            .flatten()
            .map(String::from_utf8)
            .transpose()
            .ok()
            .flatten()
            .unwrap_or_default();
        Self {
            client: reqwest::Client::new(),
            config: Mutex::new(RelayConfig {
                endpoint,
                token,
                enabled,
            }),
            secrets,
            store: Mutex::new(None),
            state_gate: tokio::sync::Mutex::new(()),
            network_gate: tokio::sync::Mutex::new(()),
            suppressed_events: Mutex::new(HashMap::new()),
        }
    }

    /// 绑定当前本地服务持有者的数据库、嵌入器和加密同步元数据路径。
    pub fn attach_store(
        &self,
        store: Arc<MemoryStore>,
        embedder: Arc<HashEmbedder>,
        replica_path: PathBuf,
    ) -> Result<(), String> {
        *self
            .store
            .lock()
            .map_err(|_| "桌面同步存储状态不可用".to_owned())? = Some(StoreContext {
            store,
            embedder,
            replica_path,
        });
        Ok(())
    }

    /// 返回系统凭据库是否已经保存 Relay 访问令牌。
    pub fn has_access_token(&self) -> bool {
        self.config
            .lock()
            .is_ok_and(|config| !config.token.trim().is_empty())
    }

    /// 返回用户是否已经在桌面设置中启用 E2E Relay 同步。
    pub fn is_enabled(&self) -> bool {
        self.config.lock().is_ok_and(|config| config.enabled)
    }

    /// 验证零知识中继并把访问令牌写入系统凭据库。
    pub async fn configure(&self, endpoint: &str, token: &str) -> Result<(), String> {
        let endpoint = normalize_relay_endpoint(endpoint)?;
        let existing_token = self
            .config
            .lock()
            .map_err(|_| "桌面同步配置状态不可用".to_owned())?
            .token
            .clone();
        let submitted_token = token.trim();
        let token = if submitted_token.is_empty() {
            existing_token.as_str()
        } else {
            submitted_token
        };
        if token.is_empty() {
            return Err("请填写 Relay 访问令牌".into());
        }
        validate_relay(&self.client, &endpoint, token).await?;
        self.secrets
            .store(ACCESS_TOKEN_STORAGE_KEY, token.as_bytes())?;
        *self
            .config
            .lock()
            .map_err(|_| "桌面同步配置状态不可用".to_owned())? = RelayConfig {
            endpoint,
            token: token.into(),
            enabled: true,
        };
        Ok(())
    }

    /// 暂停桌面 E2E 同步但保留身份和令牌，便于稍后无损恢复。
    pub fn disable(&self) -> Result<(), String> {
        self.config
            .lock()
            .map(|mut config| config.enabled = false)
            .map_err(|_| "桌面同步配置状态不可用".into())
    }

    /// 返回当前系统凭据库中的 E2E 配置摘要。
    pub fn status(&self) -> Result<E2eStatus, String> {
        let key = self.load_sync_key()?;
        let identity = self.load_identity()?;
        let configured = key.is_some() && identity.is_some();
        Ok(E2eStatus {
            configured,
            workspace_id: key.as_ref().map(SyncKey::workspace_id),
            device_id: identity.map(|identity| identity.device_id().to_owned()),
            pending_join: self.secrets.load(PENDING_JOIN_STORAGE_KEY)?.is_some(),
            outgoing_pairing: self.secrets.load(OUTGOING_PAIRING_STORAGE_KEY)?.is_some(),
        })
    }

    /// 创建首个 E2E 工作区和桌面设备身份，并登记恢复签名公钥。
    pub async fn initialize(&self, device_name: &str) -> Result<E2eStatus, String> {
        validate_device_name(device_name)?;
        let config = self.connection()?;
        validate_relay(&self.client, &config.endpoint, &config.token).await?;
        if self.load_sync_key()?.is_some() {
            return Err("当前桌面设备已经配置端到端同步".into());
        }
        let key = SyncKey::generate();
        let identity =
            DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
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
            self.client
                .post(relay_url(&config.endpoint, "/v1/sync/devices/bootstrap")?)
                .bearer_auth(&config.token)
                .json(&request),
        )
        .await?;
        self.persist_current_identity(&key, &identity)?;
        self.reset_and_seed_replica(&key).await?;
        if self.store_context().is_ok() {
            self.sync_content().await?;
        }
        self.status()
    }

    /// 使用 24 词恢复短语登记桌面设备，短语与根密钥不会发送到中继。
    pub async fn restore(&self, phrase: &str, device_name: &str) -> Result<E2eStatus, String> {
        validate_device_name(device_name)?;
        let config = self.connection()?;
        validate_relay(&self.client, &config.endpoint, &config.token).await?;
        let key = SyncKey::from_recovery_phrase(phrase).map_err(|error| error.to_string())?;
        let identity =
            DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
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
            self.client
                .post(relay_url(&config.endpoint, "/v1/sync/devices/recover")?)
                .bearer_auth(&config.token)
                .json(&request),
        )
        .await?;
        self.persist_current_identity(&key, &identity)?;
        self.reset_and_seed_replica(&key).await?;
        if self.store_context().is_ok() {
            self.sync_content().await?;
        }
        self.status()
    }

    /// 返回当前根密钥对应的 24 词恢复短语。
    pub fn recovery_phrase(&self) -> Result<String, String> {
        self.load_sync_key()?
            .ok_or_else(|| "当前桌面设备尚未配置端到端同步".to_owned())?
            .recovery_phrase()
            .map_err(|error| error.to_string())
    }

    /// 创建十分钟有效的中继会话并生成包含高熵秘密的二维码。
    pub async fn create_pairing_offer(&self) -> Result<PairingOfferResponse, String> {
        let config = self.connection()?;
        let (key, identity) = self.current_identity()?;
        let offer = PairingOffer::create(&key);
        let action = format!("pairing:create:{}:{}", key.workspace_id(), offer.session_id);
        let request = CreatePairingRequest {
            session_id: offer.session_id,
            workspace_id: key.workspace_id(),
            proof: create_proof(&identity, &action),
        };
        let created: CreatedPairingResponse = send_json(
            self.client
                .post(relay_url(&config.endpoint, "/v1/sync/pairings")?)
                .bearer_auth(&config.token)
                .json(&request),
        )
        .await?;
        let pairing_uri = offer.to_uri();
        self.secrets
            .store(OUTGOING_PAIRING_STORAGE_KEY, pairing_uri.as_bytes())?;
        Ok(PairingOfferResponse {
            session_id: offer.session_id.to_string(),
            qr_data_url: qr_data_url(&pairing_uri)?,
            pairing_uri,
            verification_code: offer.verification_code(),
            expires_at: created.expires_at,
        })
    }

    /// 查询当前桌面设备创建的配对邀请状态。
    pub async fn pairing_status(&self) -> Result<PairingStatusResponse, String> {
        let config = self.connection()?;
        let (key, identity) = self.current_identity()?;
        let offer = self.load_outgoing_offer()?;
        let action = format!("pairing:status:{}:{}", key.workspace_id(), offer.session_id);
        let proof = create_proof(&identity, &action);
        let response: RelayPairingStatus = send_json(
            self.client
                .get(relay_url(
                    &config.endpoint,
                    &format!("/v1/sync/pairings/{}", offer.session_id),
                )?)
                .bearer_auth(&config.token)
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

    /// 批准当前配对会话中的新设备并上传根密钥密文包。
    pub async fn approve_pairing(&self) -> Result<SyncDevice, String> {
        let config = self.connection()?;
        let (key, identity) = self.current_identity()?;
        let offer = self.load_outgoing_offer()?;
        let status = self.pairing_status().await?;
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
            self.client
                .post(relay_url(
                    &config.endpoint,
                    &format!("/v1/sync/pairings/{}/approve", offer.session_id),
                )?)
                .bearer_auth(&config.token)
                .json(&ApprovePairingRequest {
                    sealed_key,
                    proof: create_proof(&identity, &action),
                }),
        )
        .await?;
        self.secrets.delete(OUTGOING_PAIRING_STORAGE_KEY)?;
        Ok(device)
    }

    /// 使用二维码 URI 创建桌面设备加入请求。
    pub async fn request_pairing(
        &self,
        pairing_uri: &str,
        device_name: &str,
    ) -> Result<PairingJoinResponse, String> {
        validate_device_name(device_name)?;
        let config = self.connection()?;
        let offer = PairingOffer::from_uri(pairing_uri).map_err(|error| error.to_string())?;
        let identity =
            DeviceIdentity::generate(new_device_id()).map_err(|error| error.to_string())?;
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
            self.client
                .post(relay_url(
                    &config.endpoint,
                    &format!("/v1/sync/pairings/{}/request", offer.session_id),
                )?)
                .bearer_auth(&config.token)
                .json(&request),
        )
        .await?;
        self.store_json_secret(
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

    /// 领取已批准的配对包，解封根密钥并保存到桌面系统凭据库。
    pub async fn complete_pairing(&self) -> Result<E2eStatus, String> {
        let config = self.connection()?;
        let pending: PendingJoin = self
            .load_json_secret(PENDING_JOIN_STORAGE_KEY)?
            .ok_or_else(|| "当前桌面设备没有等待完成的配对请求".to_owned())?;
        let offer =
            PairingOffer::from_uri(&pending.pairing_uri).map_err(|error| error.to_string())?;
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
            self.client
                .post(relay_url(
                    &config.endpoint,
                    &format!("/v1/sync/pairings/{}/package", offer.session_id),
                )?)
                .bearer_auth(&config.token)
                .json(&request),
        )
        .await?;
        let key = offer
            .secret
            .open_sync_key(&sealed, identity.device_id())
            .map_err(|error| error.to_string())?;
        self.persist_current_identity(&key, &identity)?;
        self.secrets.delete(PENDING_JOIN_STORAGE_KEY)?;
        self.reset_and_seed_replica(&key).await?;
        if self.store_context().is_ok() {
            self.sync_content().await?;
        }
        self.status()
    }

    /// 列出工作区全部有效和已撤销设备。
    pub async fn list_devices(&self) -> Result<Vec<SyncDevice>, String> {
        let config = self.connection()?;
        let (key, identity) = self.current_identity()?;
        let action = format!("devices:list:{}", key.workspace_id());
        let proof = create_proof(&identity, &action);
        send_json(
            self.client
                .get(relay_url(&config.endpoint, "/v1/sync/devices")?)
                .bearer_auth(&config.token)
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
    pub async fn revoke_device(&self, target_device_id: &str) -> Result<SyncDevice, String> {
        let config = self.connection()?;
        let (key, identity) = self.current_identity()?;
        if target_device_id == identity.device_id() {
            return Err("不能在当前设备上撤销自己；请先在另一台有效设备上操作".into());
        }
        let action = format!("device:revoke:{}:{target_device_id}", key.workspace_id());
        send_json(
            self.client
                .delete(relay_url(
                    &config.endpoint,
                    &format!("/v1/sync/devices/{target_device_id}"),
                )?)
                .bearer_auth(&config.token)
                .json(&RevokeDeviceRequest {
                    workspace_id: key.workspace_id(),
                    proof: create_proof(&identity, &action),
                }),
        )
        .await
    }

    /// 将一条本地数据库提交事件转换为加密待上传操作；远端回写事件会被防回环标记消费。
    pub async fn handle_local_event(&self, event: CoreEvent) -> Result<bool, String> {
        if !self.is_enabled() || !self.status()?.configured {
            return Ok(false);
        }
        let Some(entity_id) = event_entity_id(&event) else {
            return Ok(false);
        };
        if self.take_suppressed_event(&entity_id)? {
            return Ok(false);
        }
        let deleted_memory_id = match &event {
            CoreEvent::MemoryDeleted { id, .. } => Some(id.to_string()),
            _ => None,
        };
        let deleted_collection_id = match &event {
            CoreEvent::CollectionDeleted { id } => Some(id.to_string()),
            _ => None,
        };
        let context = self.store_context()?;
        let value = match event {
            CoreEvent::MemoryCreated { id, .. } | CoreEvent::MemoryUpdated { id, .. } => {
                let memory = context
                    .store
                    .get(&id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "本地提交事件对应的记忆不存在".to_owned())?;
                Some(SyncEntity::Memory(memory_to_summary(memory)?))
            }
            CoreEvent::MemoryDeleted { .. } | CoreEvent::CollectionDeleted { .. } => None,
            CoreEvent::CollectionCreated { id } | CoreEvent::CollectionUpdated { id } => {
                let collection = context
                    .store
                    .get_collection(id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "本地提交事件对应的集合不存在".to_owned())?;
                Some(SyncEntity::Collection(collection_to_summary(collection)))
            }
            CoreEvent::CollectionMembershipAdded {
                collection_id,
                memory_id,
            } => Some(SyncEntity::Membership(CollectionMembership {
                collection_id: collection_id.to_string(),
                memory_id: memory_id.to_string(),
            })),
            CoreEvent::CollectionMembershipRemoved { .. } => None,
            CoreEvent::ReviewDue { .. } | CoreEvent::ReviewGraded { .. } => return Ok(false),
        };
        let (key, identity) = self.current_identity()?;
        let _state = self.state_gate.lock().await;
        let mut replica = load_replica(&context.replica_path, &key)?;
        queue_entity_change(&mut replica, &key, &identity, entity_id, value)?;
        let related_memberships = replica
            .records
            .iter()
            .filter_map(|(entity_id, record)| match record.value.as_ref() {
                Some(SyncEntity::Membership(membership))
                    if deleted_memory_id.as_ref().is_some_and(|memory_id| {
                        membership.memory_id.as_str() == memory_id.as_str()
                    }) || deleted_collection_id.as_ref().is_some_and(|collection_id| {
                        membership.collection_id.as_str() == collection_id.as_str()
                    }) =>
                {
                    Some(entity_id.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        for membership_id in related_memberships {
            queue_entity_change(&mut replica, &key, &identity, membership_id, None)?;
        }
        persist_replica(&context.replica_path, &key, &replica)?;
        Ok(true)
    }

    /// 上传桌面待处理操作、拉取远端密文、写回本地数据库并确认最新游标。
    pub async fn sync_content(&self) -> Result<ContentSyncStatus, String> {
        if !self.is_enabled() {
            return Err("桌面端到端同步尚未启用".into());
        }
        let _network = self.network_gate.lock().await;
        let config = self.connection()?;
        let context = self.store_context()?;
        let (key, identity) = self.current_identity()?;
        let devices = self.list_devices().await?;
        let public_keys = devices
            .iter()
            .map(|device| (device.device_id.clone(), device.public_key.clone()))
            .collect::<HashMap<_, _>>();
        {
            let _state = self.state_gate.lock().await;
            let mut replica = load_replica(&context.replica_path, &key)?;
            if let Some(current) = devices
                .iter()
                .find(|device| device.device_id == identity.device_id())
            {
                replica.local_sequence = replica.local_sequence.max(current.last_sequence);
            }
            persist_replica(&context.replica_path, &key, &replica)?;
        }
        // 启动时补扫订阅建立前已经存在的本地数据；已有记录或墓碑不会被重复排队。
        self.seed_current_store(&key).await?;

        loop {
            let envelope = {
                let _state = self.state_gate.lock().await;
                load_replica(&context.replica_path, &key)?
                    .pending
                    .first()
                    .cloned()
            };
            let Some(envelope) = envelope else {
                break;
            };
            let response: PushChangeResponse = send_json(
                self.client
                    .post(relay_url(&config.endpoint, "/v1/sync/changes")?)
                    .bearer_auth(&config.token)
                    .json(&envelope),
            )
            .await?;
            if response.cursor == 0 {
                return Err("E2E 中继返回了无效上传游标".into());
            }
            let _was_duplicate = response.duplicate;
            let _state = self.state_gate.lock().await;
            let mut replica = load_replica(&context.replica_path, &key)?;
            if replica
                .pending
                .first()
                .is_some_and(|pending| pending.operation_id == envelope.operation_id)
            {
                replica.pending.remove(0);
            } else {
                replica
                    .pending
                    .retain(|pending| pending.operation_id != envelope.operation_id);
            }
            persist_replica(&context.replica_path, &key, &replica)?;
        }

        loop {
            let cursor = {
                let _state = self.state_gate.lock().await;
                load_replica(&context.replica_path, &key)?.cursor
            };
            let action = format!(
                "changes:pull:{}:{}:{}",
                key.workspace_id(),
                cursor,
                PULL_LIMIT
            );
            let proof = create_proof(&identity, &action);
            let response: PullChangesResponse = send_json(
                self.client
                    .get(relay_url(&config.endpoint, "/v1/sync/changes")?)
                    .bearer_auth(&config.token)
                    .query(&[
                        ("workspaceId", key.workspace_id()),
                        ("after", cursor.to_string()),
                        ("limit", PULL_LIMIT.to_string()),
                        ("deviceId", proof.device_id),
                        ("timestamp", proof.timestamp.to_string()),
                        ("nonce", proof.nonce),
                        ("signature", proof.signature),
                    ]),
            )
            .await?;
            let mut operations = Vec::with_capacity(response.changes.len());
            for stored in response.changes {
                let public_key = public_keys
                    .get(&stored.envelope.device_id)
                    .ok_or_else(|| "同步信封来源设备未登记，已拒绝解密".to_owned())?;
                let operation = key
                    .decrypt_operation(&stored.envelope, public_key)
                    .map_err(|error| error.to_string())?;
                operations.push((stored.cursor, operation));
            }
            {
                let _state = self.state_gate.lock().await;
                let mut replica = load_replica(&context.replica_path, &key)?;
                for (stored_cursor, operation) in operations {
                    if let Some((entity_id, record)) = apply_operation(&mut replica, operation)? {
                        self.apply_record_to_store(&context, &entity_id, &record)?;
                    }
                    replica.cursor = replica.cursor.max(stored_cursor);
                }
                replica.cursor = replica.cursor.max(response.next_cursor);
                persist_replica(&context.replica_path, &key, &replica)?;
            }
            if !response.has_more {
                break;
            }
        }

        let cursor = {
            let _state = self.state_gate.lock().await;
            load_replica(&context.replica_path, &key)?.cursor
        };
        if cursor > 0 {
            let action = format!("changes:ack:{}:{cursor}", key.workspace_id());
            let _: serde_json::Value = send_json(
                self.client
                    .post(relay_url(&config.endpoint, "/v1/sync/ack")?)
                    .bearer_auth(&config.token)
                    .json(&AcknowledgeRequest {
                        workspace_id: key.workspace_id(),
                        cursor,
                        proof: create_proof(&identity, &action),
                    }),
            )
            .await?;
        }
        let _state = self.state_gate.lock().await;
        let mut replica = load_replica(&context.replica_path, &key)?;
        replica.last_sync_at = Some(unix_millis());
        persist_replica(&context.replica_path, &key, &replica)?;
        Ok(replica_status(&replica))
    }

    /// 返回桌面加密同步元数据中的游标、待上传数与冲突留痕数。
    pub async fn content_status(&self) -> Result<ContentSyncStatus, String> {
        let context = self.store_context()?;
        let (key, _) = self.current_identity()?;
        let _state = self.state_gate.lock().await;
        Ok(replica_status(&load_replica(&context.replica_path, &key)?))
    }

    /// 将当前桌面库中尚未进入工作区的记忆、集合与成员关系加入可靠队列。
    async fn seed_current_store(&self, key: &SyncKey) -> Result<(), String> {
        let context = match self.store_context() {
            Ok(context) => context,
            Err(_) => return Ok(()),
        };
        let (_, identity) = self.current_identity()?;
        let _state = self.state_gate.lock().await;
        let mut replica = load_replica(&context.replica_path, key)?;
        for collection in context
            .store
            .list_collections()
            .map_err(|error| error.to_string())?
        {
            let summary = collection_to_summary(collection);
            let entity_id = collection_entity_id(&summary.id);
            if !replica.records.contains_key(&entity_id) {
                queue_entity_change(
                    &mut replica,
                    key,
                    &identity,
                    entity_id,
                    Some(SyncEntity::Collection(summary)),
                )?;
            }
        }
        let mut offset = 0;
        loop {
            let page = context
                .store
                .list(&ListQuery {
                    filters: MemoryFilters::default(),
                    limit: 100,
                    offset,
                })
                .map_err(|error| error.to_string())?;
            for memory in page.items {
                let summary = memory_to_summary(memory)?;
                let entity_id = memory_entity_id(&summary.id);
                if !replica.records.contains_key(&entity_id) {
                    queue_entity_change(
                        &mut replica,
                        key,
                        &identity,
                        entity_id,
                        Some(SyncEntity::Memory(summary)),
                    )?;
                }
            }
            let Some(next_offset) = page.next_offset else {
                break;
            };
            offset = next_offset;
        }
        for collection in context
            .store
            .list_collections()
            .map_err(|error| error.to_string())?
        {
            for memory_id in context
                .store
                .list_collection_memory_ids(collection.id)
                .map_err(|error| error.to_string())?
            {
                let entity_id =
                    membership_entity_id(&collection.id.to_string(), &memory_id.to_string());
                if !replica.records.contains_key(&entity_id) {
                    queue_entity_change(
                        &mut replica,
                        key,
                        &identity,
                        entity_id,
                        Some(SyncEntity::Membership(CollectionMembership {
                            collection_id: collection.id.to_string(),
                            memory_id: memory_id.to_string(),
                        })),
                    )?;
                }
            }
        }
        persist_replica(&context.replica_path, key, &replica)
    }

    /// 重置工作区同步元数据并扫描当前桌面库建立首批待上传操作。
    async fn reset_and_seed_replica(&self, key: &SyncKey) -> Result<(), String> {
        let context = match self.store_context() {
            Ok(context) => context,
            Err(_) => return Ok(()),
        };
        {
            let _state = self.state_gate.lock().await;
            persist_replica(&context.replica_path, key, &LocalReplica::empty(key))?;
        }
        self.seed_current_store(key).await
    }

    /// 将收敛后的可见记录幂等物化到桌面数据库。
    fn apply_record_to_store(
        &self,
        context: &StoreContext,
        entity_id: &str,
        record: &VersionedRecord<SyncEntity>,
    ) -> Result<(), String> {
        match record.value.as_ref() {
            Some(SyncEntity::Memory(summary)) => {
                let id = parse_uuid(&summary.id, "记忆")?;
                self.suppress_event(entity_id)?;
                let result = context.store.upsert_synced_memory(
                    summary_to_memory(summary, record.device_id.clone())?,
                    context.embedder.as_ref(),
                );
                if let Err(error) = result {
                    self.cancel_suppressed_event(entity_id)?;
                    return Err(error.to_string());
                }
                let stored = context
                    .store
                    .get(&id)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "同步记忆写回后不可见".to_owned())?;
                if stored.updated_at != summary.updated_at {
                    return Err("同步记忆写回时间戳不一致".into());
                }
            }
            Some(SyncEntity::Collection(summary)) => {
                let id = parse_uuid(&summary.id, "集合")?;
                let parent_id = summary
                    .parent_id
                    .as_deref()
                    .map(|value| parse_uuid(value, "父集合"))
                    .transpose()?
                    .filter(|parent_id| {
                        context
                            .store
                            .get_collection(*parent_id)
                            .ok()
                            .flatten()
                            .is_some()
                    });
                let existed = context
                    .store
                    .get_collection(id)
                    .map_err(|error| error.to_string())?
                    .is_some();
                let created_at = if existed {
                    context
                        .store
                        .get_collection(id)
                        .map_err(|error| error.to_string())?
                        .map_or(record.modified_at, |collection| collection.created_at)
                } else {
                    record.modified_at
                };
                self.suppress_event(entity_id)?;
                if let Err(error) = context.store.upsert_synced_collection(CoreCollection {
                    id,
                    name: summary.name.clone(),
                    icon: summary.icon.clone(),
                    parent_id,
                    sort: summary.sort,
                    created_at,
                    updated_at: record.modified_at.max(created_at),
                }) {
                    self.cancel_suppressed_event(entity_id)?;
                    return Err(error.to_string());
                }
            }
            Some(SyncEntity::Membership(membership)) => {
                let collection_id = parse_uuid(&membership.collection_id, "集合")?;
                let memory_id = parse_uuid(&membership.memory_id, "记忆")?;
                let already_exists = context
                    .store
                    .list_collection_memory_ids(collection_id)
                    .map_err(|error| error.to_string())?
                    .contains(&memory_id);
                if !already_exists {
                    self.suppress_event(entity_id)?;
                    if let Err(error) = context
                        .store
                        .add_memory_to_collection(collection_id, memory_id)
                    {
                        self.cancel_suppressed_event(entity_id)?;
                        return Err(error.to_string());
                    }
                }
            }
            None if entity_id.starts_with("memory:") => {
                let id = parse_uuid(entity_id.trim_start_matches("memory:"), "记忆")?;
                if context
                    .store
                    .get(&id)
                    .map_err(|error| error.to_string())?
                    .is_some()
                {
                    self.suppress_event(entity_id)?;
                    if let Err(error) = context.store.delete(&id) {
                        self.cancel_suppressed_event(entity_id)?;
                        return Err(error.to_string());
                    }
                }
            }
            None if entity_id.starts_with("collection:") => {
                let id = parse_uuid(entity_id.trim_start_matches("collection:"), "集合")?;
                if context
                    .store
                    .get_collection(id)
                    .map_err(|error| error.to_string())?
                    .is_some()
                {
                    self.suppress_event(entity_id)?;
                    if let Err(error) = context.store.delete_collection(id) {
                        self.cancel_suppressed_event(entity_id)?;
                        return Err(error.to_string());
                    }
                }
            }
            None if entity_id.starts_with("membership:") => {
                let (collection_id, memory_id) = parse_membership_entity_id(entity_id)?;
                let exists = context
                    .store
                    .list_collection_memory_ids(collection_id)
                    .map(|members| members.contains(&memory_id))
                    .unwrap_or(false);
                if exists {
                    self.suppress_event(entity_id)?;
                    if let Err(error) = context
                        .store
                        .remove_memory_from_collection(collection_id, memory_id)
                    {
                        self.cancel_suppressed_event(entity_id)?;
                        return Err(error.to_string());
                    }
                }
            }
            None => return Err("同步墓碑实体命名空间无效".into()),
        }
        Ok(())
    }

    /// 返回当前持有者绑定的本地数据库上下文。
    fn store_context(&self) -> Result<StoreContext, String> {
        self.store
            .lock()
            .map_err(|_| "桌面同步存储状态不可用".to_owned())?
            .clone()
            .ok_or_else(|| "当前 Orbit 实例不是本地记忆服务持有者".into())
    }

    /// 为即将发生的远端数据库回写登记一次事件抑制。
    fn suppress_event(&self, entity_id: &str) -> Result<(), String> {
        let mut suppressed = self
            .suppressed_events
            .lock()
            .map_err(|_| "桌面同步防回环状态不可用".to_owned())?;
        *suppressed.entry(entity_id.into()).or_default() += 1;
        Ok(())
    }

    /// 远端数据库写回失败时撤销未消费的事件抑制。
    fn cancel_suppressed_event(&self, entity_id: &str) -> Result<(), String> {
        self.take_suppressed_event(entity_id).map(|_| ())
    }

    /// 消费一个与远端回写对应的本地提交事件。
    fn take_suppressed_event(&self, entity_id: &str) -> Result<bool, String> {
        let mut suppressed = self
            .suppressed_events
            .lock()
            .map_err(|_| "桌面同步防回环状态不可用".to_owned())?;
        let Some(count) = suppressed.get_mut(entity_id) else {
            return Ok(false);
        };
        *count = count.saturating_sub(1);
        if *count == 0 {
            suppressed.remove(entity_id);
        }
        Ok(true)
    }

    /// 清除桌面 Relay 令牌、E2E 根密钥、设备身份和未完成配对材料。
    pub fn clear(&self) -> Result<(), String> {
        let mut errors = Vec::new();
        for key in [
            ACCESS_TOKEN_STORAGE_KEY,
            ROOT_KEY_STORAGE_KEY,
            DEVICE_ID_STORAGE_KEY,
            DEVICE_IDENTITY_STORAGE_KEY,
            OUTGOING_PAIRING_STORAGE_KEY,
            PENDING_JOIN_STORAGE_KEY,
        ] {
            if let Err(error) = self.secrets.delete(key) {
                errors.push(error);
            }
        }
        if let Ok(mut config) = self.config.lock() {
            config.endpoint.clear();
            config.token.clear();
            config.enabled = false;
        }
        if let Ok(Some(context)) = self.store.lock().map(|store| store.clone()) {
            for path in replica_related_paths(&context.replica_path) {
                if path.exists()
                    && let Err(error) = fs::remove_file(&path)
                {
                    errors.push(format!("删除桌面同步元数据失败：{error}"));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("；"))
        }
    }

    /// 返回已经验证并可用于 Relay 请求的连接配置。
    fn connection(&self) -> Result<RelayConfig, String> {
        let config = self
            .config
            .lock()
            .map_err(|_| "桌面同步配置状态不可用".to_owned())?
            .clone();
        if config.endpoint.trim().is_empty() {
            return Err("请先配置 E2E Relay 地址".into());
        }
        if !config.enabled {
            return Err("请先在桌面设置中启用 E2E Relay 同步".into());
        }
        if config.token.trim().is_empty() {
            return Err("请先配置 E2E Relay 访问令牌".into());
        }
        Ok(config)
    }

    /// 读取当前根密钥和签名身份。
    fn current_identity(&self) -> Result<(SyncKey, DeviceIdentity), String> {
        Ok((
            self.load_sync_key()?
                .ok_or_else(|| "当前桌面设备尚未配置端到端同步".to_owned())?,
            self.load_identity()?
                .ok_or_else(|| "当前桌面设备签名身份不可用".to_owned())?,
        ))
    }

    /// 从桌面系统凭据库恢复同步根密钥。
    fn load_sync_key(&self) -> Result<Option<SyncKey>, String> {
        self.secrets
            .load(ROOT_KEY_STORAGE_KEY)?
            .map(|bytes| {
                bytes
                    .try_into()
                    .map(SyncKey::from_bytes)
                    .map_err(|_| "桌面 E2E 根密钥长度无效".to_owned())
            })
            .transpose()
    }

    /// 从桌面系统凭据库恢复设备标识和 PKCS#8 私钥。
    fn load_identity(&self) -> Result<Option<DeviceIdentity>, String> {
        let Some(device_id) = self.secrets.load(DEVICE_ID_STORAGE_KEY)? else {
            return Ok(None);
        };
        let Some(pkcs8) = self.secrets.load(DEVICE_IDENTITY_STORAGE_KEY)? else {
            return Ok(None);
        };
        DeviceIdentity::from_pkcs8(
            String::from_utf8(device_id).map_err(|error| error.to_string())?,
            pkcs8,
        )
        .map(Some)
        .map_err(|error| error.to_string())
    }

    /// 事务式保存根密钥、设备标识和设备私钥，失败时清理已写条目。
    fn persist_current_identity(
        &self,
        key: &SyncKey,
        identity: &DeviceIdentity,
    ) -> Result<(), String> {
        let result = (|| {
            self.secrets.store(ROOT_KEY_STORAGE_KEY, &key.to_bytes())?;
            self.secrets
                .store(DEVICE_ID_STORAGE_KEY, identity.device_id().as_bytes())?;
            self.secrets
                .store(DEVICE_IDENTITY_STORAGE_KEY, &identity.pkcs8_bytes())
        })();
        if let Err(error) = result {
            let _ = self.secrets.delete(ROOT_KEY_STORAGE_KEY);
            let _ = self.secrets.delete(DEVICE_ID_STORAGE_KEY);
            let _ = self.secrets.delete(DEVICE_IDENTITY_STORAGE_KEY);
            return Err(error);
        }
        Ok(())
    }

    /// 读取当前桌面设备创建的配对邀请。
    fn load_outgoing_offer(&self) -> Result<PairingOffer, String> {
        let bytes = self
            .secrets
            .load(OUTGOING_PAIRING_STORAGE_KEY)?
            .ok_or_else(|| "当前桌面设备没有待处理的配对邀请".to_owned())?;
        PairingOffer::from_uri(&String::from_utf8(bytes).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())
    }

    /// 将结构化敏感状态编码后写入系统凭据库。
    fn store_json_secret<T: Serialize>(&self, key: &str, value: &T) -> Result<(), String> {
        self.secrets.store(
            key,
            &serde_json::to_vec(value).map_err(|error| error.to_string())?,
        )
    }

    /// 从系统凭据库读取并解码结构化敏感状态。
    fn load_json_secret<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, String> {
        self.secrets
            .load(key)?
            .map(|value| serde_json::from_slice(&value).map_err(|error| error.to_string()))
            .transpose()
    }
}

/// 将核心提交事件映射为同步实体标识；复习调度事件不进入内容同步。
fn event_entity_id(event: &CoreEvent) -> Option<String> {
    match event {
        CoreEvent::MemoryCreated { id, .. }
        | CoreEvent::MemoryUpdated { id, .. }
        | CoreEvent::MemoryDeleted { id, .. } => Some(memory_entity_id(&id.to_string())),
        CoreEvent::CollectionCreated { id }
        | CoreEvent::CollectionUpdated { id }
        | CoreEvent::CollectionDeleted { id } => Some(collection_entity_id(&id.to_string())),
        CoreEvent::CollectionMembershipAdded {
            collection_id,
            memory_id,
        }
        | CoreEvent::CollectionMembershipRemoved {
            collection_id,
            memory_id,
        } => Some(membership_entity_id(
            &collection_id.to_string(),
            &memory_id.to_string(),
        )),
        CoreEvent::ReviewDue { .. } | CoreEvent::ReviewGraded { .. } => None,
    }
}

/// 将核心记忆模型转换为 Android 已使用的稳定同步载荷。
fn memory_to_summary(memory: Memory) -> Result<MemorySummary, String> {
    Ok(MemorySummary {
        id: memory.id.to_string(),
        source: memory.source.as_storage_value(),
        kind: enum_name(&memory.kind)?,
        title: memory.title,
        content: memory.content,
        content_format: enum_name(&memory.content_format)?,
        tags: memory.tags,
        pinned: memory.pinned,
        archived: memory.archived,
        created_at: memory.created_at,
        updated_at: memory.updated_at,
        captured_at: memory.captured_at,
        links: Vec::new(),
        conflict_count: 0,
    })
}

/// 将同步记忆载荷恢复为可由核心存储重建索引的完整模型。
fn summary_to_memory(summary: &MemorySummary, winning_device_id: String) -> Result<Memory, String> {
    Ok(Memory {
        id: parse_uuid(&summary.id, "记忆")?,
        source: MemorySource::from_storage_value(&summary.source)
            .ok_or_else(|| "同步记忆来源无效".to_owned())?,
        kind: parse_enum_name(&summary.kind, "同步记忆类别")?,
        title: summary.title.clone(),
        content: summary.content.clone(),
        content_format: parse_enum_name(&summary.content_format, "同步正文格式")?,
        blocks: Vec::new(),
        tags: summary.tags.clone(),
        pinned: summary.pinned,
        archived: summary.archived,
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        captured_at: summary.captured_at,
        device_id: winning_device_id.clone(),
        meta: serde_json::json!({
            "sync": {
                "sourceDeviceId": winning_device_id
            }
        }),
    })
}

/// 将核心集合模型转换为移动端既有同步载荷。
fn collection_to_summary(collection: CoreCollection) -> Collection {
    Collection {
        id: collection.id.to_string(),
        name: collection.name,
        icon: collection.icon,
        parent_id: collection.parent_id.map(|id| id.to_string()),
        sort: collection.sort,
    }
}

/// 将 serde snake_case 枚举编码为数据库与同步协议共用的稳定名称。
fn enum_name<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_value(value)
        .map_err(|error| error.to_string())?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "同步枚举无法编码为字符串".to_owned())
}

/// 从稳定 snake_case 名称恢复 serde 枚举。
fn parse_enum_name<T: DeserializeOwned>(value: &str, field: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(value.into()))
        .map_err(|error| format!("{field}无效：{error}"))
}

/// 将本地实体变更加密、合并到副本并加入可靠上传队列。
fn queue_entity_change(
    replica: &mut LocalReplica,
    key: &SyncKey,
    identity: &DeviceIdentity,
    entity_id: String,
    value: Option<SyncEntity>,
) -> Result<(), String> {
    replica.local_sequence = replica
        .local_sequence
        .checked_add(1)
        .ok_or_else(|| "桌面设备同步序号已溢出".to_owned())?;
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
        .encrypt_operation(&operation, identity)
        .map_err(|error| error.to_string())?;
    let _ = apply_operation(replica, operation)?;
    replica.pending.push(envelope);
    Ok(())
}

/// 将一条认证操作确定性合并进桌面副本，并返回需要物化的可见记录。
fn apply_operation(
    replica: &mut LocalReplica,
    operation: PlainSyncOperation,
) -> Result<Option<(String, VersionedRecord<SyncEntity>)>, String> {
    let value = operation
        .payload
        .map(serde_json::from_value::<SyncEntity>)
        .transpose()
        .map_err(|error| format!("同步实体载荷无效：{error}"))?;
    validate_entity_payload(&operation.entity_id, value.as_ref())?;
    let incoming = VersionedRecord {
        value,
        version: operation.version,
        device_id: operation.device_id,
        modified_at: operation.created_at,
        conflicts: Vec::new(),
    };
    let entity_id = operation.entity_id;
    let previous = replica.records.remove(&entity_id);
    let merged = if let Some(existing) = previous.clone() {
        existing.merge(incoming).record
    } else {
        incoming
    };
    let changed = previous.as_ref() != Some(&merged);
    replica.records.insert(entity_id.clone(), merged.clone());
    Ok(changed.then_some((entity_id, merged)))
}

/// 校验实体命名空间与解密载荷类型一致。
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

/// 汇总副本游标、待上传操作和并发失败版本数量。
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

/// 从根密钥域分离桌面同步元数据加密密钥。
fn replica_encryption_key(key: &SyncKey) -> [u8; 32] {
    *blake3::keyed_hash(&key.to_bytes(), b"nexus-desktop-replica-v1").as_bytes()
}

/// 读取并认证桌面同步元数据；主文件中断时可回退到上一份完整备份。
fn load_replica(path: &Path, key: &SyncKey) -> Result<LocalReplica, String> {
    let backup = path.with_extension("bak");
    let source = if path.exists() {
        Some(path)
    } else if backup.exists() {
        Some(backup.as_path())
    } else {
        None
    };
    let Some(source) = source else {
        return Ok(LocalReplica::empty(key));
    };
    let decrypt = |source: &Path| -> Result<LocalReplica, String> {
        let envelope: ReplicaEnvelope =
            serde_json::from_slice(&fs::read(source).map_err(|error| error.to_string())?)
                .map_err(|error| format!("桌面同步元数据格式损坏：{error}"))?;
        if envelope.version != REPLICA_ENVELOPE_VERSION {
            return Err("桌面同步元数据版本不受支持".into());
        }
        let nonce: [u8; REPLICA_NONCE_LENGTH] = STANDARD
            .decode(envelope.nonce)
            .map_err(|error| error.to_string())?
            .try_into()
            .map_err(|_| "桌面同步元数据随机数长度无效".to_owned())?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|error| error.to_string())?;
        let plaintext = Aes256Gcm::new_from_slice(&replica_encryption_key(key))
            .map_err(|error| error.to_string())?
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| "桌面同步元数据无法通过根密钥认证".to_owned())?;
        let replica: LocalReplica =
            serde_json::from_slice(&plaintext).map_err(|error| error.to_string())?;
        if replica.version != REPLICA_VERSION {
            return Err("桌面同步副本版本不受支持".into());
        }
        if replica.workspace_id != key.workspace_id() {
            return Err("桌面同步副本属于另一个工作区".into());
        }
        Ok(replica)
    };
    match decrypt(source) {
        Ok(replica) => Ok(replica),
        Err(primary_error) if source == path && backup.exists() => {
            let replica = decrypt(&backup)
                .map_err(|backup_error| format!("{primary_error}；备份恢复失败：{backup_error}"))?;
            let _ = fs::copy(&backup, path);
            Ok(replica)
        }
        Err(error) => Err(error),
    }
}

/// 加密并以临时文件、上一版备份和主文件三段式替换桌面同步元数据。
fn persist_replica(path: &Path, key: &SyncKey, replica: &LocalReplica) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let plaintext = serde_json::to_vec(replica).map_err(|error| error.to_string())?;
    let mut nonce = [0_u8; REPLICA_NONCE_LENGTH];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = Aes256Gcm::new_from_slice(&replica_encryption_key(key))
        .map_err(|error| error.to_string())?
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|error| format!("无法加密桌面同步元数据：{error}"))?;
    let content = serde_json::to_vec(&ReplicaEnvelope {
        version: REPLICA_ENVELOPE_VERSION,
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
    .map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    let backup = path.with_extension("bak");
    fs::write(&temporary, content).map_err(|error| error.to_string())?;
    if backup.exists() {
        fs::remove_file(&backup).map_err(|error| error.to_string())?;
    }
    if path.exists() {
        fs::rename(path, &backup).map_err(|error| error.to_string())?;
    }
    if let Err(error) = fs::rename(&temporary, path) {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(error.to_string());
    }
    Ok(())
}

/// 返回主文件、临时文件和备份文件路径，供断开设备时完整清理。
fn replica_related_paths(path: &Path) -> [PathBuf; 3] {
    [
        path.to_path_buf(),
        path.with_extension("tmp"),
        path.with_extension("bak"),
    ]
}

/// 构造记忆实体标识。
fn memory_entity_id(id: &str) -> String {
    format!("memory:{id}")
}

/// 构造集合实体标识。
fn collection_entity_id(id: &str) -> String {
    format!("collection:{id}")
}

/// 构造集合成员关系实体标识。
fn membership_entity_id(collection_id: &str, memory_id: &str) -> String {
    format!("membership:{collection_id}:{memory_id}")
}

/// 从成员关系实体标识恢复集合和记忆 UUID。
fn parse_membership_entity_id(entity_id: &str) -> Result<(Uuid, Uuid), String> {
    let value = entity_id
        .strip_prefix("membership:")
        .ok_or_else(|| "成员关系实体标识前缀无效".to_owned())?;
    let (collection_id, memory_id) = value
        .split_once(':')
        .ok_or_else(|| "成员关系实体标识格式无效".to_owned())?;
    Ok((
        parse_uuid(collection_id, "集合")?,
        parse_uuid(memory_id, "记忆")?,
    ))
}

/// 解析同步载荷中的 UUID 并保留字段语义。
fn parse_uuid(value: &str, field: &str) -> Result<Uuid, String> {
    Uuid::parse_str(value).map_err(|error| format!("同步{field}标识无效：{error}"))
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

/// 校验并规范化桌面 Relay 端点；发布构建只允许 HTTPS。
fn normalize_relay_endpoint(endpoint: &str) -> Result<String, String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    let url = reqwest::Url::parse(endpoint).map_err(|_| "Relay 地址不是有效 URL".to_owned())?;
    if url.host_str().is_none() {
        return Err("Relay 地址缺少主机名".into());
    }
    if url.scheme() != "https" && !(cfg!(debug_assertions) && url.scheme() == "http") {
        return Err("发布版本的 Relay 必须使用 HTTPS".into());
    }
    Ok(endpoint.into())
}

/// 拼接去除尾斜线后的中继端点与稳定路径。
fn relay_url(endpoint: &str, path: &str) -> Result<String, String> {
    let endpoint = endpoint.trim().trim_end_matches('/');
    if endpoint.is_empty() {
        return Err("请先配置 E2E Relay 地址".into());
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

/// 校验设备展示名称，避免控制字符进入中继审计元数据。
fn validate_device_name(device_name: &str) -> Result<(), String> {
    let name = device_name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err("设备名称长度必须为 1 到 80 个字符且不能包含控制字符".into());
    }
    Ok(())
}

/// 生成不含用户信息的桌面设备标识。
fn new_device_id() -> String {
    format!("desktop-{}", Uuid::now_v7().simple())
}

/// 返回签名证明使用的 Unix 毫秒时间。
fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use nexus_core::{
        CollectionPatch, ContentFormat, IngestInput, Ingestor, MemoryKind, MemoryPatch,
    };

    use super::*;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<HashMap<String, Vec<u8>>>,
    }

    impl SecretStore for MemorySecretStore {
        fn load(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
            self.values
                .lock()
                .map(|values| values.get(key).cloned())
                .map_err(|_| "测试安全存储不可用".into())
        }

        fn store(&self, key: &str, value: &[u8]) -> Result<(), String> {
            self.values
                .lock()
                .map_err(|_| "测试安全存储不可用".to_owned())?
                .insert(key.into(), value.into());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<(), String> {
            self.values
                .lock()
                .map_err(|_| "测试安全存储不可用".to_owned())?
                .remove(key);
            Ok(())
        }
    }

    /// 验证桌面 E2E 身份只进入安全存储，公开状态不包含根密钥或私钥。
    #[test]
    fn persists_identity_without_exposing_secret_material() {
        let secrets = Arc::new(MemorySecretStore::default());
        let client = DesktopSync::with_secret_store(
            "https://relay.example.com".into(),
            true,
            secrets.clone(),
        );
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("desktop-test").unwrap();
        client.persist_current_identity(&key, &identity).unwrap();

        let status = client.status().unwrap();
        assert!(status.configured);
        assert_eq!(status.workspace_id, Some(key.workspace_id()));
        assert_eq!(status.device_id.as_deref(), Some("desktop-test"));
        let public = serde_json::to_string(&status).unwrap();
        assert!(!public.contains(&STANDARD.encode(key.to_bytes())));
        assert!(!public.contains(&STANDARD.encode(identity.pkcs8_bytes())));
    }

    /// 验证断开桌面同步会清除令牌、根密钥、身份和未完成配对材料。
    #[test]
    fn clears_all_desktop_sync_credentials() {
        let secrets = Arc::new(MemorySecretStore::default());
        for key in [
            ACCESS_TOKEN_STORAGE_KEY,
            ROOT_KEY_STORAGE_KEY,
            DEVICE_ID_STORAGE_KEY,
            DEVICE_IDENTITY_STORAGE_KEY,
            OUTGOING_PAIRING_STORAGE_KEY,
            PENDING_JOIN_STORAGE_KEY,
        ] {
            secrets.store(key, b"secret").unwrap();
        }
        let client = DesktopSync::with_secret_store(
            "https://relay.example.com".into(),
            true,
            secrets.clone(),
        );
        client.clear().unwrap();
        assert!(!client.has_access_token());
        for key in [
            ACCESS_TOKEN_STORAGE_KEY,
            ROOT_KEY_STORAGE_KEY,
            DEVICE_ID_STORAGE_KEY,
            DEVICE_IDENTITY_STORAGE_KEY,
            OUTGOING_PAIRING_STORAGE_KEY,
            PENDING_JOIN_STORAGE_KEY,
        ] {
            assert!(secrets.load(key).unwrap().is_none());
        }
    }

    /// 验证发布规则下的 Relay 地址规范化不会接受无主机或非 HTTP(S) 输入。
    #[test]
    fn normalizes_desktop_relay_endpoint() {
        assert_eq!(
            normalize_relay_endpoint("https://relay.example.com/").unwrap(),
            "https://relay.example.com"
        );
        assert!(normalize_relay_endpoint("not-a-url").is_err());
        assert!(normalize_relay_endpoint("file:///tmp/relay").is_err());
    }

    /// 验证桌面副本文件不包含内容明文，并能在连续替换后恢复待上传操作。
    #[test]
    fn encrypts_and_restores_desktop_replica() {
        let directory = tempfile::tempdir().expect("应创建临时目录");
        let path = directory.path().join("desktop-sync-replica.enc");
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("desktop-encryption").unwrap();
        let mut replica = LocalReplica::empty(&key);
        queue_entity_change(
            &mut replica,
            &key,
            &identity,
            memory_entity_id("0198f11d-3cc0-7bd0-8000-000000000001"),
            Some(SyncEntity::Memory(MemorySummary {
                id: "0198f11d-3cc0-7bd0-8000-000000000001".into(),
                source: "orbit".into(),
                kind: "note".into(),
                title: Some("密文测试".into()),
                content: "desktop replica plaintext must stay hidden".into(),
                content_format: "markdown".into(),
                tags: vec!["sync".into()],
                pinned: false,
                archived: false,
                created_at: 1_700_000_000_000,
                updated_at: 1_700_000_000_000,
                captured_at: None,
                links: Vec::new(),
                conflict_count: 0,
            })),
        )
        .unwrap();
        persist_replica(&path, &key, &replica).unwrap();
        replica.last_sync_at = Some(1_700_000_001_000);
        persist_replica(&path, &key, &replica).unwrap();

        let encoded = fs::read_to_string(&path).unwrap();
        assert!(!encoded.contains("desktop replica plaintext"));
        let restored = load_replica(&path, &key).unwrap();
        assert_eq!(restored.pending.len(), 1);
        assert_eq!(restored.last_sync_at, replica.last_sync_at);
        assert!(path.with_extension("bak").exists());
    }

    /// 验证远端写回产生的核心事件会被防回环标记消费，不会生成第二条上传操作。
    #[tokio::test]
    async fn suppresses_remote_writeback_event() {
        let directory = tempfile::tempdir().expect("应创建临时目录");
        let path = directory.path().join("desktop-sync-replica.enc");
        let secrets = Arc::new(MemorySecretStore::default());
        let client =
            DesktopSync::with_secret_store("https://relay.example.com".into(), true, secrets);
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("desktop-local").unwrap();
        client.persist_current_identity(&key, &identity).unwrap();
        let store = Arc::new(MemoryStore::open_in_memory().unwrap());
        let embedder = Arc::new(HashEmbedder::default());
        client
            .attach_store(Arc::clone(&store), embedder, path.clone())
            .unwrap();
        persist_replica(&path, &key, &LocalReplica::empty(&key)).unwrap();
        let id = Uuid::now_v7();
        let entity_id = memory_entity_id(&id.to_string());
        let mut version = VersionVector::default();
        version.observe("android-a", 1).unwrap();
        let record = VersionedRecord {
            value: Some(SyncEntity::Memory(MemorySummary {
                id: id.to_string(),
                source: "orbit".into(),
                kind: "note".into(),
                title: Some("Android 写回".into()),
                content: "只应写回一次".into(),
                content_format: "markdown".into(),
                tags: Vec::new(),
                pinned: false,
                archived: false,
                created_at: 1_700_000_000_000,
                updated_at: 1_700_000_000_100,
                captured_at: None,
                links: Vec::new(),
                conflict_count: 0,
            })),
            version,
            device_id: "android-a".into(),
            modified_at: 1_700_000_000_100,
            conflicts: Vec::new(),
        };
        let context = client.store_context().unwrap();
        client
            .apply_record_to_store(&context, &entity_id, &record)
            .unwrap();
        assert!(
            !client
                .handle_local_event(CoreEvent::MemoryCreated {
                    id,
                    source: "orbit".into(),
                })
                .await
                .unwrap()
        );
        assert_eq!(client.content_status().await.unwrap().pending_changes, 0);
        assert_eq!(store.get(&id).unwrap().unwrap().content, "只应写回一次");
    }

    /// 验证桌面本地提交会生成 Android 可解密结构的签名待上传信封。
    #[tokio::test]
    async fn queues_committed_desktop_memory() {
        let directory = tempfile::tempdir().expect("应创建临时目录");
        let path = directory.path().join("desktop-sync-replica.enc");
        let secrets = Arc::new(MemorySecretStore::default());
        let client =
            DesktopSync::with_secret_store("https://relay.example.com".into(), true, secrets);
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("desktop-queue").unwrap();
        client.persist_current_identity(&key, &identity).unwrap();
        let store = Arc::new(MemoryStore::open_in_memory().unwrap());
        let embedder = Arc::new(HashEmbedder::default());
        client
            .attach_store(Arc::clone(&store), Arc::clone(&embedder), path.clone())
            .unwrap();
        persist_replica(&path, &key, &LocalReplica::empty(&key)).unwrap();
        let memory = Ingestor::new(store.as_ref(), embedder.as_ref())
            .ingest(IngestInput {
                source: MemorySource::Orbit,
                kind: MemoryKind::Note,
                title: Some("桌面提交".into()),
                content: "desktop encrypted upload".into(),
                content_format: ContentFormat::Markdown,
                tags: vec!["m5".into()],
                captured_at: None,
                device_id: "orbit-desktop".into(),
                meta: serde_json::json!({}),
            })
            .unwrap();
        assert!(
            client
                .handle_local_event(CoreEvent::MemoryCreated {
                    id: memory.id,
                    source: "orbit".into(),
                })
                .await
                .unwrap()
        );
        let replica = load_replica(&path, &key).unwrap();
        assert_eq!(replica.pending.len(), 1);
        let operation = key
            .decrypt_operation(&replica.pending[0], identity.public_key())
            .unwrap();
        let entity: SyncEntity = serde_json::from_value(operation.payload.unwrap()).unwrap();
        assert!(matches!(
            entity,
            SyncEntity::Memory(MemorySummary { content, .. })
                if content == "desktop encrypted upload"
        ));
    }

    /// 验证两个桌面副本经真实 Relay 完成初始导入、集合成员、远端编辑和墓碑删除闭环。
    #[tokio::test]
    async fn synchronizes_two_desktop_stores_through_relay() {
        let token = "orbit-test-relay-token-with-at-least-32-characters";
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let relay = nexus_relay::RelayState::in_memory(token).unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, nexus_relay::router(relay))
                .await
                .unwrap();
        });

        let directory_a = tempfile::tempdir().unwrap();
        let secrets_a = Arc::new(MemorySecretStore::default());
        let client_a = DesktopSync::with_secret_store(String::new(), false, secrets_a);
        client_a.configure(&endpoint, token).await.unwrap();
        let store_a = Arc::new(MemoryStore::open_in_memory().unwrap());
        let embedder_a = Arc::new(HashEmbedder::default());
        let memory = Ingestor::new(store_a.as_ref(), embedder_a.as_ref())
            .ingest(IngestInput {
                source: MemorySource::Orbit,
                kind: MemoryKind::Note,
                title: Some("跨设备".into()),
                content: "来自桌面 A".into(),
                content_format: ContentFormat::Markdown,
                tags: vec!["relay".into()],
                captured_at: None,
                device_id: "desktop-a".into(),
                meta: serde_json::json!({}),
            })
            .unwrap();
        let collection = store_a
            .create_collection("同步集合", None, None, 0)
            .unwrap();
        store_a
            .add_memory_to_collection(collection.id, memory.id)
            .unwrap();
        client_a
            .attach_store(
                Arc::clone(&store_a),
                Arc::clone(&embedder_a),
                directory_a.path().join("replica.enc"),
            )
            .unwrap();
        client_a.initialize("桌面 A").await.unwrap();
        let phrase = client_a.recovery_phrase().unwrap();

        let directory_b = tempfile::tempdir().unwrap();
        let secrets_b = Arc::new(MemorySecretStore::default());
        let client_b = DesktopSync::with_secret_store(String::new(), false, secrets_b);
        client_b.configure(&endpoint, token).await.unwrap();
        let store_b = Arc::new(MemoryStore::open_in_memory().unwrap());
        let embedder_b = Arc::new(HashEmbedder::default());
        client_b
            .attach_store(
                Arc::clone(&store_b),
                Arc::clone(&embedder_b),
                directory_b.path().join("replica.enc"),
            )
            .unwrap();
        client_b.restore(&phrase, "桌面 B").await.unwrap();

        assert_eq!(
            store_b.get(&memory.id).unwrap().unwrap().content,
            "来自桌面 A"
        );
        assert_eq!(
            store_b.list_collection_memory_ids(collection.id).unwrap(),
            vec![memory.id]
        );
        // 测试未运行真实事件订阅器，手工交付远端写回产生的三个核心事件以消费防回环标记。
        assert!(
            !client_b
                .handle_local_event(CoreEvent::MemoryCreated {
                    id: memory.id,
                    source: "orbit".into(),
                })
                .await
                .unwrap()
        );
        assert!(
            !client_b
                .handle_local_event(CoreEvent::CollectionCreated { id: collection.id })
                .await
                .unwrap()
        );
        assert!(
            !client_b
                .handle_local_event(CoreEvent::CollectionMembershipAdded {
                    collection_id: collection.id,
                    memory_id: memory.id,
                })
                .await
                .unwrap()
        );

        store_b
            .update(
                &memory.id,
                MemoryPatch {
                    title: Some(Some("B 已编辑".into())),
                    content: Some("来自桌面 B 的更新".into()),
                    ..Default::default()
                },
                embedder_b.as_ref(),
            )
            .unwrap();
        assert!(
            client_b
                .handle_local_event(CoreEvent::MemoryUpdated {
                    id: memory.id,
                    source: "orbit".into(),
                })
                .await
                .unwrap()
        );
        client_b.sync_content().await.unwrap();
        client_a.sync_content().await.unwrap();
        assert_eq!(
            store_a.get(&memory.id).unwrap().unwrap().content,
            "来自桌面 B 的更新"
        );

        store_b
            .update_collection(
                collection.id,
                CollectionPatch {
                    name: Some("B 集合".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(
            client_b
                .handle_local_event(CoreEvent::CollectionUpdated { id: collection.id })
                .await
                .unwrap()
        );
        client_b.sync_content().await.unwrap();
        client_a.sync_content().await.unwrap();
        assert_eq!(
            store_a.get_collection(collection.id).unwrap().unwrap().name,
            "B 集合"
        );

        store_b.delete(&memory.id).unwrap();
        assert!(
            client_b
                .handle_local_event(CoreEvent::MemoryDeleted {
                    id: memory.id,
                    source: "orbit".into(),
                })
                .await
                .unwrap()
        );
        assert_eq!(
            client_b.content_status().await.unwrap().pending_changes,
            2,
            "删除记忆应同时为已知集合成员关系生成墓碑"
        );
        client_b.sync_content().await.unwrap();
        client_a.sync_content().await.unwrap();
        assert!(store_a.get(&memory.id).unwrap().is_none());
        server.abort();
    }
}
