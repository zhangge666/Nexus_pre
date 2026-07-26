# 本脚本在已连接的 Android 设备上安装签名 APK，验证启动、包版本与 WorkManager 调度可见性。

[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)][string]$ApkPath,
    [string]$Serial = ""
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$AndroidHome = if (-not [string]::IsNullOrWhiteSpace($env:ANDROID_HOME)) {
    $env:ANDROID_HOME
} else {
    $env:ANDROID_SDK_ROOT
}
if ([string]::IsNullOrWhiteSpace($AndroidHome)) {
    throw "必须设置 ANDROID_HOME 或 ANDROID_SDK_ROOT。"
}

$Adb = Join-Path $AndroidHome "platform-tools\adb.exe"
if (-not (Test-Path -LiteralPath $Adb)) {
    throw "adb 不存在：$Adb"
}
$resolvedApk = (Resolve-Path -LiteralPath $ApkPath).Path
$buildToolsRoot = Join-Path $AndroidHome "build-tools"
$buildTools = Get-ChildItem -LiteralPath $buildToolsRoot -Directory |
    Sort-Object { try { [version]$_.Name } catch { [version]"0.0" } } -Descending |
    Select-Object -First 1
if ($null -eq $buildTools) {
    throw "Android Build-Tools 尚未安装。"
}
$Aapt2 = Join-Path $buildTools.FullName "aapt2.exe"
if (-not (Test-Path -LiteralPath $Aapt2)) {
    throw "aapt2 不存在：$Aapt2"
}

$badging = & $Aapt2 dump badging $resolvedApk
if ($LASTEXITCODE -ne 0) {
    throw "读取 APK 元数据失败，退出码：$LASTEXITCODE"
}
$packageLine = $badging | Select-Object -First 1
if ($packageLine -notmatch "name='([^']+)'") {
    throw "APK 缺少可识别的包名：$packageLine"
}
$expectedPackageName = $Matches[1]
if ($expectedPackageName -ne "com.nexus.orbit") {
    throw "APK 包名不是 com.nexus.orbit：$expectedPackageName"
}
if ($packageLine -notmatch "versionCode='([^']+)'") {
    throw "APK 缺少可识别的 versionCode：$packageLine"
}
$expectedVersionCode = $Matches[1]
if ($packageLine -notmatch "versionName='([^']+)'") {
    throw "APK 缺少可识别的 versionName：$packageLine"
}
$expectedVersionName = $Matches[1]

# 执行 adb 子命令，并在失败时保留原始输出用于定位真机问题。
function Invoke-Adb {
    param(
        [Parameter(Mandatory = $true)][string]$Label,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    Write-Host "`n==> $Label" -ForegroundColor Cyan
    $prefix = if ([string]::IsNullOrWhiteSpace($Serial)) { @() } else { @("-s", $Serial) }
    $output = & $Adb @prefix @Arguments 2>&1
    $output | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) {
        throw "$Label 失败，退出码：$LASTEXITCODE"
    }
    return @($output)
}

$deviceLines = & $Adb devices |
    Select-Object -Skip 1 |
    Where-Object { $_ -match "\S" }
if ([string]::IsNullOrWhiteSpace($Serial)) {
    $onlineDevices = @($deviceLines | Where-Object { $_ -match "\tdevice$" })
    if ($onlineDevices.Count -eq 0) {
        throw "没有已授权且在线的 Android 设备。"
    }
    if ($onlineDevices.Count -gt 1) {
        throw "检测到多台在线设备，请使用 -Serial 指定目标设备。"
    }
    $Serial = ($onlineDevices[0] -split "\t")[0]
} elseif (-not ($deviceLines | Where-Object { $_ -match "^$([regex]::Escape($Serial))\s+device$" })) {
    throw "设备 $Serial 未在线或尚未授权。"
}

Invoke-Adb "安装 Orbit APK" @("install", "-r", $resolvedApk) | Out-Null
$launchOutput = Invoke-Adb "启动 Orbit MainActivity" @(
    "shell", "am", "start", "-W", "-n", "com.nexus.orbit/.MainActivity"
)
if (-not ($launchOutput | Where-Object { $_ -match "Status:\s+ok" })) {
    throw "Orbit Activity 未返回成功启动状态。"
}

$processIdOutput = Invoke-Adb "确认 Orbit 进程存活" @("shell", "pidof", "com.nexus.orbit")
if ([string]::IsNullOrWhiteSpace(($processIdOutput -join "").Trim())) {
    throw "Orbit 启动后未保持进程存活。"
}

$packageInfo = Invoke-Adb "读取安装版本" @(
    "shell", "dumpsys", "package", "com.nexus.orbit"
)
$versionLines = $packageInfo | Where-Object { $_ -match "versionCode=|versionName=" }
if (-not ($versionLines | Where-Object { $_ -match "versionCode=$([regex]::Escape($expectedVersionCode))(?:\s|$)" }) -or
    -not ($versionLines | Where-Object { $_ -match "versionName=$([regex]::Escape($expectedVersionName))(?:\s|$)" })) {
    throw "设备中的 Orbit 版本与 APK 不一致，期望 $expectedVersionName ($expectedVersionCode)。"
}

$workManagerInfo = Invoke-Adb "读取 WorkManager/JobScheduler 状态" @(
    "shell", "dumpsys", "jobscheduler", "com.nexus.orbit"
)
$workManagerLines = $workManagerInfo | Where-Object {
    $_ -match "com\.nexus\.orbit|SystemJobService|OrbitBackgroundSyncWorker"
}
if ($workManagerLines.Count -eq 0) {
    $workManagerLines = @("尚未观察到同步任务；首次配置 E2E 云身份后需在真机验收中再次确认。")
}

$reportDirectory = Join-Path $Root "dist\orbit-android\device-smoke"
[System.IO.Directory]::CreateDirectory($reportDirectory) | Out-Null
$safeSerial = $Serial -replace "[^A-Za-z0-9._-]", "_"
$reportPath = Join-Path $reportDirectory "$safeSerial-$(Get-Date -Format 'yyyyMMdd-HHmmss').txt"
@(
    "设备：$Serial",
    "APK：$resolvedApk",
    "包名：$expectedPackageName",
    "版本：$expectedVersionName ($expectedVersionCode)",
    "时间：$((Get-Date).ToString('o'))",
    "",
    "[版本]",
    $versionLines,
    "",
    "[WorkManager / JobScheduler]",
    $workManagerLines
) | Set-Content -LiteralPath $reportPath -Encoding UTF8

Write-Host "`nAndroid 设备冒烟验收通过。" -ForegroundColor Green
Write-Host "报告：$reportPath"
Write-Host "双设备配对、并发解决、Doze、弱网与软键盘仍需按 docs/m5-mobile.md 在真机上人工执行。"
