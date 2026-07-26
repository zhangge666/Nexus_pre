//! 本文件提供 Nexus 零知识中继的自托管进程入口，生产环境由 TLS 终止层暴露 HTTPS。

use std::{env, io, net::SocketAddr, path::PathBuf};

use nexus_relay::RelayState;
use tokio::net::TcpListener;

/// 从环境变量加载绑定地址、访问令牌和状态路径并启动中继。
#[tokio::main]
async fn main() -> io::Result<()> {
    let bind = env::var("NEXUS_RELAY_BIND").unwrap_or_else(|_| "127.0.0.1:8787".into());
    let bind: SocketAddr = bind.parse().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("NEXUS_RELAY_BIND 无效: {error}"),
        )
    })?;
    let token = env::var("NEXUS_RELAY_TOKEN").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "必须设置至少 32 字符的 NEXUS_RELAY_TOKEN",
        )
    })?;
    let state_path = env::var_os("NEXUS_RELAY_DATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nexus-relay-state.json"));
    let state = RelayState::open(state_path, token).map_err(io::Error::other)?;
    let listener = TcpListener::bind(bind).await?;
    println!("Nexus 零知识中继正在监听 {bind}");
    axum::serve(listener, nexus_relay::router(state)).await
}
