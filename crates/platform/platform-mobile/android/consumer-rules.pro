# Android Keystore 插件由 Tauri 注解处理器发现，保留带注解的命令入口。
-keep @app.tauri.annotation.TauriPlugin class * { *; }
-keepclassmembers class * {
    @app.tauri.annotation.Command <methods>;
}

# WorkManager 按持久化类名恢复任务，JNI 符号也依赖桥接类与方法名保持不变。
-keep class com.nexus.platform.mobile.OrbitBackgroundSyncWorker { *; }
-keep class com.nexus.platform.mobile.BackgroundSyncNative { *; }
