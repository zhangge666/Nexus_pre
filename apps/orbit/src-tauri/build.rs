//! 本文件在 Cargo 构建阶段生成 Tauri 所需的平台资源和配置。

/// 执行 Tauri 官方构建脚本。
fn main() {
    tauri_build::build();
}
