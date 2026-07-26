//! 本文件实现 Orbit 桌面端 E2E 中继配置、系统凭据库身份、恢复、配对与设备治理。

use std::{
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use keyring::Entry;
use nexus_sync::{DeviceIdentity, PairingOffer, SealedPairingKey, SyncKey};
use qrcode::{QrCode, render::svg};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

const CREDENTIAL_SERVICE: &str = "com.nexus.orbit.sync";
const ACCESS_TOKEN_STORAGE_KEY: &str = "relay-access-token";
const ROOT_KEY_STORAGE_KEY: &str = "e2e-root-key";
const DEVICE_ID_STORAGE_KEY: &str = "e2e-device-id";
const DEVICE_IDENTITY_STORAGE_KEY: &str = "e2e-device-identity";
const OUTGOING_PAIRING_STORAGE_KEY: &str = "e2e-outgoing-pairing";
const PENDING_JOIN_STORAGE_KEY: &str = "e2e-pending-join";

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
        }
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
}
