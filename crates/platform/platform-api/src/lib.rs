//! 本文件定义 Nexus 跨端平台实现必须遵循的能力 trait 与传输模型。

use std::{path::PathBuf, time::Duration};

/// 表示平台能力不可用、权限不足或原生调用失败。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    /// 当前目标平台不支持该能力。
    Unsupported(&'static str),
    /// 用户尚未授予系统权限。
    PermissionDenied(&'static str),
    /// 原生实现返回可展示的错误信息。
    Native(String),
}

impl std::fmt::Display for PlatformError {
    /// 将平台错误转换为稳定的中文诊断信息。
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(capability) => write!(formatter, "当前平台不支持能力: {capability}"),
            Self::PermissionDenied(capability) => {
                write!(formatter, "尚未授予系统权限: {capability}")
            }
            Self::Native(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for PlatformError {}

/// 平台能力统一使用的结果类型。
pub type PlatformResult<T> = Result<T, PlatformError>;

/// 表示截取后的原始画面及其像素元数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    /// 原始 RGBA 像素。
    pub rgba: Vec<u8>,
    /// 画面宽度。
    pub width: u32,
    /// 画面高度。
    pub height: u32,
    /// 可选活动窗口标题。
    pub window_title: Option<String>,
}

/// 表示一个跨平台全局快捷键。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySpec {
    /// 规范化快捷键，例如 `CommandOrControl+Shift+Space`。
    pub accelerator: String,
}

/// 表示录音完成后的本地音频引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioClip {
    /// 音频文件路径。
    pub path: PathBuf,
    /// 音频 MIME 类型。
    pub mime: String,
    /// 音频时长。
    pub duration: Duration,
}

/// 表示已注册后台任务的稳定标识。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BackgroundTaskId(pub String);

/// 抽象 Echo 所需的活动窗口截屏能力。
pub trait ScreenCapturer: Send + Sync {
    /// 截取当前活动窗口或屏幕主要区域。
    fn capture_active(&self) -> PlatformResult<CapturedFrame>;
}

/// 抽象桌面全局快捷键注册能力。
pub trait HotkeyRegistrar: Send + Sync {
    /// 注册快捷键并返回平台分配的注册标识。
    fn register(&self, hotkey: &HotkeySpec) -> PlatformResult<String>;

    /// 注销先前注册的快捷键。
    fn unregister(&self, registration_id: &str) -> PlatformResult<()>;
}

/// 抽象 Muse 所需的音频录制能力。
pub trait AudioRecorder: Send + Sync {
    /// 开始新的录音会话。
    fn start(&self) -> PlatformResult<()>;

    /// 停止当前录音并返回本地音频引用。
    fn stop(&self) -> PlatformResult<AudioClip>;
}

/// 抽象桌面常驻和移动后台刷新任务。
pub trait BackgroundTask: Send + Sync {
    /// 注册指定最小间隔的后台任务。
    fn schedule(&self, name: &str, minimum_interval: Duration) -> PlatformResult<BackgroundTaskId>;

    /// 取消已注册的后台任务。
    fn cancel(&self, id: &BackgroundTaskId) -> PlatformResult<()>;
}

/// 抽象 DPAPI、Keychain 和 Keystore 等系统安全存储。
pub trait SecureStorage: Send + Sync {
    /// 将敏感字节写入当前用户的系统安全区。
    fn store(&self, key: &str, secret: &[u8]) -> PlatformResult<()>;

    /// 读取先前封存的敏感字节。
    fn load(&self, key: &str) -> PlatformResult<Option<Vec<u8>>>;

    /// 删除系统安全区中的敏感字节。
    fn delete(&self, key: &str) -> PlatformResult<bool>;
}
