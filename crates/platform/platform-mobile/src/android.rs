//! 本文件通过 Tauri Android 插件把 Rust 安全存储调用映射到 Android Keystore。

use base64::{Engine as _, engine::general_purpose::STANDARD};
use nexus_platform_api::{PlatformError, PlatformResult, SecureStorage};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tauri::{
    Manager, Runtime,
    plugin::{Builder, PluginApi, PluginHandle, TauriPlugin},
};

const PLUGIN_IDENTIFIER: &str = "com.nexus.platform.mobile";

/// 表示 Android 原生插件持有的 Keystore 访问句柄。
pub struct AndroidSecureStorage<R: Runtime>(PluginHandle<R>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreRequest<'a> {
    key: &'a str,
    value: String,
}

#[derive(Serialize)]
struct KeyRequest<'a> {
    key: &'a str,
}

#[derive(Deserialize)]
struct LoadResponse {
    value: Option<String>,
}

#[derive(Deserialize)]
struct DeleteResponse {
    deleted: bool,
}

/// 初始化 Android 原生插件句柄。
fn initialize<R: Runtime, C: DeserializeOwned>(
    api: PluginApi<R, C>,
) -> Result<AndroidSecureStorage<R>, Box<dyn std::error::Error>> {
    let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "SecureStoragePlugin")?;
    Ok(AndroidSecureStorage(handle))
}

impl<R: Runtime> SecureStorage for AndroidSecureStorage<R> {
    /// 使用 Keystore 管理的 AES-GCM 密钥封存敏感字节。
    fn store(&self, key: &str, secret: &[u8]) -> PlatformResult<()> {
        self.0
            .run_mobile_plugin::<()>(
                "store",
                StoreRequest {
                    key,
                    value: STANDARD.encode(secret),
                },
            )
            .map_err(|error| PlatformError::Native(error.to_string()))
    }

    /// 从 Android 安全存储读取并解码敏感字节。
    fn load(&self, key: &str) -> PlatformResult<Option<Vec<u8>>> {
        let response = self
            .0
            .run_mobile_plugin::<LoadResponse>("load", KeyRequest { key })
            .map_err(|error| PlatformError::Native(error.to_string()))?;
        response
            .value
            .map(|value| {
                STANDARD
                    .decode(value)
                    .map_err(|error| PlatformError::Native(format!("安全存储数据损坏：{error}")))
            })
            .transpose()
    }

    /// 删除指定安全存储条目，并返回条目此前是否存在。
    fn delete(&self, key: &str) -> PlatformResult<bool> {
        self.0
            .run_mobile_plugin::<DeleteResponse>("delete", KeyRequest { key })
            .map(|response| response.deleted)
            .map_err(|error| PlatformError::Native(error.to_string()))
    }
}

/// 为 Tauri Manager 提供类型安全的 Android 安全存储入口。
pub trait SecureStorageExt<R: Runtime> {
    /// 返回已经初始化的 Android Keystore 适配器。
    fn secure_storage(&self) -> &AndroidSecureStorage<R>;
}

impl<R: Runtime, T: Manager<R>> SecureStorageExt<R> for T {
    fn secure_storage(&self) -> &AndroidSecureStorage<R> {
        self.state::<AndroidSecureStorage<R>>().inner()
    }
}

/// 初始化 Nexus Android 平台插件。
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("nexus-platform-mobile")
        .setup(|app, api| {
            let storage = initialize(api)?;
            app.manage(storage);
            Ok(())
        })
        .build()
}
