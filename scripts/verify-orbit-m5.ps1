# 本脚本统一执行 Orbit M5 桌面/Android 同步、前端、Kotlin 与可选 release 产物验收。

[CmdletBinding()]
param(
    [switch]$BuildPreviewRelease
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

# 运行单项验收命令，并在任一命令失败时立即终止。
function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][scriptblock]$Command
    )

    Write-Host "`n==> $Label" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) {
        throw "$Label 失败，退出码：$LASTEXITCODE"
    }
}

$AndroidHome = if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_HOME)) {
    $env:ANDROID_HOME
} else {
    $env:ANDROID_SDK_ROOT
}
if ([string]::IsNullOrWhiteSpace($AndroidHome)) {
    throw "必须设置 ANDROID_HOME 或 ANDROID_SDK_ROOT。"
}
$env:ANDROID_HOME = $AndroidHome
$env:ANDROID_SDK_ROOT = $AndroidHome
$NdkHome = if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_NDK_HOME)) {
    $env:ANDROID_NDK_HOME
} else {
    $env:NDK_HOME
}
if ([string]::IsNullOrWhiteSpace($NdkHome)) {
    throw "必须设置 ANDROID_NDK_HOME 或 NDK_HOME。"
}
$env:ANDROID_NDK_HOME = $NdkHome
$env:NDK_HOME = $NdkHome
if ([string]::IsNullOrWhiteSpace($env:JAVA_HOME)) {
    throw "必须设置 JAVA_HOME，Android Gradle Plugin 需要 JDK 11 或更高版本。"
}

$NdkBin = Join-Path $NdkHome "toolchains\llvm\prebuilt\windows-x86_64\bin"
$env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER = Join-Path $NdkBin "aarch64-linux-android24-clang.cmd"
$env:CC_aarch64_linux_android = $env:CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER
$env:AR_aarch64_linux_android = Join-Path $NdkBin "llvm-ar.exe"
$env:PATH = "$($env:JAVA_HOME)\bin;$NdkBin;$($env:PATH)"

Invoke-Checked "同步密码学核心测试" { cargo test -p nexus-sync }
Invoke-Checked "零知识 Relay 测试" { cargo test -p nexus-relay }
Invoke-Checked "Orbit 桌面同步与 IPC 测试" { cargo test -p orbit-app }
Invoke-Checked "Orbit 桌面严格检查" {
    cargo clippy -p orbit-app --all-targets -- -D warnings
}
Invoke-Checked "Orbit Android arm64 严格检查" {
    cargo clippy -p orbit-app --target aarch64-linux-android --all-targets -- -D warnings
}
Invoke-Checked "Orbit Android 前端生产构建" {
    $previousPlatform = $env:VITE_NEXUS_PLATFORM
    try {
        $env:VITE_NEXUS_PLATFORM = "android"
        pnpm --filter "@nexus/orbit" build
    } finally {
        $env:VITE_NEXUS_PLATFORM = $previousPlatform
    }
}
Invoke-Checked "Orbit Android Kotlin 编译" {
    Push-Location "apps\orbit\src-tauri\gen\android"
    try {
        .\gradlew.bat ":app:compileUniversalDebugKotlin" "-Pkotlin.incremental=false" --no-daemon --console=plain
    } finally {
        Pop-Location
    }
}

if ($BuildPreviewRelease) {
    Invoke-Checked "构建并签名 Android 预览 release" {
        & (Join-Path $PSScriptRoot "build-orbit-android-release.ps1") `
            -PreviewSigning `
            -Targets "aarch64"
    }
}

Write-Host "`nOrbit M5 自动化验收通过。" -ForegroundColor Green
