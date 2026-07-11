//! 本文件声明 Memory Protocol v1 的基础路径与能力发现信息。

/// 返回协议健康检查和能力发现使用的版本标识。
#[must_use]
pub const fn protocol_version() -> &'static str {
    "v1"
}

/// 返回当前核心骨架可供协议层编排的模块。
#[must_use]
pub const fn core_modules() -> &'static [&'static str] {
    nexus_core::modules()
}

#[cfg(test)]
mod tests {
    use super::{core_modules, protocol_version};

    /// 验证协议版本与核心连接符合文档中的 v1 契约。
    #[test]
    fn exposes_v1_capabilities() {
        assert_eq!(protocol_version(), "v1");
        assert!(core_modules().contains(&"store"));
    }
}
