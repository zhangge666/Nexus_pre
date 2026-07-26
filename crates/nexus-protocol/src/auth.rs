//! 本文件定义 capability token 的能力域以及写入来源限制。

use std::collections::HashSet;

use sha2::{Digest, Sha256};

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

    /// 将协议请求中的稳定字符串解析为能力域。
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "memory:read" => Some(Self::MemoryRead),
            "memory:write" => Some(Self::MemoryWrite),
            "memory:delete" => Some(Self::MemoryDelete),
            "search" => Some(Self::Search),
            "subscribe" => Some(Self::Subscribe),
            "review" => Some(Self::Review),
            "admin" => Some(Self::Admin),
            _ => None,
        }
    }
}

/// 表示本地服务为一个客户端签发的令牌、能力域和可写来源。
#[derive(Debug, Clone)]
pub struct CapabilityGrant {
    token: String,
    token_is_digest: bool,
    scopes: HashSet<Scope>,
    writable_source: Option<String>,
    readable_sources: Option<HashSet<String>>,
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
            token_is_digest: false,
            scopes: scopes.into_iter().collect(),
            writable_source,
            readable_sources: None,
        }
    }

    /// 从已经持久化的 SHA-256 摘要恢复授权，不把令牌正文写入磁盘。
    #[must_use]
    pub(crate) fn from_token_digest(
        token_digest: impl Into<String>,
        scopes: impl IntoIterator<Item = Scope>,
        writable_source: Option<String>,
    ) -> Self {
        Self {
            token: token_digest.into(),
            token_is_digest: true,
            scopes: scopes.into_iter().collect(),
            writable_source,
            readable_sources: None,
        }
    }

    /// 从令牌正文创建只保留摘要的授权，供第三方长期令牌首次签发使用。
    #[must_use]
    pub(crate) fn from_token_hashing(
        token: &str,
        scopes: impl IntoIterator<Item = Scope>,
        writable_source: Option<String>,
    ) -> Self {
        Self::from_token_digest(token_digest(token), scopes, writable_source)
    }

    /// 为令牌附加可读取来源白名单；未调用时允许读取全部来源。
    #[must_use]
    pub fn with_readable_sources(mut self, sources: impl IntoIterator<Item = String>) -> Self {
        self.readable_sources = Some(sources.into_iter().collect());
        self
    }

    /// 验证 Bearer 令牌是否与当前授权一致。
    #[must_use]
    pub fn accepts_token(&self, token: &str) -> bool {
        if self.token_is_digest {
            self.token == token_digest(token)
        } else {
            self.token == token
        }
    }

    /// 返回仍保留在内存中的令牌正文；摘要授权返回 `None`。
    #[must_use]
    pub(crate) fn token_value(&self) -> Option<&str> {
        (!self.token_is_digest).then_some(self.token.as_str())
    }

    /// 返回可安全持久化的令牌摘要。
    #[must_use]
    pub(crate) fn token_digest(&self) -> String {
        if self.token_is_digest {
            self.token.clone()
        } else {
            token_digest(&self.token)
        }
    }

    /// 返回令牌绑定的全部能力域，供连接管理界面展示授权边界。
    pub fn scopes(&self) -> impl Iterator<Item = Scope> + '_ {
        self.scopes.iter().copied()
    }

    /// 判断授权是否包含指定能力域；admin 隐含全部能力。
    #[must_use]
    pub fn allows(&self, scope: Scope) -> bool {
        self.scopes.contains(&Scope::Admin) || self.scopes.contains(&scope)
    }

    /// 判断客户端是否可向指定 source 写入。
    #[must_use]
    pub fn allows_write_source(&self, source: &str) -> bool {
        self.writable_source
            .as_deref()
            .is_none_or(|allowed| allowed == source)
    }

    /// 判断客户端是否可读取指定来源。
    #[must_use]
    pub fn allows_read_source(&self, source: &str) -> bool {
        self.readable_sources
            .as_ref()
            .is_none_or(|allowed| allowed.contains(source))
    }

    /// 返回令牌绑定的可写来源限制；`None` 表示不限制写入来源。
    #[must_use]
    pub fn writable_source_restriction(&self) -> Option<&str> {
        self.writable_source.as_deref()
    }

    /// 返回令牌绑定的可读来源白名单；`None` 表示可读取全部来源。
    #[must_use]
    pub fn readable_source_restriction(&self) -> Option<&HashSet<String>> {
        self.readable_sources.as_ref()
    }
}

/// 对高熵 capability token 计算稳定 SHA-256 摘要。
fn token_digest(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}
