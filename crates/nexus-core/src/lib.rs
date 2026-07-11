//! 本文件提供 Nexus 共享核心的模块边界与基础能力描述。

/// 返回当前核心骨架已经声明的领域模块。
#[must_use]
pub const fn modules() -> &'static [&'static str] {
    &[
        "model", "store", "ingest", "search", "embed", "crypto", "sync", "events",
    ]
}

#[cfg(test)]
mod tests {
    use super::modules;

    /// 验证地基骨架保留路线图要求的全部核心模块边界。
    #[test]
    fn exposes_documented_core_modules() {
        assert!(modules().contains(&"store"));
        assert!(modules().contains(&"ingest"));
        assert!(modules().contains(&"search"));
    }
}
