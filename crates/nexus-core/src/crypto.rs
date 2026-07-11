//! 本文件实现 Argon2id 密钥派生、分块 XChaCha20-Poly1305 加密和内容寻址媒体仓库。

use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use argon2::Argon2;
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand_core::{OsRng, RngCore};
use zeroize::{Zeroize, ZeroizeOnDrop};

const MAGIC: &[u8; 8] = b"NXMEDIA1";
const DEFAULT_CHUNK_SIZE: usize = 64 * 1024;
const HEADER_SIZE: usize = 8 + 4 + 8 + 4 + 16;

/// 表示密钥派生、认证加密或媒体文件操作错误。
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// Argon2id 参数或派生过程失败。
    #[error("主密钥派生失败: {0}")]
    KeyDerivation(String),
    /// 密文认证失败，通常意味着密钥错误或数据遭到篡改。
    #[error("密文认证失败")]
    AuthenticationFailed,
    /// 加密媒体容器头、块长度或内容长度无效。
    #[error("加密媒体格式无效: {0}")]
    InvalidFormat(String),
    /// 解密后的内容与引用中的 BLAKE3 哈希不一致。
    #[error("媒体内容哈希校验失败")]
    HashMismatch,
    /// 本地媒体目录或文件操作失败。
    #[error("媒体文件操作失败: {0}")]
    Io(#[from] std::io::Error),
}

/// 表示仅在内存中使用并在释放时自动清零的 256 位主密钥。
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct MasterKey([u8; 32]);

impl MasterKey {
    /// 使用 Argon2id 从用户主密码和至少 8 字节随机盐派生主密钥。
    pub fn derive(password: &[u8], salt: &[u8]) -> Result<Self, CryptoError> {
        if salt.len() < 8 {
            return Err(CryptoError::KeyDerivation("盐长度不能少于 8 字节".into()));
        }
        let mut key = [0_u8; 32];
        Argon2::default()
            .hash_password_into(password, salt, &mut key)
            .map_err(|error| CryptoError::KeyDerivation(error.to_string()))?;
        Ok(Self(key))
    }

    /// 从系统安全区解封的 32 字节数据恢复主密钥。
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// 使用主密钥将明文编码为版本化分块加密容器。
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        encrypt_chunks(&self.0, plaintext, DEFAULT_CHUNK_SIZE)
    }

    /// 使用主密钥解密并认证版本化分块媒体容器。
    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
        decrypt_chunks(&self.0, ciphertext)
    }
}

impl fmt::Debug for MasterKey {
    /// 输出脱敏调试信息，禁止意外记录密钥字节。
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MasterKey([REDACTED])")
    }
}

/// 表示加密媒体文件在内容寻址仓库中的引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedMediaRef {
    /// 明文内容的 BLAKE3 十六进制哈希。
    pub hash: String,
    /// 加密文件的本地绝对路径。
    pub path: PathBuf,
    /// 原始 MIME 类型。
    pub mime: String,
    /// 原始明文字节数。
    pub size: u64,
}

/// 管理独立于 SQLite 的加密大媒体文件。
pub struct MediaVault {
    root: PathBuf,
    key: MasterKey,
}

impl MediaVault {
    /// 创建媒体仓库目录并接管运行时主密钥。
    pub fn open(root: impl AsRef<Path>, key: MasterKey) -> Result<Self, CryptoError> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().to_path_buf(),
            key,
        })
    }

    /// 按明文哈希去重并以分块认证密文写入本地媒体目录。
    pub fn put(
        &self,
        plaintext: &[u8],
        mime: impl Into<String>,
    ) -> Result<EncryptedMediaRef, CryptoError> {
        let hash = blake3::hash(plaintext).to_hex().to_string();
        let path = self.root.join(format!("{hash}.nxm"));
        if !path.exists() {
            let encrypted = self.key.encrypt(plaintext)?;
            let temporary = self.root.join(format!(".{hash}.tmp"));
            fs::write(&temporary, encrypted)?;
            fs::rename(temporary, &path)?;
        }
        Ok(EncryptedMediaRef {
            hash,
            path,
            mime: mime.into(),
            size: u64::try_from(plaintext.len())
                .map_err(|_| CryptoError::InvalidFormat("媒体长度超过支持范围".into()))?,
        })
    }

    /// 读取、认证并校验指定媒体引用的明文内容。
    pub fn read(&self, media: &EncryptedMediaRef) -> Result<Vec<u8>, CryptoError> {
        let encrypted = fs::read(&media.path)?;
        let plaintext = self.key.decrypt(&encrypted)?;
        if blake3::hash(&plaintext).to_hex().as_str() != media.hash {
            return Err(CryptoError::HashMismatch);
        }
        Ok(plaintext)
    }

    /// 删除指定加密媒体；文件原本不存在时返回 `false`。
    pub fn delete(&self, media: &EncryptedMediaRef) -> Result<bool, CryptoError> {
        match fs::remove_file(&media.path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

/// 将明文按固定块大小加密，并为每块生成唯一 XChaCha20 nonce。
fn encrypt_chunks(
    key: &[u8; 32],
    plaintext: &[u8],
    chunk_size: usize,
) -> Result<Vec<u8>, CryptoError> {
    let chunk_count = plaintext.len().div_ceil(chunk_size).max(1);
    let chunk_count_u32 = u32::try_from(chunk_count)
        .map_err(|_| CryptoError::InvalidFormat("媒体块数量超过支持范围".into()))?;
    let original_len = u64::try_from(plaintext.len())
        .map_err(|_| CryptoError::InvalidFormat("媒体长度超过支持范围".into()))?;
    let mut nonce_prefix = [0_u8; 16];
    OsRng.fill_bytes(&mut nonce_prefix);

    let cipher = XChaCha20Poly1305::new(key.into());
    let mut output = Vec::with_capacity(HEADER_SIZE + plaintext.len() + chunk_count * 20);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&(chunk_size as u32).to_le_bytes());
    output.extend_from_slice(&original_len.to_le_bytes());
    output.extend_from_slice(&chunk_count_u32.to_le_bytes());
    output.extend_from_slice(&nonce_prefix);

    for index in 0..chunk_count {
        let start = index * chunk_size;
        let end = (start + chunk_size).min(plaintext.len());
        let chunk = plaintext.get(start..end).unwrap_or_default();
        let nonce = chunk_nonce(nonce_prefix, index)?;
        let aad = chunk_aad(index, chunk_count)?;
        let encrypted = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: chunk,
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?;
        let encrypted_len = u32::try_from(encrypted.len())
            .map_err(|_| CryptoError::InvalidFormat("密文块长度超过支持范围".into()))?;
        output.extend_from_slice(&encrypted_len.to_le_bytes());
        output.extend_from_slice(&encrypted);
    }
    Ok(output)
}

/// 解析容器头并逐块认证解密，拒绝截断、尾随数据和长度不一致。
fn decrypt_chunks(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if ciphertext.len() < HEADER_SIZE || &ciphertext[..8] != MAGIC {
        return Err(CryptoError::InvalidFormat("缺少 NXMEDIA1 文件头".into()));
    }
    let chunk_size = u32::from_le_bytes(read_array(ciphertext, 8)?) as usize;
    let original_len = u64::from_le_bytes(read_array(ciphertext, 12)?) as usize;
    let chunk_count = u32::from_le_bytes(read_array(ciphertext, 20)?) as usize;
    let nonce_prefix = read_array::<16>(ciphertext, 24)?;
    if chunk_size == 0 || chunk_count == 0 {
        return Err(CryptoError::InvalidFormat(
            "块大小和块数量必须大于零".into(),
        ));
    }

    let cipher = XChaCha20Poly1305::new(key.into());
    let mut cursor = HEADER_SIZE;
    let mut plaintext = Vec::with_capacity(original_len);
    for index in 0..chunk_count {
        let encrypted_len = u32::from_le_bytes(read_array(ciphertext, cursor)?) as usize;
        cursor = cursor
            .checked_add(4)
            .ok_or_else(|| CryptoError::InvalidFormat("密文偏移溢出".into()))?;
        let end = cursor
            .checked_add(encrypted_len)
            .ok_or_else(|| CryptoError::InvalidFormat("密文块长度溢出".into()))?;
        let encrypted = ciphertext
            .get(cursor..end)
            .ok_or_else(|| CryptoError::InvalidFormat("密文块被截断".into()))?;
        let nonce = chunk_nonce(nonce_prefix, index)?;
        let aad = chunk_aad(index, chunk_count)?;
        let chunk = cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: encrypted,
                    aad: &aad,
                },
            )
            .map_err(|_| CryptoError::AuthenticationFailed)?;
        plaintext.extend_from_slice(&chunk);
        cursor = end;
    }
    if cursor != ciphertext.len() || plaintext.len() != original_len {
        return Err(CryptoError::InvalidFormat("容器长度与文件头不一致".into()));
    }
    Ok(plaintext)
}

/// 从随机前缀和块序号构造 24 字节 XChaCha20 nonce。
fn chunk_nonce(prefix: [u8; 16], index: usize) -> Result<[u8; 24], CryptoError> {
    let mut nonce = [0_u8; 24];
    nonce[..16].copy_from_slice(&prefix);
    nonce[16..].copy_from_slice(
        &u64::try_from(index)
            .map_err(|_| CryptoError::InvalidFormat("块序号超过支持范围".into()))?
            .to_le_bytes(),
    );
    Ok(nonce)
}

/// 构造防止块调序或跨文件块复用的认证附加数据。
fn chunk_aad(index: usize, count: usize) -> Result<[u8; 24], CryptoError> {
    let mut aad = [0_u8; 24];
    aad[..8].copy_from_slice(MAGIC);
    aad[8..16].copy_from_slice(
        &u64::try_from(index)
            .map_err(|_| CryptoError::InvalidFormat("块序号超过支持范围".into()))?
            .to_le_bytes(),
    );
    aad[16..].copy_from_slice(
        &u64::try_from(count)
            .map_err(|_| CryptoError::InvalidFormat("块数量超过支持范围".into()))?
            .to_le_bytes(),
    );
    Ok(aad)
}

/// 从指定偏移读取定长字节数组，并统一处理截断错误。
fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], CryptoError> {
    bytes
        .get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| CryptoError::InvalidFormat("加密媒体容器被截断".into()))
}
