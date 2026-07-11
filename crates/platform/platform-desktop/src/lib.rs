//! 本文件声明桌面端平台适配层当前覆盖的能力边界。

pub use nexus_platform_api::{
    AudioClip, AudioRecorder, BackgroundTask, BackgroundTaskId, CapturedFrame, HotkeyRegistrar,
    HotkeySpec, PlatformError, PlatformResult, ScreenCapturer, SecureStorage,
};

/// 返回桌面产品需要实现的平台能力名称。
#[must_use]
pub const fn capabilities() -> &'static [&'static str] {
    &[
        "screen-capture",
        "global-hotkey",
        "audio-recording",
        "background-task",
    ]
}
