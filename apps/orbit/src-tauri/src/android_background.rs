//! 本文件提供不依赖 Tauri Activity 或 WebView 的 Android WorkManager JNI 同步入口。

use std::{panic::AssertUnwindSafe, path::PathBuf};

use jni::{
    JNIEnv,
    objects::{JByteArray, JClass, JString},
    sys::{JNI_FALSE, JNI_TRUE, jboolean},
};
use nexus_sync::{DeviceIdentity, SyncKey};
use zeroize::Zeroizing;

use crate::{mobile_cache::EncryptedCache, mobile_sync};

/// 保存一次 JNI 调用携带的 Java 对象，避免普通 Rust 函数复制固定 JNI 长参数列表。
struct BackgroundSyncArguments<'local> {
    endpoint: JString<'local>,
    token: JByteArray<'local>,
    cache_path: JString<'local>,
    cache_key: JByteArray<'local>,
    root_key: JByteArray<'local>,
    device_id: JString<'local>,
    identity_pkcs8: JByteArray<'local>,
}

/// 将 Java 字节数组复制到 Rust 受控内存，调用结束后由持有类型负责清零。
fn byte_array(env: &JNIEnv<'_>, value: &JByteArray<'_>) -> Result<Vec<u8>, String> {
    env.convert_byte_array(value)
        .map_err(|error| error.to_string())
}

/// 解析 WorkManager 参数并在独立 Tokio 运行时内复用 Android 内容同步实现。
fn run_sync(env: &mut JNIEnv<'_>, arguments: BackgroundSyncArguments<'_>) -> Result<(), String> {
    let endpoint: String = env
        .get_string(&arguments.endpoint)
        .map_err(|error| error.to_string())?
        .into();
    let cache_path: String = env
        .get_string(&arguments.cache_path)
        .map_err(|error| error.to_string())?
        .into();
    let device_id: String = env
        .get_string(&arguments.device_id)
        .map_err(|error| error.to_string())?
        .into();
    let token = Zeroizing::new(
        String::from_utf8(byte_array(env, &arguments.token)?).map_err(|error| error.to_string())?,
    );
    let cache_key: Zeroizing<[u8; 32]> = Zeroizing::new(
        byte_array(env, &arguments.cache_key)?
            .try_into()
            .map_err(|_| "Android 后台缓存密钥长度无效".to_owned())?,
    );
    let root_key: [u8; 32] = byte_array(env, &arguments.root_key)?
        .try_into()
        .map_err(|_| "Android 后台同步根密钥长度无效".to_owned())?;
    let key = SyncKey::from_bytes(root_key);
    let identity =
        DeviceIdentity::from_pkcs8(device_id, byte_array(env, &arguments.identity_pkcs8)?)
            .map_err(|error| error.to_string())?;
    let cache = EncryptedCache::open(PathBuf::from(cache_path), cache_key.as_ref())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime.block_on(mobile_sync::sync_content_with_material(
        &cache,
        &reqwest::Client::new(),
        &endpoint,
        &token,
        &key,
        &identity,
    ))?;
    Ok(())
}

/// WorkManager JNI 入口；任何异常都转换为可退避重试的 false，禁止跨 FFI 边界展开。
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_nexus_platform_mobile_BackgroundSyncNative_runSync(
    mut env: JNIEnv<'_>,
    _class: JClass<'_>,
    endpoint: JString<'_>,
    token: JByteArray<'_>,
    cache_path: JString<'_>,
    cache_key: JByteArray<'_>,
    root_key: JByteArray<'_>,
    device_id: JString<'_>,
    identity_pkcs8: JByteArray<'_>,
) -> jboolean {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        run_sync(
            &mut env,
            BackgroundSyncArguments {
                endpoint,
                token,
                cache_path,
                cache_key,
                root_key,
                device_id,
                identity_pkcs8,
            },
        )
    }));
    match result {
        Ok(Ok(())) => JNI_TRUE,
        Ok(Err(error)) => {
            eprintln!("Android WorkManager 增量同步失败：{error}");
            JNI_FALSE
        }
        Err(_) => {
            eprintln!("Android WorkManager 增量同步发生未捕获异常");
            JNI_FALSE
        }
    }
}
