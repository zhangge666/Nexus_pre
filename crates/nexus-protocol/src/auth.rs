//! 本文件定义 capability token 的能力域以及写入来源限制。

use std::collections::HashSet;

/// 表示 Memory Protocol 可授予客户端的最小能力域。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// 读取单条或列表记忆。
    MemoryRead,
    /// 创建或更新记忆。
    MemoryWrite,
    /// 删除记忆。
    MemoryDelete,
    /// 执行关键词、语义或混合检索。
    Search,
    /// 订阅记忆与复习事件。
    Subscribe,
    /// 读写复习状态。
    Review,
    /// 管理连接、集合和导出。
    Admin,
}

impl Scope {
    /// 返回协议文档定义的稳定 scope 字符串。
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MemoryRead => "memory:read",
            Self::MemoryWrite => "memory:write",
            Self::MemoryDelete => "memory:delete",
            Self::Search => "search",
            Self::Subscribe => "subscribe",
            Self::Review => "review",
            Self::Admin => "admin",
        }
    }
}

/// 表示本地服务为一个客户端签发的令牌、能力域和可写来源。
#[derive(Debug, Clone)]
pub struct CapabilityGrant {
    token: String,
    scopes: HashSet<Scope>,
    writable_source: Option<String>,
}

impl CapabilityGrant {
    /// 创建一个 capability grant；外部调用方应提供高熵短期令牌。
    #[must_use]
    pub fn new(
        token: impl Into<String>,
        scopes: impl IntoIterator<Item = Scope>,
        writable_source: Option<String>,
    ) -> Self {
        Self {
            token: token.into(),
            scopes: scopes.into_iter().collect(),
            writable_source,
        }
    }

    /// 验证 Bearer 令牌是否与当前授权一致。
    #[must_use]
    pub fn accepts_token(&self, token: &str) -> bool {
        self.token == token
    }

    /// 判断授权是否包含指定能力域；admin 隐含全部能力。
    #[must_use]
    pub fn allows(&self, scope: Scope) -> bool {
        self.scopes.contains(&Scope::Admin) || self.scopes.contains(&scope)
    }

    /// 判断客户端是否可向指定 source 写入。
    #[must_use]
    pub fn allows_source(&self, source: &str) -> bool {
        self.writable_source
            .as_deref()
            .is_none_or(|allowed| allowed == source)
    }
}
