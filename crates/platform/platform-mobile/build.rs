//! 本文件在构建阶段把 Android 原生安全存储模块打包为 Tauri 移动插件。

const COMMANDS: &[&str] = &["store", "load", "delete"];

/// 生成 Android 插件绑定与 Gradle 工程接入信息。
fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .try_build()
        .expect("应生成 Nexus Android 平台插件");
}
