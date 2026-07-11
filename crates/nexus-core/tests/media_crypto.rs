//! 本文件验证主密钥派生、分块认证加密、篡改检测和媒体内容寻址去重。

use nexus_core::{CryptoError, MasterKey, MediaVault};

/// 返回测试使用的确定性主密钥。
fn test_key() -> MasterKey {
    MasterKey::derive(b"correct horse battery staple", b"nexus-test-salt")
        .expect("测试主密钥应派生成功")
}

/// 验证跨越多个块的内容可以完整加密和解密。
#[test]
fn encrypts_and_decrypts_multiple_chunks() {
    let key = test_key();
    let plaintext = vec![0x5a; 160 * 1024];
    let encrypted = key.encrypt(&plaintext).expect("分块加密应成功");
    assert!(
        !encrypted
            .windows(64)
            .any(|window| window.iter().all(|byte| *byte == 0x5a))
    );
    assert_eq!(key.decrypt(&encrypted).expect("分块解密应成功"), plaintext);
}

/// 验证任意密文修改都会触发 AEAD 认证失败。
#[test]
fn rejects_tampered_ciphertext() {
    let key = test_key();
    let mut encrypted = key.encrypt(b"sensitive media content").expect("加密应成功");
    let last = encrypted.len() - 1;
    encrypted[last] ^= 0x40;
    assert!(matches!(
        key.decrypt(&encrypted),
        Err(CryptoError::AuthenticationFailed)
    ));
}

/// 验证媒体仓库按明文哈希去重、读取校验并支持可证删除。
#[test]
fn stores_deduplicates_and_deletes_encrypted_media() {
    let root = std::env::temp_dir().join(format!("nexus-media-test-{}", uuid::Uuid::now_v7()));
    let vault = MediaVault::open(&root, test_key()).expect("媒体仓库应创建成功");
    let plaintext = b"private screenshot bytes";
    let first = vault.put(plaintext, "image/png").expect("媒体写入应成功");
    let second = vault
        .put(plaintext, "image/png")
        .expect("重复媒体写入应成功");
    assert_eq!(first.path, second.path);
    assert_eq!(vault.read(&first).expect("媒体读取应成功"), plaintext);
    let stored = std::fs::read(&first.path).expect("应能读取测试密文");
    assert!(
        !stored
            .windows(plaintext.len())
            .any(|window| window == plaintext)
    );
    assert!(vault.delete(&first).expect("媒体删除应成功"));
    assert!(!first.path.exists());
    std::fs::remove_dir_all(root).expect("测试目录应清理成功");
}
