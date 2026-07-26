//! 本文件实现 Android 离线响应缓存，缓存文件使用 Keystore 托管密钥进行 AES-256-GCM 加密。

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::{RngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};

const CACHE_VERSION: u8 = 1;
const CACHE_KEY_LENGTH: usize = 32;
const CACHE_NONCE_LENGTH: usize = 12;
const MAX_CACHE_ENTRIES: usize = 256;
const MAX_CACHE_AGE_MILLIS: i64 = 30 * 24 * 60 * 60 * 1_000;

// 前台 Tauri 状态与 WorkManager JNI 入口会各自打开缓存，所有磁盘读改写必须串行。
static CACHE_FILE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Deserialize, Serialize)]
struct CacheEntry {
    body: String,
    updated_at: i64,
}

#[derive(Deserialize, Serialize)]
struct CacheEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

/// 管理 Android 应用私有目录中的加密 HTTP 响应缓存。
pub struct EncryptedCache {
    path: PathBuf,
    key: [u8; CACHE_KEY_LENGTH],
    entries: Mutex<HashMap<String, CacheEntry>>,
}

impl Drop for EncryptedCache {
    /// 释放后台或前台缓存句柄时清零从 Keystore 解封的缓存密钥。
    fn drop(&mut self) {
        self.key.fill(0);
    }
}

impl EncryptedCache {
    /// 生成新的随机缓存主密钥；调用方必须将其保存到 Android Keystore。
    pub fn generate_key() -> [u8; CACHE_KEY_LENGTH] {
        let mut key = [0_u8; CACHE_KEY_LENGTH];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// 打开缓存；密钥变化或缓存损坏时清除不可解密文件并从空缓存继续运行。
    pub fn open(path: PathBuf, key: &[u8]) -> Result<Self, String> {
        let key: [u8; CACHE_KEY_LENGTH] = key
            .try_into()
            .map_err(|_| "Android 离线缓存密钥长度无效".to_owned())?;
        let _file_guard = CACHE_FILE_LOCK
            .lock()
            .map_err(|_| "Android 离线缓存文件锁不可用".to_owned())?;
        let entries = match Self::decrypt_file(&path, &key) {
            Ok(entries) => entries,
            Err(error) => {
                if path.exists() {
                    fs::remove_file(&path).map_err(|remove_error| {
                        format!("{error}；且无法清理损坏缓存：{remove_error}")
                    })?;
                }
                HashMap::new()
            }
        };
        Ok(Self {
            path,
            key,
            entries: Mutex::new(entries),
        })
    }

    /// 返回缓存的协议响应正文。
    pub fn get(&self, key: &str) -> Result<Option<String>, String> {
        let _file_guard = CACHE_FILE_LOCK
            .lock()
            .map_err(|_| "Android 离线缓存文件锁不可用".to_owned())?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Android 离线缓存状态不可用".to_owned())?;
        self.reload(&mut entries)?;
        Ok(entries.get(key).map(|entry| entry.body.clone()))
    }

    /// 写入最新协议响应，并按时间和数量边界清理旧条目。
    pub fn put(&self, key: String, body: String) -> Result<(), String> {
        let _file_guard = CACHE_FILE_LOCK
            .lock()
            .map_err(|_| "Android 离线缓存文件锁不可用".to_owned())?;
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| "Android 离线缓存状态不可用".to_owned())?;
        self.reload(&mut entries)?;
        let now = unix_millis();
        entries.insert(
            key,
            CacheEntry {
                body,
                updated_at: now,
            },
        );
        entries.retain(|_, entry| now.saturating_sub(entry.updated_at) <= MAX_CACHE_AGE_MILLIS);
        if entries.len() > MAX_CACHE_ENTRIES {
            let mut oldest = entries
                .iter()
                .map(|(key, entry)| (key.clone(), entry.updated_at))
                .collect::<Vec<_>>();
            oldest.sort_by_key(|(_, updated_at)| *updated_at);
            for (key, _) in oldest.into_iter().take(entries.len() - MAX_CACHE_ENTRIES) {
                entries.remove(&key);
            }
        }
        self.persist(&entries)
    }

    /// 返回是否存在至少一条可供离线展示的协议响应。
    pub fn has_entries(&self) -> bool {
        let Ok(_file_guard) = CACHE_FILE_LOCK.lock() else {
            return false;
        };
        let Ok(mut entries) = self.entries.lock() else {
            return false;
        };
        self.reload(&mut entries).is_ok() && !entries.is_empty()
    }

    /// 清除内存与磁盘中的全部离线响应，用于断开设备连接。
    pub fn clear(&self) -> Result<(), String> {
        let _file_guard = CACHE_FILE_LOCK
            .lock()
            .map_err(|_| "Android 离线缓存文件锁不可用".to_owned())?;
        self.entries
            .lock()
            .map_err(|_| "Android 离线缓存状态不可用".to_owned())?
            .clear();
        if self.path.exists() {
            fs::remove_file(&self.path).map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    /// 返回缓存密文文件路径，供 WorkManager 与前台进程打开同一份副本。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 在每次读取或改写前吸收另一入口写入的最新磁盘状态。
    fn reload(&self, entries: &mut HashMap<String, CacheEntry>) -> Result<(), String> {
        *entries = Self::decrypt_file(&self.path, &self.key)?;
        Ok(())
    }

    /// 将缓存映射整体加密并原子替换磁盘文件。
    fn persist(&self, entries: &HashMap<String, CacheEntry>) -> Result<(), String> {
        let plaintext = serde_json::to_vec(entries).map_err(|error| error.to_string())?;
        let cipher = Aes256Gcm::new_from_slice(&self.key).map_err(|error| error.to_string())?;
        let mut nonce = [0_u8; CACHE_NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
            .map_err(|error| format!("无法加密 Android 离线缓存：{error}"))?;
        let envelope = CacheEnvelope {
            version: CACHE_VERSION,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        };
        let content = serde_json::to_vec(&envelope).map_err(|error| error.to_string())?;
        let temporary = self.path.with_extension("tmp");
        fs::write(&temporary, content).map_err(|error| error.to_string())?;
        fs::rename(&temporary, &self.path).map_err(|error| error.to_string())
    }

    /// 解密现有缓存文件并恢复条目映射。
    fn decrypt_file(
        path: &Path,
        key: &[u8; CACHE_KEY_LENGTH],
    ) -> Result<HashMap<String, CacheEntry>, String> {
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let envelope: CacheEnvelope =
            serde_json::from_slice(&fs::read(path).map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
        if envelope.version != CACHE_VERSION {
            return Err("Android 离线缓存版本不受支持".into());
        }
        let nonce = STANDARD
            .decode(envelope.nonce)
            .map_err(|error| error.to_string())?;
        let nonce: [u8; CACHE_NONCE_LENGTH] = nonce
            .try_into()
            .map_err(|_| "Android 离线缓存随机数长度无效".to_owned())?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|error| error.to_string())?;
        let cipher = Aes256Gcm::new_from_slice(key).map_err(|error| error.to_string())?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|error| format!("无法解密 Android 离线缓存：{error}"))?;
        serde_json::from_slice(&plaintext).map_err(|error| error.to_string())
    }
}

/// 返回缓存条目更新时间使用的 Unix 毫秒值。
fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证缓存文件不会包含响应明文，并能使用相同密钥重新打开。
    #[test]
    fn encrypts_and_restores_cached_responses() {
        let directory = tempfile::tempdir().expect("应创建缓存临时目录");
        let path = directory.path().join("cache.enc");
        let key = EncryptedCache::generate_key();
        let cache = EncryptedCache::open(path.clone(), &key).expect("应打开缓存");
        cache
            .put("GET:/v1/memories".into(), "sensitive-memory".into())
            .expect("应写入缓存");
        let encrypted = fs::read_to_string(&path).expect("应读取密文");
        assert!(!encrypted.contains("sensitive-memory"));

        let reopened = EncryptedCache::open(path, &key).expect("应重新打开缓存");
        assert_eq!(
            reopened.get("GET:/v1/memories").unwrap().as_deref(),
            Some("sensitive-memory")
        );
    }

    /// 验证前台与 WorkManager 各自持有缓存句柄时会在每次操作前吸收对方写入。
    #[test]
    fn keeps_foreground_and_background_handles_coherent() {
        let directory = tempfile::tempdir().expect("应创建缓存临时目录");
        let path = directory.path().join("cache.enc");
        let key = EncryptedCache::generate_key();
        let foreground = EncryptedCache::open(path.clone(), &key).expect("应打开前台缓存");
        let background = EncryptedCache::open(path, &key).expect("应打开后台缓存");

        foreground
            .put("foreground".into(), "first".into())
            .expect("前台应写入缓存");
        assert_eq!(
            background.get("foreground").unwrap().as_deref(),
            Some("first")
        );

        background
            .put("background".into(), "second".into())
            .expect("后台应写入缓存");
        assert_eq!(
            foreground.get("background").unwrap().as_deref(),
            Some("second")
        );
    }
}
