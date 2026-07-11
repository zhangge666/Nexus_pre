//! 本文件声明移动端平台适配层当前覆盖的能力边界。

/// 返回 Orbit 与 Muse 移动端需要实现的平台能力名称。
#[must_use]
pub const fn capabilities() -> &'static [&'static str] {
    &["audio-recording", "background-task", "secure-storage"]
}
