//! 本文件声明移动端平台适配层，并在 Android 上提供 Keystore 安全存储。

pub use nexus_platform_api::{
    AudioClip, AudioRecorder, BackgroundTask, BackgroundTaskId, CapturedFrame, HotkeyRegistrar,
    HotkeySpec, PlatformError, PlatformResult, ScreenCapturer, SecureStorage,
};

#[cfg(target_os = "android")]
mod android;

#[cfg(target_os = "android")]
pub use android::{AndroidSecureStorage, SecureStorageExt, init};

/// 返回 Orbit 与 Muse 移动端需要实现的平台能力名称。
#[must_use]
pub const fn capabilities() -> &'static [&'static str] {
    &["audio-recording", "background-task", "secure-storage"]
}
