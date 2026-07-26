# Orbit Android 构建、签名与真机验收

本文档定义 Orbit Android 的版本、APK/AAB 构建、签名、验签和真机冒烟流程。M5 的功能代码和本地发布工具链已经闭环，但正式密钥、Play 发布凭据以及手机/平板验收仍属于发布前操作。

> M5 功能与剩余验收状态见 [m5-mobile.md](m5-mobile.md#7-android-当前交付状态2026-07-26)。iOS 继续延期到最终 M8，不在本流程内。

## 1. 环境要求

Windows 开发机需准备：

- JDK 21，并设置 `JAVA_HOME`；
- Android SDK、Platform-Tools、Build-Tools 与 NDK；
- `ANDROID_HOME` 或 `ANDROID_SDK_ROOT`；
- `ANDROID_NDK_HOME` 或 `NDK_HOME`；
- Rust Android 目标：`aarch64-linux-android`、`armv7-linux-androideabi`、`x86_64-linux-android`；
- 已完成仓库根目录的 `pnpm install`。

M5 自动化验收入口：

```powershell
pnpm verify:orbit-m5
```

该命令会执行同步核心、Relay 和 Orbit 测试，桌面与 Android arm64 Clippy，Android 前端生产构建以及 Kotlin 编译。需要同时生成本地预览签名 release 时执行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-orbit-m5.ps1 -BuildPreviewRelease
```

## 2. 版本规则

版本基线统一维护在 `apps/orbit/src-tauri/tauri.conf.json`：

```json
{
  "version": "0.1.0",
  "bundle": {
    "android": {
      "versionCode": 1000
    }
  }
}
```

- `version` 对应 Android `versionName`，仓库发布脚本要求使用 SemVer。
- `bundle.android.versionCode` 必须是 `1..2100000000` 内的整数。
- 每次向应用商店提交都必须递增 `versionCode`，已经发布的值不得复用。
- 同一产品版本的 APK 与 AAB 必须使用相同 `versionName`、`versionCode` 和签名身份。
- `-VersionName`、`-VersionCode` 仅通过临时 Tauri 配置覆盖本次构建，不会修改仓库版本基线；正式发版后仍应把最终版本提交到 `tauri.conf.json`。

临时覆盖示例：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-orbit-android-release.ps1 `
  -VersionName "0.1.1" `
  -VersionCode 1001
```

## 3. 正式签名密钥

正式密钥库、别名和密码不得提交到 Git，也不得写入 Gradle 文件、命令日志或项目文档。首次创建密钥时使用交互式 `keytool`，让密码由终端提示读取：

```powershell
keytool -genkeypair -v `
  -keystore "D:\secure\orbit-release.jks" `
  -alias "orbit-release" `
  -keyalg RSA `
  -keysize 4096 `
  -validity 10000
```

密钥库必须离线备份；遗失正式上传密钥会阻断后续更新。构建前只在当前进程设置以下环境变量：

```powershell
$env:NEXUS_ANDROID_KEYSTORE_PATH = "D:\secure\orbit-release.jks"
$env:NEXUS_ANDROID_KEY_ALIAS = "orbit-release"
$env:NEXUS_ANDROID_STORE_PASSWORD = "<密钥库密码>"
$env:NEXUS_ANDROID_KEY_PASSWORD = "<私钥密码>"
```

若私钥密码与密钥库密码相同，可省略 `NEXUS_ANDROID_KEY_PASSWORD`。脚本会临时复用密钥库密码，并在结束时恢复调用前的环境变量状态。

## 4. 构建 APK 与 AAB

正式构建：

```powershell
pnpm build:orbit-android-release
```

默认构建 `aarch64`、`armv7`、`x86_64` 三个 ABI 的通用 APK 与 AAB。只构建指定 ABI 时：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-orbit-android-release.ps1 `
  -Targets aarch64
```

本地预览可使用 Android debug keystore：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-orbit-android-release.ps1 `
  -PreviewSigning
```

`-PreviewSigning` 只用于开发机安装预览，不能上传应用商店。确需保留未签名原件时使用 `-Unsigned`；`-PreviewSigning` 与 `-Unsigned` 不能同时使用。

默认产物目录：

```text
dist/orbit-android/<versionName>-<versionCode>/
```

目录内包含：

- 带版本、ABI 和签名类型的 APK；
- 对应 AAB；
- `SHA256SUMS.txt`。

脚本会自动完成：

1. Tauri release APK/AAB 构建；
2. APK zipalign 检查；
3. APK Signature Scheme v2/v3 签名与 `apksigner verify`；
4. AAB JAR 签名、密码环境变量读取、完整性校验与签名条目检查；
5. 包名、`versionName`、`versionCode` 与实际打包 ABI 校验；
6. APK/AAB SHA-256 摘要生成。

## 5. 真机安装与冒烟

启用手机的开发者选项和 USB 调试，连接并授权后先确认：

```powershell
adb devices
```

安装、启动和采集基础状态：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\verify-orbit-android-device.ps1 `
  -ApkPath "dist\orbit-android\0.1.0-1000\Orbit-0.1.0-1000-aarch64-armv7-x86_64-preview-signed.apk"
```

多台设备在线时增加 `-Serial "<adb 序列号>"`。脚本会：

- 校验输入 APK 的包名和版本；
- 安装并启动 `com.nexus.orbit/.MainActivity`；
- 确认 Activity 启动成功且进程存活；
- 对比设备已安装版本与 APK；
- 采集 WorkManager/JobScheduler 可见状态；
- 将报告保存到 `dist/orbit-android/device-smoke/`。

冒烟通过不等同于 M5 真机验收完成。仍需按 [m5-mobile.md](m5-mobile.md#73-m5-android-剩余发布与真机验收) 在至少一台手机和一台平板上人工验证双设备同步、冲突、删除、恢复短语、Doze、进程回收、弱网、软键盘、长列表、主题与无障碍。

## 6. 发布前检查

- 使用持久化正式密钥重新构建并在隔离环境复核证书摘要。
- 保存 `SHA256SUMS.txt`、构建日志、提交 SHA、版本号和证书摘要。
- 以 AAB 作为 Play 发布产物；APK 只用于直接安装和测试。
- 在 Play Console 配置应用签名、上传权限、测试轨道、隐私政策和 Data safety。
- 先进入内部测试轨道，完成安装、升级、回滚边界和崩溃检查，再扩大范围。
- 正式上传前再次运行 `pnpm verify:orbit-m5` 和真机完整验收。

## 7. 当前验证记录

2026-07-26 已完成：

- 默认 `aarch64`、`armv7`、`x86_64` release APK/AAB 构建；
- debug keystore 预览签名；
- APK zipalign、v2/v3 验签；
- AAB JAR 签名、完整性和签名条目校验；
- 包名 `com.nexus.orbit`、`versionName=0.1.0`、`versionCode=1000` 校验；
- M5 自动化测试、Clippy、前端生产构建和 Kotlin 编译。

当前没有已授权的 Android 设备在线，因此真机安装、双设备同步、Doze、弱网和交互验收尚未执行；正式密钥和 Play 发布凭据也尚未配置。
