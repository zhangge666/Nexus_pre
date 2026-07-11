//! 本文件通过内存模拟实现验证跨端能力 trait 可以被应用统一依赖。

use std::{sync::Mutex, time::Duration};

use nexus_platform_api::{HotkeyRegistrar, HotkeySpec, PlatformResult, SecureStorage};

/// 提供测试用的快捷键和安全存储实现。
#[derive(Default)]
struct MockPlatform {
    secret: Mutex<Option<Vec<u8>>>,
}

impl HotkeyRegistrar for MockPlatform {
    /// 返回基于快捷键文本的确定性注册标识。
    fn register(&self, hotkey: &HotkeySpec) -> PlatformResult<String> {
        Ok(format!("registered:{}", hotkey.accelerator))
    }

    /// 接受测试注册标识并完成注销。
    fn unregister(&self, _registration_id: &str) -> PlatformResult<()> {
        Ok(())
    }
}

impl SecureStorage for MockPlatform {
    /// 将测试密钥保存到进程内存。
    fn store(&self, _key: &str, secret: &[u8]) -> PlatformResult<()> {
        *self.secret.lock().expect("测试锁不应污染") = Some(secret.to_vec());
        Ok(())
    }

    /// 从进程内存读取测试密钥。
    fn load(&self, _key: &str) -> PlatformResult<Option<Vec<u8>>> {
        Ok(self.secret.lock().expect("测试锁不应污染").clone())
    }

    /// 从进程内存删除测试密钥。
    fn delete(&self, _key: &str) -> PlatformResult<bool> {
        Ok(self.secret.lock().expect("测试锁不应污染").take().is_some())
    }
}

/// 验证应用可以只依赖 trait 使用快捷键和安全存储。
#[test]
fn platform_traits_support_shared_callers() {
    let platform = MockPlatform::default();
    let registration = platform
        .register(&HotkeySpec {
            accelerator: "CommandOrControl+Shift+Space".into(),
        })
        .expect("快捷键注册应成功");
    assert!(registration.starts_with("registered:"));
    platform
        .store("root-key", b"secret")
        .expect("密钥封存应成功");
    assert_eq!(platform.load("root-key").unwrap(), Some(b"secret".to_vec()));
    assert!(platform.delete("root-key").unwrap());
    let _background_interval = Duration::from_secs(60);
}
