# Android Keystore 插件由 Tauri 注解处理器发现，保留带注解的命令入口。
-keep @app.tauri.annotation.TauriPlugin class * { *; }
-keepclassmembers class * {
    @app.tauri.annotation.Command <methods>;
}
