//! 本文件组织 Memory Protocol v1 的鉴权、数据契约、路由与本地服务入口。

pub mod auth;
pub mod dto;
pub mod server;

pub use auth::{CapabilityGrant, Scope};
pub use server::{ProtocolError, ProtocolState, router, serve};

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
