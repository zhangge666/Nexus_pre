//! 本文件实现 Nexus E2E 同步密钥、设备签名、加密操作信封、配对封装、恢复短语与版本向量合并。

use std::{cmp::Ordering, collections::BTreeMap, fmt};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use bip39::{Language, Mnemonic};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};
use ring::{
    rand::SystemRandom,
    signature::{ED25519, Ed25519KeyPair, KeyPair, UnparsedPublicKey},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::{Zeroize, ZeroizeOnDrop};

const SYNC_ENVELOPE_VERSION: u8 = 1;
const PAIRING_VERSION: u8 = 1;
const KEY_LENGTH: usize = 32;
const NONCE_LENGTH: usize = 24;

/// 表示同步密钥、签名、恢复短语或加密载荷处理失败。
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    /// 同步输入字段不满足稳定协议约束。
    #[error("同步输入无效: {0}")]
    InvalidInput(String),
    /// 设备签名无效或签名身份与操作不一致。
    #[error("设备签名验证失败")]
    InvalidSignature,
    /// 密文无法通过认证，通常表示密钥错误或数据遭到篡改。
    #[error("同步密文认证失败")]
    AuthenticationFailed,
    /// JSON 编解码失败。
    #[error("同步载荷编解码失败: {0}")]
    Serialization(String),
    /// BIP39 恢复短语无效。
    #[error("恢复短语无效: {0}")]
    RecoveryPhrase(String),
    /// 设备 Ed25519 密钥生成或解析失败。
    #[error("设备身份不可用: {0}")]
    DeviceIdentity(String),
}

/// 表示仅在受控边界内暴露并在释放时清零的 256 位同步根密钥。
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SyncKey([u8; KEY_LENGTH]);

impl SyncKey {
    /// 使用系统安全随机源生成新的同步根密钥。
    #[must_use]
    pub fn generate() -> Self {
        let mut key = [0_u8; KEY_LENGTH];
        OsRng.fill_bytes(&mut key);
        Self(key)
    }

    /// 从系统安全区解封的 32 字节数据恢复同步根密钥。
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LENGTH]) -> Self {
        Self(bytes)
    }

    /// 返回用于写入系统安全区的密钥副本；调用方不得记录或写入普通设置文件。
    #[must_use]
    pub const fn to_bytes(&self) -> [u8; KEY_LENGTH] {
        self.0
    }

    /// 将根密钥编码为带 BIP39 校验位的 24 词英文恢复短语。
    pub fn recovery_phrase(&self) -> Result<String, SyncError> {
        Mnemonic::from_entropy(&self.0)
            .map(|mnemonic| mnemonic.to_string())
            .map_err(|error| SyncError::RecoveryPhrase(error.to_string()))
    }

    /// 从 24 词 BIP39 恢复短语还原同步根密钥。
    pub fn from_recovery_phrase(phrase: &str) -> Result<Self, SyncError> {
        let mnemonic = Mnemonic::parse_in_normalized(Language::English, phrase.trim())
            .map_err(|error| SyncError::RecoveryPhrase(error.to_string()))?;
        let entropy = mnemonic.to_entropy();
        let key = entropy.try_into().map_err(|_| {
            SyncError::RecoveryPhrase("必须使用可还原 256 位密钥的 24 词短语".into())
        })?;
        Ok(Self(key))
    }

    /// 派生中继可见但不可逆的稳定工作区标识。
    #[must_use]
    pub fn workspace_id(&self) -> String {
        let digest = blake3::keyed_hash(&self.0, b"nexus-sync-workspace-v1");
        digest.to_hex()[..32].to_owned()
    }

    /// 将实体明文标识映射为中继仅可观察访问模式的不可逆键。
    #[must_use]
    pub fn entity_key(&self, entity_id: &str) -> String {
        blake3::keyed_hash(&self.0, entity_id.as_bytes())
            .to_hex()
            .to_string()
    }

    /// 返回由根密钥确定性派生的恢复签名公钥，中继据此验证恢复短语持有证明。
    pub fn recovery_public_key(&self) -> Result<String, SyncError> {
        let pair = self.recovery_key_pair()?;
        Ok(URL_SAFE_NO_PAD.encode(pair.public_key().as_ref()))
    }

    /// 使用根密钥派生的独立 Ed25519 身份签署恢复登记消息。
    pub fn sign_recovery_claim(&self, message: &[u8]) -> Result<String, SyncError> {
        let pair = self.recovery_key_pair()?;
        Ok(URL_SAFE_NO_PAD.encode(pair.sign(message).as_ref()))
    }

    /// 加密并签名一条同步操作，信封头只保留中继路由所需的不可逆元数据。
    pub fn encrypt_operation(
        &self,
        operation: &PlainSyncOperation,
        identity: &DeviceIdentity,
    ) -> Result<EncryptedSyncEnvelope, SyncError> {
        operation.validate()?;
        if operation.device_id != identity.device_id() {
            return Err(SyncError::InvalidInput(
                "操作设备标识与签名设备不一致".into(),
            ));
        }
        let workspace_id = self.workspace_id();
        let entity_key = self.entity_key(&operation.entity_id);
        let mut nonce = [0_u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce);
        let mut envelope = EncryptedSyncEnvelope {
            version: SYNC_ENVELOPE_VERSION,
            workspace_id,
            operation_id: operation.operation_id,
            device_id: operation.device_id.clone(),
            device_sequence: operation.device_sequence,
            entity_key,
            kind: operation.kind,
            created_at: operation.created_at,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: String::new(),
            signature: String::new(),
        };
        let aad = envelope.aad_bytes()?;
        let plaintext = serde_json::to_vec(operation)
            .map_err(|error| SyncError::Serialization(error.to_string()))?;
        let cipher = XChaCha20Poly1305::new((&self.0).into());
        envelope.ciphertext = URL_SAFE_NO_PAD.encode(
            cipher
                .encrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &plaintext,
                        aad: &aad,
                    },
                )
                .map_err(|_| SyncError::AuthenticationFailed)?,
        );
        envelope.signature = identity.sign(&envelope.signing_bytes()?);
        Ok(envelope)
    }

    /// 验证设备签名、认证并解密一条同步信封。
    pub fn decrypt_operation(
        &self,
        envelope: &EncryptedSyncEnvelope,
        public_key: &str,
    ) -> Result<PlainSyncOperation, SyncError> {
        envelope.validate()?;
        envelope.verify_signature(public_key)?;
        if envelope.workspace_id != self.workspace_id() {
            return Err(SyncError::AuthenticationFailed);
        }
        let nonce = decode_array::<NONCE_LENGTH>(&envelope.nonce, "同步随机数")?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(&envelope.ciphertext)
            .map_err(|error| SyncError::InvalidInput(format!("同步密文编码无效: {error}")))?;
        let plaintext = XChaCha20Poly1305::new((&self.0).into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &envelope.aad_bytes()?,
                },
            )
            .map_err(|_| SyncError::AuthenticationFailed)?;
        let operation: PlainSyncOperation = serde_json::from_slice(&plaintext)
            .map_err(|error| SyncError::Serialization(error.to_string()))?;
        operation.validate()?;
        if operation.operation_id != envelope.operation_id
            || operation.device_id != envelope.device_id
            || operation.device_sequence != envelope.device_sequence
            || operation.kind != envelope.kind
            || operation.created_at != envelope.created_at
            || self.entity_key(&operation.entity_id) != envelope.entity_key
        {
            return Err(SyncError::AuthenticationFailed);
        }
        Ok(operation)
    }

    /// 从根密钥经域分离 BLAKE3 派生恢复签名种子，不复用同步加密密钥字节。
    fn recovery_key_pair(&self) -> Result<Ed25519KeyPair, SyncError> {
        let seed = blake3::keyed_hash(&self.0, b"nexus-recovery-signing-v1");
        Ed25519KeyPair::from_seed_unchecked(seed.as_bytes())
            .map_err(|_| SyncError::DeviceIdentity("无法派生恢复签名身份".into()))
    }
}

impl fmt::Debug for SyncKey {
    /// 输出脱敏调试信息，避免根密钥进入日志。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SyncKey([REDACTED])")
    }
}

/// 表示具有稳定设备标识的 Ed25519 签名身份；私钥应封存在平台安全区。
pub struct DeviceIdentity {
    device_id: String,
    pkcs8: Vec<u8>,
    public_key: String,
}

impl DeviceIdentity {
    /// 生成新的设备签名身份。
    pub fn generate(device_id: impl Into<String>) -> Result<Self, SyncError> {
        let device_id = normalized_identifier(device_id.into(), "设备标识")?;
        let rng = SystemRandom::new();
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng)
            .map_err(|_| SyncError::DeviceIdentity("无法生成 Ed25519 私钥".into()))?;
        Self::from_pkcs8(device_id, pkcs8.as_ref().to_vec())
    }

    /// 从系统安全区保存的 PKCS#8 字节恢复设备签名身份。
    pub fn from_pkcs8(device_id: impl Into<String>, pkcs8: Vec<u8>) -> Result<Self, SyncError> {
        let device_id = normalized_identifier(device_id.into(), "设备标识")?;
        let pair = Ed25519KeyPair::from_pkcs8(&pkcs8)
            .map_err(|_| SyncError::DeviceIdentity("Ed25519 私钥格式无效".into()))?;
        let public_key = URL_SAFE_NO_PAD.encode(pair.public_key().as_ref());
        Ok(Self {
            device_id,
            pkcs8,
            public_key,
        })
    }

    /// 返回稳定设备标识。
    #[must_use]
    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    /// 返回可登记到中继的 Ed25519 公钥。
    #[must_use]
    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// 返回用于封存在系统安全区的 PKCS#8 私钥副本。
    #[must_use]
    pub fn pkcs8_bytes(&self) -> Vec<u8> {
        self.pkcs8.clone()
    }

    /// 对稳定协议字节签名并返回 URL-safe Base64。
    #[must_use]
    pub fn sign(&self, message: &[u8]) -> String {
        let pair = Ed25519KeyPair::from_pkcs8(&self.pkcs8)
            .expect("设备身份仅能由已校验的 PKCS#8 数据构造");
        URL_SAFE_NO_PAD.encode(pair.sign(message).as_ref())
    }
}

/// 使用登记的 Ed25519 公钥验证任意设备协议消息签名。
pub fn verify_device_signature(
    public_key: &str,
    message: &[u8],
    signature: &str,
) -> Result<(), SyncError> {
    let public_key = URL_SAFE_NO_PAD
        .decode(public_key)
        .map_err(|_| SyncError::InvalidSignature)?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| SyncError::InvalidSignature)?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(message, &signature)
        .map_err(|_| SyncError::InvalidSignature)
}

impl Drop for DeviceIdentity {
    /// 释放设备身份时清零 PKCS#8 私钥字节。
    fn drop(&mut self) {
        self.pkcs8.zeroize();
    }
}

impl fmt::Debug for DeviceIdentity {
    /// 调试输出只包含设备标识和公钥，不暴露私钥。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeviceIdentity")
            .field("device_id", &self.device_id)
            .field("public_key", &self.public_key)
            .field("pkcs8", &"[REDACTED]")
            .finish()
    }
}

/// 表示同步操作的写入或删除语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    /// 写入或替换实体内容。
    Upsert,
    /// 删除实体并阻止旧版本复活。
    Tombstone,
}

/// 表示仅在受信设备内存在的同步操作明文。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlainSyncOperation {
    /// 全局唯一操作标识。
    pub operation_id: Uuid,
    /// 明文实体标识，仅进入加密载荷。
    pub entity_id: String,
    /// 产生操作的稳定设备标识。
    pub device_id: String,
    /// 当前设备单调递增序号。
    pub device_sequence: u64,
    /// 操作携带的版本向量。
    pub version: VersionVector,
    /// 写入或墓碑语义。
    pub kind: OperationKind,
    /// 写入时的结构化明文；墓碑操作必须为空。
    pub payload: Option<serde_json::Value>,
    /// 设备记录的 Unix 毫秒时间。
    pub created_at: i64,
}

impl PlainSyncOperation {
    /// 校验操作标识、版本向量、序号与载荷语义。
    pub fn validate(&self) -> Result<(), SyncError> {
        normalized_identifier(self.entity_id.clone(), "实体标识")?;
        normalized_identifier(self.device_id.clone(), "设备标识")?;
        if self.device_sequence == 0 || self.version.get(&self.device_id) != self.device_sequence {
            return Err(SyncError::InvalidInput(
                "设备序号必须大于零并与版本向量一致".into(),
            ));
        }
        match (self.kind, &self.payload) {
            (OperationKind::Upsert, Some(_)) | (OperationKind::Tombstone, None) => Ok(()),
            (OperationKind::Upsert, None) => {
                Err(SyncError::InvalidInput("写入操作必须包含载荷".into()))
            }
            (OperationKind::Tombstone, Some(_)) => {
                Err(SyncError::InvalidInput("墓碑操作不得包含明文载荷".into()))
            }
        }
    }
}

/// 表示云中继可存储和转发的零知识同步信封。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncryptedSyncEnvelope {
    /// 信封格式版本。
    pub version: u8,
    /// 从根密钥派生的不可逆工作区标识。
    pub workspace_id: String,
    /// 全局唯一操作标识。
    pub operation_id: Uuid,
    /// 产生操作的设备标识。
    pub device_id: String,
    /// 当前设备单调递增序号。
    pub device_sequence: u64,
    /// 从根密钥与实体明文标识派生的不可逆键。
    pub entity_key: String,
    /// 写入或墓碑语义；中继据此执行密文替换和删除。
    pub kind: OperationKind,
    /// 设备记录的 Unix 毫秒时间。
    pub created_at: i64,
    /// URL-safe Base64 编码的 24 字节 XChaCha20 随机数。
    pub nonce: String,
    /// URL-safe Base64 编码的认证密文。
    pub ciphertext: String,
    /// 设备对完整信封头和密文的 Ed25519 签名。
    pub signature: String,
}

impl EncryptedSyncEnvelope {
    /// 验证信封版本、标识长度与二进制字段编码。
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.version != SYNC_ENVELOPE_VERSION {
            return Err(SyncError::InvalidInput("不支持的同步信封版本".into()));
        }
        normalized_identifier(self.workspace_id.clone(), "工作区标识")?;
        normalized_identifier(self.device_id.clone(), "设备标识")?;
        normalized_identifier(self.entity_key.clone(), "实体键")?;
        if self.device_sequence == 0 {
            return Err(SyncError::InvalidInput("设备序号必须大于零".into()));
        }
        decode_array::<NONCE_LENGTH>(&self.nonce, "同步随机数")?;
        if URL_SAFE_NO_PAD.decode(&self.ciphertext).is_err() {
            return Err(SyncError::InvalidInput("同步密文编码无效".into()));
        }
        if URL_SAFE_NO_PAD.decode(&self.signature).is_err() {
            return Err(SyncError::InvalidInput("设备签名编码无效".into()));
        }
        Ok(())
    }

    /// 使用登记的设备公钥验证信封签名，供零知识中继拒绝伪造操作。
    pub fn verify_signature(&self, public_key: &str) -> Result<(), SyncError> {
        verify_device_signature(public_key, &self.signing_bytes()?, &self.signature)
    }

    /// 构造认证加密附加数据，防止中继修改路由元数据。
    fn aad_bytes(&self) -> Result<Vec<u8>, SyncError> {
        serde_json::to_vec(&EnvelopeHeader {
            version: self.version,
            workspace_id: &self.workspace_id,
            operation_id: self.operation_id,
            device_id: &self.device_id,
            device_sequence: self.device_sequence,
            entity_key: &self.entity_key,
            kind: self.kind,
            created_at: self.created_at,
            nonce: &self.nonce,
        })
        .map_err(|error| SyncError::Serialization(error.to_string()))
    }

    /// 构造设备签名覆盖的完整稳定字节。
    fn signing_bytes(&self) -> Result<Vec<u8>, SyncError> {
        serde_json::to_vec(&SignedEnvelope {
            header: EnvelopeHeader {
                version: self.version,
                workspace_id: &self.workspace_id,
                operation_id: self.operation_id,
                device_id: &self.device_id,
                device_sequence: self.device_sequence,
                entity_key: &self.entity_key,
                kind: self.kind,
                created_at: self.created_at,
                nonce: &self.nonce,
            },
            ciphertext: &self.ciphertext,
        })
        .map_err(|error| SyncError::Serialization(error.to_string()))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeHeader<'a> {
    version: u8,
    workspace_id: &'a str,
    operation_id: Uuid,
    device_id: &'a str,
    device_sequence: u64,
    entity_key: &'a str,
    kind: OperationKind,
    created_at: i64,
    nonce: &'a str,
}

#[derive(Serialize)]
struct SignedEnvelope<'a> {
    header: EnvelopeHeader<'a>,
    ciphertext: &'a str,
}

/// 表示由已连接设备创建、仅通过二维码传递的 256 位一次性配对秘密。
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct PairingSecret([u8; KEY_LENGTH]);

impl PairingSecret {
    /// 生成新的高熵一次性配对秘密。
    #[must_use]
    pub fn generate() -> Self {
        let mut secret = [0_u8; KEY_LENGTH];
        OsRng.fill_bytes(&mut secret);
        Self(secret)
    }

    /// 从二维码解码的 32 字节数据恢复配对秘密。
    #[must_use]
    pub const fn from_bytes(bytes: [u8; KEY_LENGTH]) -> Self {
        Self(bytes)
    }

    /// 返回供二维码 URI 编码的 URL-safe Base64 值。
    #[must_use]
    pub fn encoded(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// 返回供两台设备人工核对的六位确认码；确认码不用于派生加密密钥。
    #[must_use]
    pub fn verification_code(&self) -> String {
        let digest = blake3::hash(&self.0);
        let value = u32::from_be_bytes(digest.as_bytes()[..4].try_into().unwrap()) % 1_000_000;
        format!("{value:06}")
    }

    /// 使用一次性秘密封装根密钥，仅目标设备和当前配对会话可以解封。
    pub fn seal_sync_key(
        &self,
        key: &SyncKey,
        offer: &PairingOffer,
        target_device_id: impl Into<String>,
    ) -> Result<SealedPairingKey, SyncError> {
        let target_device_id = normalized_identifier(target_device_id.into(), "目标设备标识")?;
        if offer.workspace_id != key.workspace_id() || offer.secret.0 != self.0 {
            return Err(SyncError::InvalidInput("配对邀请与同步密钥不匹配".into()));
        }
        let aad = pairing_aad(offer.session_id, &offer.workspace_id, &target_device_id);
        let mut nonce = [0_u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = XChaCha20Poly1305::new((&self.0).into())
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &key.0,
                    aad: &aad,
                },
            )
            .map_err(|_| SyncError::AuthenticationFailed)?;
        Ok(SealedPairingKey {
            version: PAIRING_VERSION,
            session_id: offer.session_id,
            workspace_id: offer.workspace_id.clone(),
            target_device_id,
            nonce: URL_SAFE_NO_PAD.encode(nonce),
            ciphertext: URL_SAFE_NO_PAD.encode(ciphertext),
        })
    }

    /// 解封配对包中的根密钥，并验证目标设备、会话和工作区。
    pub fn open_sync_key(
        &self,
        sealed: &SealedPairingKey,
        target_device_id: &str,
    ) -> Result<SyncKey, SyncError> {
        sealed.validate()?;
        if sealed.target_device_id != target_device_id {
            return Err(SyncError::AuthenticationFailed);
        }
        let nonce = decode_array::<NONCE_LENGTH>(&sealed.nonce, "配对随机数")?;
        let ciphertext = URL_SAFE_NO_PAD
            .decode(&sealed.ciphertext)
            .map_err(|_| SyncError::AuthenticationFailed)?;
        let plaintext = XChaCha20Poly1305::new((&self.0).into())
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &pairing_aad(
                        sealed.session_id,
                        &sealed.workspace_id,
                        &sealed.target_device_id,
                    ),
                },
            )
            .map_err(|_| SyncError::AuthenticationFailed)?;
        let key = plaintext
            .try_into()
            .map_err(|_| SyncError::AuthenticationFailed)?;
        let key = SyncKey::from_bytes(key);
        if key.workspace_id() != sealed.workspace_id {
            return Err(SyncError::AuthenticationFailed);
        }
        Ok(key)
    }
}

impl fmt::Debug for PairingSecret {
    /// 调试输出只显示确认码，不暴露一次性配对秘密。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PairingSecret")
            .field(&format!("[REDACTED:{}]", self.verification_code()))
            .finish()
    }
}

/// 表示通过二维码在设备间直接传递的配对邀请。
#[derive(Clone)]
pub struct PairingOffer {
    /// 一次性配对会话标识。
    pub session_id: Uuid,
    /// 从同步根密钥派生的不可逆工作区标识。
    pub workspace_id: String,
    /// 只进入二维码、永不上传中继的一次性秘密。
    pub secret: PairingSecret,
}

impl PairingOffer {
    /// 为指定同步工作区创建一次性配对邀请。
    #[must_use]
    pub fn create(key: &SyncKey) -> Self {
        Self {
            session_id: Uuid::now_v7(),
            workspace_id: key.workspace_id(),
            secret: PairingSecret::generate(),
        }
    }

    /// 编码可生成二维码的 `nexus://pair` URI。
    #[must_use]
    pub fn to_uri(&self) -> String {
        format!(
            "nexus://pair?version={PAIRING_VERSION}&session={}&workspace={}&secret={}",
            self.session_id,
            self.workspace_id,
            self.secret.encoded()
        )
    }

    /// 解析扫描到的配对 URI，并拒绝未知版本或异常字段。
    pub fn from_uri(uri: &str) -> Result<Self, SyncError> {
        let query = uri
            .strip_prefix("nexus://pair?")
            .ok_or_else(|| SyncError::InvalidInput("配对二维码协议无效".into()))?;
        let fields = query
            .split('&')
            .filter_map(|field| field.split_once('='))
            .collect::<BTreeMap<_, _>>();
        if fields.get("version").copied() != Some("1") {
            return Err(SyncError::InvalidInput("不支持的配对二维码版本".into()));
        }
        let session_id = fields
            .get("session")
            .ok_or_else(|| SyncError::InvalidInput("配对二维码缺少会话标识".into()))?
            .parse()
            .map_err(|_| SyncError::InvalidInput("配对会话标识无效".into()))?;
        let workspace_id = normalized_identifier(
            fields
                .get("workspace")
                .ok_or_else(|| SyncError::InvalidInput("配对二维码缺少工作区标识".into()))?
                .to_string(),
            "工作区标识",
        )?;
        let secret = decode_array::<KEY_LENGTH>(
            fields
                .get("secret")
                .ok_or_else(|| SyncError::InvalidInput("配对二维码缺少一次性秘密".into()))?,
            "配对秘密",
        )?;
        Ok(Self {
            session_id,
            workspace_id,
            secret: PairingSecret::from_bytes(secret),
        })
    }

    /// 返回供两台设备人工核对的六位确认码。
    #[must_use]
    pub fn verification_code(&self) -> String {
        self.secret.verification_code()
    }
}

impl fmt::Debug for PairingOffer {
    /// 调试输出隐藏二维码中的一次性秘密。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingOffer")
            .field("session_id", &self.session_id)
            .field("workspace_id", &self.workspace_id)
            .field("verification_code", &self.verification_code())
            .field("secret", &"[REDACTED]")
            .finish()
    }
}

/// 表示中继可以存储但无法解密的配对根密钥包。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SealedPairingKey {
    /// 配对包格式版本。
    pub version: u8,
    /// 一次性会话标识。
    pub session_id: Uuid,
    /// 目标同步工作区。
    pub workspace_id: String,
    /// 唯一允许解封的目标设备。
    pub target_device_id: String,
    /// URL-safe Base64 编码的 24 字节随机数。
    pub nonce: String,
    /// URL-safe Base64 编码的根密钥认证密文。
    pub ciphertext: String,
}

impl SealedPairingKey {
    /// 校验配对包版本、标识和二进制字段。
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.version != PAIRING_VERSION {
            return Err(SyncError::InvalidInput("不支持的配对包版本".into()));
        }
        normalized_identifier(self.workspace_id.clone(), "工作区标识")?;
        normalized_identifier(self.target_device_id.clone(), "目标设备标识")?;
        decode_array::<NONCE_LENGTH>(&self.nonce, "配对随机数")?;
        if URL_SAFE_NO_PAD.decode(&self.ciphertext).is_err() {
            return Err(SyncError::InvalidInput("配对密文编码无效".into()));
        }
        Ok(())
    }
}

/// 表示版本向量之间的因果关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorRelation {
    /// 两个向量完全相同。
    Same,
    /// 当前向量在另一个向量之前。
    Before,
    /// 当前向量在另一个向量之后。
    After,
    /// 两个向量包含并发修改。
    Concurrent,
}

/// 表示按设备记录的单调逻辑时钟。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct VersionVector(BTreeMap<String, u64>);

impl VersionVector {
    /// 返回指定设备的当前逻辑时钟，不存在时为零。
    #[must_use]
    pub fn get(&self, device_id: &str) -> u64 {
        self.0.get(device_id).copied().unwrap_or(0)
    }

    /// 将指定设备时钟递增并返回新值。
    pub fn increment(&mut self, device_id: impl Into<String>) -> Result<u64, SyncError> {
        let device_id = normalized_identifier(device_id.into(), "设备标识")?;
        let counter = self.0.entry(device_id).or_default();
        *counter = counter
            .checked_add(1)
            .ok_or_else(|| SyncError::InvalidInput("设备逻辑时钟已溢出".into()))?;
        Ok(*counter)
    }

    /// 记录设备已经产生的全局逻辑时钟，只允许单调前进并返回合并后的值。
    pub fn observe(
        &mut self,
        device_id: impl Into<String>,
        counter: u64,
    ) -> Result<u64, SyncError> {
        let device_id = normalized_identifier(device_id.into(), "设备标识")?;
        if counter == 0 {
            return Err(SyncError::InvalidInput("设备逻辑时钟必须大于零".into()));
        }
        let current = self.0.entry(device_id).or_default();
        *current = (*current).max(counter);
        Ok(*current)
    }

    /// 逐设备取最大值合并另一个版本向量。
    pub fn merge(&mut self, other: &Self) {
        for (device, counter) in &other.0 {
            self.0
                .entry(device.clone())
                .and_modify(|current| *current = (*current).max(*counter))
                .or_insert(*counter);
        }
    }

    /// 比较两个版本向量的因果关系。
    #[must_use]
    pub fn relation(&self, other: &Self) -> VectorRelation {
        let mut less = false;
        let mut greater = false;
        for device in self.0.keys().chain(other.0.keys()) {
            match self.get(device).cmp(&other.get(device)) {
                Ordering::Less => less = true,
                Ordering::Greater => greater = true,
                Ordering::Equal => {}
            }
        }
        match (less, greater) {
            (false, false) => VectorRelation::Same,
            (true, false) => VectorRelation::Before,
            (false, true) => VectorRelation::After,
            (true, true) => VectorRelation::Concurrent,
        }
    }
}

/// 表示一个包含版本向量、删除状态和冲突留痕的实体快照。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", bound(deserialize = "T: Deserialize<'de>"))]
pub struct VersionedRecord<T> {
    /// 当前可见值；`None` 表示墓碑。
    pub value: Option<T>,
    /// 当前快照已经观察到的全部设备时钟。
    pub version: VersionVector,
    /// 当前胜出版本来源设备。
    pub device_id: String,
    /// 当前胜出版本的 Unix 毫秒时间。
    pub modified_at: i64,
    /// 并发失败版本，供用户恢复或手工合并。
    #[serde(default)]
    pub conflicts: Vec<ConflictVersion<T>>,
}

/// 表示一次并发合并中保留的未胜出版本。
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictVersion<T> {
    /// 未胜出值；`None` 表示并发墓碑。
    pub value: Option<T>,
    /// 未胜出版本来源设备。
    pub device_id: String,
    /// 未胜出版本时间。
    pub modified_at: i64,
}

/// 表示确定性合并结果及是否发生并发冲突。
#[derive(Debug, Clone, PartialEq)]
pub struct MergeResult<T> {
    /// 所有设备按同一规则计算出的收敛记录。
    pub record: VersionedRecord<T>,
    /// 本次合并是否观察到并发版本。
    pub had_conflict: bool,
}

impl<T: Clone + PartialEq + Serialize> VersionedRecord<T> {
    /// 依据版本向量合并两份记录；并发删除优先，否则按时间与设备标识确定胜者并保留败者。
    #[must_use]
    pub fn merge(self, other: Self) -> MergeResult<T> {
        match self.version.relation(&other.version) {
            VectorRelation::Same if self == other => MergeResult {
                record: self,
                had_conflict: false,
            },
            VectorRelation::Same | VectorRelation::Concurrent => {
                merge_concurrent_records(self, other)
            }
            VectorRelation::After => MergeResult {
                record: self,
                had_conflict: false,
            },
            VectorRelation::Before => MergeResult {
                record: other,
                had_conflict: false,
            },
        }
    }
}

/// 使用与输入顺序无关的稳定排序合并并发或异常同版本记录。
fn merge_concurrent_records<T: Clone + PartialEq + Serialize>(
    left: VersionedRecord<T>,
    right: VersionedRecord<T>,
) -> MergeResult<T> {
    let left_key = record_order_key(&left);
    let right_key = record_order_key(&right);
    let (mut winner, loser) = if left_key >= right_key {
        (left, right)
    } else {
        (right, left)
    };
    winner.version.merge(&loser.version);
    winner.conflicts.extend(loser.conflicts);
    if winner.value != loser.value {
        winner.conflicts.push(ConflictVersion {
            value: loser.value,
            device_id: loser.device_id,
            modified_at: loser.modified_at,
        });
    }
    winner.conflicts.sort_by(|left, right| {
        conflict_order_key(left)
            .cmp(&conflict_order_key(right))
            .reverse()
    });
    winner.conflicts.dedup_by(|left, right| left == right);
    MergeResult {
        record: winner,
        had_conflict: true,
    }
}

/// 构造并发记录稳定排序键；墓碑优先，随后按时间、设备和内容哈希决胜。
fn record_order_key<T: Serialize>(record: &VersionedRecord<T>) -> (bool, i64, &str, String) {
    (
        record.value.is_none(),
        record.modified_at,
        &record.device_id,
        serialized_hash(&record.value),
    )
}

/// 构造冲突留痕稳定排序键，使双向合并得到相同顺序。
fn conflict_order_key<T: Serialize>(conflict: &ConflictVersion<T>) -> (bool, i64, &str, String) {
    (
        conflict.value.is_none(),
        conflict.modified_at,
        &conflict.device_id,
        serialized_hash(&conflict.value),
    )
}

/// 返回结构化值的稳定 BLAKE3 哈希，避免把明文放入排序元数据。
fn serialized_hash<T: Serialize>(value: &Option<T>) -> String {
    serde_json::to_vec(value)
        .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
        .unwrap_or_default()
}

/// 规范化用户或设备提供的稳定标识，拒绝空白、超长和控制字符。
fn normalized_identifier(value: String, field: &str) -> Result<String, SyncError> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(SyncError::InvalidInput(format!("{field}为空或格式无效")));
    }
    Ok(value.to_owned())
}

/// 解码 URL-safe Base64 定长数组。
fn decode_array<const N: usize>(value: &str, field: &str) -> Result<[u8; N], SyncError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .map_err(|error| SyncError::InvalidInput(format!("{field}编码无效: {error}")))?
        .try_into()
        .map_err(|_| SyncError::InvalidInput(format!("{field}长度无效")))
}

/// 构造将根密钥绑定到会话、工作区和目标设备的认证附加数据。
fn pairing_aad(session_id: Uuid, workspace_id: &str, target_device_id: &str) -> Vec<u8> {
    format!("nexus-pair-v1:{session_id}:{workspace_id}:{target_device_id}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建测试操作并确保版本向量与设备序号一致。
    fn operation(identity: &DeviceIdentity, entity_id: &str) -> PlainSyncOperation {
        let mut version = VersionVector::default();
        let device_sequence = version.increment(identity.device_id()).unwrap();
        PlainSyncOperation {
            operation_id: Uuid::now_v7(),
            entity_id: entity_id.into(),
            device_id: identity.device_id().into(),
            device_sequence,
            version,
            kind: OperationKind::Upsert,
            payload: Some(serde_json::json!({"content": "仅设备可见"})),
            created_at: 1_700_000_000_000,
        }
    }

    /// 验证 24 词恢复短语能无损还原 256 位同步密钥。
    #[test]
    fn recovery_phrase_round_trip() {
        let key = SyncKey::generate();
        let phrase = key.recovery_phrase().unwrap();
        assert_eq!(phrase.split_whitespace().count(), 24);
        let recovered = SyncKey::from_recovery_phrase(&phrase).unwrap();
        assert_eq!(recovered.workspace_id(), key.workspace_id());
        assert_eq!(recovered.to_bytes(), key.to_bytes());
        let message = b"recovery-claim";
        verify_device_signature(
            &recovered.recovery_public_key().unwrap(),
            message,
            &key.sign_recovery_claim(message).unwrap(),
        )
        .unwrap();
    }

    /// 验证中继信封不包含明文，并能被登记设备公钥和根密钥认证解密。
    #[test]
    fn encrypted_envelope_round_trip() {
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("android-a").unwrap();
        let operation = operation(&identity, "memory-secret-id");
        let envelope = key.encrypt_operation(&operation, &identity).unwrap();
        let encoded = serde_json::to_string(&envelope).unwrap();
        assert!(!encoded.contains("仅设备可见"));
        assert!(!encoded.contains("memory-secret-id"));
        assert_eq!(
            key.decrypt_operation(&envelope, identity.public_key())
                .unwrap(),
            operation
        );
    }

    /// 验证中继元数据或密文被篡改后设备签名会拒绝该信封。
    #[test]
    fn rejects_tampered_envelope() {
        let key = SyncKey::generate();
        let identity = DeviceIdentity::generate("android-a").unwrap();
        let mut envelope = key
            .encrypt_operation(&operation(&identity, "memory-a"), &identity)
            .unwrap();
        envelope.device_sequence += 1;
        assert!(matches!(
            envelope.verify_signature(identity.public_key()),
            Err(SyncError::InvalidSignature)
        ));
    }

    /// 验证配对二维码秘密不经过中继也能为目标设备封装和解封根密钥。
    #[test]
    fn pairing_offer_seals_root_key() {
        let key = SyncKey::generate();
        let offer = PairingOffer::create(&key);
        let parsed = PairingOffer::from_uri(&offer.to_uri()).unwrap();
        assert_eq!(parsed.verification_code(), offer.verification_code());
        let sealed = offer
            .secret
            .seal_sync_key(&key, &offer, "android-new")
            .unwrap();
        let opened = parsed.secret.open_sync_key(&sealed, "android-new").unwrap();
        assert_eq!(opened.to_bytes(), key.to_bytes());
    }

    /// 验证版本向量能区分因果先后与并发修改。
    #[test]
    fn compares_version_vectors() {
        let mut left = VersionVector::default();
        left.increment("device-a").unwrap();
        let mut after = left.clone();
        after.increment("device-a").unwrap();
        let mut concurrent = left.clone();
        concurrent.increment("device-b").unwrap();
        assert_eq!(left.relation(&after), VectorRelation::Before);
        assert_eq!(after.relation(&left), VectorRelation::After);
        assert_eq!(after.relation(&concurrent), VectorRelation::Concurrent);
    }

    /// 验证跨实体共用的设备全局序号可以直接写入版本向量且不会倒退。
    #[test]
    fn observes_global_device_sequence_monotonically() {
        let mut version = VersionVector::default();
        assert_eq!(version.observe("device-a", 7).unwrap(), 7);
        assert_eq!(version.observe("device-a", 3).unwrap(), 7);
        assert_eq!(version.get("device-a"), 7);
        assert!(version.observe("device-a", 0).is_err());
    }

    /// 验证并发墓碑阻止旧内容复活，同时保留内容冲突供用户恢复。
    #[test]
    fn concurrent_tombstone_wins_and_keeps_conflict() {
        let mut left_version = VersionVector::default();
        left_version.increment("device-a").unwrap();
        let mut right_version = VersionVector::default();
        right_version.increment("device-b").unwrap();
        let content = VersionedRecord {
            value: Some("正文".to_owned()),
            version: left_version,
            device_id: "device-a".into(),
            modified_at: 20,
            conflicts: Vec::new(),
        };
        let tombstone = VersionedRecord {
            value: None,
            version: right_version,
            device_id: "device-b".into(),
            modified_at: 10,
            conflicts: Vec::new(),
        };
        let result = content.merge(tombstone);
        assert!(result.had_conflict);
        assert!(result.record.value.is_none());
        assert_eq!(result.record.conflicts[0].value.as_deref(), Some("正文"));
    }

    /// 验证并发合并与输入顺序无关，所有设备都能得到相同胜者和冲突顺序。
    #[test]
    fn concurrent_merge_is_commutative() {
        let mut left_version = VersionVector::default();
        left_version.increment("device-a").unwrap();
        let mut right_version = VersionVector::default();
        right_version.increment("device-b").unwrap();
        let left = VersionedRecord {
            value: Some("版本 A".to_owned()),
            version: left_version,
            device_id: "device-a".into(),
            modified_at: 20,
            conflicts: Vec::new(),
        };
        let right = VersionedRecord {
            value: Some("版本 B".to_owned()),
            version: right_version,
            device_id: "device-b".into(),
            modified_at: 20,
            conflicts: Vec::new(),
        };
        assert_eq!(
            left.clone().merge(right.clone()).record,
            right.merge(left).record
        );
    }
}
